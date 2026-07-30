//! Machine à états de l'allocation TURN : de la première requête nue au
//! rafraîchissement du bail, sans jamais toucher un socket.
//!
//! L'appelant fournit l'horloge (`avancer`) et le transport (`poll_transmit`,
//! `handle_packet`) : c'est ce qui permet d'éprouver tout le protocole, y
//! compris le nonce périmé, contre des réponses fabriquées — sans coturn.

use std::net::SocketAddr;
use std::time::Instant;

use super::messages::{cle_longue_duree, encoder_requete, Identifiants, Requete, BAIL_DEMANDE_S};

/// Ce qu'une allocation réussie procure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Allocation {
    /// Adresse que le serveur relaie vers nous. C'est elle qui devient un
    /// candidat ICE relayé.
    pub relayee: SocketAddr,
    /// Adresse réflexive vue par le serveur. Offerte par la MÊME réponse —
    /// d'où l'absence de serveur STUN séparé (voir la spec §3.3).
    pub reflexive: Option<SocketAddr>,
}

/// Nombre de tentatives d'allocation avant abandon. Au-delà, la session
/// continue sans relais : c'est une dégradation, pas une panne.
const TENTATIVES_MAX: u8 = 5;

/// Les `trans_id` retenus ici ne sont **pas relus** : `handle_packet` accepte
/// la réponse du serveur sans vérifier qu'elle apparie la requête en cours.
/// Le champ est conservé parce que c'est là que l'appariement se brancherait —
/// réserve consignée dans la recette, pas oubli.
#[allow(dead_code)]
enum Etat {
    /// Rien n'est encore parti.
    Repos,
    /// Allocation nue émise, on attend le 401.
    AttenteRefus { trans_id: [u8; 12] },
    /// Allocation signée émise, on attend le succès.
    AttenteAllocation { trans_id: [u8; 12] },
    Allouee {
        echeance_refresh: Instant,
    },
    /// Rafraîchissement émis, on attend sa confirmation.
    AttenteRefresh {
        trans_id: [u8; 12],
        echeance_refresh: Instant,
    },
    /// Abandon définitif. La session continue sans relais.
    Abandonnee,
}

pub struct TurnClient {
    serveur: SocketAddr,
    username: String,
    password: String,
    /// Les cinq champs `pub(super)` ci-dessous sont ceux dont `canaux`, dans un
    /// module frère, a besoin pour porter le second bloc `impl TurnClient` :
    /// borné à `turn`, jamais `pub`. Même conduite que
    /// `congestion::Controleur` vis-à-vis de `congestion::reconfiguration`.
    pub(super) identifiants: Option<Identifiants>,
    pub(super) cle: Option<Vec<u8>>,
    etat: Etat,
    pub(super) allocation: Option<Allocation>,
    /// Requêtes prêtes à partir, dans l'ordre.
    pub(super) sortantes: std::collections::VecDeque<Vec<u8>>,
    maintenant: Instant,
    tentatives: u8,
    /// Compteur d'identifiants de transaction. Un identifiant STUN doit être
    /// imprévisible en usage réel ; ici il doit surtout être UNIQUE, et un
    /// compteur le garantit de façon reproductible en test.
    compteur_trans: u64,
    /// Canaux liés, du numéro vers le pair.
    pub(super) canaux: std::collections::HashMap<u16, SocketAddr>,
    /// Prochain numéro à attribuer, dans la plage normative.
    pub(super) prochain_canal: u16,
}

impl TurnClient {
    pub fn new(serveur: SocketAddr, username: String, password: String, now: Instant) -> Self {
        let mut client = Self {
            serveur,
            username,
            password,
            identifiants: None,
            cle: None,
            etat: Etat::Repos,
            allocation: None,
            sortantes: std::collections::VecDeque::new(),
            maintenant: now,
            tentatives: 0,
            compteur_trans: 0,
            canaux: std::collections::HashMap::new(),
            prochain_canal: super::canaux::CANAL_MIN,
        };
        client.emettre_allocation_nue();
        client
    }

    pub fn serveur(&self) -> SocketAddr {
        self.serveur
    }

    pub fn allocation(&self) -> Option<Allocation> {
        self.allocation
    }

    /// Prochain paquet à envoyer au serveur TURN, s'il y en a un.
    pub fn poll_transmit(&mut self) -> Option<Vec<u8>> {
        self.sortantes.pop_front()
    }

    /// Instant du prochain réveil utile, pour que l'appelant ne dorme pas
    /// au-delà.
    pub fn poll_timeout(&self) -> Option<Instant> {
        match self.etat {
            Etat::Allouee { echeance_refresh }
            | Etat::AttenteRefresh {
                echeance_refresh, ..
            } => Some(echeance_refresh),
            _ => None,
        }
    }

