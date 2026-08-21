//! Les sessions d'énumération : ce qu'un répertoire contient, dans quel ordre,
//! et où l'on en est. **PUR** — aucun `cfg`, aucune dépendance à `windows`,
//! entièrement testé sur l'hôte.
//!
//! # Les deux pièges de ProjFS, tous deux SILENCIEUX
//!
//! 1. **L'ordre est IMPOSÉ.** Les entrées doivent être remplies dans l'ordre de
//!    `PrjFileNameCompare` — qui n'est **ni** l'ordre lexicographique d'`OsStr`,
//!    **ni** `Ordering::cmp`. Or `dir.values()` de la File System Access API ne
//!    garantit **aucun** ordre. Le pont trie donc lui-même, et
//!    `PrjFileNameCompare` est chargée (tâche 12) précisément pour cela.
//! 2. **Le filtre `searchExpression` est FACULTATIF et il est FOURNI**
//!    (`PRJ_GET_DIRECTORY_ENUMERATION_CB`, `mod.rs:315`). L'ignorer est une
//!    faute silencieuse : un `dir /b *.txt` rendrait tout. Il s'applique par
//!    `PrjFileNameMatch`.
//!
//! **Les deux comparateurs sont INJECTÉS**, et c'est ce qui rend ce module
//! testable : la logique — filtrer puis trier, et où en est le curseur — est
//! pure ; seules les deux fonctions de comparaison viennent de ProjFS.
//!
//! # Ce que ce module n'est PAS : un cache d'énumération
//!
//! ⚠️ Une [`Session`] retient les entrées d'**une** énumération, entre les
//! appels successifs de `GetDirectoryEnumeration` qui la servent, et meurt avec
//! `EndDirectoryEnumeration`. **Ce n'est pas le cache d'énumération
//! (`TTL_ENUMERATION`) de la spec §7.4, qui n'est PAS livré en F1** : celui-là
//! survivrait à la session, serait indexé par CHEMIN, et ne pourrait être vidé
//! que par `Rafraichir` — un livrable de F5. Poser un cache dont rien ne peut
//! vider le contenu ferait qu'un fichier ajouté côté poste local n'apparaîtrait
//! **jamais** : le défaut exact de l'ancien pont, dont le cache de données
//! n'avait aucun TTL (`src/file.js:232-241`).
//!
//! ✅ **CE PRONOSTIC A ÉTÉ VÉRIFIÉ, ET F5 EXISTE (21 août 2026).** *Ces lignes
//! annonçaient : « le critère ROUGE de F5 — le fichier apparaît SANS
//! `Rafraichir` — sera par construction rouge tant que F5 n'existe pas ».*
//! **Il l'était, et c'est mesuré** : sur le binaire de F5 avec `PONT_CACHE=0`,
//! qui reproduit exactement le produit d'avant, le fichier ajouté côté poste
//! local apparaît **sans** `Rafraichir` — 2 exécutions. Avec le cache armé, il
//! n'apparaît **qu'après** — 3 exécutions.
//!
//! ⚠️ **LE CACHE VIT DÉSORMAIS DANS [`crate::pont::cache`], PAS ICI**, et la
//! distinction que ce module énonce reste entière : une [`Session`] meurt avec
//! `EndDirectoryEnumeration`, le cache lui survit et n'est indexé que par
//! CHEMIN. **`preparer` court à chaque chargement de session, y compris sur un
//! succès de cache** — celui-ci mémorise les entrées BRUTES, précisément pour
//! qu'un `dir *.txt` n'empoisonne pas le `dir` suivant.

use std::cmp::Ordering;

/// Une entrée de répertoire, telle que le navigateur la rapporte.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entree {
    pub nom: String,
    pub repertoire: bool,
    pub taille: u64,
    /// `File.lastModified`, en millisecondes depuis l'époque Unix.
    ///
    /// ⚠️ **La File System Access API n'en donne qu'UN**, et les quatre champs
    /// horaires de `PRJ_FILE_BASIC_INFO` le portent tous. C'est une divergence
    /// assumée (spec §3.5.2), pas un oubli.
    pub modifie_ms: i64,
}

