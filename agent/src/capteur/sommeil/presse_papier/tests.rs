//! Les tests de `capteur/sommeil/presse_papier.rs`.
//!
//! **Extracted VERBATIM in sub-block P3 (task 4), BEFORE the addition that
//! made it necessary** — the `last_clipboard` memory of D-P3-2 and the
//! second take of D-P3-6. The parent was at 379 lines for a cap of
//! 500. The repository's rule is to extract BEFORE adding, never to compress
//! afterwards.
//!
//! ⚠️ **The extracted block was IN THE MIDDLE of the file**, not at the end: the
//! production code resumed right after (`write`, `write_with`,
//! `armer_les_gardes`). The `git diff` is less readable than an end-of-file
//! move, and the character-for-character transposition check
//! is all the more mandatory — it was run, and the four-space
//! de-indentation was checked REVERSIBLE.
//!
//! ⚠️ This use of `#[path]` is OUTSIDE the scope of the "Child
//! module convention" of `docs/claude/module-conventions.md`: it is the same Rust mechanism used for
//! another reason — the 500-line rule —, exactly like
//! `superviseur/table.rs` and `presse_papier.rs`. This module is NOT hoisted to the
//! crate root.

use crate::capteur::sommeil::file::ReceveurSession;
use crate::capteur::sommeil::tests::{premier_ordre, verrouiller_pour_le_test};
use crate::capteur::sommeil::{etat, inscrire, retirer, signaler, Message};
use crate::capteur::vivier::Ordre;
use crate::presse_papier::Annonce;
use crate::presse_papier::Sondeur;

/// The last clipboard received on a channel, draining what is
/// there: shares and sleep orders are freely interleaved.
fn last_clipboard(canal: &ReceveurSession) -> Option<(Option<String>, u32)> {
    canal
        .drain()
        .into_iter()
        .filter_map(|m| match m {
            Message::PressePapier { texte, octets } => Some((texte, octets)),
            _ => None,
        })
        .next_back()
}

/// **All windows receive, not only the focused one**: each
/// browser window has its own local clipboard, and it is the client
/// that decides whether it writes now or when the focus comes back.
#[test]
fn a_text_announcement_goes_to_all_subscribed_sessions() {
    let _verrou = verrouiller_pour_le_test();
    let (canal_a, generation_a) = inscrire("pp-a", 7100);
    let (canal_b, generation_b) = inscrire("pp-b", 7101);
    signaler("pp-a", true, true);

    super::distribuer(&mut etat(), Annonce::Texte("bonjour".to_string()));

    assert_eq!(
        last_clipboard(&canal_a),
        Some((Some("bonjour".to_string()), 7)),
        "the focused window must receive the text"
    );
    assert_eq!(
        last_clipboard(&canal_b),
        Some((Some("bonjour".to_string()), 7)),
        "the NON-focused window too: it is the client that decides to write"
    );

    retirer("pp-a", generation_a);
    retirer("pp-b", generation_b);
}

/// A refusal travels through the same variant, `texte` as `None` and `octets`
/// carrying the refused size — that is what lets the
/// browser's banner say *how much* rather than "too large".
#[test]
fn a_refusal_leaves_without_text_but_with_its_size() {
    let _verrou = verrouiller_pour_le_test();
    let (canal, generation) = inscrire("pp-refus", 7200);

    super::distribuer(&mut etat(), Annonce::Refus { octets: 100_000 });

    assert_eq!(
        last_clipboard(&canal),
        Some((None, 100_000)),
        "a refusal must go out, and carry its size"
    );

    retirer("pp-refus", generation);
}

