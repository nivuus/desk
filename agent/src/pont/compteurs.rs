//! Le compteur des douze causes d'échec, et **l'UNIQUE point où une cause
//! devient un `HRESULT`**. **PUR** — aucun `cfg`, aucune horloge, aucune E/S.
//!
//! # Ce que ce module livre, et ce qu'il ne livre PAS
//!
//! La table des douze `HRESULT` **existe depuis F1** ([`crate::pont::erreurs`],
//! douze variantes, douze codes, `NOMBRE = 12` et un garde structurel à deux
//! étages). La spec §8 F3 écrit pourtant que F3 « livre la table des HRESULT du
//! §5 dans son intégralité » : **c'est périmé, et le plan de F3 le relève**.
//! Ce que F3 livre réellement est le critère (4) — *chacun des douze est
//! observé au moins une fois* —, et c'est ce module qui le rend décidable.
//!
//! # 🔴 POURQUOI UN COMPTEUR, ET NON UNE TRACE PAR ÉCHEC
//!
//! La seule trace qui nommait la cause d'un refus était un `debug!`
//! (`pont::service`), alors que `scripts/run-agent.sh` pose `RUST_LOG=info` par
//! défaut et que la doctrine de ce dépôt est que l'exploitation tourne en
//! `info` : **sur une recette ordinaire, aucune cause n'était observable**, et
//! le critère (4) était donc INSATISFIABLE.
//!
//! Monter tout le pont en `debug` inonderait le journal d'une ligne par rappel
//! — c'est le piège « ne jamais tracer par paquet » que le chantier TURN a payé
//! 18 619 lignes en quelques secondes, écrites sur un partage CIFS depuis la
//! boucle : **la mesure détruisait ce qu'elle mesurait**. On compte, et on
//! recense une fois par période.
//!
//! # Le troisième étage du garde structurel
//!
//! [`crate::pont::erreurs`] en porte deux : un `match` exhaustif force à
//! classer toute variante neuve, et `NOMBRE` force à l'inscrire dans `TOUTES`.
//! [`nom`] en est un **troisième** : sans un nom, une variante neuve
//! n'apparaîtrait pas au recensement, et le critère (4) la déclarerait tenue
//! sans l'avoir jamais vue.

use std::sync::atomic::{AtomicU64, Ordering};

use crate::pont::erreurs::{hresult, Erreur, NOMBRE};

/// Le nom d'une cause **sur la ligne de recensement**.
///
/// ⚠️ **Kebab-case, comme `proto::fichiers::CodeEchec` sur le fil**, et pour la
/// même raison : c'est ce qu'un `grep` de recette écrira. Le `match` est
/// **exhaustif** — une variante neuve ne peut pas hériter du nom d'une autre.
///
/// ⚠️ **CE N'EST PAS `CodeEchec`, et les deux ne se recouvrent pas.**
/// `CodeEchec` est ce que le NAVIGATEUR dit ; `Erreur` est ce que le PONT rend
/// à Windows. `casse-ambigue` existe côté navigateur et pas ici ;
/// `delai-depasse` existe ici et pas là-bas. Les confondre ferait chercher au
/// recensement un nom qui n'y sera jamais.
pub fn nom(e: Erreur) -> &'static str {
    match e {
        Erreur::Introuvable => "introuvable",
        Erreur::CheminIntrouvable => "chemin-introuvable",
        Erreur::AccesRefuse => "acces-refuse",
        Erreur::CanalFerme => "canal-ferme",
        Erreur::DelaiDepasse => "delai-depasse",
        Erreur::Abandonnee => "abandonnee",
        Erreur::DisquePlein => "disque-plein",
        Erreur::NonSupporte => "non-supporte",
        Erreur::RepertoireNonVide => "repertoire-non-vide",
        Erreur::DejaPresent => "deja-present",
        Erreur::ProtegeEnEcriture => "protege-en-ecriture",
        Erreur::Inattendue => "inattendue",
    }
}

