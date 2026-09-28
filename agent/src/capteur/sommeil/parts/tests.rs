//! Tests of `capteur::sommeil::parts` — sibling file rather than inline
//! module.
//!
//! **Extraction done in a DEDICATED task, BEFORE the one that adds** (fix
//! round 2, 25 August 2026): `parts.rs` was at 488 lines for a
//! project cap of 500, and round 2 brings both a rewritten test
//! and a doc fix to it. Extract, never compress — and "the regained
//! margin is lost again if treated as settled", paid for six times.
//!
//! ⚠️ **`#[path]` in the parent, and it is NOT the
//! `<parent>_<child>` convention**: that one only targets modules extracted from a
//! `#[cfg(windows)]` parent to compile on the host. Here the reason is
//! different — splitting a TEST module that is too long in an otherwise
//! portable file —, a case `CLAUDE.md` explicitly puts outside its scope.
//! Repository precedents: `superviseur/table.rs`, and `file/tests.rs` extracted
//! in round 1 for the same reason.
//!
//! **The module path stays `parts::tests`**: only the physical location
//! of the file changes, no visibility is touched.

use crate::capteur::sommeil::file::{ReceveurSession, PROFONDEUR_MAX};
use crate::capteur::sommeil::tests::{premier_ordre, verrouiller_pour_le_test};
use crate::capteur::sommeil::{etat, inscrire, retirer, signaler, Message};
use crate::capteur::vivier::Ordre;

/// Last share received on a channel, draining what is there.
fn derniere_part(canal: &ReceveurSession) -> Option<u32> {
    canal
        .drain()
        .into_iter()
        .filter_map(|m| match m {
            Message::Part { bps } => Some(bps),
            _ => None,
        })
        .next_back()
}

/// 🔴 THIS TEST WAS REWRITTEN ON 25 AUGUST 2026, BECAUSE THE BOUND BY
/// COALESCING (`file.rs`) MADE ITS ORIGINAL ASSERTION FALSE — and
/// it is the only red it produced among the repository's 1,022 tests.
///
/// **What it tested**, under the UNBOUNDED `mpsc` channel: that the awake
/// share arrives AFTER the wake-up order (`position_reveil <
/// position_part`), its name being
/// `une_session_qui_s_eveille_recoit_une_part_apres_son_ordre_de_reveil`.
/// It held because the queue kept BOTH shares: the sleeping floor one
/// emitted by `inscrire`, then the awake one emitted after the
/// `Reveiller`.
///
/// **What in-place coalescing does, and it is intended**: the second
/// share REPLACES the first IN ITS PLACE, that is BEFORE the
/// `Reveiller`. There is therefore no longer a share after the order — not because
/// it is missing, but because there is now only ONE, and it already carries
/// the awake value.
///
/// 🔵 **ACCEPTED CONSEQUENCE, AND IT MUST BE SAID: THE ORDER BETWEEN A `Part`
/// AND A `Sommeil` IS NO LONGER GUARANTEED** when the window has not read between
/// the two. In-place coalescing keeps the position of the SLOT, not
/// the arrival order of the VALUES: the share may precede by one message
/// the order motivating it.
///
/// ❌ ~~It is the lesser of two evils: coalescing at the TAIL would deliver a
/// share after an order that makes it void, and `dernieres_parts` filtering
/// repetitions, that stale share would never be corrected.~~
/// **THIS ARGUMENT WAS FALSE, and fix round 1 refuted it**:
/// coalescing at the tail would always put the MOST RECENT value at the
/// tail, hence `[Reveiller, Part(awake)]` — chronologically right AND
/// carrying the right value. The failure mode described does not exist.
/// ⚠️ This sentence moreover contradicted the header of `file.rs`, written
/// in the SAME commit, which claimed that replacing in place "preserves
/// the order": both now say the same thing, and the true one.
///
/// 🔵 **WHAT REALLY JUSTIFIES THE CHOICE**: the disorder only concerns
/// variants the sensor RELAYS without applying them —
/// `fenetre/transitions.rs` writes "nothing to do locally" for `Part`
/// as for `Audio`, only `Sommeil` having a local effect. The two
/// policies converge to the same final state, and the value delivered is
/// in both cases the last one computed.
///
/// ⚠️ ~~The disorder's bound is ONE message.~~ **TRUE OF THIS TEST, FALSE IN
/// GENERAL** (fix round 2): in-place coalescing brings the value back
/// up to the position of the oldest slot, hence over up to
/// `PROFONDEUR_MAX − 1` non-coalescable messages. Skipping a single message
/// is what this test observes, not a bound of the mechanism — and writing it
/// in the paragraph that claims to JUSTIFY the choice was the worst place
/// for an unestablished claim. **What justifies the choice is
/// above, and depends on no bound**: these variants are relayed,
/// not applied, and the value delivered is the last one computed.
#[test]
fn une_session_qui_s_eveille_recoit_la_part_d_une_eveillee_et_une_seule() {
    let _verrou = verrouiller_pour_le_test();
    let (messages, generation) = inscrire("t6-a", 6001);
    signaler("t6-a", true, true);

    let recus: Vec<Message> = messages.drain();
    assert!(
        recus
            .iter()
            .any(|m| matches!(m, Message::Sommeil(Ordre::Reveiller))),
        "the wake-up order must be present: {recus:?}"
    );
    let parts: Vec<u32> = recus
        .iter()
        .filter_map(|m| match m {
            Message::Part { bps } => Some(*bps),
            _ => None,
        })
        .collect();
    // **A SINGLE one**, and it is the direct proof of coalescing: without
    // it there would be two, the sleeping floor from `inscrire` then
    // the awake one.
    assert_eq!(
        parts.len(),
        1,
        "the two shares must have coalesced: {recus:?}"
    );
    // **The value that survives is the LAST one computed**, that of an
    // awake window — not the floor it replaced. That is what makes the
    // one-message disorder harmless, and without this assertion the test
    // would also pass if coalescing had kept the FIRST value.
    assert!(
        parts[0] > crate::capteur::repartiteur::PART_DORMANTE_BPS,
        "the share that survives is that of an AWAKE one, not the floor: {parts:?}"
    );
    retirer("t6-a", generation);
}