/// Same remedy, and for the same reason, as
/// `un_canal_rompu_detecte_par_les_parts_est_retire_du_vivier`: a window
/// thread dying without going through `retirer` (a panic short-circuits
/// the single passage point of `Fenetre::servir`) leaves an entry in
/// `canaux` AND in the pool, where it would occupy an encoder place
/// for the whole life of the process. `oublier` — never a direct `remove` —
/// is what removes both.
///
/// ⚠️ **This test observes the pool DIRECTLY, and that is what makes it
/// discriminating.** A first draft judged on "does a new session
/// manage to wake up" — and it passed WITH AN EMPTY
/// DISTRIBUTOR: `inscrire` and `signaler` both call
/// `parts::distribuer_les_parts`, which detects the same break through its
/// own path and frees the place instead of this one. The check
/// therefore could not fail — exactly the pattern this repository has been paying for
/// since D6. Here, nothing comes between the break and the observation.
#[test]
fn un_canal_rompu_detecte_par_le_presse_papier_est_retire_du_vivier() {
    let _verrou = verrouiller_pour_le_test();
    let (canal_mort, generation_morte) = inscrire("pp-mort", 7300);
    signaler("pp-mort", true, false);
    assert_eq!(
        premier_ordre(&canal_mort),
        Some(Ordre::Reveiller),
        "pp-mort should wake up before its thread is killed"
    );

    // The thread "dies": its receiver is thrown away WITHOUT going through `retirer`.
    drop(canal_mort);

    // No public call between the break and the observation: neither `inscrire`
    // nor `signaler`, which would detect the break through the shares path.
    super::distribuer(&mut etat(), Annonce::Texte("bonjour".to_string()));

    assert!(
        !etat().vivier.eveillees().iter().any(|s| s == "pp-mort"),
        "the session whose channel is broken must be removed from the POOL, \
         not only from `canaux`: its encoder place would otherwise stay \
         taken for the whole life of the process"
    );

    retirer("pp-mort", generation_morte);
}

// -----------------------------------------------------------------------
// Sub-block P2 — writing by the owner, and arming the guards.
// -----------------------------------------------------------------------

/// A `Sondeur` that has already taken its reference: it is the nominal state after
/// the first round, and the only one in which the guards are judged.
fn sondeur_amorce() -> Sondeur {
    let mut sondeur = Sondeur::new();
    assert_eq!(
        sondeur.observer(1, || Some(String::from("etat-initial"))),
        None
    );
    sondeur
}

/// The window thread puts our write into `Etat`; `armer_les_gardes`
/// consumes it and arms the `Sondeur`. The witness is not that nothing is
/// announced — that would also be true of guard no. 2 — but that the clipboard
/// is **not even reopened**: the read closure PANICS if it
/// is called.
///
/// RED if `armer_les_gardes` consumes nothing, or does not set
/// `reference`.
///
/// 🔴 **WHAT THIS TEST DOES NOT COVER, AND IT IS MEASURED, NOT ASSUMED.** It
/// establishes that the mechanism is right **when called before
/// `tour()`**; it does **not** establish that the wheel round calls it
/// in that order. Checked by mutation on 21 August 2026: swapping the
/// two lines of `registre.rs::start_the_round` leaves **the seven
/// tests of this module GREEN**. The body of the wheel round is an infinite
/// loop in a `thread::spawn`, which no host test reaches —
/// the order there is a fact of READING, and its only test is the acceptance run
/// (criterion ④, which counts the return messages).
#[test]
fn armer_les_gardes_empeche_de_relire_notre_ecriture() {
    let _verrou = verrouiller_pour_le_test();
    etat().notre_ecriture = Some((42, String::from("colle")));

    let mut sondeur = sondeur_amorce();
    super::armer_les_gardes(&mut sondeur);

    assert_eq!(
        sondeur.observer(42, || panic!("guard no. 1 let the clipboard be reopened")),
        None
    );
    // Consumed: a second arming finds nothing any more.
    assert!(etat().notre_ecriture.is_none());
}

/// 🔴 **The guard stays EXACT in D5's sense, and that is what this test
/// measures.** Setting `reference` on *our* `seq` does not mask a THIRD-PARTY
/// copy that happened since: the counter moved again, and that copy must
/// be announced.
///
/// RED if the arming set a "we stay silent from now on" state. Without this
/// test, an overly broad guard would pass the previous one.
#[test]
fn une_copie_tierce_survenue_apres_notre_ecriture_est_quand_meme_annoncee() {
    let _verrou = verrouiller_pour_le_test();
    etat().notre_ecriture = Some((42, String::from("colle")));

    let mut sondeur = sondeur_amorce();
    super::armer_les_gardes(&mut sondeur);

    assert_eq!(
        sondeur.observer(43, || Some(String::from("something else"))),
        Some(Annonce::Texte(String::from("something else")))
    );
}

/// 🔴 **A FAILED write arms NO guard**, and it is the most
/// dangerous case: arming before knowing would take out of observation a
/// content that never reached the clipboard. That content would then become
/// invisible **forever** — the next round would not see it as
/// a change.
///
/// The writer is INJECTED, exactly like the read closure of
/// `Sondeur::observer`: that is what makes this path testable on the host
/// without any `cfg`.
///
/// RED if `write_with` set `notre_ecriture` before calling
/// the writer, or if it ignored its `Err`.
#[test]
fn une_ecriture_echouee_n_arme_aucun_garde() {
    let _verrou = verrouiller_pour_le_test();
    etat().notre_ecriture = None;

    let result = super::write_with("colle", |_| anyhow::bail!("OpenClipboard refused"));

    assert!(result.is_err(), "the failure must surface to the caller");
    assert!(
        etat().notre_ecriture.is_none(),
        "nothing must be set when the write failed"
    );
}

