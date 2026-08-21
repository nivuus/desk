//! La RÉDACTION des messages de contrôle au journal : ce que `Debug` montre,
//! et surtout ce qu'il ne montre PAS.
//!
//! **Extrait de `control.rs` au relevé de clôture du sous-bloc P2** : les deux
//! `impl` ci-dessous l'avaient porté à **494 lignes, marge 6**, sur un plafond
//! de 500. C'est la TROISIÈME porte de ce sous-bloc, et la règle du dépôt est
//! d'extraire, jamais de comprimer un commentaire pour repasser sous la ligne.
//!
//! ⚠️ **Cet emploi de `#[path]` est HORS de la portée de la « Convention de
//! module enfant » de `CLAUDE.md`**, qui déclare elle-même son exclusion : même
//! mécanisme Rust, autre raison — la règle des 500 lignes —, exactement comme
//! `superviseur/table.rs`. Ce module ne se hisse PAS à la racine.

use super::{AgentControl, ClientControl};

// ---------------------------------------------------------------------------
// 🔴 LE PRESSE-PAPIER NE DOIT JAMAIS ATTEINDRE UN JOURNAL, ET `#[derive(Debug)]`
//    L'Y METTAIT.
// ---------------------------------------------------------------------------
//
// **Mesuré, pas conjecturé** : la recette du sous-bloc P2 a relevé, dans
// `agent.log`, quatre lignes de la forme
//
//     INFO agent::demarrage: contrôle reçu session=… \
//          Clipboard { version: 3, text: "alpha-arme-1-crwor9" }
//
// c'est-à-dire **le contenu du presse-papier de l'utilisateur, en clair, dans
// un journal que ce dépôt VERSE DANS GIT**. La décision D-P1-7 l'interdit
// nommément (« UNE SEULE TRACE, ET JAMAIS LE TEXTE »), et P1 l'avait tenue
// dans le sens descendant — `capteur::sommeil::presse_papier::distribuer` ne
// journalise qu'`octets` et `refus`.
//
// ⚠️ **Ce n'est PAS un défaut du site de journalisation.** La trace fautive
// (`demarrage.rs`, `on_control`) est ANTÉRIEURE au chantier presse-papier :
// elle imprime le message reçu par `?message`, ce qui était inoffensif tant
// qu'aucune variante ne portait de contenu privé. C'est P2 qui a rendu cette
// trace dangereuse en ajoutant `ClientControl::Clipboard`, et un remède posé
// sur le site aurait laissé le PROCHAIN site fuir.
//
// **Le remède est donc au TYPE, et il est exhaustif par construction** : ces
// deux `impl` sont écrites à la main, si bien qu'ajouter une variante oblige à
// décider ce qu'elle montre. Le texte est remplacé par sa TAILLE — ce qui
// garde au journal tout son pouvoir de diagnostic, la taille étant justement
// ce que la borne et le refus mettent en jeu.

impl std::fmt::Debug for ClientControl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClientControl::Resize { version, width, height } => f
                .debug_struct("Resize")
                .field("v", version)
                .field("width", width)
                .field("height", height)
                .finish(),
            ClientControl::Visibility { version, visible, focused } => f
                .debug_struct("Visibility")
                .field("v", version)
                .field("visible", visible)
                .field("focused", focused)
                .finish(),
            // 🔴 LA TAILLE, JAMAIS LE TEXTE.
            ClientControl::Clipboard { version, text } => f
                .debug_struct("Clipboard")
                .field("v", version)
                .field("octets", &text.len())
                .finish(),
        }
    }
}

impl std::fmt::Debug for AgentControl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            // 🔴 LA TAILLE ET LE REFUS, JAMAIS LE TEXTE. `bytes` porte déjà la
            // taille dans le protocole : on ne la recalcule pas, on la montre.
            AgentControl::Clipboard { version, text, bytes } => f
                .debug_struct("Clipboard")
                .field("v", version)
                .field("refus", &text.is_none())
                .field("octets", bytes)
                .finish(),
            // Les autres variantes ne portent aucun contenu privé : leur forme
            // dérivée est reproduite telle quelle, par le seul moyen qui reste
            // une fois `derive` retiré.
            AgentControl::Ready { version, width, height, mic } => f
                .debug_struct("Ready").field("v", version).field("width", width)
                .field("height", height).field("mic", mic).finish(),
            AgentControl::SessionEnd { version, reason } => f
                .debug_struct("SessionEnd").field("v", version).field("reason", reason).finish(),
            AgentControl::Pointer { version, visible, shape } => f
                .debug_struct("Pointer").field("v", version).field("visible", visible)
                .field("shape", shape).finish(),
            AgentControl::Asleep { version, asleep, reason } => f
                .debug_struct("Asleep").field("v", version).field("asleep", asleep)
                .field("reason", reason).finish(),
            AgentControl::Rumble { version, left, right } => f
                .debug_struct("Rumble").field("v", version).field("left", left)
                .field("right", right).finish(),
            AgentControl::Capabilities { version, gamepad, clipboard } => f
                .debug_struct("Capabilities").field("v", version).field("gamepad", gamepad)
                .field("clipboard", clipboard).finish(),
            AgentControl::Fullscreen { version, active } => f
                .debug_struct("Fullscreen").field("v", version).field("active", active).finish(),
            AgentControl::Link { version, bitrate, width, height, quality, adaptation } => f
                .debug_struct("Link").field("v", version).field("bitrate", bitrate)
                .field("width", width).field("height", height).field("quality", quality)
                .field("adaptation", adaptation).finish(),
        }
    }
}
