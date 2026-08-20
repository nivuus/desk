//! Messages du canal de contrôle (fiable, ordonné, faible débit).
//!
//! Format JSON versionné : `{"type":"...","v":3,"...":...}` (`type` sert de tag
//! interne à l'enum et est toujours émis en premier par serde). Le champ `v` est
//! obligatoire et vérifié à la désérialisation : un message sans `v`, ou avec un
//! `v` différent de [`CONTROL_VERSION`], est rejeté.

use serde::{Deserialize, Serialize};

/// Version du protocole de contrôle. Incrémenter à tout changement de format.
///
/// v2 (chantier B) : ajout de `Pointer`, `Rumble` et `Capabilities`.
/// v3 (chantier C) : ajout de `Link`.
pub const CONTROL_VERSION: u8 = 3;

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

// Note : pas de `default` sur le champ `v` — un message sans champ `v` doit être
// rejeté (champ obligatoire), pas silencieusement complété avec la version
// courante. `default` court-circuiterait `deserialize_with` quand le champ est
// absent, ce qui romprait la vérification.
fn verifie_version<'de, D>(deserializer: D) -> Result<u8, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let v = u8::deserialize(deserializer)?;
    if v != CONTROL_VERSION {
        return Err(serde::de::Error::custom(format!(
            "version de contrôle non supportée : {v}"
        )));
    }
    Ok(v)
}

/// Ce que l'utilisateur doit comprendre de l'état du lien.
///
/// Trois valeurs et non un booléen : « dégradé » et « insuffisant » sont deux
/// situations distinctes, et la seconde ne se déduit pas de la première par
/// une négation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LinkQuality {
    /// Pleine résolution, lien confortable.
    Bonne,
    /// Résolution réduite pour tenir le lien.
    Degradee,
    /// Plancher atteint : le lien ne permet plus le jeu nerveux. C'est
    /// l'avertissement explicite exigé par le cadrage jeu (§3).
    Insuffisante,
}

/// L'agent reçoit-il de quoi s'asservir ?
///
/// Indépendant de `LinkQuality` : une session sans estimation de bande
/// passante peut très bien tourner en `Bonne` sur un lien large.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LinkAdaptation {
    Active,
    /// Aucune estimation ne parvient à l'agent : le débit reste figé au
    /// plafond configuré. À dire, pas à taire.
    Indisponible,
}

/// Message du client web vers l'agent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case", deny_unknown_fields)]
pub enum ClientControl {
    Resize {
        #[serde(rename = "v", deserialize_with = "verifie_version")]
        version: u8,
        width: u32,
        height: u32,
    },
    /// Visibilité de la fenêtre navigateur, et si elle a le focus.
    ///
    /// **Deux signaux dans un seul message, et le second n'est pas
    /// décoratif** : la visibilité seule ne suffirait pas à ordonner le vivier
    /// du capteur quand plusieurs fenêtres sont visibles en même temps — elles
    /// ont alors exactement la même visibilité.
    Visibility {
        #[serde(rename = "v", deserialize_with = "verifie_version")]
        version: u8,
        visible: bool,
        focused: bool,
    },
}

