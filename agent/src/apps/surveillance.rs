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

/// Le fil unique : l'attente, les N racines, l'arrêt.
#[cfg(windows)]
mod fil;
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
/// régler le produit sur son test.
///
/// 🔴 **ET LE VERDICT EST TOMBÉ : NON MESURABLE, ET IL EST ÉCRIT TEL QUEL.**
/// La porte S1 monte jusqu'à **60 000 fichiers à 2 850/s** sans un seul
/// débordement (sept exécutions), et la rafale rejouée sur le produit rend
/// **96 742 puis 96 328 notifications réelles pour `debordements=0`** (deux
/// exécutions). **Le tampon n'a PAS été rétréci**, et il ne doit pas l'être.
///
/// 🔵 **LA RAISON EST ARITHMÉTIQUE, ET ELLE SURVIVRA À CETTE MACHINE** : un
/// débordement exige plus de ~1 260 événements **entre deux réarmements**,
/// c'est-à-dire dans les microsecondes qui les séparent. À 2 850 fichiers par
/// seconde ils arrivent toutes les ~350 µs. **Le plafond mesuré est celui du
/// SYSTÈME DE FICHIERS, pas celui du tampon** — il est quasi constant de 1 000 à
/// 60 000 fichiers.
pub const TAMPON_NOTIFICATIONS: usize = 65_536;

/// Démarre le fil de surveillance, ou rend une `Veille` inerte.
///
/// ⚠️ LA POIGNÉE EST RENDUE POUR ÊTRE **CONSERVÉE SANS ÊTRE ATTENDUE** : la
/// lâcher ne terminerait pas le fil — un `JoinHandle` lâché détache —, mais
/// `apps::Poignees` la garde par symétrie avec les deux autres, et parce que
/// c'est ce qui rendra un jour l'extinction observable. **L'arrêt propre passe
/// par `Veille::arreter`, jamais par la poignée.**
///
/// ⚠️ `Mode::Desarmee` NE JOURNALISE RIEN ICI : l'`info!` du mode est émis par
/// l'appelant, **inconditionnellement**, ce qui vaut mieux qu'une trace émise
/// par la seule branche qui désarme.
#[cfg(windows)]
pub fn demarrer(mode: mode::Mode) -> (partage::Veille, Option<std::thread::JoinHandle<()>>) {
    let veille = partage::Veille::default();
    if !mode.surveille() {
        return (veille, None);
    }
    let pour_le_fil = veille.clone();
    let poignee = std::thread::Builder::new()
        .name("surveillance-apps".into())
        .spawn(move || fil::tourner(pour_le_fil))
        .map_err(|erreur| {
            // Le fil ne démarre pas : la découverte reste ENTIÈREMENT
            // fonctionnelle, à sa période. C'est exactement ce que « G4
            // n'ajoute aucune fonctionnalité » veut dire, et la trace le dit
            // plutôt que de laisser lire une panne de découverte.
            tracing::error!(
                %erreur,
                "fil de surveillance non démarré : la réconciliation périodique reste la source de vérité"
            );
        })
        .ok();
    (veille, poignee)
}

/// La variante hors Windows : une `Veille` inerte, **et rien à journaliser**.
///
/// Même raison qu'`apps::demarrer` : un agent Linux n'a pas à se plaindre de ne
/// pas surveiller des raccourcis Windows, et le confondre avec un désarmement
/// explicite — qui, lui, DIT qu'il est désarmé — brouillerait deux états
/// distincts.
#[cfg(not(windows))]
pub fn demarrer(_mode: mode::Mode) -> (partage::Veille, Option<std::thread::JoinHandle<()>>) {
    (partage::Veille::default(), None)
}
