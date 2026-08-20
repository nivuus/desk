//! Ce qu'un raccourci Windows devient, et ce qui le fait entrer au catalogue.
//!
//! 🔴 CE MODULE EST PUR : aucun `#[cfg]`, aucun objet COM, aucun accès au
//! système de fichiers. C'est ce qui permet de le juger sur l'hôte Linux
//! contre `agent/testdata/gapps-corpus-vm.json`, les 218 raccourcis réels de
//! la VM. La lecture COM vit dans `apps::lecture`, qui est `#[cfg(windows)]`.
//!
//! ⚠️ L'EXISTENCE DE LA CIBLE EST UN PRÉDICAT INJECTÉ, et c'est la seule
//! raison pour laquelle la règle de filtrage est testable ici : écrite avec un
//! `Path::exists()` en dur, elle rendrait `false` pour les 167 cibles du
//! corpus — aucune n'existe sur l'hôte — et le test serait inerte tout en
//! restant vert.

use proto::plateforme::Application;

use super::sha256;

/// Un raccourci tel que le Shell le rend, avant toute décision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Brut {
    /// Le nom du `.lnk`, sans son extension.
    pub nom: String,
    /// Le chemin du `.lnk` lui-même.
    pub chemin: String,
    pub cible: String,
    pub arguments: String,
    pub repertoire: String,
}

/// Pourquoi un raccourci n'entre pas au catalogue.
///
/// ⚠️ TROIS MOTIFS, ET AUCUN N'EST UN FILTRE PAR NOM NI PAR CHEMIN. Un motif
/// « Uninstall » dépend de la langue — cette VM est en français, et l'un de ses
/// désinstalleurs s'appelle `maintenancetool.exe` — et il écarterait en
/// silence des applications légitimes. Un filtre par chemin système perdrait
/// Bloc-notes et Paint, dont les cibles sont deux des 63 sous `C:\Windows`.
/// Les 15 désinstalleurs entrent donc au catalogue, et c'est l'utilisateur qui
/// les masque, par un geste explicite et réversible.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ecart {
    /// Cible de l'espace de noms Shell (PIDL), sans chemin de fichier —
    /// `Paramètres Windows`, `Ce PC`, la Corbeille. **Ce n'est pas une
    /// erreur** : 7 des 218 raccourcis de la VM sont dans ce cas.
    CibleVide,
    /// L'extension observée, repliée en minuscules. Vide si la cible n'en
    /// porte aucune.
    Extension(String),
    /// La cible est nommée mais le fichier n'est pas là — 3 des 170 `.exe` de
    /// la VM, tous sous une installation Python 3.13 disparue.
    CibleAbsente,
}

/// La seule extension de cible qui entre au catalogue en v1.
///
/// ⚠️ C'EST UNE LISTE D'AUTORISATION, PAS UNE LISTE DE REFUS, et la différence
/// porte sur les extensions qu'on n'a pas vues : une liste de refus les
/// laisserait toutes entrer. Les `.msc` et les `.url` sont lançables et
/// délibérément exclus — `.msc` ouvre une console MMC dont la fenêtre est
/// celle de `mmc.exe`, que le rattachement de fenêtre du chantier D n'a jamais
/// éprouvé ; `.url` ouvre le navigateur par défaut, c'est-à-dire une
/// application qui n'est pas celle qu'on croit lancer. Hors périmètre v1,
/// nommés et non oubliés.
const EXTENSIONS_RETENUES: &[&str] = &["exe"];

/// L'extension du dernier composant d'un chemin Windows, repliée en
/// minuscules. Vide si le nom de fichier n'en porte aucune.
fn extension_de(chemin: &str) -> String {
    let fichier = chemin.rsplit(['\\', '/']).next().unwrap_or(chemin);
    match fichier.rsplit_once('.') {
        Some((_, ext)) => ext.to_lowercase(),
        None => String::new(),
    }
}

