//! The encoder pool: who sleeps, who wakes.
//!
//! **No `#[cfg(windows)]`, no COM object, no channel.** This module only
//! decides; applying the decisions lives in `capteur/sommeil.rs`
//! and `capteur/fenetre.rs`. It is the pattern set by D4 for
//! `capteur/protocole.rs` and `capteur/distante.rs`: what decides is tested on
//! the host, and here it is the piece that costs most to get wrong.
//!
//! **Why an LRU and not "first come, first served".** The window
//! in the foreground must always win: it is the user's hand that
//! arbitrates, without them having anything to configure.

use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Number of simultaneously live encoders the sensor allows itself.
///
/// **Read on this VM, not a system bound**: measured on 30 and
/// 31 July 2026 (the 9th creation refused at the NVIDIA MFT's `SetOutputType`,
/// `MF_E_UNSUPPORTED_D3D_TYPE`), unchanged whether the encoders share a
/// D3D11 device or each have a new one. **The layer that
/// imposes it is not identified.**
pub const PLAFOND_EVEIL: usize = 8;

/// Minimum wake time before a window can be EVICTED.
///
/// ⚠️ **NOT CALIBRATED value.** It bounds flapping — ten visible windows
/// and a user going from one to another would otherwise rebuild a
/// DXGI duplication and an encoder per focus change. The number
/// of fall-asleeps recorded in the acceptance run is what will judge it, not an
/// intuition.
///
/// It does NOT protect against a deliberate standby: hiding is an
/// explicit gesture of the user.
pub const HYSTERESIS: Duration = Duration::from_secs(2);

/// Wait time after a window's wake-up failure before proposing it again.
///
/// ⚠️ **NOT CALIBRATED value.** It bounds the replay frequency of a refused
/// wake-up: without it, a window whose encoder construction fails
/// would be relaunched at every arbitration in a tight loop. The number of
/// wake-up attempts observed in the acceptance run is what will judge it.
pub const REPIT_APRES_ECHEC: Duration = Duration::from_millis(500);

/// Why a window falls asleep. The two cases are not equivalent for
/// the user, and the client shows them differently.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Raison {
    /// They wanted it: the window is minimised or its tab is hidden.
    Masquee,
    /// The pool took it from them while they were looking at it.
    Evincee,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ordre {
    Dormir(Raison),
    Reveiller,
}

struct Entree {
    visible: bool,
    /// Instant of the last focus or of the last return to visibility.
    ///
    /// **Visibility alone would not be enough to order an LRU**: ten
    /// windows all visible have exactly the same visibility, and
    /// eviction would then be arbitrary.
    last_seen: Instant,
    eveillee: bool,
    eveillee_depuis: Instant,
    /// Instant of the last wake-up failure, or `None` if it never failed or not for
    /// a long time. Excludes the window from the candidates as long as the respite has not
    /// elapsed.
    last_failure: Option<Instant>,
}

pub struct Vivier {
    plafond: usize,
    hysteresis: Duration,
    entrees: HashMap<String, Entree>,
}

impl Vivier {
    pub fn new(plafond: usize, hysteresis: Duration) -> Vivier {
        Vivier {
            plafond,
            hysteresis,
            entrees: HashMap::new(),
        }
    }

    pub fn inscrire(&mut self, session: &str, maintenant: Instant) -> Vec<(String, Ordre)> {
        // A window is born ASLEEP: the client will announce its visibility, and
        // it is that which will wake it up. Being born awake would exceed the
        // cap between the attach and the first signal.
        self.entrees.insert(
            session.to_string(),
            Entree {
                visible: false,
                last_seen: maintenant,
                eveillee: false,
                eveillee_depuis: maintenant,
                last_failure: None,
            },
        );
        self.arbitrer(maintenant)
    }

    pub fn retirer(&mut self, session: &str, maintenant: Instant) -> Vec<(String, Ordre)> {
        self.entrees.remove(session);
        self.arbitrer(maintenant)
    }

    pub fn signaler(
        &mut self,
        session: &str,
        visible: bool,
        focalisee: bool,
        maintenant: Instant,
    ) -> Vec<(String, Ordre)> {
        let Some(entree) = self.entrees.get_mut(session) else {
            // A signal may arrive from a child whose window has just been
            // removed. Ignore, never panic.
            return Vec::new();
        };
        // Recency is refreshed on focus AND on return to visibility: these
        // are the two ways the user says "I am looking at this one".
        if focalisee || (visible && !entree.visible) {
            entree.last_seen = maintenant;
        }
        entree.visible = visible;
        self.arbitrer(maintenant)
    }

