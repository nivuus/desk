use super::*;

// ─ ce qui a MIGRÉ, et où le retrouver ─────────────────────────────────────
// Les trois tests de FORME sur le fil qui vivaient ici — sérialisation épinglée
// des en-têtes de requête, relecture des en-têtes de réponse, rejet d'un
// en-tête incomplet — sont partis avec les structures qu'ils épinglaient, dans
// `proto/src/fichiers/entetes/tests.rs`. Ils y sont plus forts qu'ici : leur
// vecteur est un FICHIER PARTAGÉ que le jumeau TypeScript lit aussi, là où ces
// trois-là n'épinglaient les formes que d'un seul côté.
//
// Ne restent donc ici que les tests de `filetime_depuis_ms`, qui n'a pas de
// jumeau navigateur.

/// 🔴 **L'époque de FILETIME n'est pas celle d'Unix, et l'écart est de 369
/// ans.** Se tromper d'époque ou d'unité rend des dates de 1601 dans
/// l'Explorateur — visible, mais seulement si quelqu'un regarde. Se tromper de
/// FACTEUR (10⁷ contre 10⁶) ne se voit quasiment pas.
#[test]
fn l_epoque_unix_devient_l_epoque_filetime() {
    // 1ᵉʳ janvier 1970, 00:00:00 UTC = 116 444 736 000 000 000 unités de 100 ns
    // depuis le 1ᵉʳ janvier 1601.
    assert_eq!(filetime_depuis_ms(0), 116_444_736_000_000_000);
    // Une milliseconde vaut 10 000 unités de 100 ns.
    assert_eq!(filetime_depuis_ms(1), 116_444_736_000_010_000);
    assert_eq!(filetime_depuis_ms(1000), 116_444_736_010_000_000);
}

/// Une date antérieure à 1970 est licite (`lastModified` peut être négatif) ;
/// une date antérieure à **1601** ne l'est pas, et rendrait un FILETIME négatif
/// que Windows interprète comme un temps relatif. Elle est ramenée à zéro.
#[test]
fn une_date_anterieure_a_1601_est_ramenee_a_zero() {
    assert_eq!(filetime_depuis_ms(-11_644_473_600_000), 0);
    assert_eq!(filetime_depuis_ms(-11_644_473_600_001), 0);
    assert_eq!(filetime_depuis_ms(i64::MIN), 0);
    // Juste au-dessus de l'époque FILETIME : toujours positif.
    assert_eq!(filetime_depuis_ms(-11_644_473_599_999), 10_000);
}

/// Un `lastModified` absurde ne doit pas faire déborder le calcul : un
/// débordement en `release` boucle en silence et rendrait une date arbitraire.
#[test]
fn une_date_absurde_ne_deborde_pas() {
    assert_eq!(filetime_depuis_ms(i64::MAX), i64::MAX);
}