/// Un compteur par variante d'[`Erreur`].
///
/// ⚠️ **Des `AtomicU64` et non un champ nu sous verrou** : les compteurs sont
/// incrémentés depuis **les fils de rappel que le système possède**, où la
/// discipline de fil interdit d'attendre un verrou. Un `Mutex` y ferait
/// attendre l'application qui lit le fichier.
#[derive(Debug, Default)]
pub struct Compteurs {
    cases: [AtomicU64; NOMBRE],
}

impl Compteurs {
    pub fn nouveaux() -> Self {
        Self::default()
    }

    /// 🔴 **LE SEUL POINT OÙ UNE CAUSE DEVIENT UN `HRESULT`.**
    ///
    /// L'invariant que F3 livre est qu'`erreurs::hresult` n'a plus qu'UN
    /// appelant hors tests : celui-ci. Le contrôle qui l'établit vit dans le
    /// journal de la tâche 5, et il a été vu ROUGE **avant** que la tâche ne
    /// commence — 26 lignes de code appelaient `hresult` directement.
    ///
    /// ⚠️ **Compter ET traduire dans le même appel est délibéré.** Deux
    /// fonctions — l'une qui compte, l'autre qui traduit — se laisseraient
    /// séparer par un appelant pressé, et le compteur cesserait de compter sans
    /// que rien ne le dise. Ici, obtenir le code EST l'incrémenter.
    pub fn rendre(&self, cause: Erreur) -> i32 {
        self.cases[rang(cause)].fetch_add(1, Ordering::Relaxed);
        hresult(cause)
    }

    /// Le compte d'une cause.
    pub fn compte(&self, cause: Erreur) -> u64 {
        self.cases[rang(cause)].load(Ordering::Relaxed)
    }

    pub fn total(&self) -> u64 {
        Erreur::TOUTES.iter().map(|e| self.compte(*e)).sum()
    }

    /// Les causes encore à **zéro** — c'est-à-dire ce qui manque au critère (4).
    ///
    /// 🔴 **C'est le rouge le plus important de ce module** : un `manquants()`
    /// toujours vide ferait déclarer le critère (4) TENU sur une exécution où
    /// rien n'a été exercé. Le test `manquants_rend_exactement_les_causes_a_zero`
    /// l'attrape.
    pub fn manquants(&self) -> Vec<Erreur> {
        Erreur::TOUTES.into_iter().filter(|e| self.compte(*e) == 0).collect()
    }

    /// La ligne de recensement, **dans l'ordre d'`Erreur::TOUTES`**.
    ///
    /// ⚠️ **L'ordre est épinglé par un test.** Un recensement dont l'ordre
    /// dériverait ferait lire un compteur pour un autre — c'est le piège des
    /// « deux messages qui partagent une sous-chaîne » sous une autre forme.
    /// Les noms sont écrits en toutes lettres à chaque champ, donc un `grep`
    /// de recette lit un nom et jamais un rang.
    pub fn recensement(&self) -> String {
        let mut ligne = format!("total={}", self.total());
        for e in Erreur::TOUTES {
            ligne.push_str(&format!(" {}={}", nom(e), self.compte(e)));
        }
        ligne
    }
}

/// Le rang d'une variante dans [`Erreur::TOUTES`].
///
/// ⚠️ **Dérivé de `TOUTES` et non écrit à la main.** `erreurs::index` existe
/// déjà et fait autorité, mais il est **privé** — et le dupliquer ici ferait
/// deux vérités que rien ne confronte, exactement le défaut de `TYPES_AGENT`
/// (`proto/ts/control.ts`), liste écrite à la main que rien ne compare à
/// l'union qu'elle reflète. Chercher dans `TOUTES` est en O(12) sur un chemin
/// d'échec : le coût est nul, et l'unicité de la source ne l'est pas.
fn rang(e: Erreur) -> usize {
    Erreur::TOUTES
        .iter()
        .position(|c| *c == e)
        .expect("toute variante d'Erreur figure dans TOUTES — NOMBRE l'impose")
}

#[cfg(test)]
mod tests;
