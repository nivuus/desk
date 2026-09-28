use super::*;
use std::cell::Cell;

/// A poller that has already taken its reference: it is the nominal state after
/// the first turn, and the one in which all the properties below
/// are judged.
pub(super) fn amorce(sondeur: &mut Sondeur) {
    assert_eq!(
        sondeur.observer(1, || Some(String::from("etat-initial"))),
        None
    );
}

/// RED if `normaliser` lets `\r\n` through: P2's round trip
/// would then double lines at each turn.
#[test]
fn normalising_turns_crlf_into_lf() {
    assert_eq!(normaliser("a\r\nb"), "a\nb");
}

/// RED if only `\r\n` is handled: classic Mac line endings
/// would pass through as is.
#[test]
fn normalising_turns_a_lone_cr_into_lf() {
    assert_eq!(normaliser("a\rb"), "a\nb");
}

/// RED if `normaliser` replaced `\n` with `\r\n`: the function would
/// no longer be idempotent and P2's round trip would diverge.
#[test]
fn normalising_is_idempotent() {
    let une = normaliser("a\r\nb\rc\nd");
    assert_eq!(une, "a\nb\nc\nd");
    assert_eq!(normaliser(&une), une);
}

/// 🔴 "We do not open the clipboard for nothing", and it is checkable
/// WITHOUT Windows: the witness is a `Cell<bool>`.
///
/// ROUGE si `observer` appelle `lire` inconditionnellement.
#[test]
fn an_unchanged_number_does_not_open_the_clipboard() {
    let mut sondeur = Sondeur::new();
    amorce(&mut sondeur);
    let appele = Cell::new(false);
    let annonce = sondeur.observer(1, || {
        appele.set(true);
        Some(String::from("bonjour"))
    });
    assert_eq!(annonce, None);
    assert!(
        !appele.get(),
        "lire() must not be called with an unchanged number"
    );
}

#[test]
fn a_new_number_announces_the_text() {
    let mut sondeur = Sondeur::new();
    amorce(&mut sondeur);
    assert_eq!(
        sondeur.observer(2, || Some(String::from("bonjour"))),
        Some(Annonce::Texte(String::from("bonjour")))
    );
}

/// 🔴 D5's guard no. 2, and the red of the acceptance run's criterion ②.
///
/// The counter MOVES on an identical rewrite — measured by probe
/// P0 (`q2` answered "moves", two runs). Without the content comparison, this
/// gesture would push a message for nothing.
///
/// ROUGE si l'on retire la comparaison : `Some` serait rendu deux fois.
#[test]
fn the_same_text_at_a_different_number_is_announced_only_once() {
    let mut sondeur = Sondeur::new();
    amorce(&mut sondeur);
    assert_eq!(
        sondeur.observer(2, || Some(String::from("bonjour"))),
        Some(Annonce::Texte(String::from("bonjour")))
    );
    assert_eq!(sondeur.observer(3, || Some(String::from("bonjour"))), None);
}

/// 🔴 Beyond the bound we REFUSE, we NEVER truncate: a silently
/// truncated paste is the worst possible outcome.
///
/// RED if the implementation truncates — the assertion on the variant fails.
#[test]
fn a_too_large_text_is_refused_never_truncated() {
    let mut sondeur = Sondeur::new();
    amorce(&mut sondeur);
    let gros = "a".repeat(PRESSE_PAPIER_MAX + 1);
    let annonce = sondeur.observer(2, || Some(gros));
    assert_eq!(
        annonce,
        Some(Annonce::Refus {
            octets: (PRESSE_PAPIER_MAX + 1) as u32
        })
    );
}

/// RED if the bound is written `>=` instead of `>`.
#[test]
fn a_text_of_exactly_the_bound_size_passes() {
    let mut sondeur = Sondeur::new();
    amorce(&mut sondeur);
    let pile = "a".repeat(PRESSE_PAPIER_MAX);
    assert_eq!(
        sondeur.observer(2, || Some(pile.clone())),
        Some(Annonce::Texte(pile))
    );
}

