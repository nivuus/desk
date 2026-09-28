//! TURN allocation state machine: from the first bare request to
//! lease refresh, without ever touching a socket.
//!
//! The caller provides the clock (`avancer`) and the transport (`poll_transmit`,
//! `handle_packet`): it is what lets the whole protocol be exercised,
//! including the stale nonce, against fabricated responses — without coturn.

use std::net::SocketAddr;
use std::time::{Duration, Instant};

use super::messages::{
    cle_longue_duree, encoder_requete, methode_de, Identifiants, Requete, BAIL_DEMANDE_S,
    METHODE_ALLOCATE, METHODE_REFRESH,
};

/// What a successful allocation provides.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Allocation {
    /// Address the server relays to us. It is the one that becomes a
    /// relayed ICE candidate.
    pub relayee: SocketAddr,
    /// Reflexive address seen by the server. Offered by the SAME response —
    /// hence no separate STUN server (see spec §3.3).
    pub reflexive: Option<SocketAddr>,
}

/// Number of allocation attempts before giving up. Beyond it, the session
/// continues without a relay: it is a degradation, not a failure.
const TENTATIVES_MAX: u8 = 5;

/// The `trans_id`s retained here are **not reread**: `handle_packet` accepts
/// the server's response without checking that it matches the current request.
/// The field is kept because that is where matching would plug in —
/// a reservation recorded in the acceptance run, not an oversight.
#[allow(dead_code)]
enum Etat {
    /// Nothing has gone out yet.
    Repos,
    /// Bare allocation sent, waiting for the 401.
    AttenteRefus {
        trans_id: [u8; 12],
    },
    /// Signed allocation sent, waiting for success.
    AttenteAllocation {
        trans_id: [u8; 12],
    },
    Allouee {
        echeance_refresh: Instant,
    },
    /// Refresh sent, waiting for its confirmation.
    AttenteRefresh {
        trans_id: [u8; 12],
        echeance_refresh: Instant,
    },
    /// Final abandonment. The session continues without a relay.
    Abandonnee,
}

pub struct TurnClient {
    serveur: SocketAddr,
    username: String,
    password: String,
    /// The five `pub(super)` fields below are those `canaux`, in a
    /// sibling module, needs to carry the second `impl TurnClient` block:
    /// bounded to `turn`, never `pub`. Same approach as
    /// `congestion::Controleur` with respect to `congestion::reconfiguration`.
    pub(super) identifiants: Option<Identifiants>,
    pub(super) cle: Option<Vec<u8>>,
    etat: Etat,
    pub(super) allocation: Option<Allocation>,
    /// Requests ready to go, in order.
    pub(super) sortantes: std::collections::VecDeque<Vec<u8>>,
    maintenant: Instant,
    tentatives: u8,
    /// Transaction identifier counter. A STUN identifier must be
    /// unpredictable in real use; here it must above all be UNIQUE, and a
    /// counter guarantees that reproducibly in tests.
    compteur_trans: u64,
    /// Instant of the last diagnostic reading (see `avancer`).
    last_reading: Instant,
    /// Bound channels, from number to binding (peer served and
    /// reaffirmation deadline).
    pub(super) canaux: std::collections::HashMap<u16, super::canaux::Canal>,
    /// Next number to assign, within the normative range.
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
            last_reading: now,
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

    /// Next packet to send to the TURN server, if there is one.
    pub fn poll_transmit(&mut self) -> Option<Vec<u8>> {
        self.sortantes.pop_front()
    }

    /// Instant of the next useful wake-up, so that the caller does not sleep
    /// beyond it.
    ///
    /// Two deadlines compete: the allocation's lease and the nearest
    /// channel binding to reaffirm. The second falls much more often (150 s
    /// against 300 s), and forgetting it here would make keeping the relay depend on
    /// the media cadence.
    pub fn poll_timeout(&self) -> Option<Instant> {
        let bail = match self.etat {
            Etat::Allouee { echeance_refresh }
            | Etat::AttenteRefresh {
                echeance_refresh, ..
            } => Some(echeance_refresh),
            _ => None,
        };
        match (bail, self.prochaine_echeance_canal()) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
    }

    /// Current instant of the internal clock. `pub(super)`: `canaux` needs it
    /// to date the deadline of a binding it has just emitted.
    pub(super) fn maintenant(&self) -> Instant {
        self.maintenant
    }