/// Filtre puis trie, avec les deux fonctions de ProjFS **injectées**.
///
/// `expression` absente : l'apparieur n'est **jamais** consulté. ProjFS n'en
/// fournit pas toujours une, et en forger une (`*`) ferait dépendre le résultat
/// du comportement de `PrjFileNameMatch` sur un motif qu'on aurait inventé.
pub fn preparer(
    entrees: Vec<Entree>,
    expression: Option<&str>,
    mut apparier: impl FnMut(&str, &str) -> bool,
    mut comparer: impl FnMut(&str, &str) -> Ordering,
) -> Vec<Entree> {
    let mut retenues: Vec<Entree> = match expression {
        Some(motif) => entrees.into_iter().filter(|e| apparier(&e.nom, motif)).collect(),
        None => entrees,
    };
    // `sort_by` et non `sort_unstable_by` : le comparateur vient de ProjFS et
    // peut déclarer deux noms égaux (la casse, notamment). Un tri instable
    // rendrait alors un ordre différent d'un appel à l'autre sur la même
    // entrée, ce qui est exactement ce que l'énumération ProjFS interdit entre
    // deux `GetDirectoryEnumeration` d'une même session.
    retenues.sort_by(|a, b| comparer(&a.nom, &b.nom));
    retenues
}

/// Une session d'énumération : les entrées préparées, et où l'on en est.
///
/// ⚠️ **Indexée par le GUID d'ÉNUMÉRATION, jamais par le chemin** (spec §7.2) —
/// c'est l'appelant qui tient l'index, mais la raison vit ici : deux
/// applications qui listent le même répertoire en même temps ouvrent deux
/// sessions distinctes, et indexer par chemin ferait que la seconde écraserait
/// la première ; l'une des deux recevrait un répertoire vide.
#[derive(Debug, Default)]
pub struct Session {
    entrees: Option<Vec<Entree>>,
    curseur: usize,
}

impl Session {
    pub fn nouvelle() -> Self {
        Self::default()
    }

    /// Vrai dès que le navigateur a répondu **une fois** pour cette session.
    ///
    /// Un répertoire vide est bien « chargé » : sans cette distinction, une
    /// session sur un répertoire vide redemanderait la liste à chaque appel de
    /// `GetDirectoryEnumeration`, indéfiniment.
    pub fn chargee(&self) -> bool {
        self.entrees.is_some()
    }

    /// Pose les entrées **et remet le curseur à zéro**.
    ///
    /// La remise à zéro n'est pas une commodité : sans elle, une seconde
    /// réponse `Entrees` laisserait le curseur au-delà de la nouvelle liste, et
    /// l'énumération rendrait vide.
    pub fn poser(&mut self, entrees: Vec<Entree>) {
        self.entrees = Some(entrees);
        self.curseur = 0;
    }

    /// Honore `PRJ_CB_DATA_FLAG_ENUM_RESTART_SCAN` (`mod.rs:177`, valeur
    /// `1i32`) : le curseur revient au début, **et les entrées sont
    /// conservées**.
    ///
    /// Les jeter obligerait à redemander la liste au navigateur, ce qui est un
    /// aller-retour pour rien — et surtout ferait rendre `S_OK` avec un tampon
    /// vide en attendant, c'est-à-dire un répertoire vide, silencieusement.
    pub fn redemarrer(&mut self) {
        self.curseur = 0;
    }

    /// L'entrée courante, ou `None` si la session est épuisée ou pas chargée.
    pub fn prochaine(&self) -> Option<&Entree> {
        self.entrees.as_ref()?.get(self.curseur)
    }

    /// Passe à la suivante. Appelée **après** un remplissage accepté par
    /// `PrjFillDirEntryBuffer`, jamais avant : avancer sur un tampon plein
    /// perdrait l'entrée pour toujours.
    pub fn avancer(&mut self) {
        self.curseur += 1;
    }
}

#[cfg(test)]
mod tests;
