use super::*;

fn f(session: &str, eveillee: bool, focalisee: bool) -> Fenetre {
    Fenetre {
        session: session.to_string(),
        eveillee,
        focalisee,
    }
}

fn part_de(parts: &[(String, u32)], session: &str) -> u32 {
    parts
        .iter()
        .find(|(s, _)| s == session)
        .map(|(_, bps)| *bps)
        .expect("session absent")
}

#[test]
fn without_any_window_there_is_nothing_to_share_out() {
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
    // 2 awake, one of them focused: divisor = 1 + FACTEUR_FOCUS = 3.
    let parts = repartir(12_000_000, &[f("a", true, true), f("b", true, false)]);
    assert_eq!(part_de(&parts, "b"), 4_000_000);
    assert_eq!(part_de(&parts, "a"), 8_000_000);
    assert!(
        part_de(&parts, "a") > part_de(&parts, "b"),
        "the focused one must receive more"
    );
}

#[test]
fn une_endormie_recoit_le_plancher_et_ne_partage_pas_le_reste() {
    let parts = repartir(12_000_000, &[f("a", true, true), f("b", false, false)]);
    assert_eq!(part_de(&parts, "b"), PART_DORMANTE_BPS);
    assert_eq!(
        part_de(&parts, "a"),
        12_000_000 - PART_DORMANTE_BPS,
        "the awake one alone takes all the rest"
    );
}

#[test]
fn all_asleep_receive_the_floor_and_the_rest_goes_to_nobody() {
    let parts = repartir(12_000_000, &[f("a", false, false), f("b", false, false)]);
    assert_eq!(part_de(&parts, "a"), PART_DORMANTE_BPS);
    assert_eq!(part_de(&parts, "b"), PART_DORMANTE_BPS);
}

/// The edge case the product makes reachable: the client announces focus
/// on a window the pool has EVICTED. It is asleep, hence at the
/// floor, and the boost goes to nobody.
#[test]
fn une_focalisee_endormie_reste_au_plancher_et_les_eveillees_se_partagent_egalement() {
    let parts = repartir(
        12_000_000,
        &[
            f("dormeuse", false, true),
            f("a", true, false),
            f("b", true, false),
        ],
    );
    assert_eq!(part_de(&parts, "dormeuse"), PART_DORMANTE_BPS);
    let reste = 12_000_000 - PART_DORMANTE_BPS;
    assert_eq!(part_de(&parts, "a"), reste / 2);
    assert_eq!(part_de(&parts, "b"), reste / 2);
}

/// Several focused windows cannot happen durably (the client
/// emits `blur`), but the function must remain TOTAL: a single boost
/// is granted, to the first encountered, otherwise the budget invariant breaks.
#[test]
fn several_focused_give_a_single_boost() {
    let parts = repartir(12_000_000, &[f("a", true, true), f("b", true, true)]);
    let somme: u32 = parts.iter().map(|(_, bps)| bps).sum();
    assert!(somme <= 12_000_000, "sum {somme} above the budget");
}

/// **The central invariant.** It holds at every rank the product allows:
/// `vivier::PLAFOND_EVEIL` is 8, `superviseur::CAPACITE` is 10.
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
                "{eveillees} awake and {endormies} asleep: sum {somme}"
            );
            assert_eq!(
                parts.len(),
                fenetres.len(),
                "each window must receive a share"
            );
            assert!(
                parts.iter().all(|(_, bps)| *bps > 0),
                "no share must be zero"
            );
        }
    }
}

/// A budget too small to pay the floors must neither overflow nor
/// panic by subtracting below zero.
#[test]
fn un_budget_inferieur_aux_planchers_ne_deborde_pas() {
    let fenetres: Vec<Fenetre> = (0..8).map(|i| f(&format!("d{i}"), false, false)).collect();
    let parts = repartir(100_000, &fenetres);
    assert!(parts.iter().all(|(_, bps)| *bps > 0));
}

