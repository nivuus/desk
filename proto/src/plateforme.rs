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
pub const PLATEFORME_VERSION: u8 = 4;

/// Les trois lecteurs de champ appelés par `deserialize_with`, extraits pour
/// que ce fichier ne franchisse pas 500 lignes en accueillant le sous-bloc G3.
///
/// 🔴 LE `use` N'EST PAS COSMÉTIQUE : `serde` résout le chemin d'un
/// `deserialize_with = "verifie_version"` **dans la portée du module qui porte
/// l'attribut**. C'est lui qui permet à l'extraction de ne toucher AUCUN des
/// attributs des structures ci-dessous, donc d'être une transposition pure.
mod champs;
use champs::{icone_obligatoire, option_obligatoire, verifie_version, version_toleree};

/// La table des motifs de refus, extraite pour la même raison.
mod motifs;
pub use motifs::MotifCanal;


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

/// Les types de charge utile de l'INSTALLATION, dans un module frère de `apps`,
/// pour la même raison et par le même mécanisme.
mod installation;
pub use installation::{Issue, Phase};

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
    /// Où en est une installation en cours.
    ///
    /// 🔴 **ÉCHANTILLONNÉE, ET C'EST UNE CONTRAINTE DE SÛRETÉ, PAS DE
    /// CONFORT.** La file montante de l'agent est bornée à `FILE_EMISSION`
    /// (32) et **abandonne ce qui déborde**. Une progression émise par tranche
    /// de 64 Kio la saturerait et noierait le journal partagé — c'est la
    /// doctrine que ce dépôt a payée au chantier TURN : *compter ou
    /// échantillonner, jamais tracer par paquet*. La règle vit dans
    /// `agent/src/apps/installation/cadence.rs`, horloge en paramètre.
    ///
    /// ⚠️ `octets_total` VAUT ZÉRO EN PHASE `Execution`, où il n'y a rien à
    /// totaliser : c'est `ecoule_ms` qui porte l'information, et l'interface
    /// affiche un état indéterminé.
    Progression {
        #[serde(rename = "v", deserialize_with = "verifie_version")]
        version: u8,
        installation: String,
        phase: Phase,
        octets_faits: u64,
        octets_total: u64,
        ecoule_ms: u64,
    },
    /// L'installation est finie, et voici ce qui s'est réellement passé.
    ///
    /// 🔴 **`code_sortie` EST UNE `Option`, JAMAIS UN `i32` AVEC UN `-1`
    /// SENTINELLE** : « pas de code » et « code −1 » sont deux faits
    /// différents, et une sentinelle les confondrait exactement comme un
    /// `source_max_px` à `0` confondrait « inconnu » et « nul ». Il est
    /// RAPPORTÉ, jamais interprété — voir [`Issue`].
    ///
    /// ⚠️ **UN `journal` VIDE EST LE CAS NORMAL**, pas un échec : la plupart
    /// des installeurs Windows sont graphiques et n'écrivent rien sur les flux
    /// standard. L'interface ne doit pas le présenter comme une panne.
    ///
    /// ⚠️ `journal_tronque` DIT QUE LA QUEUE A ÉTÉ COUPÉE, et il est distinct
    /// d'un journal vide : sans lui, un utilisateur lirait les derniers
    /// 64 Kio en croyant lire tout.
    Termine {
        #[serde(rename = "v", deserialize_with = "verifie_version")]
        version: u8,
        installation: String,
        issue: Issue,
        /// Obligatoire sur le fil — voir `champs::option_obligatoire`.
        #[serde(deserialize_with = "option_obligatoire")]
        motif: Option<String>,
        /// Idem. `None` = « le code n'a pas pu être recueilli ».
        #[serde(deserialize_with = "option_obligatoire")]
        code_sortie: Option<i32>,
        journal: String,
        journal_tronque: bool,
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

    pub fn progression(
        installation: impl Into<String>,
        phase: Phase,
        octets_faits: u64,
        octets_total: u64,
        ecoule_ms: u64,
    ) -> Self {
        Self::Progression {
            version: PLATEFORME_VERSION,
            installation: installation.into(),
            phase,
            octets_faits,
            octets_total,
            ecoule_ms,
        }
    }

    pub fn termine(
        installation: impl Into<String>,
        issue: Issue,
        motif: Option<String>,
        code_sortie: Option<i32>,
        journal: impl Into<String>,
        journal_tronque: bool,
    ) -> Self {
        Self::Termine {
            version: PLATEFORME_VERSION,
            installation: installation.into(),
            issue,
            motif,
            code_sortie,
            journal: journal.into(),
            journal_tronque,
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
    /// Installer un logiciel que l'utilisateur a téléversé.
    ///
    /// 🔴 **LES OCTETS N'EMPRUNTENT JAMAIS CE MESSAGE**, et c'est la même
    /// règle que pour [`Self::IconesManquantes`] : ce canal est en JSON, il
    /// porte le battement de cœur, et une tranche de 8 Mio y coûterait +33 %
    /// en base64 tout en bloquant ce battement. L'ordre porte une **URL**, et
    /// l'agent va tirer les octets en HTTP, avec son jeton d'agent.
    ///
    /// ⚠️ `sha256` EST L'EMPREINTE DU FICHIER ENTIER, la même valeur que le
    /// navigateur a annoncée et que la plateforme a recalculée au scellement.
    /// **Une seule valeur, comparable partout** — y compris par un humain avec
    /// un `sha256sum`. C'est pourquoi ce n'est PAS une empreinte d'arbre sur
    /// les tranches, qui aurait été native et gratuite côté navigateur mais
    /// incomparable partout ailleurs.
    ///
    /// 🔴 **L'AGENT RECALCULE CETTE EMPREINTE APRÈS ÉCRITURE**, et c'est la
    /// TROISIÈME des trois vérifications : le navigateur peut mentir, le
    /// disque de la plateforme peut se corrompre, le transfert peut tronquer.
    /// **Aucun saut ne fait confiance au précédent.**
    ///
    /// ⚠️ **CE MESSAGE EST RÉÉMIS À CHAQUE ENRÔLEMENT** tant que l'installation
    /// est en attente : un `push` WebSocket n'a aucune garantie de livraison,
    /// et sans cette réémission un ordre émis pendant une coupure serait perdu
    /// SANS TERME. C'est le même filet que `complet = true` du catalogue.
    /// **L'agent déduplique donc par `installation`, et sa mémoire est SUR LE
    /// DISQUE** — voir `agent/src/apps/installation/depot.rs`.
    Installer {
        #[serde(rename = "v", deserialize_with = "verifie_version")]
        version: u8,
        installation: String,
        url: String,
        nom: String,
        taille: u64,
        sha256: String,
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

    pub fn installer(
        installation: impl Into<String>,
        url: impl Into<String>,
        nom: impl Into<String>,
        taille: u64,
        sha256: impl Into<String>,
    ) -> Self {
        Self::Installer {
            version: PLATEFORME_VERSION,
            installation: installation.into(),
            url: url.into(),
            nom: nom.into(),
            taille,
            sha256: sha256.into(),
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

// 🔴 UN TROISIÈME FICHIER DE TESTS, ET IL A SA RAISON PROPRE : les vecteurs
// partagés sont un jeu de ROUND-TRIPS, qui ne dit rien de ce qui doit être
// REFUSÉ. La garde la plus fragile de v4 — un `termine` dont une clé
// facultative MANQUE — n'y est donc pas éprouvable, et elle vit ici.
#[cfg(test)]
#[path = "plateforme/tests_installation.rs"]
mod tests_installation;
