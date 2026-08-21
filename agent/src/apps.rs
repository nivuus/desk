//! Découverte et lancement des applications de la VM.
//!
//! ⚠️ CE MODULE EST DÉCLARÉ SANS `cfg` dans `main.rs`, et ce sont ses enfants
//! Windows qui portent le leur. C'est ce qui fait exister `apps::raccourci` et
//! `apps::reconciliation` sur l'hôte Linux, où leurs tests courent — la
//! « Convention de module enfant » de `CLAUDE.md` n'est donc pas mobilisée :
//! aucun module ne franchit ici de frontière `#[cfg(windows)]`.

use std::time::Duration;

pub mod icone;
pub mod installation;
pub mod raccourci;
pub mod reconciliation;
pub mod sha256;
pub mod surveillance;

#[cfg(windows)]
pub mod boucle;
#[cfg(windows)]
pub mod lancement;
#[cfg(windows)]
pub mod lecture;

/// Entre deux lectures complètes des quatre racines.
///
/// **NON CALIBRÉE.** Ce qui l'autorise est mesuré : sur cette VM, énumérer les
/// quatre racines coûte 31 ms et résoudre les 218 raccourcis 82 ms, soit
/// 113 ms — 0,38 % d'une période. Ce qui n'est PAS mesuré est le délai que
/// l'utilisateur ressent entre l'installation d'une application et son
/// apparition : il vaut jusqu'à trente secondes, et c'est le vrai arbitrage.
///
/// ⚠️ LA RÉCONCILIATION PÉRIODIQUE EST LA SOURCE DE VÉRITÉ, et le restera : la
/// notification par `ReadDirectoryChangesW` d'un sous-bloc ultérieur ne sera
/// qu'un ACCÉLÉRATEUR. Une notification manquée ne doit jamais pouvoir figer
/// un catalogue.
pub const PERIODE_RECONCILIATION: Duration = Duration::from_secs(30);

/// 🔴 `APPS=0` DÉSARME ; UNE SIMPLE PRÉSENCE N'ACTIVE PAS.
///
/// Tester `is_ok()` — ou `is_some()` ici — ACTIVERAIT le mécanisme en écrivant
/// `APPS=0` POUR LE COUPER. C'est la convention de `PLEIN_ECRAN`, `AUDIO` et
/// `PART_SONDAGE`, et `CLAUDE.md` l'écrit pour cette raison exacte.
///
/// La décision est isolée ici PARCE QUE `brancher` est `#[cfg(windows)]` et
/// qu'aucun test d'hôte ne peut donc l'atteindre : ce prédicat, lui, est pur,
/// et c'est la seule part de la garde qu'on puisse voir rougir sur l'hôte.
pub fn desarme(valeur: Option<&str>) -> bool {
    valeur == Some("0")
}

