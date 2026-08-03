use super::*;

fn f(session: &str, eveillee: bool, focalisee: bool) -> Fenetre {
    Fenetre { session: session.to_string(), eveillee, focalisee }
}

fn part_de(parts: &[(String, u32)], session: &str) -> u32 {
    parts.iter().find(|(s, _)| s == session).map(|(_, bps)| *bps).expect("session absente")
}

#[test]
fn sans_aucune_fenetre_il_n_y_a_rien_a_repartir() {
    assert!(repartir(12_000_000, &[]).is_empty());
}

#[test]
fn une_seule_eveillee_recoit_tout_le_budget() {
    let parts = repartir(12_000_000, &[f("a", true, true)]);
    assert_eq!(part_de(&parts, "a"), 12_000_000);
}

#[test]
fn sans_focalisee_les_eveillees_se_partagent_a_parts_egales() {
    let parts = repartir(12_000_000, &[f("a", true, false), f("b", true, false)]);
    assert_eq!(part_de(&parts, "a"), 6_000_000);
    assert_eq!(part_de(&parts, "b"), 6_000_000);
}

#[test]
fn la_focalisee_recoit_le_facteur_de_majoration() {
    // 2 éveillées, dont une focalisée : diviseur = 1 + FACTEUR_FOCUS = 3.
    let parts = repartir(12_000_000, &[f("a", true, true), f("b", true, false)]);
    assert_eq!(part_de(&parts, "b"), 4_000_000);
    assert_eq!(part_de(&parts, "a"), 8_000_000);
    assert!(part_de(&parts, "a") > part_de(&parts, "b"), "la focalisée doit recevoir plus");
}

#[test]
fn une_endormie_recoit_le_plancher_et_ne_partage_pas_le_reste() {
    let parts = repartir(12_000_000, &[f("a", true, true), f("b", false, false)]);
    assert_eq!(part_de(&parts, "b"), PART_DORMANTE_BPS);
    assert_eq!(
        part_de(&parts, "a"),
        12_000_000 - PART_DORMANTE_BPS,
        "l'éveillée seule prend tout le reste"
    );
}

#[test]
fn toutes_endormies_recoivent_le_plancher_et_le_reste_n_est_donne_a_personne() {
    let parts = repartir(12_000_000, &[f("a", false, false), f("b", false, false)]);
    assert_eq!(part_de(&parts, "a"), PART_DORMANTE_BPS);
    assert_eq!(part_de(&parts, "b"), PART_DORMANTE_BPS);
}

/// Le cas limite que le produit rend atteignable : le client annonce le focus
/// sur une fenêtre que le vivier a ÉVINCÉE. Elle est endormie, donc au
/// plancher, et la majoration ne revient à personne.
#[test]
fn une_focalisee_endormie_reste_au_plancher_et_les_eveillees_se_partagent_egalement() {
    let parts =
        repartir(12_000_000, &[f("dormeuse", false, true), f("a", true, false), f("b", true, false)]);
    assert_eq!(part_de(&parts, "dormeuse"), PART_DORMANTE_BPS);
    let reste = 12_000_000 - PART_DORMANTE_BPS;
    assert_eq!(part_de(&parts, "a"), reste / 2);
    assert_eq!(part_de(&parts, "b"), reste / 2);
}

/// Plusieurs focalisées ne peuvent pas se produire durablement (le client
/// émet `blur`), mais la fonction doit rester TOTALE : une seule majoration
/// est accordée, à la première rencontrée, sinon l'invariant de budget saute.
#[test]
fn plusieurs_focalisees_ne_donnent_qu_une_seule_majoration() {
    let parts = repartir(12_000_000, &[f("a", true, true), f("b", true, true)]);
    let somme: u32 = parts.iter().map(|(_, bps)| bps).sum();
    assert!(somme <= 12_000_000, "somme {somme} au-dessus du budget");
}

/// **L'invariant central.** Il vaut à tous les rangs que le produit permet :
/// `vivier::PLAFOND_EVEIL` vaut 8, `superviseur::CAPACITE` vaut 10.
#[test]
fn la_somme_des_parts_ne_depasse_jamais_le_budget() {
    for eveillees in 0..=8usize {
        for endormies in 0..=(10 - eveillees) {
            let mut fenetres = Vec::new();
            for i in 0..eveillees {
                fenetres.push(f(&format!("e{i}"), true, i == 0));
            }
            for i in 0..endormies {
                fenetres.push(f(&format!("d{i}"), false, false));
            }
            let parts = repartir(12_000_000, &fenetres);
            let somme: u32 = parts.iter().map(|(_, bps)| bps).sum();
            assert!(
                somme <= 12_000_000,
                "{eveillees} éveillées et {endormies} endormies : somme {somme}"
            );
            assert_eq!(parts.len(), fenetres.len(), "chaque fenêtre doit recevoir une part");
            assert!(parts.iter().all(|(_, bps)| *bps > 0), "aucune part ne doit être nulle");
        }
    }
}

/// Un budget trop petit pour payer les planchers ne doit ni déborder ni
/// paniquer par soustraction en dessous de zéro.
#[test]
fn un_budget_inferieur_aux_planchers_ne_deborde_pas() {
    let fenetres: Vec<Fenetre> = (0..8).map(|i| f(&format!("d{i}"), false, false)).collect();
    let parts = repartir(100_000, &fenetres);
    assert!(parts.iter().all(|(_, bps)| *bps > 0));
}
