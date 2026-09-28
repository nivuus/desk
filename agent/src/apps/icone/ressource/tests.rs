//! Host tests of the icon directory reader.
//!
//! 🔴 THEY READ TWO REAL FILES, CHECKED IN, AND STRUCTURALLY DIFFERENT
//! ON THE ONLY BYTE THAT MATTERS — `48` versus `0`. They are produced by
//! `agent/testdata/fabriquer-temoins-ico.py`, checked in with them: nobody has to
//! take their content on faith, the script can be re-read and replayed on the host, without
//! Windows.

use super::*;

/// The witness that contains ONLY 48×48 — and that the Shell nevertheless renders as
/// 256×256 32bpp.
const TEMOIN_48: &[u8] = include_bytes!("../../../../testdata/g2-temoin-48.ico");
/// The witness that contains a REAL 256×256, hence `bWidth == 0`.
const TEMOIN_256: &[u8] = include_bytes!("../../../../testdata/g2-temoin-256.ico");

#[test]
fn le_temoin_48_annonce_48_et_rien_d_autre() {
    assert_eq!(icondir_sizes(TEMOIN_48), Some(vec![48]));
    assert_eq!(maximum(&[48]), SourceMax::Pixels(48));
}

/// 🔴 THE MAIN RED, AND THE ONLY ONE INVISIBLE WITHOUT THE WITNESS.
///
/// `bWidth == 0` means 256. A reader that returned `0` would make this test
/// yield `Some(vec![0])`, and `maximum` would rank the largest icon of the corpus
/// BELOW a 16×16 — silently, on an image that itself would be correct.
#[test]
fn le_temoin_256_porte_bwidth_zero_et_vaut_256() {
    // The byte itself, re-read from the file: it is what makes the test
    // decidable rather than trusting.
    assert_eq!(TEMOIN_256[6], 0, "bWidth of the 256 sample must be byte 0");
    assert_eq!(TEMOIN_48[6], 48, "bWidth of the 48 sample must be 48");
    assert_eq!(icondir_sizes(TEMOIN_256), Some(vec![256]));
    assert_eq!(maximum(&[256]), SourceMax::Pixels(256));
}

/// 🔴 WHAT THE SUB-BLOCK EXISTS TO HOLD: the two witnesses are DISTINGUISHED
/// by the resource, where any measurement on the rendered image would confuse them.
#[test]
fn les_deux_temoins_se_distinguent_par_la_ressource() {
    let a = maximum(&icondir_sizes(TEMOIN_48).expect("48 readable"));
    let b = maximum(&icondir_sizes(TEMOIN_256).expect("256 readable"));
    assert_ne!(a, b);
    assert_eq!(a, SourceMax::Pixels(48));
    assert_eq!(b, SourceMax::Pixels(256));
}

