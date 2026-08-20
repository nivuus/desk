//! Le journal de reprise des écritures dues. **PUR** — aucun `cfg`, aucune
//! entrée-sortie : il rend les LIGNES à ajouter, et c'est l'appelant qui les
//! écrit.
//!
//! # 🔴 Ce qu'il est, et pourquoi il ne peut pas être autre chose
//!
//! **ProjFS ne met JAMAIS le fournisseur sur le chemin de l'écriture** (spec
//! §6.1). Quand nous apprenons qu'un fichier a été modifié, l'application a
//! déjà refermé son handle et **cru avoir enregistré**. Entre cet instant et
//! l'arrivée des octets sur le poste local s'ouvre une **fenêtre de perte** que
//! rien ne peut fermer.
//!
//! Ce module ne la ferme pas non plus. Il fait la seule chose qui reste :
//! **écrire sur le disque de la VM, HORS de la racine, la liste de ce qui n'est
//! pas encore arrivé** — pour qu'un pont relancé la repousse, et pour qu'un
//! utilisateur qui referme son onglet apprenne ce qu'il risque de perdre.
//!
//! *Savoir ce qu'on a perdu n'est pas l'avoir* (spec §6.4). Ce module tient le
//! premier terme.
//!
//! # La forme du fichier, et les quatre raisons de celle-là
//!
//! ```text
//! +<octets> <chemin JSON>\n     inscription
//! -<chemin JSON>\n              retrait
//! ```
//!
//! 1. **EN AJOUT SEUL, jamais de réécriture en place.** Une réécriture
//!    interrompue perdrait les entrées **ANTÉRIEURES** — c'est-à-dire les plus
//!    anciennes, donc celles qui attendent depuis le plus longtemps. La spec
//!    §4.4 désigne nommément ce cas comme le rouge de ce module.
//! 2. **Le chemin est encodé en JSON.** Il peut porter des espaces, des
//!    accents, et — le poste local pouvant être sur macOS ou Linux — **un saut
//!    de ligne**, qui y est un caractère de nom de fichier parfaitement licite.
//!    Un séparateur naïf couperait une entrée en deux. F1 mesure déjà un nom
//!    accentué avec espace (`éphémère été.txt`) ; le saut de ligne est le cas
//!    que personne n'essaie et que tout le monde casse.
//! 3. **La dernière ligne peut être TRONQUÉE, et [`Journal::relire`] la jette
//!    en la comptant.** Une ligne partielle est le seul dommage qu'un arrêt
//!    brutal puisse causer à un fichier en ajout — et lever plutôt que la jeter
//!    ferait perdre TOUTES les entrées antérieures, qui sont intactes.
//! 4. **Le compactage n'a lieu QUE sur un journal vide.** Tronquer un fichier
//!    qui porte encore une due perdrait la donnée **exactement quand elle
//!    sert**.
//!
//! ⚠️ **Ce module ne connaît ni `%LOCALAPPDATA%` ni ProjFS** : c'est
//! `pont::executer` — déjà `#[cfg(windows)]` — qui résout le chemin. Le
//! journal vit **hors de la racine** (`projfs/racine.rs`), pour la raison que
//! ce fichier-là écrit : un état qui vivrait DANS la racine serait lui-même un
//! objet projeté, donc dépendant du pont pour être lu — circulaire — et il
//! disparaîtrait avec la racine le jour où il faudrait la recréer, c'est-à-dire
//! **exactement le jour où il sert**.

/// Au-delà de cette taille, un journal **vide** est tronqué à zéro.
///
/// ⚠️ **NON CALIBRÉE.** Elle rejoint `DELAI_ECRIRE`, `TAILLE_TRAME_MAX`,
/// `DELAI_ATTRIBUTS`, `DELAI_LIRE`, `DELAI_LISTER`, `BPP_MIN`, `FACTEUR_FOCUS`,
/// `PART_DORMANTE_BPS`, `HYSTERESIS`, `REPIT_APRES_ECHEC`, `TAILLE_MAX_SORTIE`,
/// `REPIT_REARMEMENT_AUDIO` et `REARMEMENTS_MAX` dans la liste des constantes
/// de ce dépôt qu'aucune mesure n'a jugées.
pub const TAILLE_JOURNAL_COMPACTAGE: u64 = 256 * 1024;

/// L'ensemble des écritures dues, dans leur ordre d'inscription.
///
/// ⚠️ **Un `Vec` et non un `HashMap`, et ce n'est pas une commodité** : l'ordre
/// d'inscription est la seule chose qui rende la reprise déterministe, et un
/// `HashMap` en rendrait un différent à chaque exécution. Le coût est un
/// balayage linéaire par opération, sur un ensemble qui compte les écritures
/// **non encore acquittées** — quelques unités en régime nominal.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Journal {
    dues: Vec<(String, u64)>,
}

