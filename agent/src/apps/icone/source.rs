//! D'où vient l'icône d'un raccourci : de son `IconLocation`, ou de sa cible.
//!
//! **PUR, sans aucun `cfg`.** Il découpe une chaîne et regarde une extension ;
//! il n'ouvre rien.
//!
//! 🔴 LA RÈGLE « CHEMIN VIDE ⇒ C'EST LA CIBLE QUI PORTE L'ICÔNE » N'EST PAS UN
//! DÉTAIL. Mesuré le 20 août 2026 : **92 des 153 raccourcis retenus de la VM
//! de développement** portent un `IconLocation` SANS chemin — la spécification
//! en relève 135 sur 218 avant filtrage. Les traiter comme « pas d'icône »
//! ferait perdre son icône à **plus d'une application sur deux, en silence**.
//!
//! C'est exactement là que les icônes du produit historique se perdaient :
//! `convertToLinuxPath('')` rend la chaîne vide (`src/lnkParser.js:172`, cité
//! par la spécification).

/// D'où lire le répertoire d'icônes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Provenance {
    /// Un module PE — `.exe`, `.dll`, `.mun` : sa ressource `RT_GROUP_ICON`.
    Module(String),
    /// Un `.ico` autonome : son `ICONDIR`, c'est-à-dire ses premiers octets.
    Ico(String),
    /// 🔴 RIEN DE LISIBLE, ET CE N'EST PAS UNE ERREUR. Une association de
    /// type, un espace de noms Shell, un `ProductIcon` d'installeur MSI sans
    /// extension. L'image sera extraite quand même — le Shell sait la rendre —
    /// mais sa PROVENANCE restera `SourceMax::NonMesuree`. **Mesuré : 37 des
    /// 153 applications de cette VM.**
    Aucune,
}

/// ⚠️ LA DERNIÈRE VIRGULE, JAMAIS LA PREMIÈRE.
///
/// Un chemin Windows peut en contenir une — `C:\Program Files\Machin, Inc\a.exe`
/// est parfaitement légal — et découper sur la première rendrait
/// `C:\Program Files\Machin` comme chemin et ` Inc\a.exe,0` comme index. Le
/// format est `<chemin>,<index>` : c'est la DERNIÈRE virgule qui sépare.
fn couper(icon_location: &str) -> (&str, Option<&str>) {
    match icon_location.rfind(',') {
        Some(i) => (&icon_location[..i], Some(&icon_location[i + 1..])),
        None => (icon_location, None),
    }
}

/// D'où vient l'icône, `IconLocation` du `.lnk` et cible à l'appui.
pub fn provenance(icon_location: &str, cible: &str) -> Provenance {
    let (chemin, _) = couper(icon_location);
    // 🔴 LES 92 SUR 153 : chemin vide — y compris la chaîne entièrement vide,
    // et le `,0` seul — renvoie à la CIBLE.
    let chemin = if chemin.trim().is_empty() { cible } else { chemin };
    let chemin = chemin.trim();
    if chemin.is_empty() {
        return Provenance::Aucune;
    }
    match extension(chemin).as_deref() {
        Some("ico") => Provenance::Ico(chemin.to_string()),
        // `.mun` est le conteneur de ressources que Windows 10+ emploie pour
        // les icônes du système (`imageres.dll` y renvoie).
        Some("exe" | "dll" | "mun" | "cpl" | "scr" | "ocx") => Provenance::Module(chemin.to_string()),
        // ⚠️ SANS EXTENSION, ON NE SAIT PAS LIRE — et le dire vaut mieux que
        // de deviner. `C:\Windows\Installer\{1BEA…}\ProductIcon` en est le cas
        // le plus fréquent de ce corpus.
        _ => Provenance::Aucune,
    }
}

/// L'extension en minuscules, ou `None` — jamais celle d'un répertoire parent.
fn extension(chemin: &str) -> Option<String> {
    let dernier = chemin.rsplit(['\\', '/']).next()?;
    let point = dernier.rfind('.')?;
    if point + 1 >= dernier.len() {
        return None;
    }
    Some(dernier[point + 1..].to_ascii_lowercase())
}

/// L'index de l'icône dans le module, `0` à défaut.
///
/// ⚠️ IL PEUT ÊTRE NÉGATIF : un index négatif désigne une ressource par son
/// IDENTIFIANT et non par son rang, et c'est un usage courant de
/// `imageres.dll`. D'où l'`i32`, jamais un `u32`.
pub fn index(icon_location: &str) -> i32 {
    match couper(icon_location) {
        (_, Some(i)) => i.trim().parse().unwrap_or(0),
        (_, None) => 0,
    }
}

#[cfg(test)]
#[path = "source/tests.rs"]
mod tests;