/// Branche la découverte **ET L'INSTALLATION**, ou rend `None` en DISANT
/// laquelle des trois raisons.
///
/// ❌ **CETTE LIGNE NE DISAIT QUE « LA DÉCOUVERTE » JUSQU'AU SOUS-BLOC G3**, et
/// c'était vrai jusqu'à lui. Elle rend désormais DEUX poignées : le fil COM de
/// découverte, et la tâche `tokio` d'installation. Les deux se désarment
/// ensemble par `APPS=0` — déclaré, pas découvert.
///
/// ⚠️ « PAS DE CANAL » N'EST PAS « `APPS=0` », ET UN SILENCE LES CONFONDRAIT.
/// Sans canal il n'y a nulle part où envoyer un catalogue ; c'est un état
/// DIFFÉRENT d'un désarmement explicite, et il doit se lire comme tel dans le
/// journal.
///
/// ⚠️ RIEN N'EST BRANCHÉ EN MODE CAPTEUR, et ce n'est pas un oubli : le mode
/// `CAPTEUR` retourne AVANT l'enrôlement dans `main.rs`, donc un capteur n'a
/// ni canal, ni jeton, ni préfixe, et ne peut porter aucun catalogue.
///
/// ⚠️ **RIEN N'EST BRANCHÉ NON PLUS DANS LE PONT NI DANS UN ENFANT DE FENÊTRE,
/// ET C'EST DEVENU LE CAS ORDINAIRE** (correction du 20 août 2026). Ils tiennent
/// leur jeton du superviseur par `AGENT_JETON` et n'ouvrent aucun canal, pour
/// qu'un seul socket par VM existe. **C'était même une des conséquences du
/// défaut corrigé** : avant, le pont s'enrôlait, donc réconciliait lui aussi,
/// et le travail COM/Shell était fait DEUX FOIS toutes les trente secondes —
/// mesuré, 6 réconciliations en 64 s contre 3 après correction.
///
/// 🔴 D'OÙ LE LIBELLÉ DU `warn!` CI-DESSOUS, QUI NOMME LES DEUX CAUSES SANS
/// PRÉTENDRE LES DISTINGUER : cette fonction ne reçoit qu'une `Option`, elle ne
/// peut pas savoir si le canal manque parce que l'identité est héritée (normal)
/// ou parce que `AGENT_VM`/`AGENT_SECRET` manquent (panne). Les nommer toutes
/// deux vaut mieux qu'un « inactive » sec qui se lit comme une panne dans le
/// cas devenu le plus fréquent. Le processus qui HÉRITE, lui, écrit sa propre
/// ligne juste avant (`main.rs`), et les deux se lisent ensemble.
///
/// Elle prend l'`Option` plutôt que le canal pour que `main.rs` ne reçoive
/// qu'un appel : c'est la règle des 500 lignes appliquée là où elle mord, le
/// fichier étant déjà au-dessus de la porte de 450.
/// Les poignées que `brancher` rend, et que `main.rs` se contente de LIER.
///
/// 🔴 UNE STRUCTURE PLUTÔT QU'UN SECOND RETOUR, ET C'EST UN ENGAGEMENT DE
/// PÉRIMÈTRE : `main.rs` écrit `let _apps = apps::brancher(…)`, et cette ligne
/// **ne bouge pas** — quatre chantiers travaillent en concurrence dans cet
/// arbre, et `main.rs` est le fichier qu'ils touchent tous.
///
/// ⚠️ LES DEUX POIGNÉES SONT CONSERVÉES SANS ÊTRE ATTENDUES : les lâcher
/// terminerait les fils. C'est la raison pour laquelle `main.rs` lie le retour
/// au lieu de l'ignorer, et elle vaut désormais pour deux fils au lieu d'un.
pub struct Poignees {
    /// Le fil COM de découverte.
    _decouverte: Option<std::thread::JoinHandle<()>>,
    /// La tâche tokio d'installation.
    _installation: Option<tokio::task::JoinHandle<()>>,
    /// Le fil de surveillance des quatre racines (sous-bloc G4).
    ///
    /// ⚠️ **TROISIÈME POIGNÉE, ET LA LIGNE DE `main.rs` NE BOUGE TOUJOURS PAS**
    /// — c'est tout l'objet de la structure : `let _apps = apps::brancher(…)`
    /// est le fichier que quatre chantiers concurrents touchent tous.
    _surveillance: Option<std::thread::JoinHandle<()>>,
}

