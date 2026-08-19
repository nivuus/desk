use super::*;

/// Un résolveur qui rend une adresse non nulle pour tout nom connu de [`NOMS`],
/// et `None` pour les autres. L'adresse est `rang + 1` : non nulle, et
/// **différente pour chaque entrée**, ce qui est ce qui rend le test des rangs
/// capable d'échouer.
fn resolveur_complet(nom: &str) -> Option<usize> {
    NOMS.iter().position(|n| *n == nom).map(|rang| rang + 1)
}

#[test]
fn les_treize_noms_sont_distincts() {
    for (rang, nom) in NOMS.iter().enumerate() {
        assert!(
            !NOMS[..rang].contains(nom),
            "« {nom} » figure deux fois dans NOMS : le second rang écraserait le premier"
        );
    }
}

/// 🔴 **LE garde de l'appariement champ ↔ entrée.** Sans lui, deux champs
/// intervertis dans `resoudre` feraient `transmute` de deux adresses vers la
/// mauvaise signature — corruption de pile, aucun diagnostic. Une première
/// rédaction reposait sur des constantes de RANG, et une mutation jouée après
/// le vert a montré qu'échanger deux rangs au site d'appel **survivait** :
/// l'appariement était épinglé au mauvais endroit.
///
/// Le résolveur rend une adresse **différente pour chaque nom**, ce qui est ce
/// qui rend ce test capable d'échouer : à adresse commune, tout appariement
/// passerait.
#[test]
fn chaque_champ_recoit_l_adresse_de_son_entree() {
    let a = resoudre(resolveur_complet).expect("les treize sont là");
    let attendue = |nom: &str| resolveur_complet(nom).expect("nom connu");
    assert_eq!(a.allouer_tampon_aligne, attendue("PrjAllocateAlignedBuffer"));
    assert_eq!(a.vider_cache_negatif, attendue("PrjClearNegativePathCache"));
    assert_eq!(a.completer_commande, attendue("PrjCompleteCommand"));
    assert_eq!(a.supprimer_fichier, attendue("PrjDeleteFile"));
    assert_eq!(a.comparer_noms, attendue("PrjFileNameCompare"));
    assert_eq!(a.apparier_nom, attendue("PrjFileNameMatch"));
    assert_eq!(a.remplir_tampon_entrees, attendue("PrjFillDirEntryBuffer"));
    assert_eq!(a.rendre_tampon_aligne, attendue("PrjFreeAlignedBuffer"));
    assert_eq!(a.marquer_racine, attendue("PrjMarkDirectoryAsPlaceholder"));
    assert_eq!(a.demarrer_virtualisation, attendue("PrjStartVirtualizing"));
    assert_eq!(a.arreter_virtualisation, attendue("PrjStopVirtualizing"));
    assert_eq!(a.ecrire_donnees, attendue("PrjWriteFileData"));
    assert_eq!(a.ecrire_info_marqueur, attendue("PrjWritePlaceholderInfo"));
}

/// 🔴 **Le contrôle qui est la raison d'être de ce module.** Chacune des treize
/// entrées, retirée à son tour, doit produire une erreur qui **nomme
/// l'entrée** — pas un `Ok`, pas une erreur muette. Le balayage est exhaustif :
/// éprouver une seule entrée laisserait douze chemins non couverts.
#[test]
fn chaque_entree_absente_est_nommee_par_l_erreur() {
    for manquante in NOMS {
        let erreur = resoudre(|nom| if nom == manquante { None } else { resolveur_complet(nom) })
            .expect_err("une entrée manque : la résolution doit échouer");
        assert_eq!(erreur.nom, manquante);
        assert!(
            erreur.to_string().contains(manquante),
            "le libellé « {erreur} » ne nomme pas « {manquante} »"
        );
    }
}

/// Une adresse nulle n'est pas une adresse. `GetProcAddress` rend `NULL` sur
/// échec ; l'envelopper dans un `Some` sans le regarder ferait `transmute`
/// d'un pointeur nul en pointeur de fonction, et le premier appel sauterait à
/// l'adresse 0.
#[test]
fn une_adresse_nulle_vaut_une_entree_absente() {
    for manquante in NOMS {
        let erreur =
            resoudre(|nom| if nom == manquante { Some(0) } else { resolveur_complet(nom) })
                .expect_err("une adresse nulle doit être refusée");
        assert_eq!(erreur.nom, manquante);
    }
}

/// L'échec est **immédiat** : rien ne sert d'interroger les entrées suivantes,
/// et surtout la première manquante est celle que le journal doit nommer. Sans
/// cette propriété, une DLL d'une génération antérieure nommerait sa dernière
/// entrée absente plutôt que la première, et le diagnostic partirait du mauvais
/// bout.
#[test]
fn la_resolution_s_arrete_a_la_premiere_entree_manquante() {
    let mut interroges = Vec::new();
    let erreur = resoudre(|nom| {
        interroges.push(nom.to_string());
        if nom == "PrjCompleteCommand" { None } else { resolveur_complet(nom) }
    })
    .expect_err("PrjCompleteCommand manque");
    assert_eq!(erreur.nom, "PrjCompleteCommand");
    let rang = NOMS.iter().position(|n| *n == "PrjCompleteCommand").expect("nom connu");
    assert_eq!(
        interroges.len(),
        rang + 1,
        "la résolution a continué au-delà de l'entrée manquante : {interroges:?}"
    );
}