/// 🔴 D-P1-2: we normalise FIRST, we bound AFTER.
///
/// The text weighs `PRESSE_PAPIER_MAX + 8` raw bytes and carries 12 `\r`s
/// paired with as many `\n`s: normalisation removes 12 of them, so it
/// fits. RED if we bound before normalising — it would be refused.
#[test]
fn normalise_before_clamping() {
    let mut sondeur = Sondeur::new();
    amorce(&mut sondeur);
    let corps = "a".repeat(PRESSE_PAPIER_MAX + 8 - 24);
    let brut = format!("{corps}{}", "\r\n".repeat(12));
    assert_eq!(brut.len(), PRESSE_PAPIER_MAX + 8);
    let attendu = normaliser(&brut);
    assert_eq!(attendu.len(), PRESSE_PAPIER_MAX - 4);
    assert_eq!(
        sondeur.observer(2, || Some(brut)),
        Some(Annonce::Texte(attendu))
    );
}

/// 🔴 Bounding counts UTF-8 BYTES, not `char`s.
///
/// RED if we bound on `.chars().count()`: this text is
/// `PRESSE_PAPIER_MAX / 4 + 1` characters, far below the bound
/// counted that way, and would pass while weighing more than 64 KiB.
#[test]
fn bounding_counts_bytes_not_characters() {
    let mut sondeur = Sondeur::new();
    amorce(&mut sondeur);
    let emoji = "🙂".repeat(PRESSE_PAPIER_MAX / 4 + 1);
    assert_eq!(emoji.chars().count(), PRESSE_PAPIER_MAX / 4 + 1);
    assert!(emoji.len() > PRESSE_PAPIER_MAX);
    assert!(matches!(
        sondeur.observer(2, || Some(emoji)),
        Some(Annonce::Refus { .. })
    ));
}

/// 🔴 D-P1-5: a read that FAILS does not advance the reference.
///
/// Otherwise the corresponding content would be lost forever: the next turn
/// would see an "unchanged" counter and would retry nothing.
///
/// RED if we memorise the number before reading — the second call,
/// at the SAME number, would no longer call `lire`.
#[test]
fn a_failed_read_does_not_advance_the_reference() {
    let mut sondeur = Sondeur::new();
    amorce(&mut sondeur);
    assert_eq!(sondeur.observer(2, || None), None);
    let rappelee = Cell::new(false);
    let annonce = sondeur.observer(2, || {
        rappelee.set(true);
        Some(String::from("rattrape"))
    });
    assert!(
        rappelee.get(),
        "the same number must be retried after a failure"
    );
    assert_eq!(annonce, Some(Annonce::Texte(String::from("rattrape"))));
}

/// RED if the refusal is not memorised: the banner would flicker at
/// each neighbouring copy as long as the huge content stays in place.
#[test]
fn an_identically_repeated_refusal_is_announced_only_once() {
    let mut sondeur = Sondeur::new();
    amorce(&mut sondeur);
    let gros = "a".repeat(PRESSE_PAPIER_MAX + 1);
    assert!(sondeur.observer(2, || Some(gros.clone())).is_some());
    assert_eq!(sondeur.observer(3, || Some(gros)), None);
}

/// The state read at the FIRST turn serves as reference, and is not announced (D-P1-4,
/// pattern of `SuiviBordure`).
///
/// ❌ **THIS COMMENT ADDED "a window that attaches does not receive the
/// content already present, it receives the first copy THAT FOLLOWS", AND
/// SUB-BLOCK P3 REFUTED IT** — it was P1's legacy no. 3, now closed by
/// `Etat::last_clipboard` and its emission at registration. The property
/// THIS test exercises, for its part, is intact: the `Sondeur` announces nothing at its
/// first turn.
///
/// RED if the first turn announces — which would make each
/// attach receive a content the user did not copy for it.
#[test]
fn the_first_round_takes_a_reference_and_announces_nothing() {
    let mut sondeur = Sondeur::new();
    assert_eq!(sondeur.observer(7, || Some(String::from("deja-la"))), None);
    // And that content is indeed kept: copying it again restarts nothing.
    assert_eq!(sondeur.observer(8, || Some(String::from("deja-la"))), None);
    assert_eq!(
        sondeur.observer(9, || Some(String::from("neuf"))),
        Some(Annonce::Texte(String::from("neuf")))
    );
}
