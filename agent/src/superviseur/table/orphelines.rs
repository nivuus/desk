//! The entries that no longer advance: those whose child is dead but whose
//! Windows window still lives, those the shell page left
//! waiting for their viewport, and — since August 30th, 2026 — those that must be
//! TOLD AGAIN to a shell page that has just arrived.
//!
//! ⚠️ **This module's title said "the PERIODIC check", and that is no
//! longer true of all its content**: `reannoncer_les_attentes` is called
//! on an EVENT (the arrival of a `client` peer), not on a clock tick.
//! The two functions do share exactly what matters here —
//! the `DELAI_ATTENTE_VIEWPORT_MAX` clock —, and that is what earns them
//! living side by side rather than splitting.
//!
//! Extracted from `table.rs` (task 17 of sub-block P3) to stay under the
//! project's 500-line ceiling — not for a design reason,
//! exactly like `attribution.rs`: this method stays a method of
//! `Table` like the others, in the same logical module, just in a
//! neighbouring file. **VERBATIM extraction**: no line was
//! reworded, and above all no comment was compressed — the
//! compression to get back under the line has already been played elsewhere in this
//! repository (`agent/src/encode/arret.rs`) and it is only played once.

use super::*;

impl Table {
    /// Offers again the windows whose session is dead but which still exist
    /// on the Windows side, and abandons entries frozen too long
    /// waiting for a viewport. Called by the supervisor's periodic
    /// check.
    ///
    /// **Reads no clock**: `maintenant` is received as an argument, on the
    /// model of `FenetreDeReprise` (`agent/src/capture/reprise.rs`) — it is
    /// what keeps this table testable on the Linux host.
    ///
    /// Two independent safeguards, both necessary:
    /// - `RELANCES_MAX` bounds the number of times a window whose CHILD
    ///   dies is restarted (the entry changes session identifier at
    ///   each restart — a reused identifier would pair a late
    ///   browser message with the wrong window — so each orphan
    ///   is removed then reinserted under a new session, `relances`
    ///   carried over; `self.compteur` keeps growing without ever going back);
    /// - `DELAI_ATTENTE_VIEWPORT_MAX` bounds the time spent in `AttendLeViewport`,
    ///   restarted or not since sub-block D3, if the SHELL PAGE, for its part, never
    ///   answers.
    pub fn relancer_les_orphelines(&mut self, maintenant: std::time::Instant) -> Vec<Effet> {
        let orphelines: Vec<IdSession> = self
            .entrees
            .iter()
            .filter(|(_, e)| e.etat == Etat::SansSession)
            .map(|(s, _)| s.clone())
            .collect();
        let mut effets = Vec::new();
        for ancienne in orphelines {
            let entree = self.entrees.remove(&ancienne).expect("read just now");
            if entree.relances >= RELANCES_MAX {
                effets.extend(rendre_la_sortie_de(&entree));
                effets.push(Effet::AnnoncerRefus {
                    titre: entree.titre,
                    motif: format!("the session did not hold after {RELANCES_MAX} attempts"),
                });
                continue;
            }
            let session = self.prochaine_session();
            self.entrees.insert(
                session.clone(),
                Entree {
                    fenetre: entree.fenetre,
                    titre: entree.titre.clone(),
                    etat: Etat::AttendLeViewport,
                    // Carried over, like `relances`: the virtual output survives
                    // the restart (§7.1).
                    sortie_pilote: entree.sortie_pilote,
                    nom_sortie: entree.nom_sortie.clone(),
                    output_size: entree.output_size,
                    relances: entree.relances + 1,
                    attente_depuis: Some(maintenant),
                },
            );
            effets.push(Effet::AnnoncerOuverture {
                session,
                titre: entree.titre,
            });
        }

        // LAZY timestamp (§7.3 of sub-block D2, fixed in D3). An entry
        // coming from `fenetre_apparue` was not timestamped: this function
        // is the only place that receives an instant, and `fenetre_apparue`
        // must stay pure. So it is timestamped here, on the first pass.
        //
        // ⚠️ **Behaviour change at startup**: windows from the
        // initial enumeration stop being exempted. If the shell page
        // connects more than `DELAI_ATTENTE_VIEWPORT_MAX` after the supervisor,
        // they will be abandoned — and an abandoned entry is NEVER
        // offered again, the hook re-emitting nothing for an already
        // open window. Consistent with the trap of acceptance run D1 ("launch the
        // browser BEFORE the supervisor"), but worth knowing.
        //
        // The delay runs from this first pass, paced by
        // `PERIODE_PLACEMENT` (1 s), and not from startup.
        for entree in self.entrees.values_mut() {
            if entree.etat == Etat::AttendLeViewport && entree.attente_depuis.is_none() {
                entree.attente_depuis = Some(maintenant);
            }
        }

        // Second safeguard: an entry in `AttendLeViewport` whose
        // shell page never answers — neither `SansSession` (it no longer
        // is, or never was), nor `Vivante` (it will never
        // reach it) — and would therefore NEVER be picked up by the filter
        // above. Every waiting entry now carries an
        // `attente_depuis` since the preceding loop.
        let figees: Vec<IdSession> = self
            .entrees
            .iter()
            .filter(|(_, e)| {
                e.etat == Etat::AttendLeViewport
                    && e.attente_depuis.is_some_and(|depuis| {
                        maintenant.duration_since(depuis) > DELAI_ATTENTE_VIEWPORT_MAX
                    })
            })
            .map(|(s, _)| s.clone())
            .collect();
        for figee in figees {
            let entree = self.entrees.remove(&figee).expect("read just now");
            effets.extend(rendre_la_sortie_de(&entree));
            effets.push(Effet::AnnoncerRefus {
                titre: entree.titre,
                motif: "the shell page never answered after the relaunch".into(),
            });
        }

        effets
    }