    /// Fait avancer l'horloge interne, et émet le rafraîchissement s'il est dû.
    pub fn avancer(&mut self, now: Instant) {
        self.maintenant = now;
        if let Etat::Allouee { echeance_refresh } = self.etat {
            if now >= echeance_refresh {
                self.emettre_refresh(echeance_refresh);
            }
        }
    }

    /// `pub(super)` : `canaux::lier_canal` numérote ses deux requêtes avec.
    pub(super) fn prochain_trans_id(&mut self) -> [u8; 12] {
        self.compteur_trans += 1;
        let mut id = [0u8; 12];
        id[..8].copy_from_slice(&self.compteur_trans.to_be_bytes());
        id
    }

    fn emettre_allocation_nue(&mut self) {
        let trans_id = self.prochain_trans_id();
        self.sortantes
            .push_back(encoder_requete(&Requete::AllocateNu, trans_id, None));
        self.etat = Etat::AttenteRefus { trans_id };
    }

    fn emettre_allocation_signee(&mut self) {
        let (Some(ids), Some(cle)) = (self.identifiants.clone(), self.cle.clone()) else {
            return;
        };
        let trans_id = self.prochain_trans_id();
        self.sortantes.push_back(encoder_requete(
            &Requete::AllocateSigne,
            trans_id,
            Some((&ids, &cle)),
        ));
        self.etat = Etat::AttenteAllocation { trans_id };
    }

    fn emettre_refresh(&mut self, echeance_refresh: Instant) {
        let (Some(ids), Some(cle)) = (self.identifiants.clone(), self.cle.clone()) else {
            return;
        };
        let trans_id = self.prochain_trans_id();
        self.sortantes.push_back(encoder_requete(
            &Requete::Refresh {
                duree_s: BAIL_DEMANDE_S,
            },
            trans_id,
            Some((&ids, &cle)),
        ));
        self.etat = Etat::AttenteRefresh {
            trans_id,
            echeance_refresh,
        };
    }

    /// Traite un paquet venant du serveur TURN.
    ///
    /// Rend `Ok(None)` pour tout message de service (réponse d'allocation,
    /// de rafraîchissement, erreur) : il est absorbé par la machine à états.
    /// Les données relayées sont traitées par `desencapsuler` (voir `canaux`).
    pub fn handle_packet(&mut self, data: &[u8]) -> anyhow::Result<Option<()>> {
        let message = is::stun::StunMessage::parse(data)
            .map_err(|e| anyhow::anyhow!("message TURN illisible : {e}"))?;

        if let Some((code, _raison)) = message.error_code() {
            self.traiter_erreur(code, &message);
            return Ok(None);
        }

        // Réponse de succès : allocation ou rafraîchissement.
        if let Some(relayee) = message.xor_relayed_address() {
            self.allocation = Some(Allocation {
                relayee,
                reflexive: message.mapped_address(),
            });
            self.tentatives = 0;
        }
        let bail = message.lifetime().unwrap_or(BAIL_DEMANDE_S);
        // Rafraîchir à la MOITIÉ du bail : une seule perte de paquet ne doit
        // pas suffire à perdre l'allocation.
        let echeance = self.maintenant + std::time::Duration::from_secs((bail / 2).max(1) as u64);
        self.etat = Etat::Allouee {
            echeance_refresh: echeance,
        };
        Ok(None)
    }

    fn traiter_erreur(&mut self, code: u16, message: &is::stun::StunMessage<'_>) {
        match code {
            // 401 : premier refus, porteur du realm et du nonce. Étape
            // normale du protocole, pas un échec.
            // 438 : nonce périmé — coturn les fait tourner. Même conduite :
            // adopter le nouveau nonce et rejouer.
            401 | 438 => {
                let (Some(realm), Some(nonce)) = (message.realm(), message.nonce()) else {
                    self.abandonner("401/438 sans realm ni nonce");
                    return;
                };
                self.identifiants = Some(Identifiants {
                    username: self.username.clone(),
                    realm: realm.to_string(),
                    nonce: nonce.to_string(),
                });
                self.cle = Some(cle_longue_duree(&self.username, realm, &self.password));

                self.tentatives += 1;
                if self.tentatives > TENTATIVES_MAX {
                    self.abandonner("trop de refus d'authentification");
                    return;
                }

                // Un 438 sur un rafraîchissement ne doit PAS réallouer :
                // l'allocation existe toujours côté serveur, il faut rejouer
                // le rafraîchissement avec le nouveau nonce.
                match self.etat {
                    Etat::AttenteRefresh {
                        echeance_refresh, ..
                    } => self.emettre_refresh(echeance_refresh),
                    _ => self.emettre_allocation_signee(),
                }
            }
            autre => {
                self.abandonner(&format!("erreur TURN {autre}"));
            }
        }
    }