    /// Advances the internal clock, and emits what is due: the
    /// lease refresh, and the channel bindings to reaffirm.
    pub fn avancer(&mut self, now: Instant) {
        self.maintenant = now;

        // Periodic diagnostic reading: one line per minute, saying what
        // the state machine believes it must do. Without it, a lease not
        // refreshed is only noticed after the fact, in the server's logs.
        if now.saturating_duration_since(self.last_reading) >= Duration::from_secs(60) {
            self.last_reading = now;
            let etat = match self.etat {
                Etat::Repos => "idle",
                Etat::AttenteRefus { .. } => "awaiting-refusal",
                Etat::AttenteAllocation { .. } => "awaiting-allocation",
                Etat::Allouee { .. } => "allocated",
                Etat::AttenteRefresh { .. } => "awaiting-refresh",
                Etat::Abandonnee => "abandoned",
            };
            let in_s = self
                .poll_timeout()
                .map(|e| e.saturating_duration_since(now).as_secs() as i64)
                .unwrap_or(-1);
            tracing::info!(
                etat,
                prochaine_echeance_s = in_s,
                canaux = self.canaux.len(),
                "TURN client state"
            );
        }
        if let Etat::Allouee { echeance_refresh } = self.etat {
            if now >= echeance_refresh {
                self.emettre_refresh(echeance_refresh);
            }
        }
        // Independent of the lease state: a permission expires on its own
        // account, including while waiting for the response to a `Refresh`.
        self.rafraichir_canaux(now);
    }

