//! Canaux TURN : démultiplexage STUN/données, liaison d'un canal à un pair, et
//! encapsulation ChannelData.
//!
//! Second bloc `impl TurnClient`, dans un module frère de `allocation` — même
//! découpage que `congestion::reconfiguration` vis-à-vis de
//! `congestion::controleur`, et pour la même raison : la limite de 500 lignes
//! par fichier.

use std::net::SocketAddr;

use super::allocation::TurnClient;
use super::messages::{encoder_requete, Requete};

/// Premier numéro de canal de la plage normative (RFC 5766 §2.5).
pub(super) const CANAL_MIN: u16 = 0x4000;
const CANAL_MAX: u16 = 0x7FFF;

/// Vrai si ce datagramme est une trame ChannelData plutôt qu'un message STUN.
///
/// Les deux bits de poids fort du premier octet suffisent : STUN vaut `00`,
/// ChannelData vaut `01` (par construction, les numéros de canal valides
/// commencent tous par ces deux bits). C'est le démultiplexage prévu par la
/// RFC, et le seul possible sur un socket partagé.
pub fn est_channel_data(data: &[u8]) -> bool {
    matches!(data.first(), Some(premier) if premier >> 6 == 0b01)
}

impl TurnClient {
    /// Attribue un canal à un pair et émet les requêtes nécessaires.
    ///
    /// `CreatePermission` d'abord, `ChannelBind` ensuite : la seconde
    /// implique la première côté serveur, mais les émettre toutes deux évite
    /// une fenêtre pendant laquelle le serveur jetterait nos paquets si le
    /// `ChannelBind` se perdait.
    ///
    /// Rend `None` si aucune allocation n'est en place, ou si la plage de
    /// canaux est épuisée.
    pub fn lier_canal(&mut self, pair: SocketAddr) -> Option<u16> {
        // Aucune allocation : il n'y a rien à quoi lier un canal.
        self.allocation?;
        if let Some(canal) = self
            .canaux
            .iter()
            .find(|(_, p)| **p == pair)
            .map(|(c, _)| *c)
        {
            return Some(canal);
        }
        if self.prochain_canal > CANAL_MAX {
            tracing::warn!("plage de canaux TURN épuisée");
            return None;
        }
        let canal = self.prochain_canal;
        self.prochain_canal += 1;

        let (ids, cle) = (self.identifiants.clone()?, self.cle.clone()?);
        let trans_permission = self.prochain_trans_id();
        self.sortantes.push_back(encoder_requete(
            &Requete::CreatePermission { pair },
            trans_permission,
            Some((&ids, &cle)),
        ));
        let trans_bind = self.prochain_trans_id();
        self.sortantes.push_back(encoder_requete(
            &Requete::ChannelBind { canal, pair },
            trans_bind,
            Some((&ids, &cle)),
        ));

        self.canaux.insert(canal, pair);
        Some(canal)
    }

    /// Enveloppe une charge utile pour le pair donné.
    ///
    /// Rend `None` si aucun canal n'est lié à ce pair : l'appelant doit alors
    /// envoyer en direct, pas fabriquer une trame que le serveur jetterait.
    ///
    /// Aucun remplissage : la RFC 5766 §11.5 ne l'exige pas sur UDP.
    pub fn encapsuler(&self, pair: SocketAddr, charge: &[u8]) -> Option<Vec<u8>> {
        let canal = self
            .canaux
            .iter()
            .find(|(_, p)| **p == pair)
            .map(|(c, _)| *c)?;
        let mut trame = Vec::with_capacity(4 + charge.len());
        trame.extend_from_slice(&canal.to_be_bytes());
        trame.extend_from_slice(&(charge.len() as u16).to_be_bytes());
        trame.extend_from_slice(charge);
        Some(trame)
    }

