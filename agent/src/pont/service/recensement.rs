//! Le **recensement** du pont : la ligne des douze causes, celle du banc de
//! latence de F4, et la complétion de masse qui les accompagne.
//!
//! **EXTRAIT PLUTÔT QUE COMPRIMÉ**, et **AVANT l'addition qui l'imposait** —
//! la doctrine du dépôt est de faire l'extraction avant d'avoir franchi le
//! plafond, pas après. `service.rs` était à **458 lignes pour une porte à
//! 460** (plan F4, §2.4) quand F4 y a câblé son histogramme ; les trois blocs
//! ci-dessous en sont partis **verbatim**, doc-comments compris.
//!
//! ⚠️ **Un `mod` ORDINAIRE, à l'intérieur de son parent** : il n'y a aucune
//! frontière `#[cfg(windows)]` à franchir ici — tout `pont::service` est déjà
//! `#[cfg(windows)]` —, donc la convention `#[path]` de `CLAUDE.md` **ne
//! s'applique pas**, comme pour les deux extractions de D11.

use std::sync::atomic::Ordering;
use std::time::Instant;

use windows::core::HRESULT;

use super::{oublier_contexte, prevenir_l_ecriture, verbes};
use crate::pont::erreurs::Erreur;
use crate::pont::projfs::Etat;

/// **F4** — le banc de latence est-il armé ?
///
/// 🔴 **`=1` ARME ; l'ABSENCE désarme.** C'est la convention de `MICRO_MESURE`,
/// **et NON celle de `PONT`, `PONT_ECRITURE` et `PONT_MUTATION`**, qui sont
/// désarmées par `=0`. La règle du dépôt est : *on désarme sur `=0` ce qui est
/// LIVRÉ, on arme sur `=1` ce qui ne l'est pas.* Le pont, l'écriture et les
/// mutations sont livrés ; la ligne de latence est un **instrument de banc,
/// jamais une configuration livrée**.
///
/// ⚠️ **Ce qu'elle arme est l'ÉMISSION, pas la COLLECTE** : voir
/// [`crate::pont::projfs::Etat::latences`].
pub(super) fn mesure_armee() -> bool {
    static ARMEE: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ARMEE.get_or_init(|| {
        let armee = std::env::var("PONT_MESURE").map(|v| v == "1").unwrap_or(false);
        if armee {
            // ⚠️ **`warn!`, comme `PART_SONDAGE` et `AUDIO_FAUTE_*`** : c'est ce
            // qui la rend visible sous `RUST_LOG=info` et ce qui empêche de la
            // confondre avec une configuration ordinaire.
            tracing::warn!(
                "banc de latence du pont ARME (PONT_MESURE=1) : instrument de banc, \
                 jamais une configuration livree"
            );
        }
        armee
    })
}