/// 🔴 THE TWO ENTRY STRIDES ARE NOT INTERCHANGEABLE — BUT NOT ON A
/// SINGLE ENTRY, AND IT IS MEASURED RATHER THAN ASSUMED.
///
/// ❌ **A FIRST DRAFT OF THIS TEST WAS VACUOUS, and running it
/// exposed it.** It claimed that "the witness with ONE SINGLE entry shows it
/// unambiguously: read with a stride of 14, the 48 witness no longer returns 48".
/// **That is false.** The `bWidth` of the FIRST entry is at offset 6 in both
/// formats — the stride only separates the FOLLOWING entries. On a
/// directory with a single entry, both readers therefore return `[48]`,
/// and the test passed for the wrong reason… until it
/// failed, because it required the opposite.
///
/// What the stride really decides is **the length check** and **the
/// entries from the second one on**. That is therefore where this test looks.
#[test]
fn le_mauvais_pas_d_entree_se_voit_a_partir_de_la_seconde_entree() {
    // The measured fact, written down rather than kept quiet: on ONE entry, both strides
    // agree.
    assert_eq!(icondir_sizes(TEMOIN_48), Some(vec![48]));
    assert_eq!(grpicondir_sizes(TEMOIN_48), Some(vec![48]));

    // 🔴 WITH TWO ENTRIES, THEY DIVERGE. A `GRPICONDIR` of two entries holds
    // 6 + 2×14 = 34 bytes; read with a stride of 16, it would need 38, and the
    // length check returns `None` — never a partial list.
    let mut grp = vec![0u8, 0, 1, 0, 2, 0];
    grp.extend_from_slice(&[32, 32, 0, 0, 1, 0, 32, 0, 0, 0, 0, 0, 7, 0]);
    grp.extend_from_slice(&[0, 0, 0, 0, 1, 0, 32, 0, 0, 0, 0, 0, 8, 0]);
    assert_eq!(grp.len(), 6 + 2 * 14);
    assert_eq!(grpicondir_sizes(&grp), Some(vec![32, 256]));
    assert_eq!(
        icondir_sizes(&grp),
        None,
        "the stride of 16 does not fit in 34 bytes"
    );

    // And the other way round, on a buffer large enough for both: the
    // SECOND entries are read at different offsets, so the lists
    // differ. That is the real shape of the noise.
    let mut ico = vec![0u8, 0, 1, 0, 2, 0];
    ico.extend_from_slice(&[48, 48, 0, 0, 1, 0, 32, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    ico.extend_from_slice(&[16, 16, 0, 0, 1, 0, 32, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    assert_eq!(icondir_sizes(&ico), Some(vec![48, 16]));
    let au_mauvais_pas = grpicondir_sizes(&ico).expect("34 bytes are enough at a stride of 14");
    assert_ne!(
        au_mauvais_pas,
        vec![48, 16],
        "at a stride of 14, the second entry is read at the wrong offset"
    );
}

/// The `GRPICONDIR` shape, on a hand-made buffer: two entries of
/// 14 bytes, one of them with `bWidth == 0`.
#[test]
fn lit_un_grpicondir_a_deux_entrees() {
    let mut o = vec![0u8, 0, 1, 0, 2, 0]; // reserved=0, type=1, count=2
    let mut entree = |w: u8| {
        o.extend_from_slice(&[w, w, 0, 0]); // bWidth, bHeight, bColorCount, bReserved
        o.extend_from_slice(&[1, 0, 32, 0]); // wPlanes, wBitCount
        o.extend_from_slice(&[0, 0, 0, 0]); // dwBytesInRes
        o.extend_from_slice(&[7, 0]); // nID — TWO bytes, that is what makes 14
    };
    entree(32);
    entree(0);
    assert_eq!(grpicondir_sizes(&o), Some(vec![32, 256]));
    assert_eq!(maximum(&[32, 256]), SourceMax::Pixels(256));
}

/// 🔴 AN EMPTY LIST RETURNS `NonMesuree`, NEVER `Pixels(0)`.
#[test]
fn an_empty_list_is_not_a_zero_size() {
    assert_eq!(maximum(&[]), SourceMax::NonMesuree);
    assert_ne!(maximum(&[]), SourceMax::Pixels(0));
}

/// A directory with ZERO entries is read fine — and it returns `NonMesuree`, not
/// `None`: the format is valid, there is simply nothing in it.
#[test]
fn un_repertoire_a_zero_entree_se_lit_et_ne_mesure_rien() {
    let o = [0u8, 0, 1, 0, 0, 0];
    assert_eq!(icondir_sizes(&o), Some(vec![]));
    assert_eq!(
        maximum(&icondir_sizes(&o).expect("valid")),
        SourceMax::NonMesuree
    );
}

/// 🔴 AN UNCHECKED HEADER WOULD LET FOUR ARBITRARY BYTES PASS FOR
/// AN ICON DIRECTORY.
#[test]
fn refuse_un_en_tete_qui_n_en_est_pas_un() {
    // `reserved` non nul.
    assert_eq!(icondir_sizes(&[9, 0, 1, 0, 0, 0]), None);
    // `type` = 2, it is a CURSOR, not an icon.
    assert_eq!(icondir_sizes(&[0, 0, 2, 0, 0, 0]), None);
    // Du texte quelconque.
    assert_eq!(icondir_sizes(b"MZ\x90\x00\x03\x00"), None);
    assert_eq!(grpicondir_sizes(b"MZ\x90\x00\x03\x00"), None);
}

/// 🔴 A TRUNCATED BUFFER RETURNS `None`, IT DOES NOT OVERFLOW AND DOES NOT RETURN A
/// PARTIAL LIST. A partial list would be a FALSE measurement.
#[test]
fn un_tampon_tronque_rend_none() {
    assert_eq!(icondir_sizes(&[]), None);
    assert_eq!(icondir_sizes(&[0, 0, 1, 0, 1]), None); // incomplete header
                                                       // Announces three entries, carries only one.
    let mut o = vec![0u8, 0, 1, 0, 3, 0];
    o.extend_from_slice(&[48; ENTREE_ICO]);
    assert_eq!(icondir_sizes(&o), None);
    // The real witness, cut in two.
    assert_eq!(icondir_sizes(&TEMOIN_256[..10]), None);
}

/// An absurd entry count must not make the arithmetic overflow.
#[test]
fn un_compte_absurde_ne_deborde_pas() {
    let o = [0u8, 0, 1, 0, 0xFF, 0xFF]; // 65,535 entries announced, none carried
    assert_eq!(icondir_sizes(&o), None);
    assert_eq!(grpicondir_sizes(&o), None);
}
