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
//!
//! ⚠️ **CE PARAGRAPHE A ÉTÉ RÉFUTÉ PAR LA MESURE, ET IL EST REDEVENU VRAI PAR
//! LA CORRECTION DU 20 AOÛT 2026.** Le relevé qui l'a réfuté (recette du
//! sous-bloc G1, UNE exécution, journal
//! `docs/superpowers/plans/journaux-gestion-apps/step3-version-v1-contre-v2-plat.log`) :
//! un agent v1 opposé à une plateforme v2 a journalisé **0** ligne « la
//! plateforme REFUSE la version » et **10** couples « message de la plateforme
//! illisible (version divergente ?) » / « reprise du canal /agent », jusqu'au
//! palier de 30 s, sans terme.
//!
//! **La cause était dans ce fichier**, et elle était structurelle :
//! `verifie_version` est un `deserialize_with` posé sur le champ `v` de
//! **tout** message, et la plateforme émet son refus avec SA version —
//! `{"type":"refus","v":2,"motif":"version"}`. Un agent de version N ne
//! pouvait donc JAMAIS LIRE le refus d'une plateforme de version M ≠ N : il
//! tombait dans la branche « illisible », qui est reprenable, et le bras
//! `version` de `sur_refus` n'était atteignable que si les deux bouts
//! s'accordaient déjà sur `v` — c'est-à-dire jamais dans le seul cas pour
//! lequel il existe.
//!
//! 🔴 **LA DÉCISION DE PROTOCOLE, ET SON PRIX.** Le refus n'est plus un message
//! versionné comme les autres : c'est une **ENVELOPPE MINIMALE HORS
//! VERSIONNEMENT**, et cela se lit en trois clauses.
//!
//!   1. **Son champ `v` est TOLÉRÉ, jamais vérifié** (`version_toleree`). Il
//!      reste OBLIGATOIRE et reste un entier — il dit qui parle, et c'est
//!      journalisé — mais aucune valeur ne le fait rejeter. Un refus est le
//!      seul message dont le sens ne dépend d'aucune version : il dit « je ne
//!      te servirai pas », et cela se comprend sans négociation.
//!   2. **Son champ `motif` est un MOT LIBRE sur le fil** (`String`), pas un
//!      enum fermé. Sans cette seconde clause le remède ne tiendrait que
//!      jusqu'au premier motif ajouté par une version future : le refus
//!      redeviendrait illisible, dans la branche « illisible », et le mode de
//!      panne reviendrait à l'identique. La table des motifs connus vit dans
//!      [`MotifCanal::depuis_mot`] ; ce qu'elle ne reconnaît pas est
//!      journalisé **verbatim** plutôt que perdu.
//!   3. 🔴 **SA FORME EST GELÉE : `type`, `v`, `motif`, ET RIEN D'AUTRE,
//!      JAMAIS.** Cet enum porte `deny_unknown_fields` ; un champ ajouté au
//!      refus par une version future serait rejeté par les versions
//!      antérieures, et rendrait à lui seul les clauses 1 et 2 sans effet.
//!      C'est le prix de la décision, et il est écrit ici parce que rien dans
//!      le type ne l'empêche.
//!
//! **Ce qui n'a PAS changé, et ne doit pas changer** : tous les autres
//! messages restent strictement versionnés. Un `enrole` d'une version inconnue
//! peut donner à un champ connu un sens que nous ignorons ; l'accepter serait
//! pire que le rejeter. Deux tests gardent chaque moitié, sur chacun des deux
//! bouts.

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
///
/// ❌ **« LA VM SE TAIT SANS BOUCLER » EST FAUX, MESURÉ** — voir l'encadré de
/// l'en-tête de ce module. La VM boucle, à 30 s d'intervalle et sans terme.
/// L'obligation de déployer les deux bouts au même commit, elle, est
/// INCHANGÉE et même renforcée : c'est la seule parade qui existe aujourd'hui.
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

/// Lit le champ `v` d'un REFUS **sans le vérifier** — voir la clause 1 de
/// l'en-tête de ce module.
///
/// 🔴 CE N'EST PAS « SANS `v` » : le champ reste obligatoire et reste un
/// entier. Le rendre facultatif rouvrirait le trou que
/// [`verifie_version`] refuse — un `v: null`, ou un `v` absent, deviendrait
/// acceptable — et priverait le journal de la seule information qui dise
/// QUELLE version nous refuse.
fn version_toleree<'de, D>(deserializer: D) -> Result<u8, D::Error>
where
    D: serde::Deserializer<'de>,
{
    u8::deserialize(deserializer)
}

/// Pourquoi la plateforme refuse.
///
/// ⚠️ `Enrolement` NE DISTINGUE PAS « VM inconnue » de « secret faux », et
/// c'est délibéré : les distinguer donnerait à quiconque ouvre le canal un
/// oracle d'énumération des VMs enrôlées. Le diagnostic vit dans le journal de
/// la plateforme, jamais sur le fil.
/// ⚠️ **PLUS DE `Serialize`/`Deserialize` DEPUIS LA CORRECTION DU 20 AOÛT
/// 2026, ET C'EST DÉLIBÉRÉ.** Le motif voyage en MOT LIBRE dans
/// [`DepuisLaPlateforme::Refus`] (clause 2 de l'en-tête) : cet enum n'est plus
/// une forme de fil, c'est la table des motifs que NOUS savons interpréter.
/// La correspondance mot ↔ variante est écrite une seule fois, dans
/// [`MotifCanal::mot`] et [`MotifCanal::depuis_mot`], et un test la parcourt
/// dans les deux sens sur les quatre variantes — ce qu'un `rename_all` ne
/// permettait pas de faire rougir tant qu'aucune variante n'a deux mots.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

