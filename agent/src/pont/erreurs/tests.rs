use super::*;

#[test]
fn deux_causes_distinctes_ne_partagent_jamais_un_code() {
    // ⚠️ The test spec §4.4 names, and the counter-example is the old
    // bridge: `cb(-1)` (EPERM) at NINE distinct sites of `src/file.js`.
    //
    // It is written by SWEEPING `TOUTES`, never by listing by hand:
    // otherwise a variant added tomorrow would escape the check, and the table
    // would become decorative again with nothing saying so.
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
    // The guard's guard: without it, `TOUTES` could forget a variant and
    // the sweep above would pass while exercising nothing — the exact pattern
    // this repository caught four times in D10.
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
    // The severity bit (0x8000_0000) must be set on all twelve: a
    // success `HRESULT` returned to ProjFS would make it believe the operation
    // succeeded, and the Windows application would read an empty file instead of an
    // error — the worst possible failure mode for this module.
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
    // "The standard I/O error" of framing §7: it is what Explorer
    // displays as "the device is not accessible", and not as
    // "file not found", when the browser tab closes.
    assert_eq!(hresult(Erreur::CanalFerme), 0x8007_045Du32 as i32);
}

#[test]
fn la_protection_en_ecriture_rend_0x80070013() {
    // ❌ *This comment said "every write, every CREATION, every
    // deletion ends up there". F1's acceptance run refuted creation: a
    // file created from scratch in the root SUCCEEDS (2 runs
    // recorded out of 2 that reach this phase). `NEW_FILE_CREATED` is a
    // POST notification, hence unrefusable — see `pont::notifications`.*
    assert_eq!(hresult(Erreur::ProtegeEnEcriture), 0x8007_0013u32 as i32);
}

#[test]
fn les_douze_codes_sont_epingles_un_a_un() {
    // Pins the whole table: the distinction test above would stay
    // green if two variants SWAPPED their codes, which would return
    // "disk full" for an absent file.
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

/// `HRESULT_FROM_WIN32(ERROR_IO_PENDING)` is `0x800703E5`, and it is the value
/// EVERY asynchronous callback returns. A transcription error would return
/// a failure code where ProjFS expects "in progress": the application would receive
/// a failed I/O on every read, and the bridge would wait indefinitely for a
/// completion ProjFS would no longer accept.
#[test]
fn en_cours_vaut_le_hresult_de_error_io_pending() {
    assert_eq!(EN_COURS, 0x8007_03E5u32 as i32);
}

/// `EN_COURS` is the `hresult` of NO `Erreur` variant. If it were,
/// a real failure would be indistinguishable from an operation in progress, and ProjFS
/// would wait for a completion that would never come.
#[test]
fn en_cours_ne_collide_avec_aucune_cause_d_echec() {
    for cause in Erreur::TOUTES {
        assert_ne!(hresult(cause), EN_COURS, "{cause:?}");
    }
}