    /// Records a window's wake-up failure and updates the state.
    ///
    /// Called by the sensor when the encoder construction fails.
    /// Puts the entry back to `eveillee = false` and sets a respite, then
    /// re-arbitrates to try to fill the place thus freed.
    pub fn echec_de_reveil(&mut self, session: &str, maintenant: Instant) -> Vec<(String, Ordre)> {
        let Some(entree) = self.entrees.get_mut(session) else {
            // A signal may arrive from a child whose window has just been
            // removed. Ignore, never panic.
            return Vec::new();
        };
        entree.eveillee = false;
        entree.last_failure = Some(maintenant);
        self.arbitrer(maintenant)
    }

    /// Cancels the state mutation an order produced, when that order
    /// **could not be dropped** into its window's queue.
    ///
    /// 🔴 WHY THIS METHOD EXISTS — THE SIXTH MEMORISATION SITE,
    /// FOUND IN FIX ROUND 2 (25 August 2026). `arbitrer` writes
    /// `eveillee` **BEFORE the order goes out** (steps 1 and 5). It is
    /// exactly the pattern of `dernieres_parts` and `derniers_audio` that
    /// round 1 fixed in `sommeil/` — but on the only variant the
    /// window APPLIES instead of relaying it, and with a cost in BOTH
    /// directions:
    ///
    /// - **Refused `Reveiller`**: `eveillee` stays `true`, the pool's place
    ///   is occupied without any real encoder occupying it, and `arbitrer`
    ///   being idempotent, **no future re-arbitration re-emits the order** —
    ///   measured: ten `rearbitrer` in a row return nothing. It is a window
    ///   that no longer wakes up, for the life of the process.
    /// - **Refused `Dormir`**: `eveillee` goes to `false` **for good**
    ///   while the window still holds its encoder — and **no
    ///   arbitration will ever order it again**, since the pool believes it already
    ///   asleep. ⚠️ **It is the costlier half, and it is the one we
    ///   had missed**: there is an `echec_de_reveil` for the first
    ///   direction, there is **no** `echec_de_sommeil`.
    ///
    /// 🔴 **WHAT THIS METHOD DOES NOT PREVENT, AND WHICH WAS OVER-CLAIMED**
    /// (fix round 3): it does **not** prevent over-subscription of the
    /// encoder cap. `arbitrer` frees the slot at step 1, elects the
    /// replacement at step 4 and emits its `Reveiller` at step 5 — **all
    /// in the same pass, before the drop is even attempted**;
    /// the cancellation only runs afterwards. Measured: `eveillees()` does rise to 9
    /// for a cap of 8. **What it achieves is that this
    /// over-subscription is TRANSIENT instead of permanent** — at the
    /// next re-arbitration, the pool sees 9 > 8 and puts someone back to sleep, whereas
    /// without it it would never see 9 and would let the drift settle in.
    /// The test
    /// `sommeil::tests_refus::an_oversubscription_from_an_undelivered_sleep_is_absorbed_next_round`
    /// measures it at both times.
    ///
    /// 🔴 THE REMEDY IS "DO NOT LIE", NOT "RETRY". The state
    /// becomes again that from BEFORE the order, so the pool describes
    /// reality again; the next arbitration sees the window in its old state and
    /// **re-emits the order by itself**. Nothing is retried inside
    /// `distribuer`, so **the termination of its loop is not touched** —
    /// it is the next wheel round that takes over.
    ///
    /// ⚠️ **WHAT THE CANCELLATION DOES NOT RESTORE, and it must be said**: on a
    /// `Reveiller`, `arbitrer` set `last_failure = None`, and the previous value
    /// is not memorised. It is therefore not given back. The
    /// consequence is **intended**: the session becomes a candidate again without
    /// respite, which is precisely what we want — that the next
    /// arbitration re-elects it and re-emits its order. `eveillee_depuis`, for its part,
    /// is only read on an awake entry: resetting it would have no effect.
    /// On a `Dormir`, the cancellation is exact — `arbitrer` only touches
    /// `eveillee` there.
    ///
    /// **No effect if the session has disappeared in the meantime**, never a panic:
    /// it is the regime of `echec_de_reveil` just above, and for the same
    /// reason.
    ///
    /// **Does NOT re-arbitrate and returns no order**, unlike
    /// `echec_de_reveil`: it is called FROM the loop of
    /// `sommeil::registre::distribuer`, which is in the middle of distributing a batch.
    /// Generating one more batch there would make its termination depend on a path
    /// its proof does not cover.
    pub fn annuler_ordre_non_livre(&mut self, session: &str, ordre: Ordre) {
        let Some(entree) = self.entrees.get_mut(session) else {
            return;
        };
        match ordre {
            Ordre::Reveiller => entree.eveillee = false,
            Ordre::Dormir(_) => entree.eveillee = true,
        }
    }