pub fn brancher(canal: Option<&mut crate::plateforme::Canal>) -> Option<Poignees> {
    let Some(canal) = canal else {
        tracing::warn!(
            "decouverte d'applications inactive : ce processus n'a pas de canal /agent \
             (identité héritée du superviseur, ou AGENT_VM/AGENT_SECRET absents)"
        );
        return None;
    };
    // 🔴 `APPS=0` DÉSARME AUSSI L'INSTALLATION, et c'est DÉCLARÉ plutôt que
    // découvert : `demarrer` retourne avant tout, donc aucune des deux moitiés
    // ne se branche. Aucune variable séparée n'est ajoutée pour désarmer la
    // seule installation, faute de besoin démontré — et une variable de plus
    // qui ne servirait à personne est une variable qu'on oubliera de
    // transmettre par `scripts/run-agent.sh`, piège que ce dépôt a payé cinq
    // fois.
    let partage = installation::partage::Partage::neuf();
    // 🔴 LE MODE EST LU ICI, ET SON `info!` EST INCONDITIONNEL. Aucune
    // exécution ne peut alors être mal attribuée : une recette qui lit un
    // verdict sait sous quel mode il a été rendu.
    //
    // ⚠️ **LA TRACE PROUVE QUE LA VARIABLE A ATTEINT LE PROCESSUS ; ELLE NE
    // PROUVE PAS QUE LE MÉCANISME EST COUPÉ** — leçon que le sous-bloc P1 a
    // payée sur `PRESSE_PAPIER=0`. Ce qui discrimine est le COMPTE de lignes
    // `catalogue reconcilie`, jamais la présence de celle-ci.
    let brut = std::env::var("APPS_SURVEILLANCE").ok();
    let (mode, inconnue) = surveillance::mode::Mode::lire(brut.as_deref());
    if let Some(valeur) = inconnue {
        // 🔴 UNE VALEUR INCONNUE EST NOMMÉE, ET LE COMPORTEMENT LIVRÉ EST
        // RETENU. Sans ce `warn!`, une coquille dans une rouge (`seul` pour
        // `seule`) ferait tourner le comportement VERT sous le nom du ROUGE, et
        // la recette lirait un verdict faux — « un contrôle qui ne peut pas
        // échouer », sous une forme neuve.
        tracing::warn!(
            valeur,
            "APPS_SURVEILLANCE : valeur inconnue, le comportement LIVRÉ est retenu \
             (attendu : 0, sans-rebond, seule, ou la variable absente)"
        );
    }
    tracing::info!(mode = ?mode, "mode de surveillance retenu");
    // 🔴 `APPS=0` DÉSARME **AUSSI** LA SURVEILLANCE, ET C'EST DÉCLARÉ PLUTÔT QUE
    // DÉCOUVERT — la formulation que G3 a employée pour l'installation. Sans
    // cette ligne, `APPS=0` couperait la découverte et l'installation, et
    // laisserait un fil de surveillance tourner pour alimenter des compteurs
    // que **plus personne ne sonde** : quatre handles, 256 Kio de pool NON
    // PAGINÉ et un fil, au service de rien.
    //
    // ⚠️ La lecture est faite ICI, en plus des deux `demarrer` qui la font déjà
    // chacun pour eux-mêmes : `desarme` est pur et sa lecture ne coûte rien,
    // là où déduire l’état d’un `Option<JoinHandle>` rendu par un autre étage
    // coupleraient deux mécanismes par une valeur.
    let mode = if desarme(std::env::var("APPS").ok().as_deref()) {
        surveillance::mode::Mode::Desarmee
    } else {
        mode
    };
    let (veille, surveillance) = surveillance::demarrer(mode);
    let decouverte = demarrer(canal, partage.clone(), veille, mode);
    let installation = demarrer_installation(canal, partage);
    if decouverte.is_none() && installation.is_none() {
        return None;
    }
    Some(Poignees {
        _decouverte: decouverte,
        _installation: installation,
        _surveillance: surveillance,
    })
}

/// Le fil d'installation, sur `tokio` — jamais sur le fil COM.
///
/// ⚠️ IL N'EST PAS BRANCHÉ SI LA FILE A DÉJÀ ÉTÉ PRISE : `installations()` rend
/// `None` au second appel, exactement comme `ordres()`, et pour la même
/// raison — deux consommateurs se voleraient les ordres l'un à l'autre.
#[cfg(windows)]
fn demarrer_installation(
    canal: &mut crate::plateforme::Canal,
    partage: installation::partage::Partage,
) -> Option<tokio::task::JoinHandle<()>> {
    if desarme(std::env::var("APPS").ok().as_deref()) {
        return None;
    }
    let installations = canal.installations()?;
    let base = canal.url_signaling().to_string();
    let identite = canal.veille_identite();
    let emetteur = canal.emetteur();
    Some(tokio::spawn(installation::fil::tourner(
        installations,
        move |message| emetteur.emettre(message),
        identite,
        base,
        partage,
    )))
}

