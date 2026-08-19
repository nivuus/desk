//! Le délai avant la prochaine tentative d'ouverture du canal `/agent`.
//!
//! Pur, sans horloge et sans socket : c'est la seule part du client de canal
//! qui soit éprouvable sur l'hôte Linux, et c'est pour cela qu'elle vit dans
//! son propre fichier.

/// Attente avant la PREMIÈRE reprise. ⚠️ **NON CALIBRÉE** : aucune mesure de
/// ce dépôt ne dit combien de temps une coupure dure en pratique. Choisie
/// assez courte pour qu'une coupure d'une seconde ne coûte pas une minute de
/// `vu_a` périmé, assez longue pour qu'un relais qui refuse ne soit pas
/// martelé.
pub const REPLI_MIN_MS: u64 = 500;

/// Plafond de l'attente. ⚠️ **NON CALIBRÉE** de la même façon.
///
/// 🔴 **Ce plafond n'est PAS un confort, c'est le garde-fou du critère ④.**
/// Sans lui, le repli exponentiel atteint l'heure à la treizième tentative et
/// le jour à la dix-huitième : la VM resterait `injoignable` alors que le
/// réseau est revenu, et **rien ne le dirait** — l'agent serait vivant, en
/// train d'attendre.
pub const REPLI_MAX_MS: u64 = 30_000;

/// Délai avant la tentative n°`tentative + 1`, la tentative 0 étant la
/// première reprise après une chute.
pub fn delai_de_repli(tentative: u32) -> u64 {
    // `checked_shl` et non `1 << tentative` : le décalage déborde à 64, et un
    // débordement en `debug` est un `panic`, donc la mort du fil de reprise —
    // le seul fil qui pouvait encore ramener la VM. `saturating_mul` couvre
    // le même risque un cran plus loin.
    let facteur = 1u64.checked_shl(tentative).unwrap_or(u64::MAX);
    REPLI_MIN_MS.saturating_mul(facteur).min(REPLI_MAX_MS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_premiere_reprise_attend_le_minimum() {
        assert_eq!(delai_de_repli(0), REPLI_MIN_MS);
    }

    /// Un délai constant, c'est un martèlement : dix mille tentatives par
    /// heure contre un relais qui vient de refuser.
    #[test]
    fn le_delai_croit_avec_la_tentative() {
        assert!(
            delai_de_repli(1) > delai_de_repli(0),
            "delai_de_repli(1) = {} n'est pas > delai_de_repli(0) = {}",
            delai_de_repli(1),
            delai_de_repli(0)
        );
        assert!(
            delai_de_repli(2) > delai_de_repli(1),
            "delai_de_repli(2) = {} n'est pas > delai_de_repli(1) = {}",
            delai_de_repli(2),
            delai_de_repli(1)
        );
    }

    /// 🔴 Le test qui compte. Sans le plafond, la vingtième tentative attend
    /// **145 heures** : la VM est injoignable pour toujours, et l'agent qui
    /// attend a exactement l'air d'un agent qui va reprendre.
    #[test]
    fn le_delai_est_borne_par_le_plafond() {
        for tentative in 0..200u32 {
            assert!(
                delai_de_repli(tentative) <= REPLI_MAX_MS,
                "delai_de_repli({tentative}) = {} dépasse REPLI_MAX_MS = {REPLI_MAX_MS}",
                delai_de_repli(tentative)
            );
        }
        assert_eq!(
            delai_de_repli(20),
            REPLI_MAX_MS,
            "le plafond doit être ATTEINT, pas seulement respecté"
        );
    }

    /// `1 << tentative` déborde à 64 — et une boucle de reprise qui tourne
    /// depuis assez longtemps y arrive. Un débordement en `debug` est un
    /// `panic`, donc la mort du fil de reprise : la VM ne revient jamais.
    #[test]
    fn le_delai_ne_deborde_pas_sur_une_tentative_enorme() {
        assert_eq!(delai_de_repli(63), REPLI_MAX_MS);
        assert_eq!(delai_de_repli(64), REPLI_MAX_MS);
        assert_eq!(delai_de_repli(u32::MAX), REPLI_MAX_MS);
    }
}