    /// Periodic re-arbitration, called by the thread of `sommeil.rs`.
    ///
    /// **Indispensable, not a luxury**: under hysteresis, a window that
    /// asks to wake may be refused. It is then already visible and
    /// already focused — no signal will ever come to unblock it, and it
    /// would sleep forever without this wheel round.
    pub fn rearbitrer(&mut self, maintenant: Instant) -> Vec<(String, Ordre)> {
        self.arbitrer(maintenant)
    }

    #[cfg(test)]
    pub fn eveillee(&self, session: &str) -> Option<bool> {
        self.entrees.get(session).map(|e| e.eveillee)
    }

    /// The currently awake sessions, in an unspecified order.
    ///
    /// Read by `sommeil/parts.rs` to feed the bitrate distributor (D6):
    /// a window's share depends on its wakefulness, and the pool is the only
    /// source of truth on that point. The present tense is indeed right — this
    /// reader has existed since task 4 of the sub-block.
    pub fn eveillees(&self) -> Vec<String> {
        self.entrees
            .iter()
            .filter(|(_, entree)| entree.eveillee)
            .map(|(session, _)| session.clone())
            .collect()
    }

    /// The heart: computes the target set of awake windows, and derives the
    /// transitions from it. **Idempotent** — called twice in a row without a change
    /// of state or time, it returns nothing the second time.
    ///
    /// **Order of the returned vector**: all `Dormir` precede any
    /// `Reveiller`, each group being sorted by session name. A wake-up
    /// applied before the sleep that finances it would transiently require one
    /// encoder more than the cap.
    fn arbitrer(&mut self, maintenant: Instant) -> Vec<(String, Ordre)> {
        let mut ordres_dormir = Vec::new();
        let mut ordres_reveiller = Vec::new();

        // 1. Any awake window that became invisible falls asleep. Without hysteresis: the
        //    hiding is explicit.
        let masquees: Vec<String> = self
            .entrees
            .iter()
            .filter(|(_, e)| e.eveillee && !e.visible)
            .map(|(nom, _)| nom.clone())
            .collect();
        for nom in masquees {
            if let Some(e) = self.entrees.get_mut(&nom) {
                e.eveillee = false;
            }
            ordres_dormir.push((nom, Ordre::Dormir(Raison::Masquee)));
        }

        // 2. The pinned ones: awake, still visible, and woken up less
        //    than the hysteresis ago. They keep their place no matter
        //    what.
        let epinglees: Vec<String> = self
            .entrees
            .iter()
            .filter(|(_, e)| {
                e.eveillee
                    && e.visible
                    && maintenant.saturating_duration_since(e.eveillee_depuis) < self.hysteresis
            })
            .map(|(nom, _)| nom.clone())
            .collect();

        // 3. The candidates: all visible ones, except those in respite after
        //    a failure, from the most recently seen to the oldest. A total order
        //    is necessary so that the result does not depend on the traversal
        //    of a hash table: at equal recency, the name breaks the tie.
        let mut candidates: Vec<(String, Instant)> = self
            .entrees
            .iter()
            .filter(|(_, e)| {
                e.visible
                    && e.last_failure.is_none_or(|t| {
                        maintenant.saturating_duration_since(t) >= REPIT_APRES_ECHEC
                    })
            })
            .map(|(nom, e)| (nom.clone(), e.last_seen))
            .collect();
        candidates.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

        // 4. The target set: the pinned ones first, then the most recent
        //    candidates until the cap is filled.
        let mut cible: Vec<String> = epinglees.clone();
        for (nom, _) in candidates {
            if cible.len() >= self.plafond {
                break;
            }
            if !cible.contains(&nom) {
                cible.push(nom);
            }
        }

        // 5. Les transitions.
        let noms: Vec<String> = self.entrees.keys().cloned().collect();
        for nom in noms {
            let doit_veiller = cible.contains(&nom);
            let Some(e) = self.entrees.get_mut(&nom) else {
                continue;
            };
            if doit_veiller && !e.eveillee {
                e.eveillee = true;
                e.eveillee_depuis = maintenant;
                e.last_failure = None;
                ordres_reveiller.push((nom, Ordre::Reveiller));
            } else if !doit_veiller && e.eveillee {
                e.eveillee = false;
                ordres_dormir.push((nom, Ordre::Dormir(Raison::Evincee)));
            }
        }

        // Sort each group for total determinism.
        ordres_dormir.sort_by(|a, b| a.0.cmp(&b.0));
        ordres_reveiller.sort_by(|a, b| a.0.cmp(&b.0));

        // Returned in order: all sleeps before all wake-ups.
        ordres_dormir.extend(ordres_reveiller);
        ordres_dormir
    }
}

/// The tests live in a sibling file: this file is approaching the project's
/// 500-line cap, which this extraction prevents it from crossing. The test
/// module remains entirely compiled and run.
#[cfg(test)]
mod tests;