#[test]
fn une_part_inchangee_n_est_pas_reemise() {
    let _verrou = verrouiller_pour_le_test();
    let (messages, generation) = inscrire("t6-b", 6002);
    signaler("t6-b", true, true);
    let _ = messages.drain();

    // Same signal, hence same state, hence same share: nothing must go out.
    signaler("t6-b", true, true);
    let parts: Vec<Message> = messages
        .drain()
        .into_iter()
        .filter(|m| matches!(m, Message::Part { .. }))
        .collect();
    assert!(
        parts.is_empty(),
        "an unchanged share is not re-emitted: {parts:?}"
    );
    retirer("t6-b", generation);
}

#[test]
fn l_arrivee_d_une_seconde_fenetre_reduit_la_part_de_la_premiere() {
    let _verrou = verrouiller_pour_le_test();
    let (a, generation_a) = inscrire("t6-c", 6003);
    signaler("t6-c", true, true);
    let premiere = derniere_part(&a).expect("the first one must have a share");

    let (b, generation_b) = inscrire("t6-d", 6004);
    signaler("t6-d", true, false);
    let apres = derniere_part(&a).expect("the first one must be served again");
    assert!(
        apres < premiere,
        "share of the first one: {premiere} then {apres} — it must drop"
    );
    assert!(
        derniere_part(&b).is_some(),
        "the second one must receive a share"
    );

    retirer("t6-c", generation_a);
    retirer("t6-d", generation_b);
}

