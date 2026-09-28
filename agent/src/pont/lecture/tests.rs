//! The read window. **Pure, run on the host.**

use super::*;

fn morceaux(n: usize) -> VecDeque<Morceau> {
    (0..n)
        .map(|i| Morceau {
            position: (i as u64) * 4096,
            longueur: 4096,
        })
        .collect()
}

/// 🔴 **THE BOUND APPLIES TO THE TOTAL IN FLIGHT, NOT TO THE BATCH.**
///
/// Red: remove the bound. The SCTP queue would fill without end, and the channel
/// would become the latency source of everything else — which this module exists
/// precisely to prevent.
#[test]
fn la_fenetre_ne_demande_jamais_plus_de_morceaux_en_vol() {
    let mut f = Fenetre::nouvelle(morceaux(50));
    let lot = f.a_demander();
    assert_eq!(lot.len(), MORCEAUX_EN_VOL);
    assert_eq!(f.en_vol(), MORCEAUX_EN_VOL);
    // A second call WITHOUT receipt must add NOTHING: bounding the batch instead
    // of the total would make sixteen in flight here.
    assert!(
        f.a_demander().is_empty(),
        "aucun morceau de plus tant que rien n'est reçu"
    );
    assert_eq!(f.en_vol(), MORCEAUX_EN_VOL);
}

/// 🔴 **THE RED OF THE DELIVERABLE ITSELF.**
///
/// With `MORCEAUX_EN_VOL = 1`, the mechanism is **INERT**: the bridge is
/// never ahead, and the browser's back-pressure never has anything to
/// hold back. F3 would then have delivered a flow control unable to bite —
/// that is, a check we would never see red, applied to a
/// PRODUCT mechanism.
///
/// This test fails if the constant falls back to 1, and it also fails if the window
/// stops filling after a receipt.
#[test]
#[allow(non_snake_case)]
fn la_fenetre_atteint_reellement_MORCEAUX_EN_VOL_sur_une_lecture_longue() {
    const {
        assert!(
            MORCEAUX_EN_VOL > 1,
            "une fenêtre de 1 est INERTE : voir la doc du module"
        )
    };
    let mut f = Fenetre::nouvelle(morceaux(50));
    for _ in 0..20 {
        for m in f.a_demander() {
            let _ = m;
        }
        let position = *f.en_vol.front().expect("il en reste");
        f.recu(position).expect("dans l'ordre");
    }
    assert_eq!(
        f.en_vol_max(),
        MORCEAUX_EN_VOL,
        "la fenêtre n'a jamais été pleine : le mécanisme est inerte"
    );
}

/// 🔴 **AN OUT-OF-ORDER RESPONSE IS DENOUNCED, AND NOT APPLIED.**
///
/// Red: apply it anyway. The file would have its ranges out of
/// order, and **the SHA-256 digest would be the only criterion to catch it**
/// — the one F1 NEVER established.
#[test]
fn une_reponse_hors_ordre_est_denoncee_et_pas_appliquee() {
    let mut f = Fenetre::nouvelle(morceaux(10));
    f.a_demander();
    let erreur = f
        .recu(4096)
        .expect_err("la position 4096 n'est pas la plus ancienne");
    assert_eq!(
        erreur,
        HorsOrdre {
            recue: 4096,
            attendue: Some(0)
        }
    );
    // Nothing moved: the response was not consumed.
    assert_eq!(f.en_vol(), MORCEAUX_EN_VOL);
    // And the right response still goes through.
    assert!(f.recu(0).is_ok());
    assert_eq!(f.en_vol(), MORCEAUX_EN_VOL - 1);
}

/// An **unknown** position is denounced too, and the message says what
/// we expected.
#[test]
fn une_position_inconnue_est_denoncee() {
    let mut f = Fenetre::nouvelle(morceaux(2));
    f.a_demander();
    assert_eq!(
        f.recu(999_999).expect_err("position jamais demandée"),
        HorsOrdre {
            recue: 999_999,
            attendue: Some(0)
        }
    );
}

/// A response while **nothing** is in flight is denounced, and `attendue` is
/// `None` — which distinguishes "you answered me at the wrong moment" from "you
/// answered me the wrong range".
#[test]
fn une_reponse_sans_rien_en_vol_est_denoncee_avec_attendue_none() {
    let mut f = Fenetre::nouvelle(morceaux(1));
    f.a_demander();
    f.recu(0).expect("la seule");
    assert_eq!(
        f.recu(0).expect_err("plus rien en vol"),
        HorsOrdre {
            recue: 0,
            attendue: None
        }
    );
}

/// The window empties and fills: after a receipt, one more chunk
/// is requested, and not two.
#[test]
fn une_reception_libere_exactement_une_place() {
    let mut f = Fenetre::nouvelle(morceaux(10));
    f.a_demander();
    f.recu(0).unwrap();
    let lot = f.a_demander();
    assert_eq!(lot.len(), 1);
    assert_eq!(lot[0].position, (MORCEAUX_EN_VOL as u64) * 4096);
    assert_eq!(f.en_vol(), MORCEAUX_EN_VOL);
}

/// Chunks are requested in **increasing** position order: it is
/// what makes the ordering invariant true, and it is not assumed.
#[test]
fn les_morceaux_sont_demandes_dans_l_ordre_croissant() {
    let mut f = Fenetre::nouvelle(morceaux(12));
    let mut vues = Vec::new();
    while !f.terminee() {
        for m in f.a_demander() {
            vues.push(m.position);
        }
        let position = *f.en_vol.front().unwrap();
        f.recu(position).unwrap();
    }
    let mut triees = vues.clone();
    triees.sort_unstable();
    assert_eq!(vues, triees, "les positions doivent être croissantes");
    assert_eq!(vues.len(), 12);
}

/// An empty read is finished straight away — and requests nothing.
///
/// ⚠️ `decoupe::decouper` deliberately returns ZERO chunks for a zero
/// length. Without this case, the window would wait for a response that would never
/// come, and the ProjFS command would expire on a perfectly read file.
#[test]
fn une_lecture_sans_morceau_est_terminee_d_emblee() {
    let mut f = Fenetre::nouvelle(VecDeque::new());
    assert!(f.terminee());
    assert!(f.a_demander().is_empty());
    assert_eq!(f.en_vol_max(), 0);
}

/// `terminee()` is only true when **both** are empty: what remains to
/// request AND what is in flight.
///
/// Red: only test `restants`. The read would declare itself complete while
/// four chunks are still in flight, and the file would be truncated by
/// four frames — without any error being returned.
#[test]
fn terminee_exige_que_le_vol_soit_vide_aussi() {
    let mut f = Fenetre::nouvelle(morceaux(2));
    f.a_demander();
    assert!(!f.terminee(), "deux morceaux sont en vol");
    f.recu(0).unwrap();
    assert!(!f.terminee(), "un morceau est encore en vol");
    f.recu(4096).unwrap();
    assert!(f.terminee());
}
