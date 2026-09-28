//! Distribution of audio orders — the branch of `capteur::audio::arbitrer`
//! on the registry of `capteur::sommeil`.
//!
//! **Extracted from `sommeil.rs` and not added into it**, exactly like
//! `parts.rs`: that file is close to its cap, and the repository's rule
//! requires a substantial addition to come with an extraction.
//!
//! It is not called `audio`: that name is already taken by the module carrying
//! the pure RULE. This one only carries its BRANCH on this registry — same
//! distinction as `repartiteur` / `parts`.

use std::sync::MutexGuard;

use crate::capteur::audio::{arbitrer, FenetreAudio};

use super::file::Envoi;
use super::{oublier, Etat, Message};

/// Recomputes who carries the sound and sends only what changed.
///
/// **Called AFTER `distribuer_les_parts`**, in last position on all the
/// registry's entry paths. The order matters little with respect to shares —
/// sound and bitrate are orthogonal — but a single, documented order is
/// better than an order that depends on the caller.
///
/// A channel broken here is removed by `oublier`, exactly as in
/// `distribuer` and `distribuer_les_parts`: it is the registry's single passage point,
/// and bypassing it would leave phantom entries in the pool.
pub(super) fn distribuer_l_audio(garde: &mut MutexGuard<'static, Etat>) {
    let fenetres: Vec<FenetreAudio> = garde
        .canaux
        .keys()
        .filter_map(|session| {
            // A session without a known PID or arrival rank does not exist
            // : `inscrire` sets both together. The `filter_map` is a
            // safety net, not a nominal case.
            //
            // ⚠️ `arrivee` and `last_focus` are NOT symmetric despite
            // appearances: `0` is the DOCUMENTED sentinel of
            // `last_focus` ("never focused", the lowest
            // priority — see `FenetreAudio`), so `unwrap_or(0)` is the right
            // fallback there. For `arrivee`, `0` BEATS any real window of its
            // PID group (`l_emporte` compares `candidat.arrivee <
            // actuel.arrivee`): a fallback to 0 would therefore be the worst possible
            // choice there, not a neutral one. No live path produces
            // this case — `inscrire` always sets `arrivees` before any call
            // to `distribuer_l_audio` —, but returning it through `?` rather than through
            // a default makes it impossible to misread.
            let pid = *garde.pids.get(session)?;
            let arrivee = *garde.arrivees.get(session)?;
            Some(FenetreAudio {
                session: session.clone(),
                pid,
                arrivee,
                last_focus: garde.derniers_focus.get(session).copied().unwrap_or(0),
                inapte: garde.inaptes.contains_key(session),
            })
        })
        .collect();

    let decisions = arbitrer(&fenetres);

    let vivantes: std::collections::HashSet<&String> =
        decisions.iter().map(|(session, _)| session).collect();
    garde
        .derniers_audio
        .retain(|session, _| vivantes.contains(session));

    // The orders to GO SILENT go out first, the orders to CARRY next.
    // This order REDUCES the overlap window, it does not close it: the
    // two orders take two DISTINCT channels (`file.rs` since
    // 25 August 2026, `mpsc` before it), each read by the
    // thread of ITS window. The SENDING order is guaranteed, not the
    // HANDLING order — if the thread that must go silent is descheduled by
    // the scheduler before reading its message, both windows stay
    // audible together until it gets control back. The real bound is
    // therefore the scheduling of the two threads, not this channel.
    let (a_porter, a_taire): (Vec<_>, Vec<_>) =
        decisions.into_iter().partition(|(_, actif)| *actif);

    // ⚠️ A channel broken here is NOT re-arbitrated in the same pass: if the
    // broken session carried its group's sound, its neighbour will only
    // take it over at the NEXT WHEEL ROUND — bound `PERIODE_REARBITRAGE`,
    // 250 ms. Same residue, and same bound, as the `rompus` path of
    // `distribuer_les_parts` (see the doc of `Message` just above), but
    // the symptom is not of the same nature: there, a transient bitrate
    // floor; here, a silence NOTICEABLE by the user for
    // up to 250 ms.
    let mut rompus = Vec::new();
    for (session, actif) in a_taire.into_iter().chain(a_porter) {
        // ⚠️ The reset lived HERE until D10, on the arbitration
        // DECISION — and D9's cross-cutting review established that it could
        // then NOT bite in the majority case: for a window
        // alone in its PID group, leaving the respite makes it
        // automatically the carrier, hence resets the counter at every
        // round. The safeguard was decorative. It now restarts from
        // `sommeil::signaler_audio_vivant`, on a PROOF of sound.
        if garde.derniers_audio.get(&session) == Some(&actif) {
            continue;
        }
        // ⚠️ **`None` IS NOT A BREAK, and round 3 fixed this
        // wording** — same grievance as `registre::distribuer` in round 2.
        // It is unreachable today (the sessions come out of
        // `canaux.keys()` under the SAME lock, a few lines above),
        // hence without consequence; but this batch had set itself the rule of no
        // longer FABRICATING an outcome, and writing it as `Envoi::Rompu` would purge a
        // session on a fact that did not happen if that invariance were to
        // fall. An `Option` names the thing: there was no send.
        let issue = garde
            .canaux
            .get(&session)
            .map(|canal| canal.envoyer(Message::Audio { actif }));
        match issue {
            // No channel: nothing went out, and there is nothing to purge — the
            // session is already no longer in `canaux`.
            None => {}
            Some(Envoi::Depose(_)) => {
                garde.derniers_audio.insert(session, actif);
            }
            // 🔴 REFUSED: WE DO NOT MEMORISE. Same fix as
            // `parts::distribuer_les_parts`, and **the costlier of the two**:
            // memorising an audio order that never went out makes the overwrite guard at the head of the loop
            // judge the state "already delivered", and the
            // window stays in its previous state — hence potentially
            // SILENT, or audible at the same time as a neighbour, **without bound**.
            // ⚠️ The comment above worries about a noticeable silence
            // of up to 250 ms on the broken channel path; this
            // residue was bounded by nothing at all.
            //
            // ⚠️ **And above all NOT `rompus.push`**: the session is ALIVE.
            Some(Envoi::Refuse) => {}
            Some(Envoi::Rompu) => rompus.push(session),
        }
    }

    let mut ordres_du_retrait = Vec::new();
    for session in rompus {
        ordres_du_retrait.extend(oublier(garde, &session));
    }
    if !ordres_du_retrait.is_empty() {
        super::distribuer(garde, ordres_du_retrait);
    }
}