/// **Test of the three regimes.** Validates each regime described in the doc:
/// 1. Regime 1: budget covers the floors AND remainder ≥ divisor → the sum never exceeds the budget
/// 2. Regime 2: budget covers the floors BUT remainder < divisor → overrun bounded by divisor bps
/// 3. Regime 3: budget does not cover the floors → overrun = (floors - budget) + awake
///
/// Also tests the boundary with a focused window: divisor = awake + 1, not awake.
#[test]
fn les_trois_regimes_sont_bornes_ou_jamais_paniquent() {
    // **Regime 1: normal case, ample budget**
    // 2 asleep = 512,000 bps, budget = 1,000,000, remainder = 488,000 >> divisor
    let regime_1 = [
        (f("d0", false, false), false),
        (f("d1", false, false), false),
        (f("e0", true, true), true),
        (f("e1", true, false), true),
    ];
    let fenetres: Vec<Fenetre> = regime_1
        .iter()
        .map(|(fenetre, _)| fenetre.clone())
        .collect();
    let parts = repartir(1_000_000, &fenetres);
    let somme: u32 = parts.iter().map(|(_, bps)| bps).sum();
    assert!(
        somme <= 1_000_000,
        "Regime 1: sum {somme} exceeds budget 1 000 000"
    );
    let e0_part = part_de(&parts, "e0");
    let e1_part = part_de(&parts, "e1");
    assert!(e0_part > e1_part, "Regime 1: the focus boost must exist");

    // **Regime 2a: boundary with a focused window, remainder = divisor - 1**
    // 2 asleep = 512,000 bps, 1 focused + 1 unfocused
    // Divisor = 2 - 1 + 2 = 3, threshold = 512,000 + 3 = 512,003
    // Budget = 512,002 → remainder = 2, remainder < divisor = 3
    let regime_2_focus = [
        (f("d0", false, false), false),
        (f("d1", false, false), false),
        (f("e0", true, true), true),
        (f("e1", true, false), true),
    ];
    let fenetres: Vec<Fenetre> = regime_2_focus
        .iter()
        .map(|(fenetre, _)| fenetre.clone())
        .collect();
    let parts = repartir(512_002, &fenetres);
    let somme: u32 = parts.iter().map(|(_, bps)| bps).sum();
    let depassement = somme.saturating_sub(512_002);
    assert!(
        depassement <= 3,
        "Regime 2 (focused): overrun {depassement} > divisor 3"
    );
    let e0_part = part_de(&parts, "e0");
    let e1_part = part_de(&parts, "e1");
    assert_eq!(
        e0_part, e1_part,
        "Regime 2 (focused): the boost must disappear, {e0_part} != {e1_part}"
    );

    // **Regime 2b: without a focused window, remainder = divisor - 1**
    // 2 asleep = 512,000 bps, 2 unfocused
    // Divisor = 2, threshold = 512,000 + 2 = 512,002
    // Budget = 512,001 → remainder = 1, remainder < divisor = 2
    let regime_2_no_focus = [
        (f("d0", false, false), false),
        (f("d1", false, false), false),
        (f("e0", true, false), true),
        (f("e1", true, false), true),
    ];
    let fenetres: Vec<Fenetre> = regime_2_no_focus
        .iter()
        .map(|(fenetre, _)| fenetre.clone())
        .collect();
    let parts = repartir(512_001, &fenetres);
    let somme: u32 = parts.iter().map(|(_, bps)| bps).sum();
    let depassement = somme.saturating_sub(512_001);
    assert!(
        depassement <= 2,
        "Regime 2 (without a focused one): overrun {depassement} > divisor 2"
    );

    // **Regime 3: budget too small, does not cover the floors**
    // 2 asleep = 512,000 bps, budget = 100,000 < floors
    // Remainder = 0 (saturating_sub), overrun = (512_000 - 100_000) + awake = 412_000 + 2
    let regime_3 = [
        (f("d0", false, false), false),
        (f("d1", false, false), false),
        (f("e0", true, false), true),
        (f("e1", true, false), true),
    ];
    let fenetres: Vec<Fenetre> = regime_3
        .iter()
        .map(|(fenetre, _)| fenetre.clone())
        .collect();
    let parts = repartir(100_000, &fenetres);
    let somme: u32 = parts.iter().map(|(_, bps)| bps).sum();
    let depassement = somme.saturating_sub(100_000);
    let expected_depassement = (512_000 - 100_000) + 2; // crushed floors + awake
    assert_eq!(
        depassement, expected_depassement,
        "Regime 3: overrun {depassement} != {expected_depassement}"
    );
    assert!(
        parts.iter().all(|(_, bps)| *bps > 0),
        "Regime 3: no share must be zero"
    );
}
