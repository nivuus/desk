use super::*;
use proto::fichiers::TAILLE_TRAME_MAX;

#[test]
fn une_plage_de_taille_exactement_taille_trame_max_fait_un_seul_morceau() {
    // At the EXACT threshold: one chunk, not two with one empty. It is the
    // classic error of a loop that tests `>=` where `>` is needed.
    let m = decouper(0, TAILLE_TRAME_MAX as u64, TAILLE_TRAME_MAX);
    assert_eq!(m.len(), 1);
    assert_eq!(
        m[0],
        Morceau {
            position: 0,
            longueur: TAILLE_TRAME_MAX as u32
        }
    );

    // …and one more byte makes exactly two, the second being one byte.
    let m = decouper(0, TAILLE_TRAME_MAX as u64 + 1, TAILLE_TRAME_MAX);
    assert_eq!(m.len(), 2);
    assert_eq!(
        m[1],
        Morceau {
            position: TAILLE_TRAME_MAX as u64,
            longueur: 1
        }
    );
}

#[test]
fn une_plage_de_taille_nulle_ne_fait_aucun_morceau() {
    // A chunk of length 0 would cause an empty response frame, which
    // nothing would distinguish from an end of file.
    assert!(decouper(0, 0, TAILLE_TRAME_MAX).is_empty());
    assert!(decouper(4096, 0, TAILLE_TRAME_MAX).is_empty());
}

#[test]
fn la_somme_des_longueurs_egale_la_longueur_demandee() {
    // Over a hundred sizes from 1 to 3·max: it is the property that makes the
    // SHA-256 digest of criterion (2) fail if it is wrong by a single byte.
    let max = 1000usize;
    for n in 1..=100u64 {
        let longueur = n * 3 * max as u64 / 100 + 1;
        let somme: u64 = decouper(7, longueur, max)
            .iter()
            .map(|m| u64::from(m.longueur))
            .sum();
        assert_eq!(somme, longueur, "longueur demandée {longueur}");
    }
}

#[test]
fn les_morceaux_sont_contigus_et_croissants() {
    // Neither hole (the file would be truncated in the middle), nor overlap (some
    // bytes would be written twice).
    let morceaux = decouper(1_000, 10_000, 3_000);
    assert_eq!(morceaux.len(), 4);
    let mut attendu = 1_000u64;
    for m in &morceaux {
        assert_eq!(m.position, attendu, "trou ou recouvrement avant {m:?}");
        attendu += u64::from(m.longueur);
    }
    assert_eq!(attendu, 11_000);
    // And no empty chunk in the middle.
    assert!(morceaux.iter().all(|m| m.longueur > 0));
}

#[test]
fn le_premier_morceau_part_de_la_position_demandee() {
    // The position is ignored by any implementation that would restart from 0:
    // ProjFS commonly requests a range IN THE MIDDLE of a file.
    assert_eq!(decouper(1_234_567, 10, 4)[0].position, 1_234_567);
    assert_eq!(decouper(0, 10, 4)[0].position, 0);
}

#[test]
fn une_longueur_superieure_a_u32_max_se_decoupe_quand_meme() {
    // `PRJ_GET_FILE_DATA_CB` receives `byteoffset: u64`: the file can
    // exceed 4 GiB. An implementation that converted the length to `u32`
    // before splitting would lose everything beyond the first round.
    let longueur = u64::from(u32::MAX) + 5;
    let morceaux = decouper(u64::from(u32::MAX), longueur, TAILLE_TRAME_MAX);
    let somme: u64 = morceaux.iter().map(|m| u64::from(m.longueur)).sum();
    assert_eq!(somme, longueur);
    assert_eq!(morceaux[0].position, u64::from(u32::MAX));
    // The last chunk does end well beyond 4 GiB + the starting position.
    let dernier = morceaux.last().unwrap();
    assert_eq!(
        dernier.position + u64::from(dernier.longueur),
        u64::from(u32::MAX) + longueur
    );
}

#[test]
fn un_max_plus_grand_que_u32_max_ne_deborde_pas_a_la_conversion() {
    // On a 64-bit target, `usize` is wider than `u32`: a `max` received
    // beyond `u32::MAX` would make the length conversion overflow. The
    // module bounds it BEFORE converting, and this test is the only thing that
    // establishes it — no F1 caller passes such a value.
    let morceaux = decouper(0, u64::from(u32::MAX) + 2, usize::MAX);
    let somme: u64 = morceaux.iter().map(|m| u64::from(m.longueur)).sum();
    assert_eq!(somme, u64::from(u32::MAX) + 2);
    assert!(morceaux.iter().all(|m| m.longueur > 0));
}
