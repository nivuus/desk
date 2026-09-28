use super::*;

/// A resolver returning a non-null address for any name known to [`NOMS`],
/// and `None` for the others. The address is `rang + 1`: non-null, and
/// **different for each entry point**, which is what makes the ranks test
/// able to fail.
fn resolveur_complet(nom: &str) -> Option<usize> {
    NOMS.iter().position(|n| *n == nom).map(|rang| rang + 1)
}

#[test]
fn the_thirteen_names_are_distinct() {
    for (rang, nom) in NOMS.iter().enumerate() {
        assert!(
            !NOMS[..rang].contains(nom),
            "« {nom} » appears twice in NOMS: the second rank would overwrite the first"
        );
    }
}

/// 🔴 **THE guard of the field ↔ entry point pairing.** Without it, two fields
/// swapped in `resoudre` would `transmute` two addresses to the
/// wrong signature — stack corruption, no diagnostic. A first
/// draft relied on RANK constants, and a mutation played after
/// green showed that swapping two ranks at the call site **survived**:
/// the pairing was pinned at the wrong place.
///
/// The resolver returns an address **different for each name**, which is what
/// makes this test able to fail: with a common address, any pairing
/// would pass.
#[test]
fn each_field_receives_the_address_of_its_entry() {
    let a = resoudre(resolveur_complet).expect("all thirteen are there");
    let attendue = |nom: &str| resolveur_complet(nom).expect("nom connu");
    assert_eq!(
        a.allouer_tampon_aligne,
        attendue("PrjAllocateAlignedBuffer")
    );
    assert_eq!(
        a.clear_negative_path_cache,
        attendue("PrjClearNegativePathCache")
    );
    assert_eq!(a.completer_commande, attendue("PrjCompleteCommand"));
    assert_eq!(a.delete_file, attendue("PrjDeleteFile"));
    assert_eq!(a.comparer_noms, attendue("PrjFileNameCompare"));
    assert_eq!(a.apparier_nom, attendue("PrjFileNameMatch"));
    assert_eq!(a.remplir_tampon_entrees, attendue("PrjFillDirEntryBuffer"));
    assert_eq!(a.rendre_tampon_aligne, attendue("PrjFreeAlignedBuffer"));
    assert_eq!(a.marquer_racine, attendue("PrjMarkDirectoryAsPlaceholder"));
    assert_eq!(a.start_virtualizing, attendue("PrjStartVirtualizing"));
    assert_eq!(a.arreter_virtualisation, attendue("PrjStopVirtualizing"));
    assert_eq!(a.write_file_data, attendue("PrjWriteFileData"));
    assert_eq!(
        a.write_placeholder_info,
        attendue("PrjWritePlaceholderInfo")
    );
}

/// 🔴 **The check that is this module's reason to exist.** Each of the thirteen
/// entry points, removed in turn, must produce an error that **names
/// the entry point** — not an `Ok`, not a mute error. The sweep is exhaustive:
/// exercising a single entry point would leave twelve paths uncovered.
#[test]
fn each_missing_entry_is_named_by_the_error() {
    for manquante in NOMS {
        let error = resoudre(|nom| {
            if nom == manquante {
                None
            } else {
                resolveur_complet(nom)
            }
        })
        .expect_err("an entry is missing: resolution must fail");
        assert_eq!(error.nom, manquante);
        assert!(
            error.to_string().contains(manquante),
            "the label « {error} » does not name « {manquante} »"
        );
    }
}

/// A null address is not an address. `GetProcAddress` returns `NULL` on
/// failure; wrapping it in a `Some` without looking at it would `transmute`
/// a null pointer into a function pointer, and the first call would jump to
/// address 0.
#[test]
fn a_null_address_counts_as_a_missing_entry() {
    for manquante in NOMS {
        let error = resoudre(|nom| {
            if nom == manquante {
                Some(0)
            } else {
                resolveur_complet(nom)
            }
        })
        .expect_err("a null address must be refused");
        assert_eq!(error.nom, manquante);
    }
}

/// The failure is **immediate**: there is no point querying the next entry points,
/// and above all the first missing one is the one the log must name. Without
/// this property, a DLL of an earlier generation would name its last
/// absent entry point rather than the first, and the diagnosis would start from the wrong
/// end.
#[test]
fn resolution_stops_at_the_first_missing_entry() {
    let mut interroges = Vec::new();
    let error = resoudre(|nom| {
        interroges.push(nom.to_string());
        if nom == "PrjCompleteCommand" {
            None
        } else {
            resolveur_complet(nom)
        }
    })
    .expect_err("PrjCompleteCommand manque");
    assert_eq!(error.nom, "PrjCompleteCommand");
    let rang = NOMS
        .iter()
        .position(|n| *n == "PrjCompleteCommand")
        .expect("nom connu");
    assert_eq!(
        interroges.len(),
        rang + 1,
        "resolution went on past the missing entry: {interroges:?}"
    );
}
