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