/// Message de l'agent vers le client web.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case", deny_unknown_fields)]
pub enum AgentControl {
    Ready {
        #[serde(rename = "v", deserialize_with = "verifie_version")]
        version: u8,
        width: u32,
        height: u32,
        /// Le micro est-il disponible pour cette session (chantier E) ?
        ///
        /// **Aucun bump de `CONTROL_VERSION` n'est nécessaire**, dans les deux
        /// sens : le parseur TypeScript vérifie `v` puis `type` puis CASTE, si
        /// bien qu'un champ supplémentaire est ignoré par un client ancien ; et
        /// un client récent face à un agent ancien lit `mic === undefined`,
        /// donc falsy, donc n'affiche pas de bouton — la règle de la spec §10,
        /// obtenue gratuitement.
        ///
        /// ⚠️ **`#[serde(default)]` est OBLIGATOIRE, pas décoratif** :
        /// `AgentControl` porte `deny_unknown_fields`, ce qui n'empêche pas
        /// d'AJOUTER un champ, mais un champ MANQUANT reste une erreur de
        /// désérialisation côté Rust.
        ///
        /// ⚠️ **Ce drapeau est décidé à l'ÉTABLISSEMENT et ne peut pas
        /// exprimer un refus ultérieur** : l'exclusivité du câble s'acquiert au
        /// premier paquet montant (bloc E2), donc après ce message. Un second
        /// utilisateur verra le bouton et n'aura pas le son. Lacune NOMMÉE,
        /// pas dissimulée — le refus est journalisé une fois côté agent
        /// (`transport/piste_micro.rs`).
        #[serde(default)]
        mic: bool,
    },
    SessionEnd {
        #[serde(rename = "v", deserialize_with = "verifie_version")]
        version: u8,
        reason: String,
    },
    /// État du pointeur. `visible: false` signifie à la fois « verrouille le
    /// pointeur » et « n'affiche aucun curseur » : c'est une seule
    /// observation côté agent (le curseur système est masqué), donc un seul
    /// message.
    Pointer {
        #[serde(rename = "v", deserialize_with = "verifie_version")]
        version: u8,
        visible: bool,
        shape: CursorShape,
    },
    /// La fenêtre dort — son encodeur et sa duplication ont été relâchés.
    ///
    /// `reason` vaut `"masquee"` (l'utilisateur l'a voulu) ou `"evincee"` (le
    /// vivier lui a pris sa place alors qu'il la regardait). Les deux ne se
    /// valent pas pour lui : la seconde mérite d'être dite.
    Asleep {
        #[serde(rename = "v", deserialize_with = "verifie_version")]
        version: u8,
        asleep: bool,
        reason: String,
    },
    Rumble {
        #[serde(rename = "v", deserialize_with = "verifie_version")]
        version: u8,
        left: u8,
        right: u8,
    },
    /// Émis une seule fois par session, mais PAS après `Ready` en pratique :
    /// `agent/src/demarrage.rs` pousse ce message dans le canal `mpsc` de contrôle
    /// dès le démarrage du transport, avant même l'ouverture du canal de
    /// données — le drainage (`transport/tick.rs::act_on_timeout`) le met donc en
    /// file avant que `Event::ChannelOpen` n'y ajoute `Ready`. L'ordre réel
    /// est `Capabilities`, éventuellement un premier `Pointer`, puis `Ready`.
    /// Sans conséquence aujourd'hui (le client traite les types
    /// indépendamment, voir `client/src/main.ts`), mais un client qui
    /// gaterait son initialisation sur `Ready` perdrait ce message et le
    /// premier `Pointer` : ne pas le faire.
    Capabilities {
        #[serde(rename = "v", deserialize_with = "verifie_version")]
        version: u8,
        gamepad: bool,
    },
    /// L'application Windows est passée en plein écran, ou en est sortie.
    ///
    /// **Le client ARME, il n'agit pas** : `requestFullscreen()` exige une
    /// activation utilisateur transitoire qu'un message de canal de données ne
    /// fournit pas. Voir `client/src/fullscreen.ts`.
    ///
    /// Le sens est UNIQUE — le navigateur ne force jamais l'état de la fenêtre
    /// Windows —, et c'est ce qui rend toute oscillation impossible.
    Fullscreen {
        #[serde(rename = "v", deserialize_with = "verifie_version")]
        version: u8,
        active: bool,
    },
    /// Le presse-papier de la VM a changé.
    ///
    /// Poussé **non sollicité**, et **au changement seulement** : le
    /// propriétaire compare le contenu au dernier émis avant d'émettre
    /// (garde n°2 de D5), parce que le compteur de séquence Windows bouge
    /// même sur une réécriture identique — mesuré, sonde P0 du 20 août 2026,
    /// `q2="bouge"` sur deux exécutions.
    ///
    /// `text` vaut `None` quand le contenu dépasse la borne du propriétaire
    /// (`agent::presse_papier::PRESSE_PAPIER_MAX`) : il est **REFUSÉ, jamais
    /// tronqué** — un collage silencieusement amputé est le pire résultat
    /// possible, et il est pire que pas de collage du tout, l'utilisateur ne
    /// pouvant pas voir qu'il lui manque la fin.
    ///
    /// `bytes` porte alors la taille refusée, en octets d'UTF-8 **après
    /// normalisation des fins de ligne**, pour que le bandeau puisse la dire ;
    /// dans le cas normal il porte la taille du texte émis, ce qui rend le
    /// message auto-descriptif au journal. Il n'est donc pas redondant avec
    /// `text`.
    ///
    /// ⚠️ **`text` n'est pas `Option` par commodité de sérialisation** : le
    /// champ doit rester PRÉSENT et valoir `null` sur un refus. Le rendre
    /// omissible (`skip_serializing_if`) ferait qu'un client ne pourrait plus
    /// distinguer un refus d'un message tronqué en route.
    ///
    /// **Une variante, pas deux** : le précédent du dépôt est `Asleep`
    /// (un état plus sa raison) et `Link` (une décision plus ses grandeurs) ;
    /// le dépôt n'a aucun précédent de deux variantes pour un seul état.
    Clipboard {
        #[serde(rename = "v", deserialize_with = "verifie_version")]
        version: u8,
        text: Option<String>,
        bytes: u32,
    },
    /// État du lien réseau, émis à chaque changement de décision
    /// d'adaptation — donc rarement, pas à chaque seconde.
    Link {
        #[serde(rename = "v", deserialize_with = "verifie_version")]
        version: u8,
        bitrate: u32,
        width: u32,
        height: u32,
        quality: LinkQuality,
        adaptation: LinkAdaptation,
    },
}