/// The counterpart: a SUCCESSFUL write does set the pair, with the number
/// the writer returned — the one re-read AFTER `CloseClipboard`.
///
/// RED if `write_with` set a fabricated number instead of the
/// writer's: guard no. 1 would then be off by one step, that is
/// silently inoperative.
#[test]
fn une_ecriture_reussie_pose_le_numero_rendu_par_l_ecrivain() {
    let _verrou = verrouiller_pour_le_test();
    etat().notre_ecriture = None;

    super::write_with("colle", |texte| {
        assert_eq!(texte, "colle", "the text must reach Win32 as is");
        Ok(1234)
    })
    .expect("write");

    assert_eq!(
        etat().notre_ecriture.clone(),
        Some((1234, String::from("colle")))
    );
    etat().notre_ecriture = None;
}

/// The SECOND TAKE of D-P3-6, on ITS BRANCH: it must CONSUME
/// `etat().notre_ecriture` and discard the announcement carrying our own text.
///
/// ⚠️ **This test covers the read of `etat()`, which the pure rule of
/// `Sondeur::ecarter` cannot cover** — it is the only place where the
/// branch is testable. The CALL SITE (`registre.rs`, between `tour()`
/// and `distribuer`) is covered by no host test: it lives in the
/// wheel-round thread. Said rather than kept quiet.
///
/// RED if `filtrer_nos_ecritures_tardives` does not take the pair, or if it
/// reads it without consuming it — the second assertion would fail.
#[test]
fn la_seconde_prise_consomme_notre_ecriture_et_ecarte_notre_texte() {
    let _verrou = verrouiller_pour_le_test();
    etat().notre_ecriture = Some((11, String::from("pasted-by-B")));

    let mut sondeur = Sondeur::new();
    let annonce = Some(Annonce::Texte(String::from("pasted-by-B")));

    assert_eq!(
        super::filtrer_nos_ecritures_tardives(&mut sondeur, annonce),
        None,
        "our own text must not go back out to the N windows"
    );
    assert_eq!(
        etat().notre_ecriture.clone(),
        None,
        "the pair must be CONSUMED: leaving it would replay it on the next round"
    );
}

/// Without a write of ours, the second take is transparent.
///
/// RED if it discarded everything: a copy made in the VM would never
/// arrive anywhere, and the downward direction would be dead.
#[test]
fn la_seconde_prise_laisse_passer_une_copie_de_la_vm() {
    let _verrou = verrouiller_pour_le_test();
    etat().notre_ecriture = None;

    let mut sondeur = Sondeur::new();
    let annonce = Some(Annonce::Texte(String::from("copied-in-the-vm")));

    assert_eq!(
        super::filtrer_nos_ecritures_tardives(&mut sondeur, annonce.clone()),
        annonce
    );
}

// ── THE CURRENT STATE AT REGISTRATION — AGENT half of P1's hand-over no. 3 ──────
//
// 🔴 **THESE TESTS WERE WRITTEN IN `sommeil/tests.rs` THEN MOVED HERE,
// BECAUSE THEY MADE IT CROSS THE CAP** — 417 → 562 lines, for a
// gate of 500. The plan's E13 had announced it ("if the addition takes it beyond
// 480, extract BEFORE writing, never compress") and the measurement was not
// taken in advance. **The extraction is done, never a compression**:
// that is the repository's rule, which D9 paid for twice for having forgotten it.
//
// ⚠️ **And it is not merely a move of convenience.** E13 puts the
// tests of `registre.rs` in `sommeil/tests.rs`, for lack of a test module of
// its own; but what these tests exercise is `emit_current_state`, which lives
// in the PARENT of this file. They are therefore next to the function they
// test, and the `last_clipboard` helper that already lives here serves them
// as is — the twin written in `sommeil/tests.rs` was redundant and
// did not follow.

