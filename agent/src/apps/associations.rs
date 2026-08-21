//! Les associations de fichiers d'une application, lues sur la VM.
//!
//! 🔴 CE QUI EST PUR VIT ICI ; SEULE LA LECTURE DU REGISTRE EST
//! `#[cfg(windows)]` (`associations/registre.rs`). C'est la coupure que G2 a
//! établie deux fois — `apps/icone/ressource.rs`, `apps/installation` — et son
//! intérêt est identique : **sans elle, la partie qui DÉCIDE n'aurait aucun
//! test d'hôte**, et le seul moyen de l'éprouver serait de reconstruire un
//! binaire pour la VM.
//!
//! 🔴 L'APPARIEMENT SE FAIT PAR IDENTITÉ DE CHEMIN, JAMAIS PAR SOUS-CHAÎNE DE
//! NOM (décision D12 du plan de G5). Le modèle que la conception cite,
//! `src/app.js:15-37`, apparie le ProgID à l'application **par sous-chaîne sur
//! son nom**, après en avoir retiré les chiffres. C'est **la même heuristique
//! que G1 a déjà remplacée pour les identifiants, et pour la même raison** :
//! « Nsight 2020.3 » et « Nsight 2024.6 » y produisaient la même clé. La
//! reprendre ici attribuerait `.cu` à la mauvaise version de l'une, et
//! personne ne le verrait.
//!
//! La chaîne, pour une extension :
//!   ① `HKCU\…\FileExts\<ext>\UserChoice\ProgId` — LE CHOIX RÉEL DE
//!      L'UTILISATEUR, et il l'emporte ;
//!   ② à défaut, la valeur par défaut de `HKCR\.<ext>` ;
//!   ③ puis `HKCR\<ProgID>\shell\open\command`, dont on EXTRAIT le chemin de
//!      l'exécutable ;
//!   ④ que l'on NORMALISE avec la normalisation déjà écrite
//!      (`raccourci::normaliser_chemin`) — jamais une seconde —, et que l'on
//!      compare à `application.cible`.

#[cfg(windows)]
pub mod registre;

#[cfg(test)]
#[path = "associations/tests.rs"]
mod tests;

use crate::apps::raccourci::normaliser_chemin;

/// La table des associations de CETTE machine, ou une table VIDE hors Windows.
///
/// 🔴 UN SEUL POINT D'ENTRÉE POUR LE PRODUIT, ET IL COMPILE PARTOUT. Le
/// `#[cfg]` vit ici et nulle part ailleurs : l'appelant n'a pas à savoir sur
/// quel système il tourne, et la boucle de découverte reste lisible sur
/// l'hôte comme sur la VM.
pub fn table_de_la_machine() -> std::collections::BTreeMap<String, Vec<String>> {
    #[cfg(windows)]
    {
        table(registre::couples())
    }
    #[cfg(not(windows))]
    {
        // ⚠️ VIDE, ET NON UNE PANIQUE : l'agent se compile sur l'hôte pour ses
        // tests, et une table vide y est la vérité — cette machine n'a pas de
        // registre Windows.
        std::collections::BTreeMap::new()
    }
}

/// Extrait le chemin de l'exécutable d'une ligne de commande Windows.
///
/// 🔴 LA RÈGLE EST CELLE DE WINDOWS, PAS UNE APPROXIMATION COMMODE : si la
/// ligne commence par un guillemet, le chemin court **jusqu'au guillemet
/// fermant** et peut donc contenir des espaces ; sinon il court **jusqu'au
/// premier espace**. C'est ce qui distingue
/// `"C:\Program Files\App\a.exe" "%1"` — un chemin à espaces — de
/// `C:\Windows\notepad.exe %1`.
///
/// ⚠️ UNE LIGNE NON CITÉE DONT LE CHEMIN PORTE UN ESPACE EST DONC TRONQUÉE, et
/// c'est **le comportement de Windows lui-même**, pas une lacune d'ici : le
/// système essaie alors plusieurs découpes. Nous ne les essayons pas — une
/// association mal appariée serait pire qu'une association absente, et le
/// silence est ici la réponse honnête.
///
/// Rend `None` sur une ligne vide, ou sur un guillemet ouvrant jamais fermé.
pub fn executable_de_commande(commande: &str) -> Option<String> {
    let taille = commande.trim();
    if taille.is_empty() {
        return None;
    }
    let chemin = if let Some(reste) = taille.strip_prefix('"') {
        // ⚠️ On refuse un guillemet ouvrant non fermé plutôt que de prendre
        // tout le reste : une ligne mal formée n'est pas un chemin.
        reste.split_once('"').map(|(avant, _)| avant)?
    } else {
        taille.split_whitespace().next()?
    };
    if chemin.is_empty() {
        return None;
    }
    Some(chemin.to_string())
}