/// La règle de filtrage, dans l'ordre où la spec l'écrit.
///
/// `existe` est INJECTÉ : voir l'en-tête de ce module.
pub fn retenir(brut: &Brut, existe: &dyn Fn(&str) -> bool) -> Result<(), Ecart> {
    if brut.cible.trim().is_empty() {
        return Err(Ecart::CibleVide);
    }
    let extension = extension_de(&brut.cible);
    if !EXTENSIONS_RETENUES.contains(&extension.as_str()) {
        return Err(Ecart::Extension(extension));
    }
    if !existe(&brut.cible) {
        return Err(Ecart::CibleAbsente);
    }
    Ok(())
}

/// Replie un chemin Windows pour la comparaison : espaces de bord retirés,
/// casse repliée, barre oblique inverse finale retirée.
///
/// ⚠️ LA CASSE EST REPLIÉE PARCE QUE NTFS EST INSENSIBLE À LA CASSE : deux
/// chemins qui n'en diffèrent que désignent le même fichier, et les traiter
/// comme deux applications dédoublerait le catalogue au gré de la casse
/// qu'un installeur a écrite dans son `.lnk`.
///
/// ⚠️ LA BARRE FINALE EST RETIRÉE SAUF SUR UNE RACINE (`c:\`), qui n'est pas
/// un répertoire sans elle. 25 des 167 répertoires de travail de la VM en
/// portent une ; **la retirer ne change AUCUN des deux comptes de clés**
/// (mesuré : 154 et 104 dans les deux cas). Ce n'est donc pas une correction
/// mais une prudence, et c'est écrit pour qu'on ne la croie pas mesurée
/// nécessaire.
pub fn normaliser_chemin(chemin: &str) -> String {
    let taille = chemin.trim().to_lowercase();
    if taille.len() > 3 && taille.ends_with('\\') {
        taille.trim_end_matches('\\').to_string()
    } else {
        taille
    }
}

/// L'identité d'une application : l'empreinte du triplet.
///
/// 🔴 LES ARGUMENTS SONT BRUTS, LA CIBLE ET LE RÉPERTOIRE SONT NORMALISÉS, et
/// c'est le chiffre qui l'impose : sur les 167 raccourcis retenus de la VM, le
/// triplet rend **154** clés distinctes quand la cible seule en rend **104**.
/// Les 26 raccourcis de `smartmontools` visent tous le même `runcmdu.exe` avec
/// des arguments différents ; les fondre serait perdre 25 applications, et
/// l'écart total est de 50.
///
/// Replier la casse des arguments referait la même perte en plus discret :
/// `-Mode admin` et `-mode Admin` sont deux invocations distinctes.
///
/// Le séparateur est l'octet nul, qui ne peut apparaître dans aucun des trois
/// champs : sans lui, `("ab", "", "c")` et `("a", "b", "c")` auraient la même
/// empreinte.
pub fn cle(cible: &str, arguments: &str, repertoire: &str) -> String {
    let mut matiere = Vec::new();
    matiere.extend_from_slice(normaliser_chemin(cible).as_bytes());
    matiere.push(0);
    matiere.extend_from_slice(arguments.as_bytes());
    matiere.push(0);
    matiere.extend_from_slice(normaliser_chemin(repertoire).as_bytes());
    sha256::hex(&matiere)
}

/// Ce qui voyage sur le canal : la clé, plus les cinq champs.
///
/// ⚠️ `chemin` EST CELUI DU `.lnk`, ET C'EST LUI QU'ON LANCERA. Il ne
/// participe pas à l'identité — un raccourci qui se déplace du Bureau vers le
/// menu Démarrer reste la même application — mais il doit voyager, parce que
/// le lancement passe par le raccourci et non par la cible reconstruite.
pub fn depuis_brut(brut: Brut) -> Application {
    Application {
        cle: cle(&brut.cible, &brut.arguments, &brut.repertoire),
        nom: brut.nom,
        chemin: brut.chemin,
        cible: normaliser_chemin(&brut.cible),
        arguments: brut.arguments,
        repertoire: normaliser_chemin(&brut.repertoire),
    }
}

#[cfg(test)]
#[path = "raccourci/tests.rs"]
mod tests;
