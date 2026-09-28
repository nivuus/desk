//! Clipboard tests for the browser → VM direction (sub-block P2) and for the
//! second catch of D-P3-6, apart from `tests.rs` to stay under 500 lines.

use super::tests::amorce;
use super::*;
use std::cell::Cell;

// ---------------------------------------------------------------------------
// Sub-block P2 — the browser → VM direction: D5's guard no. 1, the reciprocal
// of `normaliser`, and the bound on INCOMING text.
// ---------------------------------------------------------------------------

/// 🔴 **It is D5's guard no. 1, and nothing else measures it.**
///
/// The witness is not that `observer` returns `None` — guard no. 2 would return it
/// too. The witness is that the read closure **is not called at
/// all**: the Windows clipboard is not even reopened. Hence a
/// closure that PANICS.
///
/// RED if `apres_notre_ecriture` does not set `reference`: `observer` reads,
/// and the test blows up.
#[test]
fn after_our_write_the_next_round_does_not_open_the_clipboard() {
    let mut sondeur = Sondeur::new();
    amorce(&mut sondeur);
    sondeur.apres_notre_ecriture(7, "colle");
    assert_eq!(
        sondeur.observer(7, || panic!("guard no. 1 let the clipboard be reopened")),
        None
    );
}

/// 🔴 **It is guard no. 2 ARMED ON OUR OWN WRITE**, that is, the
/// case D5 gives as no. 2's reason to exist: a THIRD-PARTY write slipped
/// in between our `SetClipboardData` and our reread of the counter,
/// so that the number we reread is already no longer the current one.
///
/// RED if `apres_notre_ecriture` only sets `reference`: the counter having
/// moved, `observer` reads, finds our own text, and sends it back to the
/// browser — a round trip for nothing.
#[test]
fn after_our_write_a_counter_that_moved_does_not_send_back_our_text() {
    let mut sondeur = Sondeur::new();
    amorce(&mut sondeur);
    sondeur.apres_notre_ecriture(7, "colle");
    assert_eq!(sondeur.observer(8, || Some(String::from("colle"))), None);
}

/// The counterpart of the previous one: guard no. 2 must not absorb EVERYTHING that
/// follows a write. A third-party copy of ANOTHER text is indeed announced.
///
/// RED if `apres_notre_ecriture` set a "we keep quiet from now on" state.
/// Without this test, a too-wide guard would pass the two previous ones.
#[test]
fn after_our_write_a_third_party_copy_is_still_announced() {
    let mut sondeur = Sondeur::new();
    amorce(&mut sondeur);
    sondeur.apres_notre_ecriture(7, "colle");
    assert_eq!(
        sondeur.observer(8, || Some(String::from("autre chose"))),
        Some(Annonce::Texte(String::from("autre chose")))
    );
}

/// `apres_notre_ecriture` normalises the text it memorises, as
/// `observer` normalises the one it reads — otherwise guard no. 2 would compare
/// a text with `\r\n` (what Windows will give back to us) to a text with `\n`, and would
/// never recognise our own write.
///
/// RED if the raw text is memorised.
#[test]
fn after_our_write_memorises_the_normalised_text() {
    let mut sondeur = Sondeur::new();
    amorce(&mut sondeur);
    // What we HANDED to Windows carries `\r\n`s (it is `denormaliser` that
    // puts them there); what we reread will therefore carry them too.
    sondeur.apres_notre_ecriture(7, "une\r\ndeux");
    assert_eq!(
        sondeur.observer(8, || Some(String::from("une\r\ndeux"))),
        None
    );
}

/// 🔴 **The first thing a test must see red** (spec §7.1):
/// the round trip must change nothing.
///
/// RED if `denormaliser` doubles the `\r`s — `normaliser` would then return two
/// lines where there was one.
#[test]
fn the_normalise_denormalise_round_trip_is_the_identity() {
    let normalise = "une\ndeux\ntrois";
    assert_eq!(normaliser(&denormaliser(normalise)), normalise);
}

/// RED if `denormaliser` added a `\r\n` where there is no break.
#[test]
fn denormalising_leaves_a_text_without_line_break_intact() {
    assert_eq!(denormaliser("abc"), "abc");
}

