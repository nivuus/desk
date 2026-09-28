//! The rule: who carries the sound.
//!
//! **Pure, without any `cfg`, without COM, without a window** — like `vivier.rs` and
//! `repartiteur.rs` before it. It knows neither `IAudioClient` nor `HWND`: it
//! receives PIDs and returns booleans.
//!
//! **Sleep does NOT enter the rule**, and its absence from this file is
//! the best place to say so: `FenetreAudio` carries no
//! `eveillee` field. A sleeping window (sub-block D5) has released its video
//! encoder; its application may perfectly well keep playing music,
//! and that is precisely the case where we want sound without picture.

use std::cmp::Ordering;
use std::collections::HashMap;

/// What the registry knows about a window, from the sound's point of view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FenetreAudio {
    pub session: String,
    /// PID of the process owning the Windows window.
    pub pid: u32,
    /// Arrival rank, strictly increasing. Breaks the tie between two windows of the
    /// same process of which **neither** has ever been focused.
    pub arrivee: u64,
    /// Rank of the last focus received, `0` if this session has never been
    /// focused. **A rank, not a timestamp**: an `Instant` is not
    /// comparable across processes and would bring nothing here.
    pub dernier_focus: u64,
    /// This window cannot carry the sound right now.
    ///
    /// True when the child has reported `AudioMort` and the session observes
    /// its re-arm respite.
    ///
    /// ⚠️ **This field said "when its WASAPI capture has died — ten consecutive read
    /// errors (`LECTURES_ECHOUEES_MAX`)", and that has no longer been
    /// the trigger since sub-block D10** (cross-cutting review; same
    /// fix as on `VersCapteur::AudioMort`, `capteur/protocole.rs`).
    /// Those ten errors set `capture_morte` on the child side, nothing more:
    /// `AudioMort` — hence `inapte` — only arrives after the rebuild budget
    /// is exhausted (`crate::audio::RECONSTRUCTIONS_MAX`), or in
    /// the absence of a rebuilder.
    ///
    /// ⚠️ **A `bool`, never an `Instant`.** The respite's expiry lives in the
    /// registry, which has the clock; this module keeps its doctrine — "a rank, not
    /// a timestamp" — and remains testable without a clock.
    pub inapte: bool,
}

/// Returns, for each window, whether it carries the sound.
///
/// **One entry per window, including silent ones**: the registry needs the
/// `false` to order the one that carried the sound a moment
/// before to go silent.
pub fn arbitrer(fenetres: &[FenetreAudio]) -> Vec<(String, bool)> {
    let mut porteur: HashMap<u32, &FenetreAudio> = HashMap::new();
    for f in fenetres {
        // An unfit one is never a CANDIDATE. It still receives its
        // verdict below, which will be `false`: it is this `false` that orders
        // the one that carried the sound a moment before to go silent.
        if f.inapte {
            continue;
        }
        match porteur.get(&f.pid) {
            Some(actuel) if !l_emporte(f, actuel) => {}
            _ => {
                porteur.insert(f.pid, f);
            }
        }
    }

    fenetres
        .iter()
        .map(|f| {
            let actif = porteur.get(&f.pid).is_some_and(|p| p.session == f.session);
            (f.session.clone(), actif)
        })
        .collect()
}

