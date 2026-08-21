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
/// L'anti-rebond, horloge en paramètre. **PUR.**
pub mod rebond;