/// The test covering the defect found in review: a broken channel detected
/// DURING the distribution of SHARES (not during that of orders) must
/// free its place in the pool, not only in `canaux`. Before the
/// remedy, the entry survived there for the whole life of the process as soon as a
/// window thread died without going through `retirer` — the nominal case
/// of a panic, short-circuiting the single passage point of
/// `Fenetre::servir`.
#[test]
fn un_canal_rompu_detecte_par_les_parts_est_retire_du_vivier() {
    let _verrou = verrouiller_pour_le_test();
    // Sature les PLAFOND_EVEIL (8) places.
    let mut recepteurs_pleins = Vec::new();
    for i in 0..8 {
        let nom = format!("t7-plein-{i}");
        let (ordres, generation) = inscrire(&nom, 6100 + i as u32);
        signaler(&nom, true, false);
        assert_eq!(
            premier_ordre(&ordres),
            Some(Ordre::Reveiller),
            "{nom} should wake up"
        );
        recepteurs_pleins.push((nom, ordres, generation));
    }

    // The first one's thread "dies": its receiver is thrown away WITHOUT going
    // through `retirer`, exactly what happens when a window thread
    // panics before reaching its single removal point. `canaux`
    // therefore keeps an entry nobody reads any more.
    let (session_morte, recepteur_mort, generation_morte) = recepteurs_pleins.remove(0);
    drop(recepteur_mort);

    // "t7-attend" arrives. The mere REGISTRATION of a new session
    // (asleep) already changes the share computation of the eight existing
    // awake ones: each sleeping one subtracts its `PART_DORMANTE_BPS` from the
    // shared budget BEFORE the rest is divided (see `repartir`),
    // so `reste` changes, so EACH awake one's share changes —
    // including that of the dead session. The resulting send attempt
    // on its broken channel triggers the remedy: it is removed
    // from the POOL (and not only from `canaux`), which frees its place.
    let (ordres_attend, generation_attend) = inscrire("t7-attend", 6200);

    // Signalling itself visible is now enough: the place is already free.
    // Without the remedy (removal from the pool in addition to `canaux`), the dead
    // session would stay counted there as awake forever, the place would
    // never be freed, and "t7-attend" would stay asleep here.
    signaler("t7-attend", true, false);
    assert_eq!(
        premier_ordre(&ordres_attend),
        Some(Ordre::Reveiller),
        "removing the dead session, detected by the share distribution, \
         must free its place in the pool"
    );

    // Cleanup. `retirer` on the session already removed by the remedy is
    // a no-op on an already absent key, for `Vivier::retirer`
    // (HashMap::remove) as well as for `canaux` — not a double removal.
    retirer("t7-attend", generation_attend);
    retirer(&session_morte, generation_morte);
    for (nom, _, generation) in recepteurs_pleins {
        retirer(&nom, generation);
    }
}

