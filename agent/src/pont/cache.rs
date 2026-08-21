//! Le cache d'énumération : ce qu'un répertoire contenait, et depuis quand.
//! **PUR** — aucun `cfg`, aucune dépendance à `windows`, entièrement testé sur
//! l'hôte, **horloge injectée**.
//!
//! # Ce qu'il est, et ce qu'il n'est PAS
//!
//! Une [`crate::pont::enumeration::Session`] retient les entrées d'**une**
//! énumération, entre les appels successifs de `GetDirectoryEnumeration` qui la
//! servent, et meurt avec `EndDirectoryEnumeration`. **Ce cache-ci lui
//! survit** : il est indexé par **CHEMIN**, il expire par `TTL_ENUMERATION`, et
//! il ne peut être vidé de force que par l'annonce `Rafraichir`.
//!
//! # 🔴 LE DÉFAUT DE L'ANCIEN PONT, ET POURQUOI CE MODULE EST LE RISQUE N°1
//!
//! `src/file.js` servait des octets depuis un cache **sans aucun TTL**, invalidé
//! seulement par une écriture passant par ce même pont. La spec §7.4 en tire la
//! phrase qui gouverne ce fichier : *« Une modification faite sur le poste local
//! n'était donc jamais vue, pour toujours. »*
//!
//! **C'est la seule addition de tout le sous-projet ③ qui puisse rendre FAUX un
//! comportement déjà recetté par F2 et F3** : un fichier créé, renommé ou
//! supprimé qui cesserait d'être vu. D'où [`CacheEnumeration::invalider`], et
//! d'où le critère ④ de la recette, dont la rouge retire l'invalidation.
//!
//! # Pourquoi les entrées sont stockées BRUTES
//!
//! Le filtrage par `searchExpression` et le tri par `PrjFileNameCompare`
//! dépendent de la **requête** (`dir *.txt` et `dir` n'ont pas le même
//! résultat) et de comparateurs que ProjFS seul fournit. Stocker le résultat
//! *préparé* ferait qu'un `dir *.txt` **empoisonnerait** le cache pour le `dir`
//! suivant. Le cache vit donc **en amont** de
//! [`crate::pont::enumeration::preparer`], qui court à chaque chargement de
//! session.
//!
//! # L'horloge est un PARAMÈTRE
//!
//! Jamais `Instant::now()` à l'intérieur : c'est ce qui rend l'expiration
//! testable **sans dormir**, exactement comme `pont::table` se l'est donné pour
//! ses délais.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use crate::pont::enumeration::Entree;

/// Combien de temps une énumération mémorisée reste servie.
///
/// ⚠️ **NON CALIBRÉE.** Elle rejoint `BPP_MIN`, `FACTEUR_FOCUS`,
/// `PART_DORMANTE_BPS`, `HYSTERESIS`, `REPIT_APRES_ECHEC`, `TAILLE_MAX_SORTIE`,
/// `REPIT_REARMEMENT_AUDIO`, `REARMEMENTS_MAX`, `TAILLE_TRAME_MAX`,
/// `DELAI_LISTER`, `ATTENTE_MAX` et les treize seaux de `pont::latence` dans la
/// liste des constantes de ce dépôt qu'**aucune mesure n'a jugées**.
///
/// Ce qui a présidé au choix, et qui n'est PAS une calibration :
///
/// - elle doit être **plus longue** qu'un geste de recette complet — F4 mesure
///   un listage de mille entrées à ~6 s, et le protocole du critère ① enchaîne
///   deux listages plus une addition de fichier ;
/// - elle doit être **plus courte** que le temps qu'un utilisateur accepte de
///   voir un contenu périmé sans que rien ne le lui dise ;
/// - ⚠️ **elle ne doit PAS valoir 60 s**, qui est `PERIODE_HYDRATATION`
///   (`service.rs`). *Deux constantes qui se recalibreraient séparément et qui
///   portent le même nombre finissent par se croire liées* — ce dépôt l'écrit
///   déjà de `PLAFOND_DISSIMULATION` et `micro::PLAFOND`.
pub const TTL_ENUMERATION: Duration = Duration::from_secs(30);