    /// Tells a shell page that has just arrived what the table already knows,
    /// and resets the countdown of each entry told again.
    ///
    /// 🔴 **THE DEFECT IT FIXES, MEASURED IN PRODUCTION ON AUGUST 30TH,
    /// 2026** (network capture `vnet30`, `tcpdump` + `tshark`): the
    /// supervisor announces its windows at t = 12.0 s, in a control
    /// session where NO `client` peer is connected yet. The relay
    /// drops the announcement without a trace
    /// (`plateforme/src/signaling/relais.ts`, `send(peer, …)` on an absent
    /// `peer`), and at t = 43.1 s the same windows are refused by the
    /// loop above, for lack of a `viewport` in return. The user spends
    /// precisely those seconds in the proxy's authentication: they
    /// therefore arrive in front of an empty desktop, on a VM full of windows.
    ///
    /// 🔴 **IT IS NOT A LENGTHENING OF THE DELAY, AND THAT WAS THE INSTRUCTION.**
    /// Lengthening `DELAI_ATTENTE_VIEWPORT_MAX` would move the race without
    /// removing it: a slower user would lose it again. Here the clock
    /// RESTARTS from the moment a shell page is actually there — which is
    /// precisely what the constant claims to measure ("the time a
    /// shell page takes to answer"), and never what it really
    /// measured (the time elapsed since the supervisor's startup,
    /// shell page or not).
    ///
    /// 🔴 **WHAT THE BOUND PROTECTED IS INTACT.** It protects two
    /// things: a place in `capacite` (10), and — for a
    /// RESTARTED entry — the virtual output RETAINED by `enfant_mort` (§7.1 of
    /// D3), which is the costly resource. Neither is
    /// released here: the bound keeps running, it simply runs from
    /// an instant that makes sense. An entry whose shell stays
    /// silent thirty seconds AFTER its arrival is abandoned as before.
    ///
    /// ⚠️ **ONLY ENTRIES IN `AttendLeViewport` ARE TOLD AGAIN, AND THE
    /// REST IS DELIBERATE:**
    /// - `AttendLaSortie`: the viewport is already known, the output is being
    ///   created — the shell that requested it is gone, but
    ///   the child that follows will land on a session whose offer no one
    ///   is waiting for any more; it is a case no measurement has exercised and
    ///   that we do not guess at here.
    /// - `Vivante`: **telling it again would OPEN a page that cannot
    ///   connect.** The child consumes ONE offer and never renegotiates
    ///   (`agent/src/demarrage.rs`: "no renegotiation in this
    ///   session"), so the reopened page would send an offer no one
    ///   would take. **It is a named legacy, not an oversight**: a
    ///   reload of the shell page does not recover already live
    ///   windows.
    /// - `SansSession`: already served by `relancer_les_orphelines`
    ///   above, which offers it again under a new session. Telling it again here
    ///   would duplicate it.
    ///
    /// ⚠️ **WHAT IS MISSING HERE IS ELSEWHERE, AND IT IS INTENDED**: windows
    /// already ABANDONED are no longer in the table, so this method
    /// can do nothing for them. It is `boucle.rs` that replays the
    /// startup enumeration for them (`hook::enumerer_existantes`), whose
    /// `fenetre_apparue` is idempotent per `HWND` — hence the ORDER imposed
    /// there: tell again first, enumerate afterwards, otherwise an entry
    /// still waiting would be announced TWICE and the shell page
    /// would reload the window it has just opened.
    pub fn reannoncer_les_attentes(&mut self, maintenant: std::time::Instant) -> Vec<Effet> {
        let mut effets = Vec::new();
        for (session, entree) in self.entrees.iter_mut() {
            if entree.etat != Etat::AttendLeViewport {
                continue;
            }
            entree.attente_depuis = Some(maintenant);
            effets.push(Effet::AnnoncerOuverture {
                session: session.clone(),
                titre: entree.titre.clone(),
            });
        }
        effets
    }
}
