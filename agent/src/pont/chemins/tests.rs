use super::*;

#[test]
fn une_remontee_sort_de_la_racine_et_est_refusee() {
    // Le test que la spec §4.4 nomme. `a\..\..\secret` remonte DEUX crans au
    // dessus de la racine : s'il était résolu, il désignerait un fichier hors
    // du répertoire que l'utilisateur a partagé.
    assert_eq!(normaliser(r"a\..\..\secret"), Err(CheminRefuse::Remontee));
    assert_eq!(normaliser(r"..\secret"), Err(CheminRefuse::Remontee));
    assert_eq!(normaliser(r"a\b\.."), Err(CheminRefuse::Remontee));
}

#[test]
fn une_remontee_deguisee_est_refusee() {
    // Le `.` intercalé est ce qui casse une détection naïve par sous-chaîne
    // `"\.."` ou par préfixe.
    assert_eq!(normaliser(r"a\.\..\..\x"), Err(CheminRefuse::Remontee));
    // …et la barre oblique ordinaire aussi : ProjFS livre des contre-obliques,
    // mais une application peut fabriquer autre chose.
    assert_eq!(normaliser("a/../x"), Err(CheminRefuse::Remontee));
}

#[test]
fn un_flux_alternatif_est_refuse() {
    // Un flux de données alternatif NTFS n'a pas d'équivalent dans la File
    // System Access API : le servir n'aurait aucun sens, et l'ignorer
    // silencieusement rendrait le CONTENU du fichier pour une demande de
    // métadonnées de zone.
    assert_eq!(
        normaliser("fichier.txt:Zone.Identifier"),
        Err(CheminRefuse::FluxAlternatif)
    );
    assert_eq!(
        normaliser(r"dossier\fichier.txt:$DATA"),
        Err(CheminRefuse::FluxAlternatif)
    );
}

#[test]
fn un_nom_reserve_est_refuse() {
    for nom in ["CON", "PRN", "NUL", "AUX", "COM1", "LPT1"] {
        assert_eq!(normaliser(nom), Err(CheminRefuse::NomReserve), "{nom}");
    }
    // Avec extension, et à n'importe quelle profondeur : Windows résout ces
    // noms AVANT de regarder le système de fichiers.
    assert_eq!(normaliser("CON.txt"), Err(CheminRefuse::NomReserve));
    assert_eq!(normaliser(r"dossier\NUL.log"), Err(CheminRefuse::NomReserve));
    // Points et espaces de fin : Win32 les retire avant de résoudre.
    assert_eq!(normaliser("con. "), Err(CheminRefuse::NomReserve));

    // …et ce qui n'est PAS réservé passe. Sans cette moitié, le test serait
    // satisfait par un module qui refuse tout.
    assert_eq!(normaliser("CONTRAT.txt").unwrap(), "CONTRAT.txt");
    assert_eq!(normaliser("COM10").unwrap(), "COM10");
    assert_eq!(normaliser("console").unwrap(), "console");
}

#[test]
fn un_chemin_absolu_est_refuse() {
    assert_eq!(normaliser(r"C:\x"), Err(CheminRefuse::Absolu));
    assert_eq!(normaliser(r"\\serveur\part"), Err(CheminRefuse::Absolu));
    assert_eq!(normaliser(r"\depuis-la-racine"), Err(CheminRefuse::Absolu));
    assert_eq!(normaliser("/depuis-la-racine"), Err(CheminRefuse::Absolu));
}

#[test]
fn la_racine_elle_meme_est_la_chaine_vide_et_est_licite() {
    // C'est le chemin de l'énumération de la racine : le refuser rendrait le
    // lecteur vide, et rien ne le dirait.
    assert_eq!(normaliser("").unwrap(), "");
}

#[test]
fn les_contre_obliques_deviennent_des_barres() {
    assert_eq!(normaliser(r"a\b\c").unwrap(), "a/b/c");
    assert_eq!(normaliser("a").unwrap(), "a");
    // Un `.` intercalé se laisse tomber, il ne devient pas un composant.
    assert_eq!(normaliser(r"a\.\b").unwrap(), "a/b");
    // Un séparateur doublé produit un composant vide : refus, pas
    // écrasement silencieux.
    assert_eq!(normaliser(r"a\\b"), Err(CheminRefuse::Vide));
}

#[test]
fn la_casse_est_conservee_mais_la_comparaison_ne_l_est_pas() {
    // ⚠️ La casse est CONSERVÉE : la File System Access API est sensible à la
    // casse, et replier le chemin ferait échouer toutes les ouvertures. Deux
    // chemins qui ne diffèrent que par la casse restent donc distincts en
    // sortie — c'est la limite connue de F1, documentée en tête de module.
    assert_eq!(normaliser("Rapport.TXT").unwrap(), "Rapport.TXT");
    assert_eq!(normaliser("rapport.txt").unwrap(), "rapport.txt");
    assert_ne!(
        normaliser("Rapport.TXT").unwrap(),
        normaliser("rapport.txt").unwrap()
    );

    // …alors que la COMPARAISON des noms réservés, elle, replie bien la casse,
    // parce que Windows la replie. Les trois désignent la console.
    for nom in ["CON", "con", "CoN"] {
        assert_eq!(normaliser(nom), Err(CheminRefuse::NomReserve), "{nom}");
    }
}

#[test]
fn des_unites_utf16_invalides_sont_refusees_avant_toute_normalisation() {
    // 0xD800 est une demi-paire de substitution isolée : ProjFS livre des
    // `PCWSTR`, et rien ne garantit qu'ils forment du texte valide.
    assert_eq!(normaliser_utf16(&[0xD800]), Err(CheminRefuse::NonUtf16Valide));
    // …et une chaîne UTF-16 valide traverse bien jusqu'à la normalisation,
    // refus compris. Sans cette moitié, le test passerait sur une fonction qui
    // refuse tout.
    let valide: Vec<u16> = "a\\..\\b".encode_utf16().collect();
    assert_eq!(normaliser_utf16(&valide), Err(CheminRefuse::Remontee));
    let simple: Vec<u16> = "dossier\\éléphant.txt".encode_utf16().collect();
    assert_eq!(normaliser_utf16(&simple).unwrap(), "dossier/éléphant.txt");
}