#[cfg(test)]
mod tests {
    use crate::capteur::sommeil::file::{ReceveurSession, PROFONDEUR_MAX};
    use crate::capteur::sommeil::tests::verrouiller_pour_le_test;
    use crate::capteur::sommeil::{etat, inscrire, retirer, signaler, Message};
    use crate::capteur::vivier::Ordre;

    /// Last audio order received on a channel, draining what is there.
    ///
    /// **Lives here, not in `sommeil::tests`**: `sommeil.rs` is close to its
    /// 500-line cap, and this suite covers the branch of THIS
    /// module — same set-up as `parts::tests`, which imports
    /// `verrouiller_pour_le_test` the same way rather than adding its
    /// own tests to the parent file.
    fn last_audio(canal: &ReceveurSession) -> Option<bool> {
        canal
            .drain()
            .into_iter()
            .filter_map(|m| match m {
                Message::Audio { actif } => Some(actif),
                _ => None,
            })
            .next_back()
    }

    #[test]
    fn deux_fenetres_d_un_meme_pid_se_disputent_le_son_et_le_focus_tranche() {
        let _verrou = verrouiller_pour_le_test();
        let (a, generation_a) = inscrire("t9-a", 4242);
        let (b, generation_b) = inscrire("t9-b", 4242);

        // No focused window: the first arrival carries the sound.
        assert_eq!(
            last_audio(&a),
            Some(true),
            "the first to arrive carries the sound"
        );
        assert_eq!(last_audio(&b), Some(false), "the second one goes quiet");

        // "b" takes the focus: the sound switches, and "a" receives the order to go
        // silent — otherwise both would be audible at the same time.
        signaler("t9-b", true, true);
        assert_eq!(
            last_audio(&b),
            Some(true),
            "the focused one takes the sound"
        );
        assert_eq!(
            last_audio(&a),
            Some(false),
            "the previous carrier goes quiet"
        );

        // "b" disappears: "a" must take the sound back, otherwise the group becomes
        // permanently silent.
        retirer("t9-b", generation_b);
        assert_eq!(
            last_audio(&a),
            Some(true),
            "the sound goes back to the survivor"
        );

        retirer("t9-a", generation_a);
    }

