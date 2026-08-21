//! Le contrat du fil d'écriture : ce qu'il **reçoit** ([`Ordre`]) et ce dont il
//! a **besoin** ([`Config`]).
//!
//! # Ce que cette extraction EST, et ce qu'elle n'est PAS
//!
//! **Elle n'ajoute aucun comportement.** Les deux déclarations sont transposées
//! **VERBATIM** depuis `fil.rs` (l. 97-127), avec leurs doc-commentaires, et
//! `fil.rs` les réexporte par `pub use contrat::{Config, Ordre};` : **aucun
//! site d'appel ne bouge.**
//!
//! Elle vient **AVANT** l'addition qu'elle accueille, et non après : `fil.rs`
//! était à **472** lignes, marge **28**, et F5 doit y ajouter `Ordre::Bonjour`
//! plus son bras dans `Fil::traiter` — ce qui l'aurait porté entre 482 et 490,
//! trop serré pour la revue qui suit. C'est le geste que D9 a inventé
//! (`capteur/serveur/instances.rs`, marge rendue de 10 à 65) et que D10 a joué
//! trois fois. **Jamais une compression**, que `CLAUDE.md` interdit nommément.
//!
//! ⚠️ **Pourquoi CES deux-là et pas `Fil` ni `EnCours`.** Les deux extraites
//! sont `pub` : leur type suit leur visibilité. `Fil::en_cours` est `pub(super)`
//! et son type `EnCours` aussi ; les emporter demanderait de hisser une
//! visibilité, et `fil.rs:138-142` porte déjà le constat qu'une **extraction
//! rigoureusement verbatim ne compile pas** quand un élément privé d'un module
//! enfant doit devenir visible de son parent — `rustc` le dit par
//! `private_interfaces`, *un avertissement d'une autre famille que `dead_code`*.

use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

use crate::pont::ecriture::Evenement;
use crate::pont::table::Table;
use crate::pont::transport::VersNavigateur;
use proto::fichiers::CodeEchec;
/// Ce que le fil d'écriture reçoit.
///
/// ⚠️ **UN SEUL CANAL, et c'est une divergence déclarée avec le plan de F2**,
/// dont la signature prend **deux** `Receiver` (les événements, les faits).
/// Deux récepteurs sur un fil bloquant imposeraient un sondage alterné, donc
/// une latence bornée par un délai arbitraire de plus — et un test qui dépend
/// d'un `sleep`. Un canal unique rend la boucle déterministe, donc testable
/// sans dormir : la propriété que `pont::table` s'est donnée pour l'expiration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ordre {
    /// Une notification ProjFS a désigné un chemin.
    Survenu(Evenement),
    /// Le navigateur a acquitté un morceau.
    Fait { correlation: u32 },
    /// Le navigateur a refusé, ou la commande a expiré.
    Echec { correlation: u32, code: CodeEchec },
    /// **F5** — le navigateur s'est annoncé, et il dit sur quelle racine.
    ///
    /// 🔴 **C'EST LE SEUL ORDRE QUI DÉCLENCHE LA REPRISE**, et c'est ce qui
    /// ferme la fenêtre de trente secondes que F2 a mesurée deux fois sur deux.
    /// Avant F5, `Fil::demarrer` appelait `reprendre()` **au démarrage du
    /// fil** — c'est-à-dire au démarrage du pont, *sans savoir si un navigateur
    /// est là, ni lequel, ni sur quel répertoire*. F2 a relevé la poussée du
    /// rejeu **0,8 s AVANT** que le navigateur n'annonce son montage, puis
    /// `commande expirée … correlation=0` **+30,2 s** plus tard : *« l'indicateur
    /// qui existe pour dénoncer la perte est MUET pendant trente secondes. »*
    ///
    /// C'est **littéralement le remède que F2 a nommé** : « que le pont n'ouvre
    /// son canal d'écriture qu'après un acquittement de l'écrivain ».
    ///
    /// ⚠️ **Il n'y avait AUCUN ordre correspondant à `CanalOuvert`**, et c'est
    /// la cause structurelle de cette fenêtre : le fil ne pouvait pas savoir que
    /// le navigateur était prêt, faute qu'on le lui dise.
    Bonjour {
        /// Le nom de la racine que l'utilisateur a choisie. **Un indice, pas une
        /// preuve** — voir [`crate::pont::bonjour`].
        racine: String,
        /// L'utilisateur a confirmé vouloir pousser malgré un nom différent.
        forcer: bool,
    },
}

/// Ce dont le fil a besoin pour tourner.
pub struct Config {
    /// La racine de virtualisation, où vivent les fichiers hydratés.
    pub racine: PathBuf,
    /// Le journal de reprise, **hors de la racine**.
    pub chemin_journal: PathBuf,
    /// **La MÊME table que les lectures** : deux sources de corrélations sur un
    /// canal unique se collisionneraient en silence.
    pub table: Arc<Mutex<Table>>,
    pub vers_navigateur: Sender<VersNavigateur>,
    /// `PONT_ECRITURE` : `false` = le bras désarmé de l'A/B.
    pub armee: bool,
}