/// 🔴 **It is the REAL case, not a curiosity**: the text comes from a
/// browser, and nothing guarantees it does not already carry `\r\n`s — a copy
/// from a local Windows editor carries them.
///
/// RED if `denormaliser` is a naive `replace("\n", "\r\n")`: it would return
/// `a\r\r\nb`, and Notepad would show one more empty line.
#[test]
fn denormalising_does_not_double_crlf_already_present() {
    assert_eq!(denormaliser("a\r\nb"), "a\r\nb");
}

/// A lone `\r` becomes `\r\n` too: Windows does not show a bare `\r`
/// as a line break in Notepad.
#[test]
fn denormalising_also_handles_a_lone_cr() {
    assert_eq!(denormaliser("a\rb"), "a\r\nb");
}

/// RED if the comparison is a `>=` instead of a `>`: the exact limit case
/// would be refused while it fits.
#[test]
fn bounding_incoming_accepts_exactly_the_bound_and_refuses_one_byte_more() {
    let pile = "a".repeat(PRESSE_PAPIER_MAX);
    assert_eq!(borner_entrant(&pile), Some(pile.clone()));
    let un_de_trop = "a".repeat(PRESSE_PAPIER_MAX + 1);
    assert_eq!(borner_entrant(&un_de_trop), None);
}

/// 🔴 The bound counts **UTF-8 bytes**, never `char`s — it is the
/// same unit as the outgoing direction's, which protects a channel.
///
/// RED if the implementation is `texte.chars().count()`: this text has
/// `PRESSE_PAPIER_MAX / 4` characters, so would pass, for exactly
/// `PRESSE_PAPIER_MAX` bytes — then one more character would make it overflow
/// by four bytes without the `char` count noticing.
#[test]
fn bounding_incoming_counts_utf8_bytes_not_chars() {
    let emojis = "😀".repeat(PRESSE_PAPIER_MAX / 4);
    assert_eq!(emojis.len(), PRESSE_PAPIER_MAX);
    assert_eq!(emojis.chars().count(), PRESSE_PAPIER_MAX / 4);
    assert_eq!(borner_entrant(&emojis), Some(emojis.clone()));

    let un_de_trop = format!("{emojis}😀");
    assert_eq!(un_de_trop.chars().count(), PRESSE_PAPIER_MAX / 4 + 1);
    assert_eq!(borner_entrant(&un_de_trop), None);
}

/// 🔴 **THE DISARMED ARM OF CRITERION ④, AND IT MUST DISARM BOTH GUARDS.**
///
/// The observable is twofold, and both halves count:
/// - the read closure **is called** ⟹ `reference` was not set,
///   so guard no. 1 is indeed disarmed;
/// - `observer` returns **`Some`** ⟹ `last_emitted` was not set either,
///   so guard no. 2 is disarmed too.
///
/// RED if `armer` only disarms `reference`: the read would happen,
/// but guard no. 2 would absorb the announcement and the acceptance run's count would stay
/// at ZERO — criterion ④'s red would be vacuous a second time.
#[test]
fn disarming_the_guards_lets_our_own_write_be_read_and_announced() {
    let mut sondeur = Sondeur::new();
    amorce(&mut sondeur);
    sondeur.armer(false, 7, "colle");

    let lu = Cell::new(false);
    let annonce = sondeur.observer(7, || {
        lu.set(true);
        Some(String::from("colle"))
    });

    assert!(
        lu.get(),
        "disarmed, the clipboard MUST be reopened (guard no. 1)"
    );
    assert_eq!(
        annonce,
        Some(Annonce::Texte(String::from("colle"))),
        "disarmed, our own text MUST be announced (guard no. 2)"
    );
}

/// The counterpart: armed — the default state, without the variable —, both guards
/// bite. It is the test `apres_notre_ecriture` already carries; this one
/// checks that `armer(true, …)` is indeed the same path, and not a second one.
///
/// RED if `apres_notre_ecriture` stopped delegating to `armer`.
#[test]
fn arming_to_true_is_the_same_path_as_after_our_write() {
    let mut par_defaut = Sondeur::new();
    amorce(&mut par_defaut);
    par_defaut.apres_notre_ecriture(7, "colle");

    let mut explicite = Sondeur::new();
    amorce(&mut explicite);
    explicite.armer(true, 7, "colle");

    assert_eq!(
        par_defaut.observer(7, || panic!("garde n°1")),
        explicite.observer(7, || panic!("garde n°1"))
    );
    assert_eq!(
        par_defaut.observer(8, || Some(String::from("colle"))),
        explicite.observer(8, || Some(String::from("colle")))
    );
}