/// Does `candidat` win over `actuel` within their PID group?
///
/// The MOST RECENT focus wins; on a tie — two never-focused windows,
/// hence `dernier_focus == 0` for both — the FIRST arrival. Sleep
/// does not enter the comparison, and no field carries it.
fn l_emporte(candidat: &FenetreAudio, actuel: &FenetreAudio) -> bool {
    match candidat.dernier_focus.cmp(&actuel.dernier_focus) {
        Ordering::Greater => true,
        Ordering::Less => false,
        Ordering::Equal => candidat.arrivee < actuel.arrivee,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fenetre(session: &str, pid: u32, arrivee: u64, dernier_focus: u64) -> FenetreAudio {
        FenetreAudio {
            session: session.into(),
            pid,
            arrivee,
            dernier_focus,
            inapte: false,
        }
    }

    fn porteurs(fenetres: &[FenetreAudio]) -> Vec<String> {
        let mut noms: Vec<String> = arbitrer(fenetres)
            .into_iter()
            .filter(|(_, actif)| *actif)
            .map(|(session, _)| session)
            .collect();
        noms.sort();
        noms
    }

    #[test]
    fn une_fenetre_seule_de_son_processus_porte_le_son() {
        let f = vec![fenetre("a", 100, 1, 0)];
        assert_eq!(porteurs(&f), vec!["a".to_string()]);
    }

    #[test]
    fn deux_processus_distincts_portent_chacun_le_leur() {
        // The product's nominal case: one application per window.
        let f = vec![fenetre("a", 100, 1, 0), fenetre("b", 200, 2, 0)];
        assert_eq!(porteurs(&f), vec!["a".to_string(), "b".to_string()]);
    }

    #[test]
    fn deux_fenetres_d_un_meme_processus_jamais_focalisees_la_premiere_arrivee_porte() {
        let f = vec![fenetre("a", 100, 1, 0), fenetre("b", 100, 2, 0)];
        assert_eq!(porteurs(&f), vec!["a".to_string()]);
    }

    #[test]
    fn le_focus_prend_le_son_a_sa_voisine_du_meme_processus() {
        // "b" arrives after "a" and takes the focus: the sound switches.
        let f = vec![fenetre("a", 100, 1, 0), fenetre("b", 100, 2, 7)];
        assert_eq!(porteurs(&f), vec!["b".to_string()]);
    }

    #[test]
    fn un_groupe_qui_perd_tout_focus_garde_son_son_sur_la_derniere_focalisee() {
        // It is rule 3 of the spec, and it is not cosmetic:
        // `focalisee` is GLOBAL — at most one focused window over the whole
        // session. Clicking on a window of ANOTHER process makes this whole group
        // lose the focus, and without this rule its sound would cut out.
        let f = vec![
            fenetre("a", 100, 1, 3),
            fenetre("b", 100, 2, 7),
            fenetre("etranger", 200, 3, 9),
        ];
        assert_eq!(porteurs(&f), vec!["b".to_string(), "etranger".to_string()]);
    }

    #[test]
    fn l_oubli_du_porteur_fait_passer_le_son_a_la_suivante_du_groupe() {
        // The registry removes "b" (channel broken, close) and calls
        // `arbitrer` again on what remains: "a" must take the sound back, otherwise
        // the group would become permanently silent.
        let f = vec![fenetre("a", 100, 1, 3)];
        assert_eq!(porteurs(&f), vec!["a".to_string()]);
    }

    #[test]
    fn la_decision_est_rendue_pour_chaque_session_meme_muette() {
        // `arbitrer` returns one entry per window, not only for the
        // carriers: the registry needs the `false` to send the order to
        // go silent to the one that carried the sound just before.
        let f = vec![fenetre("a", 100, 1, 0), fenetre("b", 100, 2, 0)];
        let decisions = arbitrer(&f);
        assert_eq!(decisions.len(), 2);
        assert!(decisions.contains(&("a".to_string(), true)));
        assert!(decisions.contains(&("b".to_string(), false)));
    }

    #[test]
    fn aucune_fenetre_rend_aucune_decision() {
        assert!(arbitrer(&[]).is_empty());
    }

    #[test]
    fn une_fenetre_inapte_ne_porte_jamais_le_son() {
        // Hand-over 1's case: its WASAPI capture died after ten consecutive
        // failures. It must no longer be elected, otherwise the whole group
        // stays silent — that is the state before D9.
        let fenetres = vec![
            FenetreAudio {
                session: "w-1".into(),
                pid: 42,
                arrivee: 1,
                dernier_focus: 9,
                inapte: true,
            },
            FenetreAudio {
                session: "w-2".into(),
                pid: 42,
                arrivee: 2,
                dernier_focus: 0,
                inapte: false,
            },
        ];
        let verdict = arbitrer(&fenetres);
        assert_eq!(verdict, vec![("w-1".into(), false), ("w-2".into(), true)]);
    }

    #[test]
    fn une_inapte_recoit_quand_meme_son_verdict_false() {
        // The registry needs this `false` to order the one that carried
        // the sound a moment before to go silent.
        let fenetres = vec![FenetreAudio {
            session: "w-1".into(),
            pid: 42,
            arrivee: 1,
            dernier_focus: 1,
            inapte: true,
        }];
        assert_eq!(arbitrer(&fenetres), vec![("w-1".into(), false)]);
    }

    #[test]
    fn un_groupe_entierement_inapte_reste_muet() {
        // No neighbour to promote: this is the MAJORITY case — one
        // application, one window.
        //
        // ❌ **This comment said "re-arming after a respite is the only
        // remedy, and it lives in the registry, not here". The remedy in question
        // is INERT** (VM acceptance run of task 15, sub-block D9): re-electing the
        // same session rebuilds NO capture — `set_audio_source` is
        // called only once (`demarrage/audio.rs`), the capture thread
        // (`windows_audio.rs`) does a definitive `return`, and `set_actif(true)`
        // only writes an atomic this thread never re-reads. The complete
        // refutation lives next to `REPIT_REARMEMENT_AUDIO`
        // (`capteur/sommeil.rs`); it is repeated HERE because this is where
        // the majority case is read, and this repository has paid five times for having
        // fixed a claim where it was shown to it rather than where
        // it lives. **The majority case therefore remains WITHOUT A REMEDY, and it is a
        // hand-over from D9.**
        //
        // ✅ **THIS HAND-OVER IS CLOSED ON THE EVIDENCE — code plus host tests —,
        // ~~NOT EXERCISED ON THE VM~~ (sub-block D10, tasks 11 and 12).**
        // ~~The audio acceptance run that would exercise it is task 14, and it has not
        // run yet: do not read what follows as measured.~~
        //
        // ✅ **IT RAN, AND THE SOUND CAME BACK (7 August 2026, task 14 of the
        // SAME sub-block — the claim above was refuted in the
        // branch that wrote it, and it was the cross-cutting review that
        // caught it).** Under fault injection (`AUDIO_FAUTE_LECTURE`), the
        // capture is rebuilt and the window hears its own
        // tone again: dominant frequency **441 Hz at −40 dB** for an
        // assigned target of 440 Hz, floor at −158 dB, on **both** runs and
        // at both checkpoints (t+15 s and t+60 s), with
        // `compteurs audio … actif=true` read twice per run.
        // ⚠️ **Two runs, no rate** — and the capture death there is
        // INJECTED: nothing establishes that a natural cause exists. Fixed RIGHT HERE, where
        // the comment above explicitly said it had to be
        // fixed — the code itself named the place to fix. The
        // remedy is NOT re-election: it is
        // `Session::reconstruire_ou_signaler` (`transport/piste_audio.rs`),
        // called BEFORE any `AudioMort` report, which really
        // rebuilds the capture — the sentence "re-electing alone rebuilds
        // nothing" stays true of what it described, but the product no longer
        // relies ONLY on re-election for the majority case.
        // And re-election has gained a role it did not have then: it
        // replenishes the attempt budget and lifts the
        // `audio_mort_signale` latch (`appliquer_audio`, `piste_audio.rs`) — without
        // which, found in review of task 12, the dead → rebuilt →
        // proven cycle would have run only once per session, and
        // `REARMEMENTS_MAX` would have counted non-consecutive failures over
        // its whole life rather than consecutive failures.
        //
        // What this test establishes, and which remains right: the pure RULE makes
        // nobody carry the sound when the whole group is unfit — a state
        // that is no longer PERMANENT for the window alone in its group (the
        // majority case): the respite expires, it becomes a candidate again, and its
        // own rebuild retries.
        let fenetres = vec![
            FenetreAudio {
                session: "w-1".into(),
                pid: 42,
                arrivee: 1,
                dernier_focus: 0,
                inapte: true,
            },
            FenetreAudio {
                session: "w-2".into(),
                pid: 42,
                arrivee: 2,
                dernier_focus: 0,
                inapte: true,
            },
        ];
        assert_eq!(
            arbitrer(&fenetres),
            vec![("w-1".into(), false), ("w-2".into(), false)]
        );
    }

    #[test]
    fn l_inaptitude_d_un_groupe_ne_touche_pas_un_autre_pid() {
        let fenetres = vec![
            FenetreAudio {
                session: "w-1".into(),
                pid: 42,
                arrivee: 1,
                dernier_focus: 0,
                inapte: true,
            },
            FenetreAudio {
                session: "w-2".into(),
                pid: 77,
                arrivee: 2,
                dernier_focus: 0,
                inapte: false,
            },
        ];
        assert_eq!(
            arbitrer(&fenetres),
            vec![("w-1".into(), false), ("w-2".into(), true)]
        );
    }
}