/// The defect found in review of task 6: `sommeil::inscrire` replaces
/// the channel of an already known session (re-attachment after a pipe
/// break) without purging `dernieres_parts`. If the topology did not change
/// between the two registrations, the recomputed share is identical to the one
/// already memorised, the overwrite filter of `distribuer_les_parts` therefore
/// judges it already delivered, and the NEW channel never receives anything — the
/// bitrate ceiling of this child stays stale without end.
#[test]
fn a_reattach_with_unchanged_topology_resends_a_share_on_the_new_channel() {
    let _verrou = verrouiller_pour_le_test();

    // First channel: registration alone, no other window, no
    // signal — the window is born asleep and still receives the floor
    // share at registration (see the doc of `inscrire`).
    let (premier_canal, _generation_initiale) = inscrire("t8-rattache", 6300);
    let premiere_part = derniere_part(&premier_canal)
        .expect("a first share must go out at the initial registration");

    // The pipe breaks and the child re-attaches: SAME session, nothing
    // else in the topology moved (no other window, no
    // visibility signal in between). `inscrire` detects the
    // replacement (it logs "order channel replaced for
    // this session") and returns a new channel, with a new generation
    // (D9, F5 of D7).
    let (canal_neuf, generation_neuve) = inscrire("t8-rattache", 6300);

    // Without the remedy, the recomputed share is identical to `premiere_part`
    // : `dernieres_parts` judges it already delivered (it was, but on
    // the OLD channel, gone with the break) and nothing goes out on the
    // new channel, which stays silent forever as long as the topology does not
    // change.
    assert_eq!(
        derniere_part(&canal_neuf),
        Some(premiere_part),
        "the fresh channel must receive its share even if it is identical to the one \
         already sent on the old channel: dernieres_parts must be purged for \
         this session at the moment its channel is replaced"
    );

    retirer("t8-rattache", generation_neuve);
}
/// 🔴 THE RED OF ROUND 1'S CRITICAL: A REFUSED SHARE WAS MEMORISED
/// AS SENT, AND WAS NEVER RE-EMITTED.
///
/// Under `mpsc`, `send(...).is_ok()` meant "delivered". Since the bounded
/// queue, it only means "not disconnected": `Ok(Depot::Refusee)`
/// is a queue-full refusal, and reading it as a delivery made the value
/// be written into `dernieres_parts`. The overwrite guard at the head
/// of the loop (`if dernieres_parts.get(&session) == Some(&bps) { continue }`)
/// then suppressed **any future re-emission of that value**: the
/// window stayed at its previous bitrate as long as its computed share did not
/// change — without bound, and without a log line.
///
/// **This test fails on its LAST assertion before the fix**
/// (`a refused share must be RE-EMITTED on the next round`), the share having
/// been wrongly memorised. The first two pass on both sides: they
/// establish the precondition (the queue is indeed full, the share is
/// indeed not delivered), without which the third would measure nothing.
///
/// ⚠️ **REWRITTEN IN ROUND 2**: it provoked the new share through a `signaler`
/// that WAKES the session. Since `distribuer` gives back to the pool the state of an
/// order not dropped, this refused `Reveiller` is CANCELLED — the session becomes
/// asleep again, its share falls back to the ALREADY memorised floor, and no share
/// is even attempted any more: the test then measured emptiness. It now provokes
/// the new share **without touching wakefulness**, through a second session that
/// takes its share of the shared budget.
#[test]
fn a_refused_share_is_not_remembered_and_goes_again_next_round() {
    let _verrou = verrouiller_pour_le_test();
    let (canal, generation) = inscrire("t17-refus", 6400);
    // The session wakes up FIRST, queue free: its awake share goes out and is
    // memorised normally. That is the ordinary state we start from.
    signaler("t17-refus", true, true);
    let _ = canal.drain();

    // Saturates the queue with NON-COALESCABLE messages — `Sommeil` never
    // coalesces, that is what makes it possible to reach the bound.
    {
        let garde = etat();
        let emetteur = garde
            .canaux
            .get("t17-refus")
            .expect("the session is registered");
        for _ in 0..PROFONDEUR_MAX {
            let _ = emetteur.envoyer(Message::Sommeil(Ordre::Reveiller));
        }
    }

    // A second session takes its share of the budget: that of "t17-refus"
    // CHANGES, so a new share is computed for it — and REFUSED, its queue
    // being full. Its wakefulness, for its part, does not move.
    let (voisine, generation_voisine) = inscrire("t17-voisine", 6401);

    // The window resumes reading. No share is there: it was
    // never dropped.
    let recus = canal.drain();
    assert_eq!(
        recus.len(),
        PROFONDEUR_MAX,
        "precondition: the queue was indeed full"
    );
    assert!(
        !recus.iter().any(|m| matches!(m, Message::Part { .. })),
        "precondition: the refused share was NOT delivered: {recus:?}"
    );

    // The next round, without anything having changed: the SAME value must
    // go out again, since it never reached the window.
    {
        let mut garde = etat();
        super::distribuer_les_parts(&mut garde);
    }
    let parts: Vec<u32> = canal
        .drain()
        .into_iter()
        .filter_map(|m| match m {
            Message::Part { bps } => Some(bps),
            _ => None,
        })
        .collect();
    assert!(
        !parts.is_empty(),
        "a refused share must be RE-EMITTED on the next round: it was never delivered"
    );

    retirer("t17-voisine", generation_voisine);
    drop(voisine);
    retirer("t17-refus", generation);
}
