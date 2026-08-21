//! La fenêtre de lecture. **Purs, exécutés sur l'hôte.**

use super::*;

fn morceaux(n: usize) -> VecDeque<Morceau> {
    (0..n)
        .map(|i| Morceau { position: (i as u64) * 4096, longueur: 4096 })
        .collect()
}

/// 🔴 **LA BORNE PORTE SUR LE TOTAL EN VOL, PAS SUR LE LOT.**
///
/// Rouge : retirer la borne. La file SCTP se remplirait sans terme, et le canal
/// deviendrait la source de latence de tout le reste — ce que ce module existe
/// précisément pour empêcher.
#[test]
fn la_fenetre_ne_demande_jamais_plus_de_MORCEAUX_EN_VOL() {
    let mut f = Fenetre::nouvelle(morceaux(50));
    let lot = f.a_demander();
    assert_eq!(lot.len(), MORCEAUX_EN_VOL);
    assert_eq!(f.en_vol(), MORCEAUX_EN_VOL);
    // Un second appel SANS réception ne doit RIEN ajouter : borner le lot au
    // lieu du total ferait ici seize en vol.
    assert!(f.a_demander().is_empty(), "aucun morceau de plus tant que rien n'est reçu");
    assert_eq!(f.en_vol(), MORCEAUX_EN_VOL);
}

/// 🔴 **LE ROUGE DU LIVRABLE LUI-MÊME.**
///
/// Avec `MORCEAUX_EN_VOL = 1`, le mécanisme est **INERTE** : le pont n'est
/// jamais en avance, et la contre-pression du navigateur n'a jamais rien à
/// retenir. F3 aurait alors livré un contrôle de flux incapable de mordre —
/// c'est-à-dire un contrôle qu'on ne verrait jamais rouge, appliqué à un
/// mécanisme de PRODUIT.
///
/// Ce test échoue si la constante retombe à 1, et il échoue aussi si la fenêtre
/// cesse de se remplir après une réception.
#[test]
#[allow(non_snake_case)]
fn la_fenetre_atteint_reellement_MORCEAUX_EN_VOL_sur_une_lecture_longue() {
    assert!(MORCEAUX_EN_VOL > 1, "une fenêtre de 1 est INERTE : voir la doc du module");
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

/// 🔴 **UNE RÉPONSE HORS D'ORDRE EST DÉNONCÉE, ET PAS APPLIQUÉE.**
///
/// Rouge : l'appliquer quand même. Le fichier aurait ses plages dans le
/// désordre, et **le condensat SHA-256 serait le seul critère qui l'attraperait**
/// — celui que F1 n'a JAMAIS établi.
#[test]
fn une_reponse_hors_ordre_est_denoncee_et_pas_appliquee() {
    let mut f = Fenetre::nouvelle(morceaux(10));
    f.a_demander();
    let erreur = f.recu(4096).expect_err("la position 4096 n'est pas la plus ancienne");
    assert_eq!(erreur, HorsOrdre { recue: 4096, attendue: Some(0) });
    // Rien n'a bougé : la réponse n'a pas été consommée.
    assert_eq!(f.en_vol(), MORCEAUX_EN_VOL);
    // Et la bonne réponse passe toujours.
    assert!(f.recu(0).is_ok());
    assert_eq!(f.en_vol(), MORCEAUX_EN_VOL - 1);
}

/// Une position **inconnue** est dénoncée elle aussi, et le message dit ce
/// qu'on attendait.
#[test]
fn une_position_inconnue_est_denoncee() {
    let mut f = Fenetre::nouvelle(morceaux(2));
    f.a_demander();
    assert_eq!(
        f.recu(999_999).expect_err("position jamais demandée"),
        HorsOrdre { recue: 999_999, attendue: Some(0) }
    );
}

/// Une réponse alors que **rien** n'est en vol est dénoncée, et `attendue` vaut
/// `None` — ce qui distingue « tu m'as répondu au mauvais moment » de « tu m'as
/// répondu la mauvaise plage ».
#[test]
fn une_reponse_sans_rien_en_vol_est_denoncee_avec_attendue_none() {
    let mut f = Fenetre::nouvelle(morceaux(1));
    f.a_demander();
    f.recu(0).expect("la seule");
    assert_eq!(f.recu(0).expect_err("plus rien en vol"), HorsOrdre { recue: 0, attendue: None });
}

/// La fenêtre se vide et se remplit : après une réception, un morceau de plus
/// est demandé, et pas deux.
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

/// Les morceaux sont demandés dans l'ordre **croissant** des positions : c'est
/// ce qui rend l'invariant d'ordre vrai, et il ne se suppose pas.
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

/// Une lecture vide est terminée d'emblée — et ne demande rien.
///
/// ⚠️ `decoupe::decouper` rend délibérément ZÉRO morceau pour une longueur
/// nulle. Sans ce cas, la fenêtre attendrait une réponse qui ne viendrait
/// jamais, et la commande ProjFS expirerait sur un fichier parfaitement lu.
#[test]
fn une_lecture_sans_morceau_est_terminee_d_emblee() {
    let mut f = Fenetre::nouvelle(VecDeque::new());
    assert!(f.terminee());
    assert!(f.a_demander().is_empty());
    assert_eq!(f.en_vol_max(), 0);
}

/// `terminee()` n'est vrai que lorsque **les deux** sont vides : ce qui reste à
/// demander ET ce qui est en vol.
///
/// Rouge : ne tester que `restants`. La lecture se déclarerait complète alors
/// que quatre morceaux sont encore en vol, et le fichier serait tronqué de
/// quatre trames — sans qu'aucune erreur ne soit rendue.
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
