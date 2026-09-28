//! TURN channels: STUN/data demultiplexing, binding a channel to a peer, and
//! ChannelData encapsulation.
//!
//! Second `impl TurnClient` block, in a sibling module of `allocation` — same
//! split as `congestion::reconfiguration` with respect to
//! `congestion::controleur`, and for the same reason: the 500-lines-per-file
//! limit.

use std::net::SocketAddr;
use std::time::{Duration, Instant};

use super::allocation::TurnClient;
use super::messages::{encoder_requete, Requete};

/// First channel number of the normative range (RFC 5766 §2.5).
pub(super) const CANAL_MIN: u16 = 0x4000;
const CANAL_MAX: u16 = 0x7FFF;

/// Delay between two refreshes of a channel binding.
///
/// A permission lasts 300 s (RFC 5766 §8), a channel 600 s (§11): it is the
/// permission, the shorter one, that governs. We re-emit at HALF its duration,
/// for the same reason as the allocation lease — a single packet loss
/// must not be enough to lose the relay.
///
/// Without this refresh, a relayed session dies around 300 s: observed in
/// the acceptance run of 07/30/2026 (drop at 340 s, while the allocation lease
/// was indeed refreshed).
const PERIODE_RAFRAICHISSEMENT: Duration = Duration::from_secs(150);

/// A live channel binding: the peer it serves, and the instant it
/// must be reaffirmed with the server.
pub(super) struct Canal {
    pub(super) pair: SocketAddr,
    echeance: Instant,
}

/// True if this datagram is a ChannelData frame rather than a STUN message.
///
/// The two most significant bits of the first byte are enough: STUN is `00`,
/// ChannelData is `01` (by construction, valid channel numbers
/// all start with these two bits). It is the demultiplexing the
/// RFC provides for, and the only one possible on a shared socket.
pub fn est_channel_data(data: &[u8]) -> bool {
    matches!(data.first(), Some(premier) if premier >> 6 == 0b01)
}

impl TurnClient {
    /// Assigns a channel to a peer and emits the necessary requests.
    ///
    /// `CreatePermission` first, `ChannelBind` next: the second
    /// implies the first on the server side, but emitting both avoids
    /// a window during which the server would drop our packets if the
    /// `ChannelBind` got lost.
    ///
    /// Returns `None` if no allocation is in place, or if the channel
    /// range is exhausted.
    pub fn lier_canal(&mut self, pair: SocketAddr) -> Option<u16> {
        // No allocation: there is nothing to bind a channel to.
        self.allocation?;
        if let Some(canal) = self.canal_de(pair) {
            return Some(canal);
        }
        if self.prochain_canal > CANAL_MAX {
            tracing::warn!("plage de canaux TURN épuisée");
            return None;
        }
        let canal = self.prochain_canal;
        self.prochain_canal += 1;

        self.emettre_liaison(canal, pair)?;
        Some(canal)
    }

    /// Number of the channel serving this peer, if one exists.
    fn canal_de(&self, pair: SocketAddr) -> Option<u16> {
        self.canaux
            .iter()
            .find(|(_, c)| c.pair == pair)
            .map(|(numero, _)| *numero)
    }

    /// Emits `CreatePermission` then `ChannelBind` for this pair, and (re)sets
    /// the refresh deadline.
    ///
    /// `CreatePermission` first, `ChannelBind` next: the second implies
    /// the first on the server side, but emitting both avoids a window
    /// during which the server would drop our packets if the `ChannelBind` got
    /// lost.
    ///
    /// Serves the first binding as well as its refresh: the
    /// server handles both the same way, which is precisely what
    /// the RFC provides for extending a binding.
    fn emettre_liaison(&mut self, canal: u16, pair: SocketAddr) -> Option<()> {
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

        self.canaux.insert(
            canal,
            Canal {
                pair,
                echeance: self.maintenant() + PERIODE_RAFRAICHISSEMENT,
            },
        );
        Some(())
    }

    /// Reaffirms the bindings whose deadline is reached.
    ///
    /// Called by `avancer`, just like the lease refresh:
    /// an expired permission silences the relay without announcing anything, and the
    /// session dies of silence.
    pub(super) fn rafraichir_canaux(&mut self, now: Instant) {
        let echus: Vec<(u16, SocketAddr)> = self
            .canaux
            .iter()
            .filter(|(_, c)| now >= c.echeance)
            .map(|(numero, c)| (*numero, c.pair))
            .collect();
        for (canal, pair) in echus {
            // `info` and not `debug`, for the same reason as the lease: one
            // line every 150 s per channel, and it is the only observable proof
            // that the relay stays maintained.
            tracing::info!(canal, %pair, "réaffirmation d'une liaison de canal TURN");
            self.emettre_liaison(canal, pair);
        }
    }

    /// Nearest refresh deadline, so that the caller does not
    /// sleep beyond it.
    pub(super) fn prochaine_echeance_canal(&self) -> Option<Instant> {
        self.canaux.values().map(|c| c.echeance).min()
    }

    /// Wraps a payload for the given peer.
    ///
    /// Returns `None` if no channel is bound to this peer: the caller must then
    /// send direct, not fabricate a frame the server would drop.
    ///
    /// No padding: RFC 5766 §11.5 does not require it over UDP.
    pub fn encapsuler(&self, pair: SocketAddr, charge: &[u8]) -> Option<Vec<u8>> {
        let canal = self.canal_de(pair)?;
        let mut trame = Vec::with_capacity(4 + charge.len());
        trame.extend_from_slice(&canal.to_be_bytes());
        trame.extend_from_slice(&(charge.len() as u16).to_be_bytes());
        trame.extend_from_slice(charge);
        Some(trame)
    }