/// La ligne de recensement — **l'instrument du critère (4) de F3**.
///
/// ```text
/// codes rendus total=17 introuvable=3 chemin-introuvable=1 acces-refuse=0 …
/// ```
///
/// 🔴 **UN CODE JAMAIS PRODUIT AFFICHE `0`, ET C'EST TOUT L'INTÉRÊT.** Le
/// critère (4) — « chacun des douze est observé au moins une fois » — devient
/// alors un `grep` sur UNE ligne, et il **ne peut pas être satisfait par
/// accident** : une exécution qui n'exerce rien rend douze zéros.
///
/// ⚠️ **CE QUE CE RECENSEMENT NE PEUT PAS DIRE, mesuré par la recette de F3** :
/// il est émis **à la fermeture du canal**, et le fil du service RETOURNE
/// aussitôt. Un code produit APRÈS cette fermeture — `CanalFerme` sur un geste
/// qui arrive alors qu'il n'y a plus de navigateur — est bien **compté**, et
/// **personne ne l'imprime**. Le compteur est juste ; la ligne qui le rend
/// observable, elle, est déjà partie.
///
/// ⚠️ **`info!` et non `debug!`** : `scripts/run-agent.sh` pose `RUST_LOG=info`
/// par défaut, et la doctrine de ce dépôt est que l'exploitation y tourne. Une
/// mitigation muette n'en est pas une — c'est la raison écrite pour les deux
/// traces de `encode/arret.rs`, appliquée ici.
pub(super) fn recenser(etat: &Etat) {
    // ── LE RELEVÉ DE LA TABLE — l'instrument du legs n°4 de F1 ────────────
    //
    // 🔴 **C'est ce qui départage les quatre hypothèses**, et aucune n'était
    // départageable jusqu'ici. F1 a mesuré des lectures qui CALENT sans jamais
    // expirer — `commande expirée` reste à 0 pendant 540 s — et déclare qu'on
    // ne sait pas OÙ le blocage se produit. Voir `pont::table::plus_ancienne`,
    // qui porte le tableau de lecture.
    //
    // ⚠️ **Une ligne toutes les 10 s, jamais une par rappel.** Le chantier TURN
    // a payé 18 619 lignes en quelques secondes pour une trace par paquet,
    // écrites sur un partage CIFS depuis la boucle : la mesure détruisait ce
    // qu'elle mesurait.
    let maintenant = Instant::now();
    let (en_vol, sans_commande, plus_ancienne_ms) = match etat.table.lock() {
        Ok(table) => (
            table.en_vol(),
            table.sans_commande(),
            table.plus_ancienne(maintenant).map(|d| d.as_millis()).unwrap_or(0),
        ),
        // ⚠️ **Un verrou empoisonné est DIT, pas tu.** Rendre des zéros ferait
        // lire « rien en vol » là où la table est inaccessible — c'est-à-dire
        // la PREMIÈRE ligne du tableau de lecture, qui accuserait le rappel.
        Err(_) => {
            tracing::warn!("recensement impossible : le verrou de la table est empoisonne");
            return;
        }
    };
    let sessions = etat.sessions.lock().map(|s| s.len()).unwrap_or(0);
    tracing::info!(
        "pont en vol={} sans_commande={} plus_ancienne_ms={} sessions={} octets_hydrates={} \
         entrees_hydratees={}",
        en_vol,
        sans_commande,
        plus_ancienne_ms,
        sessions,
        etat.octets_hydrates.load(Ordering::Relaxed),
        etat.entrees_hydratees.load(Ordering::Relaxed),
    );

    let manquants: Vec<&str> = etat
        .compteurs
        .manquants()
        .into_iter()
        .map(crate::pont::compteurs::nom)
        .collect();
    tracing::info!(
        // ⚠️ **Un champ `tracing` porterait des séquences ANSI entre son nom et
        // sa valeur sur un journal BRUT** — c'est le piège que la recette
        // d'entrée de D8 a payé, et que le `grep` de F1 a rejoué trois fois.
        // Le recensement est donc **une chaîne unique**, `nom=valeur` séparés
        // par des espaces, et il se lit tel quel sans `sed`.
        "codes rendus {} | jamais rendus : {}",
        etat.compteurs.recensement(),
        if manquants.is_empty() { "aucun".to_string() } else { manquants.join(",") }
    );

    // ── LE BANC DE LATENCE (F4) — émis SEULEMENT si `PONT_MESURE=1` ────────
    //
    // ⚠️ **Les compteurs sont CUMULATIFS depuis le démarrage du pont** : une
    // mesure se lit par DIFFÉRENCE entre deux recensements, jamais sur une
    // ligne isolée. C'est pourquoi tout palier de mesure dure au moins six
    // périodes.
    if mesure_armee() {
        // ⚠️ **Chaîne unique, jamais des champs `tracing`** — même raison qu'au
        // recensement des codes, trois lignes plus haut.
        tracing::info!("{}", etat.latences.recensement());
    }
}

/// Complète en erreur tout ce qui reste en vol. Appelée quand plus aucune
/// réponse ne peut arriver.
pub(super) fn tout_completer(etat: &Etat, cause: Erreur) {
    let restantes = match etat.table.lock() {
        Ok(mut table) => table.vider(),
        Err(empoisonne) => empoisonne.into_inner().vider(),
    };
    for (commande, correlation) in restantes {
        oublier_contexte(etat, correlation);
        // ⚠️ **Une écriture abandonnée doit être DITE au fil d'écriture**, sans
        // quoi sa poussée resterait « en vol » à jamais et la file n'avancerait
        // plus. L'entrée, elle, RESTE au journal — c'est le fil qui décide, et
        // c'est ce qui la rend récupérable.
        prevenir_l_ecriture(etat, commande, correlation, cause);
        verbes::completer(etat, commande, HRESULT(etat.compteurs.rendre(cause)));
    }
}