/// La variante hors Windows : rien à installer, et **rien à journaliser** —
/// même raison que `demarrer`.
#[cfg(not(windows))]
fn demarrer_installation(
    _canal: &mut crate::plateforme::Canal,
    _partage: installation::partage::Partage,
) -> Option<tokio::task::JoinHandle<()>> {
    None
}

#[cfg(windows)]
fn demarrer(
    canal: &mut crate::plateforme::Canal,
    partage: installation::partage::Partage,
    veille: surveillance::partage::Veille,
    mode: surveillance::mode::Mode,
) -> Option<std::thread::JoinHandle<()>> {
    if desarme(std::env::var("APPS").ok().as_deref()) {
        tracing::warn!("decouverte d'applications DESARMEE (APPS=0)");
        return None;
    }
    let ordres = canal.ordres()?;
    // L'adresse HTTP du téléversement d'icônes est DÉRIVÉE de celle du canal :
    // les deux vivent sur le même service, et deux variables divergeraient.
    let base = canal.url_signaling().to_string();
    let identite = canal.veille_identite();
    let emetteur = canal.emetteur();
    // 🔴 UN VRAI FIL, PAS UN `spawn_blocking`. L'appartement COM appartient à
    // SON fil : `IShellLinkW` et `ShellExecuteExW` doivent tous deux courir
    // sur celui qui a appelé `CoInitializeEx`, et un fil de pool tokio peut
    // servir d'autres tâches entre deux tours.
    std::thread::Builder::new()
        .name("decouverte-apps".into())
        .spawn(move || {
            boucle::tourner(
                move |message| emetteur.emettre(message),
                ordres,
                identite,
                base,
                PERIODE_RECONCILIATION,
                partage,
                veille,
                mode,
            )
        })
        .map_err(|erreur| {
            tracing::error!(%erreur, "fil de decouverte d'applications non demarre");
        })
        .ok()
}

/// La variante hors Windows : il n'y a pas de raccourci à lire.
///
/// Elle existe pour que `main.rs` n'ait pas à porter un `cfg` de plus, et
/// **elle ne journalise rien** : un agent Linux n'a pas à se plaindre de ne
/// pas découvrir d'applications Windows, et la confondre avec `APPS=0` — qui,
/// lui, DIT qu'il est désarmé — brouillerait deux états distincts.
#[cfg(not(windows))]
fn demarrer(
    _canal: &mut crate::plateforme::Canal,
    _partage: installation::partage::Partage,
    _veille: surveillance::partage::Veille,
    _mode: surveillance::mode::Mode,
) -> Option<std::thread::JoinHandle<()>> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seule_la_valeur_zero_desarme_la_decouverte() {
        // 🔴 LA ROUGE : un `valeur.is_some()`. Il rendrait `true` pour `"1"`,
        // pour `""` et pour n'importe quoi — c'est-à-dire qu'écrire `APPS=0`
        // POUR COUPER la découverte l'activerait, et qu'écrire `APPS=1` POUR
        // L'ACTIVER la couperait. Les deux erreurs se compensent au point que
        // personne ne les verrait sans ce test.
        assert!(desarme(Some("0")));
        for valeur in [None, Some(""), Some("1"), Some("00"), Some("0 "), Some("false")] {
            assert!(!desarme(valeur), "{valeur:?} ne doit PAS désarmer");
        }
    }

    #[test]
    fn la_periode_de_reconciliation_laisse_la_place_a_son_propre_cout() {
        // Les 113 ms mesurés sur cette VM (31 ms d'énumération + 82 ms de
        // résolution COM) doivent rester une fraction négligeable de la
        // période, sans quoi la boucle passerait son temps à se lire elle-même.
        assert!(PERIODE_RECONCILIATION >= Duration::from_secs(5));
    }
}
