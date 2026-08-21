//! `CursorShape` — la forme du curseur, extraite de `control.rs`.
//!
//! ⚠️ **EXTRACTION PRÉALABLE, pas un remaniement.** Le sous-bloc E3 ajoute la
//! variante `AgentControl::MicState` à `control.rs`, qui était à **444** lignes
//! pour une porte à 500 : l'addition documentée à la densité d'`Accent` l'aurait
//! porté au-delà de 490. La doctrine du dépôt est d'**extraire AVANT
//! d'ajouter**, jamais de comprimer après avoir franchi — ce dépôt a franchi ce
//! plafond cinq fois et l'a rattrapé deux fois par une compression qu'il
//! s'interdit.
//!
//! ⚠️ **Le contenu est VERBATIM.** Une seule chose a changé, et c'est la seule
//! qui avait le droit de changer : le `use serde::…` que le parent portait déjà
//! est répété ici, un module enfant ne voyant pas les imports de son parent. Le
//! type reste `pub`, et `control.rs` le ré-exporte, de sorte qu'aucun site
//! d'appel de `proto::control::CursorShape` n'a bougé.

use serde::{Deserialize, Serialize};

/// Forme du curseur, exprimée directement dans le vocabulaire de la
/// propriété CSS `cursor` : le client la pose telle quelle, sans table de
/// correspondance à maintenir de son côté.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CursorShape {
    Default,
    Text,
    Wait,
    Progress,
    Crosshair,
    Pointer,
    Move,
    NotAllowed,
    Help,
    NsResize,
    EwResize,
    NwseResize,
    NeswResize,
}
