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

/// Number of authentication refusals ONE request may absorb before giving
/// up on it. Counted per request, not per client: a nonce rotation refuses
/// every request in flight once, and a counter shared by all of them would end
/// the relay after a few rotations although each replay succeeds. Beyond it, an
/// allocation request ends the relay (the session continues without one: a
/// degradation, not a failure) and a channel request drops that peer's channel.
const TENTATIVES_MAX: u8 = 5;

/// A request waiting for its response, keyed by its transaction id in
/// `TurnClient::pending`.
struct Pending {
    requete: Requete,
    /// Authentication refusals already absorbed by this request and the
    /// replays that preceded it.
    auth_refusals: u8,
}

/// Where the allocation stands. The transaction ids of the requests in flight
/// live in `TurnClient::pending`, which is what routes each response.
enum Etat {
    /// Nothing has gone out yet.
    Repos,
    /// Bare allocation sent, waiting for the 401.
    AttenteRefus,
    /// Signed allocation sent, waiting for success.
    AttenteAllocation,
    Allouee {
        echeance_refresh: Instant,
    },
    /// Refresh sent, waiting for its confirmation.
    AttenteRefresh {
        echeance_refresh: Instant,
    },
    /// Final abandonment. The session continues without a relay.
    Abandonnee,
}

pub struct TurnClient {
    serveur: SocketAddr,
    username: String,
    password: String,
    /// The four `pub(super)` fields below are those `canaux`, in a
    /// sibling module, needs to carry the second `impl TurnClient` block:
    /// bounded to `turn`, never `pub`. Same approach as
    /// `congestion::Controleur` with respect to `congestion::reconfiguration`.
    identifiants: Option<Identifiants>,
    cle: Option<Vec<u8>>,
    etat: Etat,
    pub(super) allocation: Option<Allocation>,
    /// Requests ready to go, in order.
    sortantes: std::collections::VecDeque<Vec<u8>>,
    maintenant: Instant,
    /// Requests sent and not yet answered, by transaction id. A response is
    /// handled only if it answers one of them, and according to what that
    /// request asked. Bounded: emitting a request removes the older one that
    /// asked for the same thing (`Requete::same_object`).
    pending: std::collections::HashMap<[u8; 12], Pending>,
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
    /// Peers whose permission or binding the server refused for good:
    /// `lier_canal` does not ask for them again, since the transport calls it
    /// before every packet it relays. Bounded by the peers ICE offers.
    pub(super) refused_peers: std::collections::HashSet<SocketAddr>,
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
            pending: std::collections::HashMap::new(),
            compteur_trans: 0,
            last_reading: now,
            canaux: std::collections::HashMap::new(),
            prochain_canal: super::canaux::CANAL_MIN,
            refused_peers: std::collections::HashSet::new(),
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
                Etat::AttenteRefus => "awaiting-refusal",
                Etat::AttenteAllocation => "awaiting-allocation",
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
                self.emettre_refresh(echeance_refresh, 0);
            }
        }
        // Independent of the lease state: a permission expires on its own
        // account, including while waiting for the response to a `Refresh`.
        self.rafraichir_canaux(now);
    }

    fn prochain_trans_id(&mut self) -> [u8; 12] {
        self.compteur_trans += 1;
        let mut id = [0u8; 12];
        id[..8].copy_from_slice(&self.compteur_trans.to_be_bytes());
        id
    }

    /// Encodes and queues a request, and records it as awaiting its response.
    ///
    /// Signed with the current credentials, except the bare allocation. Returns
    /// `None`, emitting nothing, when a signed request has no credentials yet.
    /// `pub(super)`: `canaux` emits its `CreatePermission` and `ChannelBind`
    /// through it, so that their responses are routed like the others.
    pub(super) fn send_request(&mut self, requete: Requete, auth_refusals: u8) -> Option<()> {
        let identifiants = match requete {
            Requete::AllocateNu => None,
            _ => Some((self.identifiants.clone()?, self.cle.clone()?)),
        };
        let signature = identifiants
            .as_ref()
            .map(|(ids, cle)| (ids, cle.as_slice()));
        let trans_id = self.prochain_trans_id();
        self.sortantes
            .push_back(encoder_requete(&requete, trans_id, signature));
        // The response to a request this one supersedes no longer matters:
        // forgetting it keeps the map bounded and a late answer harmless.
        self.pending
            .retain(|_, pending_request| !pending_request.requete.same_object(&requete));
        self.pending.insert(
            trans_id,
            Pending {
                requete,
                auth_refusals,
            },
        );
        Some(())
    }

    fn emettre_allocation_nue(&mut self) {
        self.send_request(Requete::AllocateNu, 0);
        self.etat = Etat::AttenteRefus;
    }

    fn emettre_allocation_signee(&mut self, auth_refusals: u8) {
        if self
            .send_request(Requete::AllocateSigne, auth_refusals)
            .is_some()
        {
            self.etat = Etat::AttenteAllocation;
        }
    }

    fn emettre_refresh(&mut self, echeance_refresh: Instant, auth_refusals: u8) {
        if self.identifiants.is_none() || self.cle.is_none() {
            tracing::warn!("TURN lease refresh impossible: credentials missing");
            return;
        }
        // One line every 300 s: rare enough to be logged at
        // `info`, and it is the only trace that says whether the lease is really
        // maintained (the acceptance run of 07/30/2026 spent an hour deducing it
        // from coturn's logs, for lack of this line).
        tracing::info!("TURN lease refresh sent");
        self.send_request(
            Requete::Refresh {
                duree_s: BAIL_DEMANDE_S,
            },
            auth_refusals,
        );
        self.etat = Etat::AttenteRefresh { echeance_refresh };
    }

    /// Traite un paquet venant du serveur TURN.
    ///
    /// Returns `Ok(None)` for any service message (allocation response,
    /// refresh response, error): it is absorbed by the state machine.
    /// Relayed data is handled by `desencapsuler` (see `canaux`).
    ///
    /// A response is routed by its transaction id to the request it answers,
    /// and handled according to that request: a response that answers no
    /// request in flight, or carries another method than its request's, is
    /// ignored.
    pub fn handle_packet(&mut self, data: &[u8]) -> anyhow::Result<Option<()>> {
        let message = is::stun::StunMessage::parse(data)
            .map_err(|e| anyhow::anyhow!("message TURN illisible : {e}"))?;
        // A parsed STUN message always holds its 20-byte header.
        let Some(trans_id) = data.get(8..20).and_then(|t| <[u8; 12]>::try_from(t).ok()) else {
            return Ok(None);
        };
        let Some(pending_request) = self.pending.remove(&trans_id) else {
            tracing::debug!("TURN response to no request in flight, ignored");
            return Ok(None);
        };
        if methode_de(data) != Some(pending_request.requete.methode()) {
            tracing::debug!("TURN response whose method differs from its request, ignored");
            self.pending.insert(trans_id, pending_request);
            return Ok(None);
        }

        if let Some((code, _raison)) = message.error_code() {
            self.handle_error(code, &message, pending_request);
            return Ok(None);
        }

        // Only responses to `Allocate` and `Refresh` carry a lease. A
        // response to `CreatePermission` or `ChannelBind` carries none, and
        // treating it as such would push the refresh back by 300 s
        // each time — to the point that it never goes out and the allocation
        // expires. Observed in the acceptance run of 07/30/2026, once channel
        // bindings were reaffirmed periodically: their responses, all arriving
        // every 150 s, pushed back a 300 s deadline indefinitely.
        match pending_request.requete {
            Requete::AllocateNu | Requete::AllocateSigne | Requete::Refresh { .. } => {
                if let Some(relayee) = message.xor_relayed_address() {
                    self.allocation = Some(Allocation {
                        relayee,
                        reflexive: message.mapped_address(),
                    });
                }
                let bail = message.lifetime().unwrap_or(BAIL_DEMANDE_S);
                // Refresh at HALF the lease: a single packet loss must
                // not be enough to lose the allocation.
                let echeance = self.maintenant + Duration::from_secs((bail / 2).max(1) as u64);
                self.etat = Etat::Allouee {
                    echeance_refresh: echeance,
                };
            }
            Requete::CreatePermission { .. } | Requete::ChannelBind { .. } => {}
        }
        Ok(None)
    }

    fn handle_error(
        &mut self,
        code: u16,
        message: &is::stun::StunMessage<'_>,
        pending_request: Pending,
    ) {
        let Pending {
            requete,
            auth_refusals,
        } = pending_request;
        match code {
            // 401: first refusal, carrying the realm and the nonce. A normal
            // step of the protocol, not a failure.
            // 438: stale nonce — coturn rotates them. Same approach:
            // adopt the new nonce and replay THE REQUEST IT REFUSED.
            401 | 438 => {
                let (Some(realm), Some(nonce)) = (message.realm(), message.nonce()) else {
                    self.fail_request(requete, "401/438 without realm or nonce");
                    return;
                };
                self.identifiants = Some(Identifiants {
                    username: self.username.clone(),
                    realm: realm.to_string(),
                    nonce: nonce.to_string(),
                });
                self.cle = Some(cle_longue_duree(&self.username, realm, &self.password));

                let auth_refusals = auth_refusals.saturating_add(1);
                if auth_refusals > TENTATIVES_MAX {
                    self.fail_request(requete, "too many authentication refusals");
                    return;
                }
                self.replay(requete, auth_refusals);
            }
            autre => {
                self.fail_request(requete, &format!("TURN error {autre}"));
            }
        }
    }

    /// Sends a refused request again, with the credentials just adopted.
    ///
    /// A 438 on a refresh, a permission or a channel binding must NOT
    /// reallocate: the allocation still exists on the server side, and a new
    /// `Allocate` on the same 5-tuple would be refused (437).
    fn replay(&mut self, requete: Requete, auth_refusals: u8) {
        match requete {
            Requete::AllocateNu | Requete::AllocateSigne => {
                self.emettre_allocation_signee(auth_refusals)
            }
            Requete::Refresh { .. } => {
                let echeance = match self.etat {
                    Etat::AttenteRefresh { echeance_refresh }
                    | Etat::Allouee { echeance_refresh } => echeance_refresh,
                    _ => self.maintenant,
                };
                self.emettre_refresh(echeance, auth_refusals);
            }
            Requete::CreatePermission { .. } | Requete::ChannelBind { .. } => {
                self.send_request(requete, auth_refusals);
            }
        }
    }

    /// A request failed for good. An allocation request ends the relay; a
    /// permission or a channel binding only costs its peer the relay.
    fn fail_request(&mut self, requete: Requete, raison: &str) {
        match requete.pair() {
            Some(pair) => self.drop_peer(pair, raison),
            None => self.abandonner(raison),
        }
    }

    /// Drops the channel serving this peer, and forgets its requests in
    /// flight. The allocation and the other peers' channels stay.
    fn drop_peer(&mut self, pair: SocketAddr, raison: &str) {
        tracing::warn!(
            %pair,
            raison,
            "TURN channel dropped: this peer goes on without a relay"
        );
        self.canaux.retain(|_, canal| canal.pair != pair);
        self.refused_peers.insert(pair);
        self.pending
            .retain(|_, pending_request| pending_request.requete.pair() != Some(pair));
    }

    fn abandonner(&mut self, raison: &str) {
        tracing::warn!(
            raison,
            "TURN allocation abandoned: the session will go on without a relay"
        );
        self.etat = Etat::Abandonnee;
        self.allocation = None;
        self.pending.clear();
    }
}

#[cfg(test)]
#[path = "allocation/tests.rs"]
mod tests;
