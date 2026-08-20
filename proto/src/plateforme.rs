//! Messages du canal plateforme <-> agent (`/agent`).
//!
//! Format JSON versionné : `{"type":"...","v":2,...}` (`type` sert de tag
//! interne à l'enum et est toujours émis en premier par serde). Le champ `v`
//! est obligatoire et vérifié à la désérialisation : un message sans `v`, ou
//! avec un `v` différent de [`PLATEFORME_VERSION`], est rejeté.
//!
//! ⚠️ CE MODULE EST DISTINCT DE [`crate::control`], ET CE N'EST PAS UN HASARD.
//! `control` versionne le canal de données **agent <-> navigateur** ; celui-ci
//! versionne le canal **agent <-> plateforme**. Les deux évoluent pour des
//! raisons sans rapport, et une constante partagée forcerait chacun à bouger
//! quand l'autre change — ce qui rendrait tout bump illisible.
//!
//! 🔴 CE QUI SE PASSE QUAND LES VERSIONS DIVERGENT N'EST PAS SYMÉTRIQUE, parce
//! que les deux bouts n'ont pas le même pouvoir :
//!   - l'agent envoie une version que la plateforme ne connaît pas : elle
//!     répond `{v, type:"refus", motif:"version"}`, journalise AVEC la version
//!     reçue, et ferme le socket. Aucune négociation à la baisse : il n'y a
//!     qu'une version ;
//!   - la plateforme envoie une version que l'agent ne connaît pas :
//!     `serde_json::from_str` échoue, l'agent journalise avec le texte de
//!     l'erreur et ferme le canal.
//!
//! 🔴 UNE DIVERGENCE DE VERSION NE DOIT JAMAIS SE LIRE COMME UNE PANNE RÉSEAU.
//! Un refus `version` NE SE RÉESSAIE PAS ; une chute de socket, si. Deux
//! comportements, deux traces distinctes — sans quoi une incompatibilité de
//! version se déguiserait en boucle de reconnexion infinie, qui est le mode de
//! panne le plus coûteux à diagnostiquer.

use serde::{Deserialize, Serialize};

/// Version du protocole du canal plateforme <-> agent. Incrémenter à tout
/// changement de format.
///
/// v1 (sous-bloc P3) : enrôlement, battement de cœur, jeton d'agent.
/// v2 (sous-bloc G1) : catalogue d'applications, ordre de lancement.
///
/// 🔴 LE PASSAGE À 2 REND PÉRIMÉ TOUT AGENT DÉJÀ DÉPLOYÉ, et c'est une
/// décision, pas un effet de bord. Un agent v1 reçoit `refus{motif:version}`
/// et NE SE RÉESSAIE PAS (en-tête de ce module) : agent et plateforme se
/// déploient AU MÊME COMMIT, sans quoi la VM se tait sans boucler, ce qui est
/// exactement le comportement voulu — un silence franc plutôt qu'une
/// reconnexion infinie.
pub const PLATEFORME_VERSION: u8 = 2;

// Note : pas de `default` sur le champ `v` — un message sans champ `v` doit être
// rejeté (champ obligatoire), pas silencieusement complété avec la version
// courante. `default` court-circuiterait `deserialize_with` quand le champ est
// absent, ce qui romprait la vérification.
fn verifie_version<'de, D>(deserializer: D) -> Result<u8, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let v = u8::deserialize(deserializer)?;
    if v != PLATEFORME_VERSION {
        return Err(serde::de::Error::custom(format!(
            "version de plateforme non supportée : {v}"
        )));
    }
    Ok(v)
}

/// Pourquoi la plateforme refuse.
///
/// ⚠️ `Enrolement` NE DISTINGUE PAS « VM inconnue » de « secret faux », et
/// c'est délibéré : les distinguer donnerait à quiconque ouvre le canal un
/// oracle d'énumération des VMs enrôlées. Le diagnostic vit dans le journal de
/// la plateforme, jamais sur le fil.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MotifCanal {
    /// La version du message reçu n'est pas [`PLATEFORME_VERSION`].
    /// 🔴 CELUI-CI NE SE RÉESSAIE PAS.
    Version,
    /// Le message n'a pas la forme attendue.
    Forme,
    /// L'enrôlement est refusé. Indistinct par construction (voir ci-dessus).
    Enrolement,
    /// Un `battement` est arrivé avant tout `enroler`.
    Sequence,
}