impl ClientControl {
    /// Construit un message de redimensionnement à la version courante du protocole.
    pub fn resize(width: u32, height: u32) -> Self {
        ClientControl::Resize { version: CONTROL_VERSION, width, height }
    }
}

impl AgentControl {
    /// Construit un message "agent prêt" à la version courante du protocole.
    pub fn ready(width: u32, height: u32, mic: bool) -> Self {
        AgentControl::Ready { version: CONTROL_VERSION, width, height, mic }
    }

    /// Construit un message de fin de session à la version courante du protocole.
    pub fn session_end(reason: impl Into<String>) -> Self {
        AgentControl::SessionEnd { version: CONTROL_VERSION, reason: reason.into() }
    }

    pub fn pointer(visible: bool, shape: CursorShape) -> Self {
        AgentControl::Pointer { version: CONTROL_VERSION, visible, shape }
    }

    pub fn asleep(asleep: bool, reason: &str) -> AgentControl {
        AgentControl::Asleep {
            version: CONTROL_VERSION,
            asleep,
            reason: reason.to_string(),
        }
    }

    pub fn fullscreen(active: bool) -> AgentControl {
        AgentControl::Fullscreen { version: CONTROL_VERSION, active }
    }

    /// Le presse-papier de la VM a changé.
    ///
    /// ⚠️ **`CONTROL_VERSION` NE MONTE PAS pour cette variante, et ce n'est
    /// pas un oubli.** Les deux vérifications de `v` — `verifie_version`
    /// ci-dessus et `parseAgentControl` côté TypeScript — sont des **égalités
    /// strictes** : monter la version ferait rejeter **tous** les messages,
    /// `Ready` et `SessionEnd` compris. Une incompatibilité TOTALE
    /// remplacerait une dégradation PAR MESSAGE. Le précédent est à trois
    /// lignes d'ici : le champ `mic` a été ajouté à `Ready` sans monter la
    /// version, pour la même raison.
    pub fn clipboard(text: Option<String>, bytes: u32) -> AgentControl {
        AgentControl::Clipboard { version: CONTROL_VERSION, text, bytes }
    }

    pub fn rumble(left: u8, right: u8) -> Self {
        AgentControl::Rumble { version: CONTROL_VERSION, left, right }
    }

    pub fn capabilities(gamepad: bool) -> Self {
        AgentControl::Capabilities { version: CONTROL_VERSION, gamepad }
    }

    pub fn link(
        bitrate: u32,
        taille: (u32, u32),
        quality: LinkQuality,
        adaptation: LinkAdaptation,
    ) -> Self {
        AgentControl::Link {
            version: CONTROL_VERSION,
            bitrate,
            width: taille.0,
            height: taille.1,
            quality,
            adaptation,
        }
    }
}

#[cfg(test)]
#[path = "control/tests.rs"]
mod tests;