    /// Extrait le pair d'origine et la charge utile d'une trame ChannelData.
    ///
    /// Rend `None` sur une trame tronquée ou dont le canal n'est pas lié :
    /// un datagramme malformé venu du réseau ne doit jamais faire paniquer
    /// l'agent — c'est le même principe que le `DatagramRecv::try_from` du
    /// transport.
    pub fn desencapsuler<'a>(&self, trame: &'a [u8]) -> Option<(SocketAddr, &'a [u8])> {
        if trame.len() < 4 {
            return None;
        }
        let canal = u16::from_be_bytes([trame[0], trame[1]]);
        let longueur = u16::from_be_bytes([trame[2], trame[3]]) as usize;
        if trame.len() < 4 + longueur {
            return None;
        }
        let pair = *self.canaux.get(&canal)?;
        Some((pair, &trame[4..4 + longueur]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::turn::fixtures::allouee;

    #[test]
    fn le_premier_octet_departage_stun_de_channel_data() {
        // Les deux bits de poids fort : 00 = STUN, 01 = ChannelData. C'est le
        // seul démultiplexage dont on dispose sur un socket UDP partagé.
        assert!(
            !est_channel_data(&[0x00, 0x03, 0, 0]),
            "Allocate est du STUN"
        );
        assert!(!est_channel_data(&[0x01, 0x01, 0, 0]), "réponse STUN");
        assert!(est_channel_data(&[0x40, 0x00, 0, 0]), "premier canal valide");
        assert!(est_channel_data(&[0x7F, 0xFF, 0, 0]), "dernier canal valide");
        assert!(!est_channel_data(&[]), "un paquet vide n'est rien");
    }

    #[test]
    fn encapsuler_prefixe_le_numero_de_canal_et_la_longueur() {
        let mut c = allouee();
        let pair: SocketAddr = "203.0.113.9:6000".parse().unwrap();
        let canal = c.lier_canal(pair).expect("canal attribué");

        let encapsule = c.encapsuler(pair, &[1, 2, 3]).expect("pair connu");
        assert_eq!(&encapsule[0..2], &canal.to_be_bytes());
        assert_eq!(&encapsule[2..4], &3u16.to_be_bytes());
        assert_eq!(&encapsule[4..7], &[1, 2, 3]);
        // Sur UDP, aucun remplissage n'est requis sur le dernier paquet.
        assert_eq!(encapsule.len(), 7);
    }

    #[test]
    fn encapsuler_refuse_un_pair_sans_canal() {
        let c = allouee();
        let inconnu: SocketAddr = "198.51.100.1:1".parse().unwrap();
        assert!(
            c.encapsuler(inconnu, &[1]).is_none(),
            "aucun canal lié pour ce pair"
        );
    }

    #[test]
    fn lier_un_canal_emet_permission_puis_channel_bind() {
        let mut c = allouee();
        // Vider ce qui restait en attente.
        while c.poll_transmit().is_some() {}

        let pair: SocketAddr = "203.0.113.9:6000".parse().unwrap();
        c.lier_canal(pair).expect("canal attribué");

        let premier = c.poll_transmit().expect("CreatePermission attendu");
        assert_eq!(&premier[0..2], &[0x00, 0x08], "CreatePermission");
        let second = c.poll_transmit().expect("ChannelBind attendu");
        assert_eq!(&second[0..2], &[0x00, 0x09], "ChannelBind");
    }

    #[test]
    fn les_numeros_de_canal_restent_dans_la_plage_normative() {
        let mut c = allouee();
        for i in 0..8u16 {
            let pair: SocketAddr = format!("203.0.113.{}:6000", i + 1).parse().unwrap();
            let canal = c.lier_canal(pair).expect("canal attribué");
            assert!(
                (0x4000..=0x7FFF).contains(&canal),
                "canal {canal:#x} hors de la plage 0x4000-0x7FFF"
            );
        }
    }

    #[test]
    fn desencapsuler_rend_le_pair_et_la_charge_utile() {
        let mut c = allouee();
        let pair: SocketAddr = "203.0.113.9:6000".parse().unwrap();
        let canal = c.lier_canal(pair).expect("canal attribué");

        let mut trame = Vec::new();
        trame.extend_from_slice(&canal.to_be_bytes());
        trame.extend_from_slice(&4u16.to_be_bytes());
        trame.extend_from_slice(&[9, 8, 7, 6]);

        let (source, charge) = c.desencapsuler(&trame).expect("trame reconnue");
        assert_eq!(source, pair);
        assert_eq!(charge, &[9, 8, 7, 6]);
    }

    #[test]
    fn desencapsuler_refuse_une_trame_tronquee_ou_inconnue() {
        let mut c = allouee();
        let pair: SocketAddr = "203.0.113.9:6000".parse().unwrap();
        let canal = c.lier_canal(pair).expect("canal");

        // Longueur annoncée plus grande que ce qui suit : une lecture naïve
        // paniquerait sur un découpage hors bornes.
        let mut tronquee = Vec::new();
        tronquee.extend_from_slice(&canal.to_be_bytes());
        tronquee.extend_from_slice(&99u16.to_be_bytes());
        tronquee.extend_from_slice(&[1, 2]);
        assert!(c.desencapsuler(&tronquee).is_none());

        // Canal jamais lié.
        let mut inconnue = Vec::new();
        inconnue.extend_from_slice(&0x7FFFu16.to_be_bytes());
        inconnue.extend_from_slice(&1u16.to_be_bytes());
        inconnue.push(0);
        assert!(c.desencapsuler(&inconnue).is_none());
    }
}