/// Une application telle que l'agent la découvre sur le disque de la VM.
///
/// ⚠️ `arguments` est BRUT et SENSIBLE À LA CASSE, contrairement à `cible` et
/// `repertoire` qui sont normalisés (casse repliée). C'est la spec D4 : deux
/// raccourcis qui ne diffèrent que par la casse d'un chemin Windows désignent
/// le même fichier, alors que deux lignes de commande qui ne diffèrent que par
/// la casse d'un argument sont deux invocations distinctes — les replier
/// fusionnerait `-Mode admin` et `-mode Admin`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Application {
    /// Empreinte du triplet `(cible, arguments, repertoire)` — l'identité.
    pub cle: String,
    /// Le nom du `.lnk`, sans son extension.
    pub nom: String,
    /// Le chemin du `.lnk` LUI-MÊME, et c'est lui qu'on lance (spec D6).
    pub chemin: String,
    /// Le chemin de la cible, normalisé.
    pub cible: String,
    /// Les arguments, BRUTS (voir ci-dessus). Vide = `""`, jamais absent.
    pub arguments: String,
    /// Le répertoire de travail, normalisé.
    pub repertoire: String,
}

/// Ce qu'un ordre de lancement a réellement fait.
///
/// 🔴 `Raccourci` CONTRE `Cible` EST CE QUI REND LE CRITÈRE DE RECETTE
/// DÉCIDABLE : lancer par la cible reconstruite au lieu du `.lnk` passerait un
/// critère qui ne dirait que « quelque chose s'est lancé ». Nommer le chemin
/// emprunté distingue les deux sans avoir à ruser.
///
/// ⚠️ CE N'EST PAS UN [`MotifCanal`], et le réemployer serait un défaut :
/// deux valeurs de `MotifCanal` FERMENT le socket, et un lancement raté ne
/// doit fermer aucun canal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum IssueLancement {
    /// Le `.lnk` lui-même a été exécuté. C'est le chemin nominal.
    Raccourci,
    /// Le `.lnk` a échoué (disparu, illisible) et la cible enregistrée a pris
    /// le relais.
    Cible,
    /// La clé n'est dans aucun catalogue de l'agent. Rendue par l'appelant,
    /// qui seul connaît le catalogue courant.
    Inconnue,
    /// Le raccourci ET la cible ont échoué. Les deux tentatives sont
    /// journalisées : une issue typée, jamais un silence.
    Echec,
}

/// Message de l'agent vers la plateforme.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case", deny_unknown_fields)]
pub enum VersLaPlateforme {
    /// S'enrôler : présenter le nom de VM et le secret d'enrôlement.
    Enroler {
        #[serde(rename = "v", deserialize_with = "verifie_version")]
        version: u8,
        vm: String,
        secret: String,
    },
    /// Battre le cœur. Fait avancer `vu_a`, et rend un jeton frais.
    Battement {
        #[serde(rename = "v", deserialize_with = "verifie_version")]
        version: u8,
    },
    /// Le catalogue d'applications de la VM, en DIFF.
    ///
    /// 🔴 `complet` A UNE SÉMANTIQUE NOMMÉE, et c'est la seule qui rende
    /// l'état de la plateforme reconstructible : à `true`, la plateforme
    /// marque disparue TOUTE ligne de cette VM absente d'`applications` et
    /// ignore `disparues` ; à `false`, elle applique le delta.
    ///
    /// L'agent émet `complet = true` à chaque (ré)enrôlement. C'est ce qui
    /// rend la perte d'un message montant sans conséquence : ce canal est un
    /// `push` WebSocket, sans garantie de livraison, et sans ce renvoi
    /// complet un `Catalogue` perdu pendant une coupure laisserait la
    /// plateforme divergente SANS TERME.
    Catalogue {
        #[serde(rename = "v", deserialize_with = "verifie_version")]
        version: u8,
        complet: bool,
        applications: Vec<Application>,
        /// Des CLÉS, jamais des objets : la plateforme n'a besoin que de
        /// l'identité pour marquer une disparition.
        disparues: Vec<String>,
    },
    /// L'issue d'un ordre de lancement, appariée par `demande`.
    Lancee {
        #[serde(rename = "v", deserialize_with = "verifie_version")]
        version: u8,
        demande: String,
        issue: IssueLancement,
    },
}

