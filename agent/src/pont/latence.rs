//! L'histogramme des traversées du pont, **par famille de commande**. **PUR** —
//! aucun `cfg`, aucune E/S, **et aucune horloge lue en interne** : la durée est
//! un paramètre, exactement comme dans [`crate::pont::table`], « ce qui rend
//! l'expiration testable sans dormir ».
//!
//! # 🔴 CE QUE CE MODULE N'EST PAS
//!
//! **Ce n'est pas une trace par commande.** *Compter ou échantillonner, jamais
//! tracer par unité* — la trace par paquet du chantier TURN a tué la session
//! qu'elle mesurait avec 18 619 lignes en quelques secondes, écrites sur un
//! partage CIFS depuis la boucle. C'est la même raison qui a fait de
//! [`crate::pont::compteurs`] un compteur plutôt qu'une trace par échec.
//!
//! **Ce n'est pas une mesure de ce que l'APPLICATION attend.** Il mesure la
//! traversée **pont → navigateur → pont**, et rien d'autre : ni l'entrée dans
//! le rappel ProjFS, ni l'inscription en table, ni le balayage à
//! `PERIODE_BALAYAGE`, ni `PrjCompleteCommand`, ni le retour de ProjFS à
//! l'application. **Le nom des choses le dit** — `traversees`, jamais
//! `latences`. Ce que l'application attend est mesuré par un chronomètre DANS
//! la VM, et la différence entre les deux est un **résidu nommé, jamais une
//! grandeur mesurée** (plan F4, §0.5).
//!
//! **Ce n'est pas un `Mutex`.** Les compteurs sont des `AtomicU64`, comme ceux
//! de [`crate::pont::compteurs`] et **pour la même raison** : ils sont touchés
//! depuis les fils de rappel que le SYSTÈME possède, où attendre un verrou
//! ferait attendre l'application.
//!
//! # La famille est le BUDGET, pas le verbe
//!
//! Les cinq familles recouvrent **exactement** les cinq budgets de
//! [`crate::pont::table`], et c'est ce qui rend le recensement lisible contre
//! eux : `Creer` partage `DELAI_ECRIRE` avec `Ecrire` (toutes deux inscrites
//! par `ecriture::fil`), `Muter` a `DELAI_MUTATION` pour lui seul. Grouper par
//! verbe au lieu de grouper par budget produirait une distribution qu'aucune
//! constante n'encadre.
//!
//! # Le garde structurel, à trois étages
//!
//! Le jumeau de celui de [`crate::pont::erreurs`] et [`crate::pont::compteurs`] :
//! [`NOMBRE`] force l'inscription dans [`Famille::TOUTES`], [`nom`] est un
//! `match` **exhaustif** — une famille neuve ne peut pas hériter du nom d'une
//! autre —, et [`Famille::de`] est un second `match` exhaustif qui force à
//! classer toute variante d'[`Attendue`] qui apparaîtrait.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use crate::pont::table::Attendue;

/// Une famille de commande, telle qu'on la lit au recensement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Famille {
    Attributs,
    Lister,
    Lire,
    Ecrire,
    Mutation,
}

/// ⚠️ **Porter une famille de plus impose de porter `NOMBRE`**, ce qui fait
/// échouer la compilation de [`Famille::TOUTES`], typé `[Famille; NOMBRE]`,
/// tant que la variante neuve n'y figure pas.
pub const NOMBRE: usize = 5;

impl Famille {
    pub const TOUTES: [Famille; NOMBRE] =
        [Famille::Attributs, Famille::Lister, Famille::Lire, Famille::Ecrire, Famille::Mutation];

    /// La famille d'une commande en vol. `match` **exhaustif** : une variante
    /// neuve d'[`Attendue`] ne compile pas tant qu'elle n'est pas classée.
    pub fn de(attendue: &Attendue) -> Famille {
        match attendue {
            Attendue::Attributs { .. } => Famille::Attributs,
            Attendue::Lister { .. } => Famille::Lister,
            Attendue::Lire { .. } => Famille::Lire,
            // ⚠️ `Creer` EST de la famille `Ecrire` : les deux sont inscrites
            // par `ecriture::fil` sous le même `DELAI_ECRIRE`.
            Attendue::Ecrire { .. } | Attendue::Creer { .. } => Famille::Ecrire,
            Attendue::Muter { .. } => Famille::Mutation,
        }
    }
}

/// Le nom d'une famille **sur la ligne de recensement**.
///
/// ⚠️ **Écrit à chaque champ, jamais déduit d'un rang** : un `grep` de recette
/// lit un nom, et un recensement dont l'ordre dériverait ferait sinon lire un
/// compteur pour un autre.
pub fn nom(f: Famille) -> &'static str {
    match f {
        Famille::Attributs => "attributs",
        Famille::Lister => "lister",
        Famille::Lire => "lire",
        Famille::Ecrire => "ecrire",
        Famille::Mutation => "mutation",
    }
}

/// Les bornes SUPÉRIEURES des seaux, en millisecondes. Un treizième seau
/// implicite, `inf`, recueille tout ce qui les dépasse.
///
/// **Choisies pour couvrir les cinq budgets** (2 s, 5 s, 15 s, 20 s, 30 s) et
/// le RTT du pont (1 à 3 ms mesurés en D1). ⚠️ **NON CALIBRÉES** — elles
/// rejoignent `TAILLE_TRAME_MAX`, `SEUIL_TAMPON`, `MORCEAUX_EN_VOL`, `BPP_MIN`
/// et tout ce que ce dépôt n'a jamais jugé à l'usage.
pub const SEAUX_MS: [u64; 12] = [1, 2, 5, 10, 20, 50, 100, 200, 500, 1_000, 2_000, 5_000];