    #[test]
    fn two_distinct_pids_each_carry_their_sound() {
        let _verrou = verrouiller_pour_le_test();
        let (a, generation_a) = inscrire("t9-c", 111);
        let (b, generation_b) = inscrire("t9-d", 222);
        assert_eq!(last_audio(&a), Some(true));
        assert_eq!(last_audio(&b), Some(true));
        retirer("t9-c", generation_a);
        retirer("t9-d", generation_b);
    }

    #[test]
    fn un_ordre_audio_inchange_n_est_pas_reemis() {
        // Without the overwrite filter, the wheel round (250 ms) would send
        // four orders per second and per window, for life. Same rampart as
        // `dernieres_parts`.
        let _verrou = verrouiller_pour_le_test();
        let (a, generation) = inscrire("t9-e", 333);
        let _ = a.drain();
        signaler("t9-e", true, true);
        let ordres: Vec<Message> = a
            .drain()
            .into_iter()
            .filter(|m| matches!(m, Message::Audio { .. }))
            .collect();
        assert!(
            ordres.is_empty(),
            "unchanged audio order re-emitted: {ordres:?}"
        );
        retirer("t9-e", generation);
    }
    /// 🔴 THE RED OF ROUND 1'S CRITICAL, AUDIO SIDE — THE SAME PATTERN AS
    /// `parts.rs`, AND THE COSTLIER OF THE TWO: a window could stay
    /// SILENT indefinitely.
    ///
    /// `envoyer(...).is_ok()` meant "delivered" under `mpsc`; since the bounded
    /// queue it only means "not disconnected". A refused `Audio` was
    /// therefore written into `derniers_audio`, and the overwrite guard at the head of the
    /// loop (`if derniers_audio.get(&session) == Some(&actif) { continue }`)
    /// suppressed any re-emission — the window stayed in its previous audio
    /// state **without bound**.
    ///
    /// ⚠️ **The comment on `distribuer_l_audio` worries about a noticeable silence
    /// of up to 250 ms on another path; this residue
    /// was bounded by nothing.**
    ///
    /// **This test fails on its LAST assertion before the fix.**
    #[test]
    fn a_refused_audio_order_is_not_remembered_and_goes_again_next_round() {
        let _verrou = verrouiller_pour_le_test();
        let (a, generation_a) = inscrire("t9-refus-a", 4300);
        // "a" is alone in its PID: it carries the sound, and `derniers_audio`
        // keeps `true`.
        assert_eq!(
            last_audio(&a),
            Some(true),
            "precondition: the only one of the PID carries the sound"
        );

        // Saturates "a"'s queue with NON-COALESCABLE messages.
        {
            let garde = etat();
            let emetteur = garde
                .canaux
                .get("t9-refus-a")
                .expect("the session is registered");
            for _ in 0..PROFONDEUR_MAX {
                let _ = emetteur.envoyer(Message::Sommeil(Ordre::Reveiller));
            }
        }

        // "b" arrives on the SAME PID and takes the focus: "a" must receive
        // the order to go silent — which is REFUSED, its queue being full.
        let (b, generation_b) = inscrire("t9-refus-b", 4300);
        signaler("t9-refus-b", true, true);

        // "a" reprend sa lecture. Aucun ordre audio ne s'y trouve.
        let recus = a.drain();
        assert!(
            !recus.iter().any(|m| matches!(m, Message::Audio { .. })),
            "precondition: the order to go quiet was NOT delivered: {recus:?}"
        );

        // The next round: the order must go out again, otherwise "a" stays
        // audible at the same time as "b", forever.
        {
            let mut garde = etat();
            super::distribuer_l_audio(&mut garde);
        }
        assert_eq!(
            last_audio(&a),
            Some(false),
            "a refused audio order must be RE-EMITTED on the next round: it was never delivered"
        );

        retirer("t9-refus-b", generation_b);
        retirer("t9-refus-a", generation_a);
        drop(b);
    }
}
