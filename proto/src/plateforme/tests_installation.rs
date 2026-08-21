//! Les messages de l'INSTALLATION, côté Rust.
//!
//! 🔴 CE FICHIER EXISTE POUR CE QUE LES VECTEURS NE PEUVENT PAS ÉPROUVER.
//! `plateforme-vectors.json` est un jeu de ROUND-TRIPS : il fige les chaînes
//! que les deux langages doivent produire et relire, et ne dit rien de ce qui
//! doit être REFUSÉ. Or la garde la plus fragile de v4 est un refus — un
//! `termine` dont la clé `motif` MANQUE.

use super::*;

fn termine_complet() -> String {
    format!(
        r#"{{"type":"termine","v":{PLATEFORME_VERSION},"installation":"i-1","issue":"reussie","motif":null,"code_sortie":0,"journal":"","journal_tronque":false}}"#
    )
}

#[test]
fn lit_un_termine_complet() {
    let lu: VersLaPlateforme = serde_json::from_str(&termine_complet()).expect("lisible");
    assert_eq!(
        lu,
        VersLaPlateforme::termine("i-1", Issue::Reussie, None, Some(0), "", false)
    );
}

/// 🔴 LA GARDE QUI COMPTE, ET SANS ELLE LE CHAMP SERAIT SILENCIEUSEMENT
/// FACULTATIF.
///
/// `serde_derive` traite tout champ de type `Option<T>` comme portant un
/// `#[serde(default)]` IMPLICITE : un champ absent devient `None` sans qu'aucun
/// `default` n'ait été écrit, et `deny_unknown_fields` n'y change rien — il
/// regarde les champs EN TROP, jamais ceux qui manquent. C'est exactement ce
/// que le sous-bloc G2 a mesuré pour son champ `icone`, et `champs::
/// option_obligatoire` est la forme générique de son remède.
///
/// **Ce qui serait perdu sans elle** : le `termine` d'un agent d'une version
/// antérieure — qui n'a ni `motif` ni `code_sortie` — serait accepté par une
/// plateforme v4, avec un motif et un code silencieusement absents. C'est le
/// déguisement précis que le bump de version existe pour empêcher.
#[test]
fn refuse_un_termine_dont_une_cle_facultative_manque() {
    for cle in ["motif", "code_sortie"] {
        let ampute: String = {
            let mut doc: serde_json::Value =
                serde_json::from_str(&termine_complet()).expect("valide");
            doc.as_object_mut().expect("objet").remove(cle);
            doc.to_string()
        };
        assert!(
            serde_json::from_str::<VersLaPlateforme>(&ampute).is_err(),
            "un `termine` sans « {cle} » doit être refusé, il ne doit pas être complété"
        );
    }
}

#[test]
fn accepte_null_sur_les_deux_cles_et_le_distingue_de_l_absence() {
    let avec_null = termine_complet().replace(r#""code_sortie":0"#, r#""code_sortie":null"#);
    let lu: VersLaPlateforme = serde_json::from_str(&avec_null).expect("lisible");
    let VersLaPlateforme::Termine {
        motif, code_sortie, ..
    } = lu
    else {
        panic!("pas un termine");
    };
    assert_eq!(motif, None);
    assert_eq!(code_sortie, None);
}

/// 🔴 UN CODE DE SORTIE NÉGATIF EST LÉGITIME sous Windows : les `HRESULT`
/// d'échec ont le bit de poids fort à 1 et se lisent en `i32` signé. Le refuser
/// confondrait « code hors norme » avec « échec ordinaire ».
#[test]
fn accepte_un_code_de_sortie_negatif() {
    let negatif = termine_complet().replace(r#""code_sortie":0"#, r#""code_sortie":-1073741510"#);
    let lu: VersLaPlateforme = serde_json::from_str(&negatif).expect("lisible");
    let VersLaPlateforme::Termine { code_sortie, .. } = lu else {
        panic!("pas un termine");
    };
    assert_eq!(code_sortie, Some(-1_073_741_510));
}

/// 🔴 C'EST LA ROUGE DU LEG N°9 DE G1, ET ELLE EST ENFIN JOUABLE.
///
/// G1 a MESURÉ qu'un `rename_all` est **inobservable** sur un enum dont toutes
/// les variantes tiennent en un mot : passer `kebab-case` à `snake_case` sur
/// `IssueLancement` — `raccourci`, `cible`, `inconnue`, `echec` — laisse
/// `cargo test -p proto` entièrement vert. [`Issue`] a **deux** variantes de
/// deux mots, et c'est délibéré : `SansEffet` et `IssueInconnue` rendent la
/// mutation visible.
///
/// ⚠️ **LE PLAN DE G3 PRESCRIVAIT CETTE ROUGE SUR `Phase`, EN CITANT
/// `sans-effet`** — une contradiction de son propre texte : `sans-effet`
/// appartient à `Issue`, et `Phase` n'a aucune variante de deux mots. Elle est
/// jouée ici, sur l'enum qui peut la porter, et **on n'a PAS inventé une
/// quatrième phase** pour rendre une mutation observable.
#[test]
fn les_deux_variantes_de_deux_mots_d_issue_voyagent_en_kebab_case() {
    let paires = [
        (Issue::Reussie, "reussie"),
        (Issue::SansEffet, "sans-effet"),
        (Issue::IssueInconnue, "issue-inconnue"),
        (Issue::Refusee, "refusee"),
    ];
    for (variante, mot) in paires {
        assert_eq!(
            serde_json::to_string(&variante).expect("sér."),
            format!("\"{mot}\""),
            "la variante {variante:?} doit voyager en kebab-case"
        );
        let relu: Issue = serde_json::from_str(&format!("\"{mot}\"")).expect("désér.");
        assert_eq!(relu, variante);
    }
    // ⚠️ ET LA FORME `snake_case` EST REFUSÉE, ce qui est la moitié qui fait
    // rougir la mutation : sans cette ligne, un `rename_all` changé rendrait
    // le test faux dans un seul sens.
    assert!(serde_json::from_str::<Issue>("\"sans_effet\"").is_err());
    assert!(serde_json::from_str::<Issue>("\"issue_inconnue\"").is_err());
}

#[test]
fn les_trois_phases_voyagent_par_leur_mot() {
    for (variante, mot) in [
        (Phase::Transfert, "transfert"),
        (Phase::Execution, "execution"),
        (Phase::Reconciliation, "reconciliation"),
    ] {
        assert_eq!(
            serde_json::to_string(&variante).expect("sér."),
            format!("\"{mot}\"")
        );
    }
    // ⚠️ `empreinte` N'EST PAS UNE PHASE DE CE CANAL : elle se déroule dans le
    // NAVIGATEUR, avant que la plateforme n'ait la moindre ligne à écrire.
    assert!(serde_json::from_str::<Phase>("\"empreinte\"").is_err());
}

/// La forme du `Installer` descendant, et le fait que l'URL y voyage — jamais
/// les octets.
#[test]
fn l_ordre_d_installation_porte_une_url_et_pas_des_octets() {
    let ordre = DepuisLaPlateforme::installer("i-1", "http://h:8080/t/c", "setup.exe", 42, "ab");
    let chaine = serde_json::to_string(&ordre).expect("sér.");
    assert!(chaine.contains(r#""url":"http://h:8080/t/c""#));
    assert!(!chaine.contains("base64"));
    let relu: DepuisLaPlateforme = serde_json::from_str(&chaine).expect("désér.");
    assert_eq!(relu, ordre);
}