/// Douze bornes, plus `inf`.
pub const SEAUX: usize = SEAUX_MS.len() + 1;

/// Le rang du seau qui **contient** `duree`.
///
/// 🔴 **La borne est INCLUSIVE en haut** : une traversée de 5 ms exactement
/// tombe dans le seau `5`, jamais dans le seau `10`. Un `<` au lieu d'un `<=`
/// décalerait toute la distribution d'un seau, en silence.
fn seau_de(duree: Duration) -> usize {
    let us = duree.as_micros();
    SEAUX_MS.iter().position(|ms| us <= u128::from(*ms) * 1_000).unwrap_or(SEAUX_MS.len())
}

/// Compte, somme, maximum et distribution, par famille.
#[derive(Debug, Default)]
pub struct Histogramme {
    compte: [AtomicU64; NOMBRE],
    somme_us: [AtomicU64; NOMBRE],
    max_us: [AtomicU64; NOMBRE],
    seaux: [[AtomicU64; SEAUX]; NOMBRE],
}

impl Histogramme {
    pub fn nouveau() -> Self {
        Self::default()
    }

    /// Enregistre une traversée **ACHEVÉE**. `duree` est calculée par
    /// l'appelant, depuis SON horloge.
    ///
    /// ⚠️ **Une traversée qui n'aboutit pas n'est PAS observée** : une commande
    /// expirée ou annulée ne passe jamais par `Table::resoudre`. Le
    /// recensement mesure donc ce qui a **abouti**, et les échecs se lisent
    /// sur la ligne des douze codes — les deux se lisent ensemble, jamais l'une
    /// pour l'autre.
    pub fn observer(&self, famille: Famille, duree: Duration) {
        let r = rang(famille);
        let us = u64::try_from(duree.as_micros()).unwrap_or(u64::MAX);
        self.compte[r].fetch_add(1, Ordering::Relaxed);
        self.somme_us[r].fetch_add(us, Ordering::Relaxed);
        self.max_us[r].fetch_max(us, Ordering::Relaxed);
        self.seaux[r][seau_de(duree)].fetch_add(1, Ordering::Relaxed);
    }

    pub fn compte(&self, f: Famille) -> u64 {
        self.compte[rang(f)].load(Ordering::Relaxed)
    }

    pub fn max_us(&self, f: Famille) -> u64 {
        self.max_us[rang(f)].load(Ordering::Relaxed)
    }

    /// La moyenne, **zéro quand rien n'a été observé** — et non une division
    /// par zéro.
    pub fn moyenne_us(&self, f: Famille) -> u64 {
        let n = self.compte(f);
        if n == 0 {
            return 0;
        }
        self.somme_us[rang(f)].load(Ordering::Relaxed) / n
    }

    pub fn seau(&self, f: Famille, rang_seau: usize) -> u64 {
        self.seaux[rang(f)][rang_seau].load(Ordering::Relaxed)
    }

    /// La ligne de recensement, **dans l'ordre de [`Famille::TOUTES`]**, en
    /// **chaîne unique** `nom=valeur`.
    ///
    /// ⚠️ **Jamais en champs `tracing`** : ceux-ci porteraient des séquences
    /// ANSI entre le nom et la valeur sur un journal BRUT — le piège que la
    /// recette d'entrée de D8 a payé et que le `grep` de F1 a rejoué trois
    /// fois. Elle se lit **sans `sed`**.
    ///
    /// ⚠️ **Les compteurs sont CUMULATIFS depuis le démarrage du pont** : une
    /// mesure se lit par DIFFÉRENCE entre deux recensements, jamais sur une
    /// ligne isolée.
    ///
    /// ⚠️ **Les seaux sont émis pour les CINQ familles**, et non pour la seule
    /// `lire` : n'en émettre qu'une ferait de ce choix une décision cachée, et
    /// un successeur qui mesurerait `lister` n'y trouverait aucune
    /// distribution. Divergence E13 du plan, déclarée.
    pub fn recensement(&self) -> String {
        let mut ligne = String::from("traversees");
        for f in Famille::TOUTES {
            ligne.push_str(&format!(
                " {}=n:{} moy_us:{} max_us:{}",
                nom(f),
                self.compte(f),
                self.moyenne_us(f),
                self.max_us(f)
            ));
        }
        ligne.push_str(" | seaux_ms");
        for f in Famille::TOUTES {
            ligne.push_str(&format!(" {}=", nom(f)));
            for (i, borne) in SEAUX_MS.iter().enumerate() {
                ligne.push_str(&format!("{}:{},", borne, self.seau(f, i)));
            }
            ligne.push_str(&format!("inf:{}", self.seau(f, SEAUX_MS.len())));
        }
        ligne
    }
}

/// Le rang d'une famille dans [`Famille::TOUTES`].
///
/// ⚠️ **Dérivé de `TOUTES` et non écrit à la main**, comme
/// `compteurs::rang` : deux vérités que rien ne confronte divergeraient en
/// silence.
fn rang(f: Famille) -> usize {
    Famille::TOUTES
        .iter()
        .position(|c| *c == f)
        .expect("toute famille figure dans TOUTES — NOMBRE l'impose")
}

#[cfg(test)]
mod tests;
