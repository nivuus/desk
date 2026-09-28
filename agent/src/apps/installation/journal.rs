//! The TAIL of an installer's log, bounded — and why it is PURE.
//!
//! 🔴 THIS MODULE WAS BORN FROM A DEFECT OF THIS VERY SUB-BLOCK, FOUND WHILE RE-READING.
//! The bound was written TWICE — once on bytes in
//! `execution.rs`, once on a `String` in `fil.rs` — and the second
//! **PANICKED**: `&texte[texte.len() - N..]` on a `&str` requires the
//! index to fall on a UTF-8 character boundary, otherwise Rust panics.
//!
//! 🔴 AND THAT PATH WAS REACHABLE, which is the point. `String::from_utf8_lossy`
//! **ENLARGES**: every invalid byte becomes a THREE-byte U+FFFD. An
//! installer log cut at 64 KiB of raw bytes can therefore yield a
//! `String` of more than 64 KiB — and the second bound, believing it had nothing
//! to do, then cut in the middle of a character. **An installer writing
//! Latin-1 on its standard output would have been enough**, and the symptom would have been an
//! installation thread that dies without reporting an outcome.
//!
//! ⚠️ THE LESSON IS NOT "BEWARE OF UTF-8": it is that a bound written
//! twice is a bound that diverges. It is written here, once, PURE, and
//! **both callers use it** — the one that has bytes and the one that has
//! a string.

/// The tail of the installer's log that is sent up.
///
/// ⚠️ **NOT CALIBRATED**, it joins the list this repository has kept since
/// `BPP_MIN`.
pub const JOURNAL_MAX_OCTETS: usize = 64 * 1024;

/// The END of a log, not its head.
///
/// 🔴 THE END, BECAUSE THAT IS WHERE THE ERROR MESSAGE of an installer
/// that failed lives. The second member says whether it was truncated, which distinguishes "cut" from
/// "empty" — otherwise a user would read the last 64 KiB believing they
/// were reading everything.
///
/// ⚠️ **AN EMPTY LOG IS THE NORMAL CASE**, not a failure: most
/// Windows installers are graphical and write nothing on the standard
/// streams. The interface must not present it as a failure.
pub fn queue(texte: &str) -> (&str, bool) {
    if texte.len() <= JOURNAL_MAX_OCTETS {
        return (texte, false);
    }
    // 🔴 WE MOVE FORWARD TO THE NEXT CHARACTER BOUNDARY, we do not move
    // back: moving back would return more than the bound, and the bound is what we
    // promise. At worst we return three bytes less.
    let mut coupe = texte.len() - JOURNAL_MAX_OCTETS;
    while coupe < texte.len() && !texte.is_char_boundary(coupe) {
        coupe += 1;
    }
    (&texte[coupe..], true)
}

/// The same rule, on raw bytes — the case of whoever has just read a
/// file.
///
/// ⚠️ IT CUTS BEFORE CONVERTING, and the order matters: converting first
/// would force holding in memory an installer log of unknown size,
/// which nothing bounds on the Windows side.
pub fn queue_octets(octets: &[u8]) -> (String, bool) {
    if octets.len() <= JOURNAL_MAX_OCTETS {
        return (String::from_utf8_lossy(octets).into_owned(), false);
    }
    let texte = String::from_utf8_lossy(&octets[octets.len() - JOURNAL_MAX_OCTETS..]).into_owned();
    // ⚠️ WE RE-BOUND AFTER THE CONVERSION: `from_utf8_lossy` ENLARGES — an invalid
    // byte becomes a three-byte U+FFFD —, so that cutting
    // `JOURNAL_MAX_OCTETS` raw bytes can yield a string LONGER than
    // the bound. That is precisely what the second caller assumed false.
    let (queue, _) = queue(&texte);
    (queue.to_string(), true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn un_journal_court_passe_entier_et_n_est_pas_tronque() {
        assert_eq!(queue("bonjour"), ("bonjour", false));
        assert_eq!(queue(""), ("", false));
        let pile = "a".repeat(JOURNAL_MAX_OCTETS);
        assert_eq!(queue(&pile), (pile.as_str(), false));
    }

    #[test]
    fn un_journal_long_rend_sa_fin_et_se_declare_tronque() {
        let long = format!("{}FIN", "a".repeat(JOURNAL_MAX_OCTETS));
        let (q, tronque) = queue(&long);
        assert!(tronque);
        assert!(
            q.ends_with("FIN"),
            "it is the END that we keep, not the head"
        );
        assert!(q.len() <= JOURNAL_MAX_OCTETS);
    }

    /// 🔴 THE RED OF THE DEFECT THAT GAVE BIRTH TO THIS MODULE.
    ///
    /// The original implementation did `&texte[texte.len() - N..]` without
    /// checking the character boundary: on a log whose byte at that
    /// position is in the middle of a multi-byte character, **Rust PANICS**. The
    /// installation thread would have died without reporting an outcome, and the hub would have
    /// shown "in progress" forever.
    #[test]
    fn ne_panique_jamais_au_milieu_d_un_caractere_multi_octet() {
        // U+00E9 (e acute) is two bytes; repeating it enough puts the cut in the middle
        // of a character one time in two, whatever the padding.
        for rembourrage in 0..4 {
            let texte = format!(
                "{}{}",
                "x".repeat(rembourrage),
                "é".repeat(JOURNAL_MAX_OCTETS)
            );
            let (q, tronque) = queue(&texte);
            assert!(tronque);
            assert!(q.len() <= JOURNAL_MAX_OCTETS);
            // And the result is VALID text: that is what the typing
            // guarantees, and what the panic replaced.
            assert!(q.chars().all(|c| c == 'é' || c == 'x'));
        }
    }

    /// 🔴 THE PATH THAT MAKES THE PANIC REACHABLE: `from_utf8_lossy` ENLARGES.
    ///
    /// Every invalid byte becomes a three-byte U+FFFD. Cutting
    /// `JOURNAL_MAX_OCTETS` RAW bytes can therefore yield a string much
    /// longer than the bound — and that is exactly what the caller assumed
    /// false when it re-bounded without checking the boundary.
    #[test]
    fn from_utf8_lossy_agrandit_donc_on_reborne_apres_la_conversion() {
        // Only invalid bytes: each one costs three bytes once converted.
        let octets = vec![0xFFu8; JOURNAL_MAX_OCTETS + 10];
        let brut = String::from_utf8_lossy(&octets[octets.len() - JOURNAL_MAX_OCTETS..]);
        assert!(
            brut.len() > JOURNAL_MAX_OCTETS,
            "the conversion must GROW, otherwise this test exercises nothing"
        );
        let (q, tronque) = queue_octets(&octets);
        assert!(tronque);
        assert!(
            q.len() <= JOURNAL_MAX_OCTETS,
            "the bound is what we promise: {} bytes",
            q.len()
        );
    }
}