impl VersLaPlateforme {
    pub fn enroler(vm: impl Into<String>, secret: impl Into<String>) -> Self {
        Self::Enroler {
            version: PLATEFORME_VERSION,
            vm: vm.into(),
            secret: secret.into(),
        }
    }

    pub fn battement() -> Self {
        Self::Battement {
            version: PLATEFORME_VERSION,
        }
    }

    pub fn catalogue(complet: bool, applications: Vec<Application>, disparues: Vec<String>) -> Self {
        Self::Catalogue {
            version: PLATEFORME_VERSION,
            complet,
            applications,
            disparues,
        }
    }

    pub fn lancee(demande: impl Into<String>, issue: IssueLancement) -> Self {
        Self::Lancee {
            version: PLATEFORME_VERSION,
            demande: demande.into(),
            issue,
        }
    }
}

/// Message de la plateforme vers l'agent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case", deny_unknown_fields)]
pub enum DepuisLaPlateforme {
    /// L'enrôlement est accepté : voici le préfixe de session et le jeton.
    Enrole {
        #[serde(rename = "v", deserialize_with = "verifie_version")]
        version: u8,
        prefixe: String,
        jeton: String,
        /// En MILLISECONDES, comme tout horodatage de ce service.
        expire_a: i64,
    },
    /// Le battement est enregistré : voici un jeton FRAIS.
    ///
    /// ⚠️ Un jeton frais À CHAQUE battement, et non le même : le jeton d'accès
    /// dure dix minutes, et un agent qui garderait le premier tomberait à son
    /// expiration sans le voir venir.
    BattementRecu {
        #[serde(rename = "v", deserialize_with = "verifie_version")]
        version: u8,
        jeton: String,
        expire_a: i64,
    },
    /// Refus, avec son motif. Le socket se ferme ensuite.
    Refus {
        #[serde(rename = "v", deserialize_with = "verifie_version")]
        version: u8,
        motif: MotifCanal,
    },
    /// Lancer une application de la VM.
    ///
    /// ⚠️ L'ORDRE NE PORTE PAS LE CHEMIN DU RACCOURCI, il porte la clé, et
    /// l'agent la résout dans SON PROPRE catalogue — celui qu'il vient de
    /// lire sur le disque. La copie de la plateforme peut être vieille d'une
    /// réconciliation ; celle de l'agent ne l'est jamais.
    ///
    /// `demande` apparie l'ordre à sa [`VersLaPlateforme::Lancee`], la route
    /// HTTP attendant cette réponse.
    Lancer {
        #[serde(rename = "v", deserialize_with = "verifie_version")]
        version: u8,
        demande: String,
        cle: String,
    },
}

impl DepuisLaPlateforme {
    pub fn enrole(prefixe: impl Into<String>, jeton: impl Into<String>, expire_a: i64) -> Self {
        Self::Enrole {
            version: PLATEFORME_VERSION,
            prefixe: prefixe.into(),
            jeton: jeton.into(),
            expire_a,
        }
    }

    pub fn battement_recu(jeton: impl Into<String>, expire_a: i64) -> Self {
        Self::BattementRecu {
            version: PLATEFORME_VERSION,
            jeton: jeton.into(),
            expire_a,
        }
    }

    pub fn refus(motif: MotifCanal) -> Self {
        Self::Refus {
            version: PLATEFORME_VERSION,
            motif,
        }
    }

    pub fn lancer(demande: impl Into<String>, cle: impl Into<String>) -> Self {
        Self::Lancer {
            version: PLATEFORME_VERSION,
            demande: demande.into(),
            cle: cle.into(),
        }
    }
}

#[cfg(test)]
#[path = "plateforme/tests.rs"]
mod tests;