/// Ce qu'un répertoire contenait, et quand on l'a appris.
struct Memoire {
    entrees: Vec<Entree>,
    pose_a: Instant,
}

/// Le cache d'énumération, indexé par chemin de répertoire.
#[derive(Default)]
pub struct CacheEnumeration {
    par_chemin: HashMap<String, Memoire>,
}

impl CacheEnumeration {
    pub fn nouveau() -> Self {
        Self::default()
    }

    /// Les entrées mémorisées pour ce répertoire, si elles n'ont pas expiré.
    ///
    /// **L'entrée expirée est RETIRÉE, pas seulement ignorée.** La laisser
    /// ferait croître le cache sans terme sur une arborescence qu'on parcourt
    /// une fois — et ce module n'a **aucune** politique d'éviction (spec §10 R4).
    pub fn lire(&mut self, chemin: &str, maintenant: Instant) -> Option<&[Entree]> {
        let perime = match self.par_chemin.get(chemin) {
            Some(m) => maintenant.duration_since(m.pose_a) >= TTL_ENUMERATION,
            None => return None,
        };
        if perime {
            self.par_chemin.remove(chemin);
            return None;
        }
        self.par_chemin.get(chemin).map(|m| m.entrees.as_slice())
    }

    /// Mémorise ce qu'un répertoire contient. Écrase toute mémoire antérieure.
    pub fn poser(&mut self, chemin: String, entrees: Vec<Entree>, maintenant: Instant) {
        self.par_chemin.insert(chemin, Memoire { entrees, pose_a: maintenant });
    }

    /// Oublie ce que contenait le répertoire **PARENT** du chemin muté.
    ///
    /// 🔴 **LE PARENT, JAMAIS LE CHEMIN LUI-MÊME, et se tromper là est
    /// SILENCIEUX.** Un cache d'énumération est indexé par **répertoire** :
    /// invalider `dossier/note.txt` ne toucherait aucune clé, le listage de
    /// `dossier` continuerait d'être servi depuis la mémoire, et **le fichier
    /// créé n'apparaîtrait jamais**. Rien ne le dirait. Un test d'hôte le
    /// verrouille, et sa rouge est de faire prendre à cette fonction le chemin
    /// lui-même.
    ///
    /// ⚠️ **Le parent de `"note.txt"` est la RACINE, `""`** — et la racine est
    /// une clé comme une autre, celle qu'un `Get-ChildItem` sur le lecteur
    /// monté sollicite. L'oublier ferait que toute création à la racine serait
    /// invisible, ce qui est exactement le geste du critère ① de la recette.
    pub fn invalider(&mut self, chemin: &str) {
        self.par_chemin.remove(parent_de(chemin));
    }

    /// Oublie tout. C'est ce que fait l'annonce `Rafraichir`.
    pub fn vider(&mut self) {
        self.par_chemin.clear();
    }

    /// Combien de répertoires sont mémorisés. **Pour la trace et les tests.**
    pub fn taille(&self) -> usize {
        self.par_chemin.len()
    }
}

/// Le répertoire qui contient `chemin`, dans la convention du pont : des
/// séparateurs `/`, et la **racine est la chaîne vide**.
///
/// ⚠️ **Cette convention est celle de `pont::chemins`**, et elle vient de la
/// File System Access API, qui ne connaît pas `\`. Un chemin ProjFS arrive avec
/// des `\` et il est converti **avant** d'atteindre ce module.
fn parent_de(chemin: &str) -> &str {
    match chemin.rfind('/') {
        Some(i) => &chemin[..i],
        None => "",
    }
}

#[cfg(test)]
mod tests;