/// La ligne de commande d'un ProgID désigne-t-elle CETTE cible ?
///
/// 🔴 UNE ÉGALITÉ DE CHEMIN NORMALISÉ, JAMAIS UNE SOUS-CHAÎNE DE NOM.
/// `normaliser_chemin` est RÉEMPLOYÉE et non recopiée : deux normalisations
/// divergeraient le jour où l'une d'elles changerait, et l'appariement
/// deviendrait faux **du seul côté qui n'aurait pas bougé**.
pub fn commande_vise(commande: &str, cible: &str) -> bool {
    match executable_de_commande(commande) {
        None => false,
        Some(exe) => normaliser_chemin(&exe) == normaliser_chemin(cible),
    }
}

/// Normalise une extension : minuscules, **avec** le point de tête.
///
/// ⚠️ LE POINT EST IMPOSÉ ICI, ET C'EST CE QUI REND LE FORMAT DU FIL
/// PRÉVISIBLE : le registre écrit `.txt` sous `HKCR` et `txt` sous `FileExts`
/// selon les clés, et laisser les deux formes voyager obligerait la plateforme
/// à choisir — c'est-à-dire à porter une règle qui n'est pas la sienne.
pub fn normaliser_extension(extension: &str) -> Option<String> {
    let taille = extension.trim().trim_start_matches('.').to_lowercase();
    if taille.is_empty() || taille.contains(['\\', '/', ' ']) {
        return None;
    }
    Some(format!(".{taille}"))
}

/// La TABLE des associations : chemin d'exécutable **normalisé** → extensions
/// rangées.
///
/// 🔴 UNE TABLE, ET NON UNE INTERROGATION PAR APPLICATION, ET C'EST UNE
/// DÉCISION DE COÛT. Le corpus de la VM porte **156 applications** ; demander
/// au registre, pour chacune, quelles extensions la visent, ferait relire
/// toutes les entrées de `FileExts` **156 fois par réconciliation** — et la
/// réconciliation tourne toutes les `PERIODE_RECONCILIATION`. Le registre est
/// donc lu **une fois**, et chaque application y **cherche** son chemin.
///
/// 🔴 ET C'EST AUSSI CE QUI REND LA RÈGLE ÉPROUVABLE : ce qui entre est une
/// liste de couples `(extension, ligne de commande)` — exactement ce qu'un
/// registre rend —, et tout le reste (extraire l'exécutable, normaliser,
/// grouper, ranger) est **pur** et vit ici.
///
/// ⚠️ UNE LIGNE DE COMMANDE ILLISIBLE EST ÉCARTÉE EN SILENCE, et c'est
/// délibéré : un ProgID dont la commande ne se lit pas ne désigne aucune
/// application, et le seul autre choix serait de l'attribuer au hasard.
pub fn table(couples: Vec<(String, String)>) -> std::collections::BTreeMap<String, Vec<String>> {
    let mut brute: std::collections::BTreeMap<String, Vec<String>> =
        std::collections::BTreeMap::new();
    for (extension, commande) in couples {
        let Some(exe) = executable_de_commande(&commande) else { continue };
        let Some(ext) = normaliser_extension(&extension) else { continue };
        brute.entry(normaliser_chemin(&exe)).or_default().push(ext);
    }
    brute.into_iter().map(|(exe, exts)| (exe, ranger(exts))).collect()
}

/// Ce que la table retient pour une cible — la liste VIDE si elle n'y est pas.
///
/// ⚠️ LA CIBLE EST NORMALISÉE ICI AUSSI, et par la MÊME fonction : la table est
/// bâtie sur des chemins normalisés, et l'interroger avec un chemin brut ne
/// trouverait jamais rien — un défaut **silencieux**, qui rendrait simplement
/// toutes les listes vides.
pub fn pour_cible(
    table: &std::collections::BTreeMap<String, Vec<String>>,
    cible: &str,
) -> Vec<String> {
    table.get(&normaliser_chemin(cible)).cloned().unwrap_or_default()
}

/// Trie et déduplique les extensions d'une application.
///
/// 🔴 L'ORDRE EST IMPOSÉ, ET CE N'EST PAS UN ORNEMENT : la plateforme compare
/// le catalogue reçu à celui qu'elle connaît pour décider ce qu'elle écrit.
/// Un ordre d'énumération du registre — qui n'est garanti par rien — ferait
/// diverger deux listes IDENTIQUES, donc écrire à chaque tour et journaliser
/// un changement qui n'a pas eu lieu.
pub fn ranger(extensions: Vec<String>) -> Vec<String> {
    let mut rangees: Vec<String> = extensions
        .iter()
        .filter_map(|e| normaliser_extension(e))
        .collect();
    rangees.sort();
    rangees.dedup();
    rangees
}