// ── THE SECOND TAKE OF D-P3-6 (sub-block P3, task 5) ─────────────────────
//
// 🔴 **A DEFECT OF THE PLAN, REPORTED AND FIXED HERE RATHER THAN COPIED.** Its
// Step 1 prescribes a TWO-line test — `armer(true, seqA, textA)` then
// `observer(seqB, || Some(textB))` — "seen RED on the intact tree". It
// was, and the evidence is recorded
// (`journaux-presse-papier-p3/rouge-t5-d-p3-6-arbre-intact.log`): the race
// is CONFIRMED, RP3-9 is not realised.
//
// ⚠️ **But these two lines alone can NEVER become green**, and the
// plan had not seen it: the remedy it settles on itself is a
// POST-FILTER — `filtrer_nos_ecritures_tardives` runs AFTER `tour()`, on its
// result. `observer` cannot know about a write that only arrived
// after it; requiring it to return `None` would be requiring it to guess.
//
// **The letter of the test is therefore kept, and one line is added to it**: the
// second take, applied to the result. The first two lines are
// the very ones that went red.

/// RED on the intact tree: the first two lines returned
/// `Some(Texte("textB"))` where the third must return `None`.
#[test]
fn our_write_occurring_after_arming_is_not_announced() {
    let mut s = Sondeur::new();
    // The wheel turn armed on the first write (window A).
    s.armer(true, 10, "textA");
    // Window B pastes: the clipboard carries `textB`, the counter has
    // moved again, and `tour()` therefore produces an announcement both of D5's guards
    // let through.
    let annonce = s.observer(11, || Some(String::from("textB")));
    assert_eq!(annonce, Some(Annonce::Texte(String::from("textB"))));
    // The second take consumes B's pair and discards ITS OWN text.
    assert_eq!(
        s.ecarter(true, Some((11, String::from("textB"))), annonce),
        None
    );
}

/// 🔴 THE FIX'S SAFEGUARD: filtering too wide would silence a REAL
/// copy. RED if the filter bears on `seq` alone instead of the text — a
/// third-party copy occurring after our write also carries a later
/// `seq`, and the number alone does not tell them apart.
#[test]
fn a_third_party_copy_after_arming_is_always_announced() {
    let mut s = Sondeur::new();
    s.armer(true, 10, "textA");
    // We wrote `textB` (seq 11), THEN a third-party application copied
    // `textC`: it is `textC` the clipboard carries, and it must go out.
    let annonce = s.observer(12, || Some(String::from("textC")));
    assert_eq!(
        s.ecarter(true, Some((11, String::from("textB"))), annonce),
        Some(Annonce::Texte(String::from("textC")))
    );
}

/// RED if the filter applies to `Annonce::Refus`, which has no text to
/// compare: the user would lose the banner telling them why nothing
/// arrived.
#[test]
fn the_filter_does_not_touch_a_size_refusal() {
    let mut s = Sondeur::new();
    let refus = Some(Annonce::Refus { octets: 99_999 });
    assert_eq!(
        s.ecarter(true, Some((11, String::from("textB"))), refus.clone()),
        refus
    );
}

/// RED if the second take discards anyway under `PRESSE_PAPIER_GARDE=0`:
/// this bench arm exists to make reachable the red of P2's criterion ④,
/// which counts the messages coming back to the window after a paste, and a
/// take that bit anyway would empty it of its meaning.
#[test]
fn the_second_take_is_disarmed_by_the_bench_arm() {
    let mut s = Sondeur::new();
    s.armer(true, 10, "textA");
    let annonce = s.observer(11, || Some(String::from("textB")));
    assert_eq!(
        s.ecarter(false, Some((11, String::from("textB"))), annonce.clone()),
        annonce
    );
}

/// Without a write on our part, the second take is transparent — and it
/// arms nothing: RED if it set `reference` on an invented `seq`.
#[test]
fn without_our_write_the_second_take_touches_nothing() {
    let mut s = Sondeur::new();
    amorce(&mut s);
    let annonce = s.observer(2, || Some(String::from("copie-tierce")));
    assert_eq!(
        s.ecarter(true, None, annonce.clone()),
        Some(Annonce::Texte(String::from("copie-tierce")))
    );
    assert_eq!(annonce, Some(Annonce::Texte(String::from("copie-tierce"))));
}