/// 🔴 RED ON THE INTACT TREE before the remedy: it is P1's hand-over no. 3 —
/// "a window attached after a copy never receives that content".
#[test]
fn a_session_that_subscribes_after_a_copy_receives_the_current_content() {
    let _verrou = verrouiller_pour_le_test();
    etat().last_clipboard = None;
    let (canal_present, generation_present) = inscrire("t11-present", 7300);
    super::distribuer(
        &mut etat(),
        crate::presse_papier::Annonce::Texte("deja-copie".into()),
    );
    let _ = last_clipboard(&canal_present);

    // The window attaches AFTER the copy.
    let (canal_tardif, generation_tardif) = inscrire("t11-tardif", 7301);

    assert_eq!(
        last_clipboard(&canal_tardif),
        Some((Some("deja-copie".to_string()), 10)),
        "a window attached after the copy must receive the current content"
    );

    etat().last_clipboard = None;
    retirer("t11-present", generation_present);
    retirer("t11-tardif", generation_tardif);
    drop(canal_present);
    drop(canal_tardif);
}

/// RED = calling `distribuer` instead of sending on the new channel alone: the
/// neighbours would receive too, and the content would be replayed to ALL
/// windows at every attach.
#[test]
fn l_emission_a_l_inscription_ne_part_que_sur_le_canal_neuf() {
    let _verrou = verrouiller_pour_le_test();
    etat().last_clipboard = None;
    let (canal_present, generation_present) = inscrire("t12-present", 7310);
    super::distribuer(
        &mut etat(),
        crate::presse_papier::Annonce::Texte("copie".into()),
    );
    // We drain what the neighbour legitimately received from `distribuer`.
    let _ = last_clipboard(&canal_present);

    let (canal_neuf, generation_neuf) = inscrire("t12-neuf", 7311);

    assert_eq!(
        last_clipboard(&canal_neuf),
        Some((Some("copie".to_string()), 5)),
        "the fresh channel receives"
    );
    assert_eq!(
        last_clipboard(&canal_present),
        None,
        "the neighbour must receive NOTHING more: that would be a round trip per attach"
    );

    etat().last_clipboard = None;
    retirer("t12-present", generation_present);
    retirer("t12-neuf", generation_neuf);
    drop(canal_present);
    drop(canal_neuf);
}

/// RED = memorising only `Annonce::Texte`: the window would then wait for a
/// content that will never arrive, without the banner telling it why.
#[test]
fn une_session_qui_s_inscrit_apres_un_refus_recoit_le_refus() {
    let _verrou = verrouiller_pour_le_test();
    etat().last_clipboard = None;
    super::distribuer(
        &mut etat(),
        crate::presse_papier::Annonce::Refus { octets: 123_456 },
    );

    let (canal, generation) = inscrire("t13-refus", 7320);

    assert_eq!(
        last_clipboard(&canal),
        Some((None, 123_456)),
        "the refusal must be replayed at attach, with its size"
    );

    etat().last_clipboard = None;
    retirer("t13-refus", generation);
    drop(canal);
}

/// RED = emitting an empty `Message::PressePapier` when the memory is
/// `None`: the client would then write an empty string into its local
/// clipboard at every attach.
#[test]
fn a_session_that_subscribes_before_any_copy_receives_nothing() {
    let _verrou = verrouiller_pour_le_test();
    etat().last_clipboard = None;

    let (canal, generation) = inscrire("t14-vierge", 7330);

    assert_eq!(last_clipboard(&canal), None);

    retirer("t14-vierge", generation);
    drop(canal);
}

/// 🔴 NO PURGE ON RE-REGISTRATION (D-P3-3) — and the symmetry with
/// `dernieres_parts` / `derniers_audio`, which `inscrire` purges a few lines
/// above, is MISLEADING.
///
/// RED = copying that purge by symmetry of form: the memory would be
/// removed at the precise moment we want to use it, and re-attachment — the
/// case where replay is MOST useful — would replay nothing.
#[test]
fn a_reattach_also_receives_the_current_content() {
    let _verrou = verrouiller_pour_le_test();
    etat().last_clipboard = None;
    let (premier_canal, premiere_generation) = inscrire("t15-rattache", 7340);
    super::distribuer(
        &mut etat(),
        crate::presse_papier::Annonce::Texte("before-cutoff".into()),
    );
    let _ = last_clipboard(&premier_canal);

    // Re-attachment, under the SAME name.
    let (second_canal, seconde_generation) = inscrire("t15-rattache", 7340);

    assert_eq!(
        last_clipboard(&second_canal),
        Some((Some("before-cutoff".to_string()), 13)),
        "a reattachment must receive the current content: nothing is purged"
    );

    etat().last_clipboard = None;
    retirer("t15-rattache", seconde_generation);
    let _ = premiere_generation;
    drop(premier_canal);
    drop(second_canal);
}