impl MotifCanal {
    /// Les quatre variantes, dans l'ordre où un test les parcourt.
    ///
    /// 🔴 ANTI-OUBLI : une variante ajoutée sans sa ligne ici serait absente
    /// du test de correspondance, qui compare cette liste à un `match`
    /// EXHAUSTIF — le compilateur exige la branche, et le test exige l'entrée.
    pub const TOUS: [Self; 4] =
        [Self::Version, Self::Forme, Self::Enrolement, Self::Sequence];

    /// Le mot exact qui voyage sur le fil.
    pub fn mot(self) -> &'static str {
        match self {
            Self::Version => "version",
            Self::Forme => "forme",
            Self::Enrolement => "enrolement",
            Self::Sequence => "sequence",
        }
    }

    /// Le motif que ce mot désigne, ou `None` si nous ne le connaissons pas.
    ///
    /// 🔴 `None` N'EST PAS UNE ERREUR : c'est un motif d'une version qui nous
    /// dépasse, et l'appelant doit le journaliser tel quel plutôt que de le
    /// perdre. C'est la clause 2 de l'en-tête de ce module.
    pub fn depuis_mot(mot: &str) -> Option<Self> {
        Self::TOUS.into_iter().find(|candidat| candidat.mot() == mot)
    }
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
///
/// ⚠️ **LACUNE DE COUVERTURE, INSCRITE PLUTÔT QUE SUBIE (recette G1, 20 août
/// 2026).** Le `rename_all` ci-dessous est INOBSERVABLE sur cet enum : ses
/// quatre variantes sont d'UN SEUL MOT, donc `kebab-case`, `snake_case`,
/// `lowercase` et `camelCase` produisent tous les quatre mêmes chaînes.
/// **Aucun test ne peut donc rougir si la convention de nommage change ici**
/// — vérifié par mutation : remplacer `kebab-case` par `snake_case` sur cet
/// enum laisse `cargo test -p proto` à **75 passed, 0 failed**.
///
/// **Le contraste est mesuré sur le même fichier** : la même mutation appliquée
/// à l'enum qui porte `BattementRecu` — deux mots, donc `battement-recu` contre
/// `battement_recu` — fait ÉCHOUER `conformite_aux_vecteurs_partages`. La
/// protection existe donc bel et bien pour les variantes composées, et pas pour
/// celles-ci.
///
/// **Conséquence pratique** : la première variante d'`IssueLancement` écrite en
/// deux mots refermera la lacune d'elle-même, et jusque-là toute modification de
/// cette ligne doit être relue à la main.
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
    ///
    /// 🔴 **LA SEULE VARIANTE HORS VERSIONNEMENT DE TOUT CE PROTOCOLE**, et
    /// les trois clauses qui la gouvernent sont en tête de module. En deux
    /// mots : `v` est toléré, `motif` est un mot libre, et **la forme est
    /// gelée — aucun champ ne doit jamais s'y ajouter**.
    Refus {
        /// La version de l'ÉMETTEUR, telle qu'elle arrive. Peut différer de
        /// [`PLATEFORME_VERSION`] : c'est même le seul cas pour lequel cette
        /// variante existe.
        #[serde(rename = "v", deserialize_with = "version_toleree")]
        version: u8,
        /// Le mot brut. [`MotifCanal::depuis_mot`] l'interprète quand elle le
        /// peut ; l'appelant journalise le mot lui-même quand elle ne le peut
        /// pas.
        motif: String,
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

    /// ⚠️ PREND LA VARIANTE TYPÉE, ET NON UN MOT : c'est ce qui garantit que la
    /// plateforme ne peut pas mettre sur le fil un motif que sa propre table
    /// ne connaît pas. La tolérance de la clause 2 est une tolérance de
    /// LECTURE ; en écriture, rien n'est libre.
    pub fn refus(motif: MotifCanal) -> Self {
        Self::Refus {
            version: PLATEFORME_VERSION,
            motif: motif.mot().to_string(),
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

// 🔴 LE SECOND FICHIER DE TESTS EST NÉ D'UNE DETTE INSCRITE, PAS D'UN GOÛT.
// `plateforme/tests.rs` était à 561 lignes — au-dessus du plafond de 500 de
// `CLAUDE.md`, qui l'inscrivait au tableau de dette SANS point de chute. Le
// sous-bloc G2 travaille dedans, donc il l'a découpé : cycle de vie ici,
// gestion d'apps là. C'est le même mécanisme `#[path]` que la ligne ci-dessus,
// employé pour la même raison — la règle des 500 lignes —, et NON la
// « Convention de module enfant » de `CLAUDE.md`, qui vise les modules extraits
// d'un parent `#[cfg(windows)]`.
#[cfg(test)]
#[path = "plateforme/tests_apps.rs"]
mod tests_apps;
