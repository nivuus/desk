//! La lecture du registre — la SEULE moitié `#[cfg(windows)]` des associations.
//!
//! 🔴 ELLE NE DÉCIDE DE RIEN. Elle lit des chaînes ; l'appariement, la
//! normalisation et l'ordre vivent dans le module parent, qui est PUR et
//! testé sur l'hôte. C'est la coupure que G2 a établie deux fois
//! (`apps/icone/ressource.rs`, `apps/installation`), et son intérêt est
//! identique : sans elle, la partie qui décide n'aurait **aucun** test d'hôte.
//!
//! ⚠️ CE MODULE N'A AUCUN TEST, ET C'EST DÉCLARÉ : il ne compile que sur la VM.
//! `cargo check --target x86_64-pc-windows-gnu` en vérifie **types, emprunts et
//! durées de vie** — jamais le comportement.

use windows::Win32::Foundation::{ERROR_SUCCESS, MAX_PATH};
use windows::Win32::System::Registry::{
    RegCloseKey, RegEnumKeyExW, RegGetValueW, RegOpenKeyExW, HKEY, HKEY_CLASSES_ROOT,
    HKEY_CURRENT_USER, KEY_READ, RRF_RT_REG_SZ,
};
use windows::core::{PCWSTR, HSTRING};

/// 🔴 LE PLAFOND EXISTE POUR QUE LE PIRE CAS SOIT BORNÉ, et il est déclaré.
/// `FileExts` porte typiquement quelques dizaines d'entrées ; un registre
/// déréglé pourrait en porter beaucoup plus, et la réconciliation tourne
/// toutes les `PERIODE_RECONCILIATION`. **Non calibré** : c'est un garde-fou
/// contre une lecture qui ne finirait pas, jamais une cible.
const EXTENSIONS_MAX: usize = 512;

/// La clé où Windows range LE CHOIX RÉEL DE L'UTILISATEUR.
const FILE_EXTS: &str =
    r"Software\Microsoft\Windows\CurrentVersion\Explorer\FileExts";

fn en_pcwstr(s: &str) -> HSTRING {
    HSTRING::from(s)
}

/// Lit une valeur `REG_SZ`, ou rend `None`.
///
/// ⚠️ `RegGetValueW` EST PRÉFÉRÉ À `RegQueryValueExW` : il ouvre, lit, vérifie
/// le type et ferme en un appel, et il **garantit la terminaison nulle** de ce
/// qu'il rend — ce que `RegQueryValueEx` ne fait pas, et qui est une source
/// classique de lecture au-delà du tampon.
fn valeur(racine: HKEY, sous_cle: &str, nom: Option<&str>) -> Option<String> {
    let mut tampon = [0u16; 2048];
    let mut octets = (tampon.len() * 2) as u32;
    let cle = en_pcwstr(sous_cle);
    let nom_h = nom.map(en_pcwstr);
    let code = unsafe {
        RegGetValueW(
            racine,
            PCWSTR(cle.as_ptr()),
            nom_h.as_ref().map_or(PCWSTR::null(), |h| PCWSTR(h.as_ptr())),
            RRF_RT_REG_SZ,
            None,
            Some(tampon.as_mut_ptr().cast()),
            Some(&mut octets),
        )
    };
    if code != ERROR_SUCCESS {
        return None;
    }
    // `octets` compte les OCTETS, terminateur compris : on repasse en `u16` et
    // on retire le terminateur.
    let unites = (octets as usize / 2).min(tampon.len());
    let sans_zero = tampon[..unites]
        .iter()
        .position(|&c| c == 0)
        .unwrap_or(unites);
    let texte = String::from_utf16_lossy(&tampon[..sans_zero]);
    if texte.is_empty() {
        None
    } else {
        Some(texte)
    }
}

/// Les extensions que l'utilisateur a un jour choisies, telles quelles.
fn extensions_connues() -> Vec<String> {
    let mut cle = HKEY::default();
    let chemin = en_pcwstr(FILE_EXTS);
    let ouvert = unsafe {
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(chemin.as_ptr()),
            Some(0),
            KEY_READ,
            &mut cle,
        )
    };
    if ouvert != ERROR_SUCCESS {
        return Vec::new();
    }
    let mut noms = Vec::new();
    let mut index = 0u32;
    loop {
        if noms.len() >= EXTENSIONS_MAX {
            tracing::warn!(
                plafond = EXTENSIONS_MAX,
                "plafond d'extensions atteint : la lecture des associations s'arrete la"
            );
            break;
        }
        let mut tampon = [0u16; MAX_PATH as usize];
        let mut taille = tampon.len() as u32;
        let code = unsafe {
            RegEnumKeyExW(
                cle,
                index,
                Some(windows::core::PWSTR(tampon.as_mut_ptr())),
                &mut taille,
                None,
                None,
                None,
                None,
            )
        };
        if code != ERROR_SUCCESS {
            break;
        }
        noms.push(String::from_utf16_lossy(&tampon[..taille as usize]));
        index += 1;
    }
    unsafe { let _ = RegCloseKey(cle); };
    noms
}

/// Le ProgID retenu pour une extension : **le choix de l'utilisateur d'abord**,
/// la valeur par défaut de `HKCR` ensuite.
///
/// 🔴 L'ORDRE EST LA RÈGLE, PAS UNE COMMODITÉ : `UserChoice` est ce que la
/// personne a réellement choisi, et `HKCR` ce que la dernière installation a
/// posé. Les intervertir attribuerait l'association au logiciel le plus
/// récemment installé plutôt qu'à celui qu'on emploie.
fn progid(extension_brute: &str) -> Option<String> {
    let choix = valeur(
        HKEY_CURRENT_USER,
        &format!(r"{FILE_EXTS}\{extension_brute}\UserChoice"),
        Some("ProgId"),
    );
    if choix.is_some() {
        return choix;
    }
    let avec_point = if extension_brute.starts_with('.') {
        extension_brute.to_string()
    } else {
        format!(".{extension_brute}")
    };
    valeur(HKEY_CLASSES_ROOT, &avec_point, None)
}

/// Les couples `(extension, ligne de commande)` que le registre porte —
/// **une seule lecture, pour toutes les applications**.
///
/// 🔴 CE QU'ELLE REND EST BRUT, ET C'EST TOUT SON OBJET. Le groupement, la
/// normalisation et l'ordre sont des règles PURES et TESTÉES
/// (`super::table`) ; ce module ne fait que lire.
///
/// ⚠️ LA PORTÉE EST CELLE DE `FileExts`, ET ELLE EST PLUS ÉTROITE QUE « TOUTES
/// LES ASSOCIATIONS DE LA MACHINE » : ce sont les extensions que
/// L'UTILISATEUR a un jour ouvertes ou choisies. Énumérer `HKEY_CLASSES_ROOT`
/// en entier — plusieurs milliers de clés, relues à chaque réconciliation —
/// serait payer très cher un ensemble que personne n'a demandé. **Déclaré,
/// et non découvert.**
pub fn couples() -> Vec<(String, String)> {
    let mut couples = Vec::new();
    for brute in extensions_connues() {
        let Some(id) = progid(&brute) else { continue };
        let Some(commande) = valeur(
            HKEY_CLASSES_ROOT,
            &format!(r"{id}\shell\open\command"),
            None,
        ) else {
            continue;
        };
        couples.push((brute, commande));
    }
    couples
}
