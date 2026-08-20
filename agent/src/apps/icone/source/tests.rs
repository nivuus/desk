//! Tests d'hôte de la provenance. Purs : aucun disque, aucun COM, aucune VM.
//!
//! ⚠️ **DIVERGENCE AVEC LE PLAN, RELEVÉE PLUTÔT QUE DISSIMULÉE.** Il prescrit
//! des cas « tirés du corpus versé (`agent/testdata/gapps-corpus-vm.json`) » —
//! or **ce corpus ne porte AUCUN champ d'`IconLocation`** : ses six champs sont
//! `nom`, `chemin`, `cible`, `arguments`, `repertoire`, `existe`, et sa propre
//! notice de provenance le dit (« les CINQ premiers champs sont extraits
//! verbatim de la sonde `WScript.Shell` »). Les CIBLES ci-dessous en viennent
//! donc bien ; les `IconLocation`, eux, sont les lignes **verbatim de la mesure
//! M1** transcrites dans le plan de G2. Aucun n'est inventé, et la source de
//! chacun est nommée.

use super::*;

/// 🔴 LE CAS DES 92 SUR 153 : chemin vide, l'icône est celle de la CIBLE.
#[test]
fn un_chemin_vide_renvoie_a_la_cible() {
    // La forme exacte relevée par M1 sur cette VM.
    assert_eq!(
        provenance(",0", r"c:\windows\system32\notepad.exe"),
        Provenance::Module(r"c:\windows\system32\notepad.exe".into())
    );
    // La chaîne entièrement vide, qui est l'autre forme du même état.
    assert_eq!(
        provenance("", r"c:\windows\system32\notepad.exe"),
        Provenance::Module(r"c:\windows\system32\notepad.exe".into())
    );
    // 🔴 LA ROUGE : traiter un chemin vide comme `Aucune`. Cet état ferait
    // perdre son icône à plus d'une application sur deux, EN SILENCE.
    assert_ne!(provenance(",0", r"c:\windows\system32\notepad.exe"), Provenance::Aucune);
}

/// Un `.ico` autonome — la ligne verbatim de M1.
#[test]
fn un_ico_autonome_est_lu_comme_un_ico() {
    assert_eq!(
        provenance(r"C:\Program Files\GSmartControl\gsmartcontrol.ico,0", r"c:\x\y.exe"),
        Provenance::Ico(r"C:\Program Files\GSmartControl\gsmartcontrol.ico".into())
    );
    // Sans index : le format l'autorise.
    assert_eq!(
        provenance(r"C:\Program Files\GSmartControl\gsmartcontrol.ico", r"c:\x\y.exe"),
        Provenance::Ico(r"C:\Program Files\GSmartControl\gsmartcontrol.ico".into())
    );
}

/// Un module PE nommé explicitement — la ligne verbatim de M1.
#[test]
fn un_module_pe_est_lu_comme_un_module() {
    assert_eq!(
        provenance(r"%windir%\system32\notepad.exe,0", r"c:\autre\chose.exe"),
        Provenance::Module(r"%windir%\system32\notepad.exe".into())
    );
    for ext in ["dll", "mun", "cpl", "scr", "ocx"] {
        let l = format!(r"C:\Windows\System32\imageres.{ext},-5301");
        assert!(matches!(provenance(&l, ""), Provenance::Module(_)), "{ext}");
    }
}

/// 🔴 SANS EXTENSION, ON NE SAIT PAS LIRE — et `Aucune` le DIT, là où deviner
/// produirait une mesure fausse. La ligne verbatim de M1.
#[test]
fn un_chemin_sans_extension_ne_se_lit_pas() {
    assert_eq!(
        provenance(r"C:\Windows\Installer\{1BEA6F9E-0000-0000-0000-000000000000}\ProductIcon,0", ""),
        Provenance::Aucune
    );
    // Une extension inconnue non plus.
    assert_eq!(provenance(r"C:\x\y.png,0", ""), Provenance::Aucune);
    // Et une cible vide avec un IconLocation vide : rien du tout.
    assert_eq!(provenance("", ""), Provenance::Aucune);
}

/// 🔴 LA DERNIÈRE VIRGULE, JAMAIS LA PREMIÈRE — un chemin Windows peut en
/// porter une, et découper sur la première couperait le chemin en deux.
#[test]
fn le_decoupage_se_fait_sur_la_derniere_virgule() {
    let l = r"C:\Program Files\Machin, Inc\outil.exe,3";
    assert_eq!(
        provenance(l, ""),
        Provenance::Module(r"C:\Program Files\Machin, Inc\outil.exe".into())
    );
    assert_eq!(index(l), 3);
    // Sur la PREMIÈRE virgule, le chemin serait `C:\Program Files\Machin`,
    // dont l'extension n'existe pas — donc `Aucune`. La rouge est visible.
    assert_ne!(provenance(l, ""), Provenance::Aucune);
}

#[test]
fn l_index_vaut_zero_a_defaut_et_accepte_le_negatif() {
    assert_eq!(index(""), 0);
    assert_eq!(index(r"c:\x\y.exe"), 0);
    assert_eq!(index(r"c:\x\y.exe,0"), 0);
    assert_eq!(index(r"c:\x\y.exe,7"), 7);
    // 🔴 UN INDEX NÉGATIF DÉSIGNE UNE RESSOURCE PAR SON IDENTIFIANT, et c'est
    // un usage courant d'`imageres.dll`. Un `u32` le perdrait.
    assert_eq!(index(r"C:\Windows\System32\imageres.dll,-5301"), -5301);
    // Un index illisible vaut 0 plutôt que de faire échouer la lecture.
    assert_eq!(index(r"c:\x\y.exe,gros"), 0);
}

/// Une extension de RÉPERTOIRE ne doit pas être prise pour celle du fichier.
#[test]
fn l_extension_est_celle_du_dernier_segment() {
    assert_eq!(provenance(r"C:\dossier.exe\fichier,0", ""), Provenance::Aucune);
    assert_eq!(
        provenance(r"C:\dossier.ico\fichier.exe,0", ""),
        Provenance::Module(r"C:\dossier.ico\fichier.exe".into())
    );
    // Un point final n'est pas une extension.
    assert_eq!(provenance(r"C:\x\y.,0", ""), Provenance::Aucune);
}

/// La casse de l'extension ne décide de rien.
#[test]
fn la_casse_de_l_extension_est_indifferente() {
    assert!(matches!(provenance(r"C:\X\Y.EXE,0", ""), Provenance::Module(_)));
    assert!(matches!(provenance(r"C:\X\Y.Ico,0", ""), Provenance::Ico(_)));
}
