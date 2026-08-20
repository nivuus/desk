use super::*;

#[test]
fn deux_causes_distinctes_ne_partagent_jamais_un_code() {
    // ⚠️ Le test que la spec §4.4 nomme, et le contre-exemple est l'ancien
    // pont : `cb(-1)` (EPERM) à NEUF sites distincts de `src/file.js`.
    //
    // Il s'écrit par BALAYAGE de `TOUTES`, jamais par énumération à la main :
    // sinon une variante ajoutée demain échapperait au contrôle, et la table
    // redeviendrait décorative sans que rien ne le dise.
    for (i, a) in Erreur::TOUTES.iter().enumerate() {
        for b in &Erreur::TOUTES[i + 1..] {
            assert_ne!(
                hresult(*a),
                hresult(*b),
                "{a:?} et {b:?} partagent le code {:#010x}",
                hresult(*a)
            );
        }
    }
}

#[test]
fn le_balayage_couvre_reellement_chaque_variante() {
    // Le garde du garde : sans lui, `TOUTES` pourrait oublier une variante et
    // le balayage ci-dessus passerait en n'exerçant rien — le patron exact que
    // ce dépôt a attrapé quatre fois en D10.
    assert_eq!(Erreur::TOUTES.len(), NOMBRE);
    let mut vus = [false; NOMBRE];
    for e in Erreur::TOUTES {
        assert!(!vus[index(e)], "{e:?} apparaît deux fois dans TOUTES");
        vus[index(e)] = true;
    }
    assert!(vus.iter().all(|v| *v), "une variante manque à TOUTES");
}

#[test]
fn aucun_code_rendu_n_est_un_succes() {
    // Le bit de sévérité (0x8000_0000) doit être posé sur les douze : un
    // `HRESULT` de succès rendu à ProjFS lui ferait croire que l'opération a
    // abouti, et l'application Windows lirait un fichier vide au lieu d'une
    // erreur — le pire mode de défaillance possible pour ce module.
    for e in Erreur::TOUTES {
        let h = hresult(e);
        assert!(h < 0, "{e:?} rend {h:#010x}, qui est un succès");
        assert_eq!(
            (h as u32) & 0xFFFF_0000,
            FACILITE_WIN32,
            "{e:?} n'est pas dans FACILITY_WIN32"
        );
    }
}

#[test]
fn le_canal_ferme_rend_bien_error_io_device() {
    // « L'erreur I/O standard » du cadrage §7 : c'est ce que l'Explorateur
    // affiche comme « Le périphérique n'est pas accessible », et non comme
    // « fichier introuvable », quand l'onglet du navigateur se ferme.
    assert_eq!(hresult(Erreur::CanalFerme), 0x8007_045Du32 as i32);
}

#[test]
fn la_protection_en_ecriture_rend_0x80070013() {
    // ❌ *Ce commentaire disait « toute écriture, toute CRÉATION, toute
    // suppression y aboutit ». La recette de F1 a réfuté la création : un
    // fichier créé de toutes pièces dans la racine RÉUSSIT (2 exécutions
    // versées sur 2 qui atteignent cette phase). `NEW_FILE_CREATED` est une
    // notification POST, donc irrefusable — voir `pont::notifications`.*
    assert_eq!(hresult(Erreur::ProtegeEnEcriture), 0x8007_0013u32 as i32);
}

#[test]
fn les_douze_codes_sont_epingles_un_a_un() {
    // Épingle la table entière : le test de distinction ci-dessus resterait
    // vert si deux variantes ÉCHANGEAIENT leurs codes, ce qui rendrait
    // « disque plein » pour un fichier absent.
    let attendu: [(Erreur, u32); NOMBRE] = [
        (Erreur::Introuvable, 0x8007_0002),
        (Erreur::CheminIntrouvable, 0x8007_0003),
        (Erreur::AccesRefuse, 0x8007_0005),
        (Erreur::CanalFerme, 0x8007_045D),
        (Erreur::DelaiDepasse, 0x8007_0079),
        (Erreur::Abandonnee, 0x8007_03E3),
        (Erreur::DisquePlein, 0x8007_0070),
        (Erreur::NonSupporte, 0x8007_0032),
        (Erreur::RepertoireNonVide, 0x8007_0091),
        (Erreur::DejaPresent, 0x8007_0050),
        (Erreur::ProtegeEnEcriture, 0x8007_0013),
        (Erreur::Inattendue, 0x8007_001F),
    ];
    for (e, code) in attendu {
        assert_eq!(hresult(e), code as i32, "{e:?}");
    }
}

/// `HRESULT_FROM_WIN32(ERROR_IO_PENDING)` vaut `0x800703E5`, et c'est la valeur
/// que TOUT rappel asynchrone rend. Une erreur de transcription y ferait rendre
/// un code d'échec là où ProjFS attend « en cours » : l'application recevrait
/// une E/S en échec sur chaque lecture, et le pont attendrait indéfiniment une
/// complétion que ProjFS n'accepterait plus.
#[test]
fn en_cours_vaut_le_hresult_de_error_io_pending() {
    assert_eq!(EN_COURS, 0x8007_03E5u32 as i32);
}

/// `EN_COURS` n'est le `hresult` d'AUCUNE variante d'`Erreur`. S'il l'était,
/// un échec réel serait indistinguable d'une opération en cours, et ProjFS
/// attendrait une complétion qui ne viendrait jamais.
#[test]
fn en_cours_ne_collide_avec_aucune_cause_d_echec() {
    for cause in Erreur::TOUTES {
        assert_ne!(hresult(cause), EN_COURS, "{cause:?}");
    }
}