    /// Extracts the originating peer and the payload of a ChannelData frame.
    ///
    /// Returns `None` on a truncated frame or one whose channel is not bound:
    /// a malformed datagram from the network must never make
    /// the agent panic — it is the same principle as the transport's
    /// `DatagramRecv::try_from`.
    pub fn desencapsuler<'a>(&self, trame: &'a [u8]) -> Option<(SocketAddr, &'a [u8])> {
        if trame.len() < 4 {
            return None;
        }
        let canal = u16::from_be_bytes([trame[0], trame[1]]);
        let length = u16::from_be_bytes([trame[2], trame[3]]) as usize;
        if trame.len() < 4 + length {
            return None;
        }
        let pair = self.canaux.get(&canal)?.pair;
        Some((pair, &trame[4..4 + length]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::turn::fixtures::{allouee, t0};
    use std::time::Duration;

    #[test]
    fn le_premier_octet_departage_stun_de_channel_data() {
        // The two most significant bits: 00 = STUN, 01 = ChannelData. It is the
        // only demultiplexing available on a shared UDP socket.
        assert!(
            !est_channel_data(&[0x00, 0x03, 0, 0]),
            "Allocate est du STUN"
        );
        assert!(!est_channel_data(&[0x01, 0x01, 0, 0]), "réponse STUN");
        assert!(
            est_channel_data(&[0x40, 0x00, 0, 0]),
            "premier canal valide"
        );
        assert!(
            est_channel_data(&[0x7F, 0xFF, 0, 0]),
            "dernier canal valide"
        );
        assert!(!est_channel_data(&[]), "un paquet vide n'est rien");
    }

    #[test]
    fn encapsulate_prefixes_the_channel_number_and_the_length() {
        let mut c = allouee();
        let pair: SocketAddr = "203.0.113.9:6000".parse().unwrap();
        let canal = c.lier_canal(pair).expect("canal attribué");

        let encapsule = c.encapsuler(pair, &[1, 2, 3]).expect("pair connu");
        assert_eq!(&encapsule[0..2], &canal.to_be_bytes());
        assert_eq!(&encapsule[2..4], &3u16.to_be_bytes());
        assert_eq!(&encapsule[4..7], &[1, 2, 3]);
        // Over UDP, no padding is required on the last packet.
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
        // Empty what was still pending.
        while c.poll_transmit().is_some() {}

        let pair: SocketAddr = "203.0.113.9:6000".parse().unwrap();
        c.lier_canal(pair).expect("canal attribué");

        let premier = c.poll_transmit().expect("CreatePermission attendu");
        assert_eq!(&premier[0..2], &[0x00, 0x08], "CreatePermission");
        let second = c.poll_transmit().expect("ChannelBind attendu");
        assert_eq!(&second[0..2], &[0x00, 0x09], "ChannelBind");
    }

    #[test]
    fn channel_numbers_stay_in_the_normative_range() {
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
    fn a_bound_channel_is_refreshed_before_the_permission_expires() {
        // Finding of the acceptance run of 07/30/2026: a relayed session dropped
        // after ~340 s. A TURN permission lasts 300 s (RFC 5766 §8) and a
        // channel 600 s (§11); without re-emission, the server stops relaying our
        // packets and the session dies — while the allocation lease, for its part,
        // was indeed refreshed.
        let mut c = allouee();
        let pair: SocketAddr = "203.0.113.9:6000".parse().unwrap();
        let canal = c.lier_canal(pair).expect("canal attribué");
        while c.poll_transmit().is_some() {}

        // At half a permission's duration, as for the lease: a
        // single packet loss must not be enough to lose the relay.
        c.avancer(t0() + Duration::from_secs(150));

        let premier = c
            .poll_transmit()
            .expect("CreatePermission de rafraîchissement attendu");
        assert_eq!(&premier[0..2], &[0x00, 0x08], "CreatePermission");
        let second = c
            .poll_transmit()
            .expect("ChannelBind de rafraîchissement attendu");
        assert_eq!(&second[0..2], &[0x00, 0x09], "ChannelBind");

        // The SAME channel, not a new one: a refresh extends the
        // existing binding, it does not create a second one.
        let message = is::stun::StunMessage::parse(&second).expect("relue");
        assert_eq!(message.channel_number(), Some(canal));
        assert_eq!(message.xor_peer_address(), Some(pair));

        // And no burst: nothing more before the next deadline.
        assert!(
            c.poll_transmit().is_none(),
            "un seul rafraîchissement par échéance"
        );
        c.avancer(t0() + Duration::from_secs(151));
        assert!(
            c.poll_transmit().is_none(),
            "pas de réémission à chaque tour"
        );
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

        // Announced length larger than what follows: a naive read
        // would panic on an out-of-bounds slice.
        let mut tronquee = Vec::new();
        tronquee.extend_from_slice(&canal.to_be_bytes());
        tronquee.extend_from_slice(&99u16.to_be_bytes());
        tronquee.extend_from_slice(&[1, 2]);
        assert!(c.desencapsuler(&tronquee).is_none());

        // Channel never bound.
        let mut inconnue = Vec::new();
        inconnue.extend_from_slice(&0x7FFFu16.to_be_bytes());
        inconnue.extend_from_slice(&1u16.to_be_bytes());
        inconnue.push(0);
        assert!(c.desencapsuler(&inconnue).is_none());
    }
}