    /// `pub(super)`: `canaux::lier_canal` numbers its two requests with it.
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
            tracing::warn!("TURN lease refresh impossible: credentials missing");
            return;
        };
        // One line every 300 s: rare enough to be logged at
        // `info`, and it is the only trace that says whether the lease is really
        // maintained (the acceptance run of 07/30/2026 spent an hour deducing it
        // from coturn's logs, for lack of this line).
        tracing::info!("TURN lease refresh sent");
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
    /// Returns `Ok(None)` for any service message (allocation response,
    /// refresh response, error): it is absorbed by the state machine.
    /// Relayed data is handled by `desencapsuler` (see `canaux`).
    pub fn handle_packet(&mut self, data: &[u8]) -> anyhow::Result<Option<()>> {
        let message = is::stun::StunMessage::parse(data)
            .map_err(|e| anyhow::anyhow!("message TURN illisible : {e}"))?;

        if let Some((code, _raison)) = message.error_code() {
            self.handle_error(code, &message);
            return Ok(None);
        }

        // Success response: allocation or refresh.
        if let Some(relayee) = message.xor_relayed_address() {
            self.allocation = Some(Allocation {
                relayee,
                reflexive: message.mapped_address(),
            });
            self.tentatives = 0;
        }

        // Only responses to `Allocate` and `Refresh` carry a lease. A
        // response to `CreatePermission` or `ChannelBind` carries none, and
        // treating it as such would push the refresh back by 300 s
        // each time — to the point that it never goes out and the allocation
        // expires. Observed in the acceptance run of 07/30/2026, once channel
        // bindings were reaffirmed periodically: their responses, all arriving
        // every 150 s, pushed back a 300 s deadline indefinitely.
        let porte_un_bail = matches!(
            methode_de(data),
            Some(METHODE_ALLOCATE) | Some(METHODE_REFRESH)
        );
        if porte_un_bail {
            let bail = message.lifetime().unwrap_or(BAIL_DEMANDE_S);
            // Refresh at HALF the lease: a single packet loss must
            // not be enough to lose the allocation.
            let echeance =
                self.maintenant + std::time::Duration::from_secs((bail / 2).max(1) as u64);
            self.etat = Etat::Allouee {
                echeance_refresh: echeance,
            };
        }
        Ok(None)
    }

    fn handle_error(&mut self, code: u16, message: &is::stun::StunMessage<'_>) {
        match code {
            // 401: first refusal, carrying the realm and the nonce. A normal
            // step of the protocol, not a failure.
            // 438: stale nonce — coturn rotates them. Same approach:
            // adopt the new nonce and replay.
            401 | 438 => {
                let (Some(realm), Some(nonce)) = (message.realm(), message.nonce()) else {
                    self.abandonner("401/438 without realm or nonce");
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
                    self.abandonner("too many authentication refusals");
                    return;
                }

                // A 438 on a refresh must NOT reallocate:
                // the allocation still exists on the server side, the refresh must be
                // replayed with the new nonce.
                match self.etat {
                    Etat::AttenteRefresh {
                        echeance_refresh, ..
                    } => self.emettre_refresh(echeance_refresh),
                    _ => self.emettre_allocation_signee(),
                }
            }
            autre => {
                self.abandonner(&format!("TURN error {autre}"));
            }
        }
    }

    fn abandonner(&mut self, raison: &str) {
        tracing::warn!(
            raison,
            "TURN allocation abandoned: the session will go on without a relay"
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
        sha1_hmac, ATTR_ERROR_CODE, ATTR_NONCE, ATTR_REALM, METHODE_ALLOCATE, METHODE_CHANNEL_BIND,
        METHODE_REFRESH,
    };
    use std::time::Duration;

    #[test]
    fn the_first_emission_is_a_bare_allocation() {
        let mut c = TurnClient::new(serveur(), "u".into(), "p".into(), t0());
        let paquet = c.poll_transmit().expect("an allocation must go out");
        assert_eq!(&paquet[0..2], &[0x00, 0x03], "Allocate expected");
        // Bare: nothing after REQUESTED-TRANSPORT.
        assert_eq!(paquet.len(), 28);
        // Nothing else as long as no response has arrived.
        assert!(c.poll_transmit().is_none(), "no burst of allocations");
        assert!(c.allocation().is_none());
    }

    #[test]
    fn a_401_triggers_a_signed_allocation() {
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
        assert!(c.handle_packet(&refus).expect("401 handled").is_none());

        let signee = c.poll_transmit().expect("signed allocation expected");
        let message = is::stun::StunMessage::parse(&signee).expect("relue");
        assert_eq!(message.realm(), Some("example.org"));
        assert_eq!(message.nonce(), Some("nonce1"));
        assert!(message.verify(&cle_longue_duree("u", "example.org", "p"), sha1_hmac));
    }

    #[test]
    fn a_success_returns_the_relayed_address_and_the_reflexive_address() {
        let c = allouee();
        let a = c.allocation().expect("allocation obtenue");
        assert_eq!(a.relayee, "192.0.2.15:50000".parse::<SocketAddr>().unwrap());
        assert_eq!(a.reflexive, Some("203.0.113.4:41234".parse().unwrap()));
    }

    #[test]
    fn a_stale_nonce_is_replayed_not_abandoned() {
        // 438 "Stale Nonce" is the error case ACTUALLY encountered:
        // coturn rotates its nonces. Treating it as a failure
        // would end the allocation after a few minutes.
        let mut c = allouee();
        c.avancer(t0() + Duration::from_secs(300));
        let refresh = c.poll_transmit().expect("refresh expected");

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
        c.handle_packet(&perime).expect("438 handled");

        let rejoue = c.poll_transmit().expect("the request must go out again");
        let message = is::stun::StunMessage::parse(&rejoue).expect("relue");
        assert_eq!(message.nonce(), Some("n2"), "the new nonce must be used");
        assert!(c.allocation().is_some(), "the allocation must not be lost");
    }

    #[test]
    fn a_channel_bind_answer_does_not_push_back_the_lease_deadline() {
        // Finding of the acceptance run of 07/30/2026, obtained through a periodic
        // state reading: the agent's allocation expired at 600 s without having been
        // refreshed. `handle_packet` reset the lease deadline at EVERY
        // success response from the server — yet responses to `CreatePermission`
        // and `ChannelBind` carry no lease. As soon as channel bindings
        // were reaffirmed periodically, their responses pushed the
        // lease refresh back to infinity, and the allocation died.
        let mut c = allouee();

        // Success response to a ChannelBind: neither LIFETIME nor relayed address.
        let reponse_bind = reponse(METHODE_CHANNEL_BIND, true, [9u8; 12], &[]);
        c.avancer(t0() + Duration::from_secs(200));
        c.handle_packet(&reponse_bind).expect("answer handled");

        // The lease deadline stays the one set by the allocation: 300 s.
        c.avancer(t0() + Duration::from_secs(300));
        let paquet = c
            .poll_transmit()
            .expect("the lease refresh must go out despite the interleaved answer");
        assert_eq!(&paquet[0..2], &[0x00, 0x04], "Refresh expected");
    }

    #[test]
    fn the_refresh_falls_at_half_the_lease() {
        let mut c = allouee();
        // 600 s lease: nothing before 300 s.
        c.avancer(t0() + Duration::from_secs(299));
        assert!(c.poll_transmit().is_none(), "refresh too early");
        c.avancer(t0() + Duration::from_secs(300));
        let paquet = c.poll_transmit().expect("refresh expected");
        assert_eq!(&paquet[0..2], &[0x00, 0x04], "Refresh expected");
    }
}