    fn abandonner(&mut self, raison: &str) {
        tracing::warn!(
            raison,
            "allocation TURN abandonnée : la session continuera sans relais"
        );
        self.etat = Etat::Abandonnee;
        self.allocation = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::turn::fixtures::{allouee, reponse, serveur, t0, trans_id_de};
    use crate::turn::messages::{
        sha1_hmac, ATTR_ERROR_CODE, ATTR_NONCE, ATTR_REALM, METHODE_ALLOCATE, METHODE_REFRESH,
    };
    use std::time::Duration;

    #[test]
    fn la_premiere_emission_est_une_allocation_nue() {
        let mut c = TurnClient::new(serveur(), "u".into(), "p".into(), t0());
        let paquet = c.poll_transmit().expect("une allocation doit partir");
        assert_eq!(&paquet[0..2], &[0x00, 0x03], "Allocate attendu");
        // Nue : rien après REQUESTED-TRANSPORT.
        assert_eq!(paquet.len(), 28);
        // Rien d'autre tant qu'aucune réponse n'est arrivée.
        assert!(c.poll_transmit().is_none(), "pas de rafale d'allocations");
        assert!(c.allocation().is_none());
    }

    #[test]
    fn un_401_declenche_une_allocation_signee() {
        let mut c = TurnClient::new(serveur(), "u".into(), "p".into(), t0());
        let nue = c.poll_transmit().expect("allocation nue");

        let refus = reponse(
            METHODE_ALLOCATE,
            false,
            trans_id_de(&nue),
            &[
                (ATTR_ERROR_CODE, vec![0, 0, 4, 1, b'U', b'n', b'a', b'u']),
                (ATTR_REALM, b"example.org".to_vec()),
                (ATTR_NONCE, b"nonce1".to_vec()),
            ],
        );
        assert!(c.handle_packet(&refus).expect("401 traité").is_none());

        let signee = c.poll_transmit().expect("allocation signée attendue");
        let message = is::stun::StunMessage::parse(&signee).expect("relue");
        assert_eq!(message.realm(), Some("example.org"));
        assert_eq!(message.nonce(), Some("nonce1"));
        assert!(message.verify(&cle_longue_duree("u", "example.org", "p"), sha1_hmac));
    }

    #[test]
    fn un_succes_rend_l_adresse_relayee_et_l_adresse_reflexive() {
        let c = allouee();
        let a = c.allocation().expect("allocation obtenue");
        assert_eq!(a.relayee, "192.0.2.15:50000".parse::<SocketAddr>().unwrap());
        assert_eq!(a.reflexive, Some("203.0.113.4:41234".parse().unwrap()));
    }

    #[test]
    fn un_nonce_perime_est_rejoue_et_non_abandonne() {
        // 438 « Stale Nonce » est le cas d'erreur RÉELLEMENT rencontré :
        // coturn fait tourner ses nonces. Le traiter comme un échec
        // terminerait l'allocation au bout de quelques minutes.
        let mut c = allouee();
        c.avancer(t0() + Duration::from_secs(300));
        let refresh = c.poll_transmit().expect("rafraîchissement attendu");

        let perime = reponse(
            METHODE_REFRESH,
            false,
            trans_id_de(&refresh),
            &[
                (ATTR_ERROR_CODE, vec![0, 0, 4, 38, b'x']),
                (ATTR_REALM, b"r".to_vec()),
                (ATTR_NONCE, b"n2".to_vec()),
            ],
        );
        c.handle_packet(&perime).expect("438 traité");

        let rejoue = c.poll_transmit().expect("la requête doit repartir");
        let message = is::stun::StunMessage::parse(&rejoue).expect("relue");
        assert_eq!(
            message.nonce(),
            Some("n2"),
            "le nouveau nonce doit être employé"
        );
        assert!(
            c.allocation().is_some(),
            "l'allocation ne doit pas être perdue"
        );
    }

    #[test]
    fn le_rafraichissement_tombe_a_la_moitie_du_bail() {
        let mut c = allouee();
        // Bail de 600 s : rien avant 300 s.
        c.avancer(t0() + Duration::from_secs(299));
        assert!(c.poll_transmit().is_none(), "rafraîchissement trop précoce");
        c.avancer(t0() + Duration::from_secs(300));
        let paquet = c.poll_transmit().expect("rafraîchissement attendu");
        assert_eq!(&paquet[0..2], &[0x00, 0x04], "Refresh attendu");
    }
}
