//! La surveillance des quatre racines de raccourcis : un ACCÉLÉRATEUR, et rien
//! d'autre.
//!
//! 🔴 CE MODULE N'AJOUTE AUCUNE FONCTIONNALITÉ. La réconciliation périodique de
//! `apps::boucle` existe depuis le sous-bloc G1, elle fonctionne, et **elle
//! n'est jamais désarmée** en configuration livrée. Tout ce que la surveillance
//! pose est un **déclencheur** : elle avance la prochaine réconciliation, elle
//! ne la remplace pas. La propriété qui compte n'est donc pas « la surveillance
//! marche » — elle marchera — mais **« la surveillance ne peut RIEN perdre »**,
//! et c'est la seule chose que ce sous-bloc cherche à rendre falsifiable.
//!
//! ⚠️ CE FICHIER EST DÉCLARÉ SANS `cfg` dans `apps.rs`, ET CE SONT SES ENFANTS
//! WINDOWS QUI PORTENT LE LEUR. C'est la figure d'`apps::icone` et
//! d'`apps::installation`, un cran plus bas, et c'est ce qui fait exister
//! `apps::surveillance::mode`, `::rebond`, `::faute` et `::partage` sur l'hôte
//! Linux, où leurs tests courent. La « Convention de module enfant » de
//! `CLAUDE.md` n'est **pas** mobilisée : aucun module ne franchit ici de
//! frontière `#[cfg(windows)]`.
//!
//! ⚠️ **L'arborescence de la spécification §6 range `rebond.rs` À CÔTÉ de
//! `surveillance.rs`, en frère.** Ce sous-bloc le range DESSOUS, et la raison
//! est écrite dans l'en-tête d'`apps/icone.rs` : *on ne peut pas déclarer un
//! PETIT-fils depuis le grand-parent sans `#[path]`*. Suivre la spec à la
//! lettre obligerait au `#[path]` que ④ évite depuis G2.

/// L'injection de fautes de surveillance. **PUR** + un budget global.
pub mod faute;
/// Les quatre états d'`APPS_SURVEILLANCE`. **PUR.**
pub mod mode;
/// Les deux compteurs monotones et le drapeau d'arrêt. **SANS `cfg`.**
pub mod partage;
/// L'anti-rebond, horloge en paramètre. **PUR.**
pub mod rebond;

// ---------------------------------------------------------------------------
// Ce qui suit est `#[cfg(windows)]` : les handles, l'attente et les
// complétions. Rien n'y DÉCIDE — les décisions vivent dans les quatre modules
// ci-dessus, et c'est ce qui les rend éprouvables sur l'hôte.
// ---------------------------------------------------------------------------

/// Une racine surveillée : ouvrir, armer, compléter, rouvrir.
#[cfg(windows)]
mod racine;

/// Le tampon que le noyau remplit, **par racine**, en octets.
///
/// ⚠️ **NON CALIBRÉ, ET CHOISI SUR SON MÉRITE.** 64 Kio est le plafond que la
/// documentation impose pour un chemin réseau et celui qu'elle recommande de ne
/// pas dépasser, le tampon étant **verrouillé en mémoire non paginée** — quatre
/// racines font donc 256 Kio de pool.
///
/// 🔴 IL N'EST PAS CHOISI POUR RENDRE UN CRITÈRE MESURABLE, ET IL NE DOIT
/// JAMAIS L'ÊTRE. Le rétrécir ferait déborder le tampon plus facilement, donc
/// « réussir » le critère qui demande d'observer un débordement — c'est-à-dire
/// régler le produit sur son test. Si aucune rafale ne le fait déborder, le
/// verdict est **NON MESURABLE**, et il s'écrit tel quel.
pub const TAMPON_NOTIFICATIONS: usize = 65_536;
