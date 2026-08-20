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
/// v3 (sous-bloc G2) : icônes 256, leur provenance, et l'inventaire des
///                     manquantes.
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
///
/// ✅ **CET ENCADRÉ EST UN RELEVÉ DATÉ (recette G1), ET IL A ÉTÉ RÉFUTÉ LE
/// 20 AOÛT 2026 — il est ANNOTÉ plutôt qu'effacé.** La correction du même
/// jour (commit `457a7f8`, les trois clauses en tête de ce module) a sorti le
/// refus du versionnement : un agent périmé LIT désormais le refus qui lui
/// apprend qu'il l'est, le journalise avec les deux versions, et RENONCE. Il
/// ne boucle plus. **La rupture reste une rupture ; elle est seulement
/// devenue DIAGNOSTICABLE**, et le passage à 3 du sous-bloc G2 est le premier
/// bump depuis cette correction — donc le premier à pouvoir le PROUVER.
/// L'obligation de déployer les deux bouts au même commit est, elle,
/// strictement inchangée.
pub const PLATEFORME_VERSION: u8 = 3;

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

/// Lit une `Option<String>` en la gardant **OBLIGATOIRE sur le fil**.
///
/// 🔴 SANS CETTE FONCTION, LE CHAMP SERAIT SILENCIEUSEMENT FACULTATIF, ET LA
/// PLANIFICATION DE G2 SE TROMPAIT SUR CE POINT PRÉCIS. `serde_derive` traite
/// tout champ de type `Option<T>` comme portant un `#[serde(default)]`
/// IMPLICITE : un champ absent devient `None` sans qu'aucun `default` n'ait
/// été écrit, et `deny_unknown_fields` n'y change rien — il regarde les champs
/// EN TROP, jamais ceux qui manquent.
///
/// **Mesuré le 20 août 2026**, deux structures identiques à ce détail près,
/// `deny_unknown_fields` sur les deux :
/// ```text
/// Option<String> nue                          -> `{"a":1}` ACCEPTÉ
/// Option<String> + deserialize_with           -> `{"a":1}` REFUSÉ
/// Option<String> + deserialize_with, `b:null` -> Ok(None), et sérialise `"b":null`
/// ```
/// La seule chose que `deserialize_with` change est donc l'implicite : il
/// coupe le défaut, et le champ redevient exigé.
///
/// **Ce qui serait perdu sans elle** : le catalogue d'un agent v2 — six champs,
/// sans `icone` — serait accepté par une plateforme v3, avec une icône
/// silencieusement absente. C'est exactement le déguisement que le bump de
/// version existe pour empêcher, et la règle que ce module s'impose déjà pour
/// le champ `v` : **un champ absent se refuse, il ne se complète pas**.
fn icone_obligatoire<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Option::<String>::deserialize(deserializer)
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

/// Les types de la GESTION D'APPLICATIONS vivent dans un module enfant.
///
/// 🔴 EXTRAITS PARCE QUE CE FICHIER A FRANCHI 500 LIGNES — 588 —, et la
/// doctrine de `CLAUDE.md` est de rattraper par une EXTRACTION, jamais par une
/// compression. ⚠️ **Elle aurait dû PRÉCÉDER l'addition** : le plan de G2 avait
/// nommé trois extractions à jouer d'avance, les trois ont été jouées, et
/// celle-ci n'était pas prévue. Le franchissement est DÉCLARÉ.
///
/// ⚠️ Ce n'est PAS la « Convention de module enfant » de `CLAUDE.md`, qui vise
/// les modules extraits d'un parent `#[cfg(windows)]` : c'est le même mécanisme
/// employé pour l'autre raison — la règle des 500 lignes.
mod apps;
pub use apps::{Application, IssueLancement, SourceMax};

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
    /// Les empreintes que la plateforme n'a PAS, parmi celles que le dernier
    /// [`VersLaPlateforme::Catalogue`] a annoncées.
    ///
    /// 🔴 ELLE N'EST PAS ÉMISE QUAND L'ENSEMBLE EST VIDE : une liste vide
    /// coûterait un message par réconciliation sur un disque au repos, ce que
    /// le diff du sous-bloc G1 existe précisément pour éviter.
    ///
    /// ⚠️ **LES OCTETS NE L'EMPRUNTENT JAMAIS** : ce message ne porte qu'un
    /// inventaire. Les images passent par `PUT /icone/:sha256`, exactement
    /// comme les installeurs de G3 (spec D7) — le canal est en JSON, il porte
    /// le battement de cœur, et 4,4 Mo en base64 y coûteraient +33 % et
    /// bloqueraient ce battement.
    ///
    /// ⚠️ **Un `IconesManquantes` perdu ne casse rien** : le canal est un
    /// `push` sans garantie de livraison, et la réconciliation suivante
    /// rejoue l'annonce. C'est le même filet que `complet = true` à chaque
    /// réenrôlement (décision D3 de G1), et la recette de G1 l'a vu
    /// fonctionner sur le chemin réel.
    IconesManquantes {
        #[serde(rename = "v", deserialize_with = "verifie_version")]
        version: u8,
        empreintes: Vec<String>,
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

    /// ⚠️ L'APPELANT DOIT VÉRIFIER QUE `empreintes` N'EST PAS VIDE avant
    /// d'émettre : ce constructeur ne le fait pas pour lui, parce qu'il ne
    /// saurait pas quoi rendre à la place. La règle vit du côté qui décide —
    /// `plateforme/src/agents/canal.ts`.
    pub fn icones_manquantes(empreintes: Vec<String>) -> Self {
        Self::IconesManquantes {
            version: PLATEFORME_VERSION,
            empreintes,
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