impl Journal {
    pub fn nouveau() -> Self {
        Self::default()
    }

    /// Relit un journal, en **TOLÉRANT une dernière ligne tronquée**.
    ///
    /// Rend le journal et le **nombre de lignes ignorées** : l'appelant les
    /// journalise, il ne les devine pas.
    ///
    /// 🔴 **Une ligne illisible est JETÉE, jamais fatale.** Lever ferait perdre
    /// toutes les entrées antérieures, qui sont pourtant intactes — et le
    /// journal existe précisément pour ne rien perdre.
    pub fn relire(contenu: &str) -> (Self, usize) {
        let mut journal = Self::nouveau();
        let mut ignorees = 0usize;
        for ligne in contenu.split('\n') {
            if ligne.is_empty() {
                // La coupe après le dernier `\n` : ce n'est pas une ligne.
                continue;
            }
            match analyser(ligne) {
                Some(Entree::Inscription { chemin, octets }) => journal.poser(&chemin, octets),
                Some(Entree::Retrait { chemin }) => journal.oter(&chemin),
                None => ignorees += 1,
            }
        }
        (journal, ignorees)
    }

    /// Inscrit une écriture due, et rend **la ligne à ajouter au fichier**.
    ///
    /// Un chemin déjà présent voit ses octets mis à jour **sans changer de
    /// place** : le rejeu d'une écriture n'est pas une écriture neuve, et le
    /// faire remonter en queue ferait passer devant lui des entrées plus
    /// jeunes.
    pub fn inscrire(&mut self, chemin: &str, octets: u64) -> String {
        self.poser(chemin, octets);
        format!("+{octets} {}\n", encoder(chemin))
    }

    /// Retire une écriture due, et rend **la ligne à ajouter au fichier**.
    ///
    /// ⚠️ **Le retrait est ÉCRIT même si le chemin était absent.** Le fichier
    /// est un journal d'événements, pas un état : y taire un retrait le rendrait
    /// dépendant de ce que la mémoire croit savoir.
    pub fn retirer(&mut self, chemin: &str) -> String {
        self.oter(chemin);
        format!("-{}\n", encoder(chemin))
    }

    /// Les écritures dues, **dans l'ordre d'inscription**.
    pub fn dues(&self) -> &[(String, u64)] {
        &self.dues
    }

    pub fn compte(&self) -> usize {
        self.dues.len()
    }

    pub fn est_vide(&self) -> bool {
        self.dues.is_empty()
    }

    /// Le journal peut-il être compacté ?
    ///
    /// 🔴 **`est_vide()` ET la taille, JAMAIS la taille seule.** Tronquer un
    /// fichier qui porte encore une due perdrait la donnée exactement quand
    /// elle sert.
    pub fn compactable(&self, taille_fichier: u64) -> bool {
        self.est_vide() && taille_fichier > TAILLE_JOURNAL_COMPACTAGE
    }

    fn poser(&mut self, chemin: &str, octets: u64) {
        match self.dues.iter_mut().find(|(c, _)| c == chemin) {
            Some((_, o)) => *o = octets,
            None => self.dues.push((chemin.to_string(), octets)),
        }
    }

    fn oter(&mut self, chemin: &str) {
        // 🔴 **ÉGALITÉ EXACTE, jamais un préfixe.** Retirer par préfixe ferait
        // que `note.txt` effacerait `note.txt.bak`, et qu'un dossier effacerait
        // tout ce qu'il contient.
        self.dues.retain(|(c, _)| c != chemin);
    }
}

enum Entree {
    Inscription { chemin: String, octets: u64 },
    Retrait { chemin: String },
}

fn encoder(chemin: &str) -> String {
    serde_json::to_string(chemin).expect("une chaîne se sérialise toujours en JSON")
}

fn decoder(brut: &str) -> Option<String> {
    serde_json::from_str::<String>(brut).ok()
}

fn analyser(ligne: &str) -> Option<Entree> {
    let (marque, reste) = ligne.split_at_checked(1)?;
    match marque {
        "+" => {
            // `+<octets> <chemin JSON>` : le premier espace sépare, et il ne
            // peut pas y en avoir dans un nombre.
            let (octets, chemin) = reste.split_once(' ')?;
            Some(Entree::Inscription {
                chemin: decoder(chemin)?,
                octets: octets.parse().ok()?,
            })
        }
        "-" => Some(Entree::Retrait { chemin: decoder(reste)? }),
        _ => None,
    }
}

#[cfg(test)]
mod tests;
