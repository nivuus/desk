//! Le registre global du sommeil : un `Vivier` partagé, un canal d'ordres par
//! fenêtre, et le tour de roue qui débloque l'hystérésis.
//!
//! **Il ne décide rien.** Toute la logique est dans `vivier.rs`, qui est pur et
//! testé ; ce module ne fait que la brancher sur des canaux.
//!
//! **Pourquoi un état global de processus plutôt qu'un objet passé de main en
//! main.** Chaque fenêtre du capteur vit sur son propre fil, créé par
//! `serveur::ouvrir_les_commandes`, et l'arbitrage est par nature transverse :
//! le signal d'une fenêtre peut endormir sa voisine. Le registre des attentes
//! de connexion média (`serveur.rs`) emploie déjà exactement ce patron, pour
//! la même raison. C'est aussi ce qui permet à ce sous-bloc de **ne pas
//! toucher `serveur.rs`**, dont la marge de taille est de 10 lignes.

// `parts` porte le calcul et la distribution des parts de débit. Extrait pour
// la même raison que `fenetre::transitions` : ce fichier a franchi le
// plafond de 500 lignes du projet en y ajoutant le remède au canal rompu
// détecté par cette voie-là (voir `parts::distribuer_les_parts`). Il ne
// s'appelle pas `repartiteur` : ce nom est déjà pris par le module qui porte
// la RÈGLE pure ; celui-ci ne porte que sa BRANCHE sur ce registre.
mod parts;
mod porteurs;

// Le registre lui-même — `Etat`, `etat`, le tour de roue, `distribuer`,
// `oublier`, `inscrire`, `retirer` — extrait pour rester sous le plafond de
// 500 lignes du projet (revue de la tâche 10, D9) : ce fichier-ci était
// tombé à exactement 500 avec ce bloc en ligne. Voir l'en-tête de
// `registre.rs`. Les quatre re-exports ci-dessous rendent l'extraction
// invisible à `parts.rs`/`porteurs.rs`/`tests.rs`, qui continuent d'écrire
// `super::{distribuer, oublier, Etat, Message}` sans le savoir.
mod registre;
use registre::{distribuer, etat, oublier, Etat};
pub use registre::{inscrire, retirer};

use std::collections::HashMap;
// `Receiver`, `Mutex` et `MutexGuard` : plus employés par le code de PRODUCTION
// de ce fichier depuis l'extraction ci-dessus — seul `sommeil::tests` s'en
// sert encore (`premier_ordre`, `VERROU_TESTS`), via `use super::*`. Gater sur
// `cfg(test)` évite un `unused_imports` en dehors de la compilation de test,
// sans toucher `tests.rs`.
#[cfg(test)]
use std::sync::mpsc::Receiver;
#[cfg(test)]
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

use crate::capteur::vivier::{Ordre, Raison};

/// Période du tour de roue. Ni une cadence de rendu ni une horloge : c'est le
/// seul moyen pour une fenêtre bloquée sous hystérésis d'être réexaminée, et
/// 250 ms est très en deçà des 2 s d'hystérésis tout en restant négligeable.
const PERIODE_REARBITRAGE: Duration = Duration::from_millis(250);

/// Répit avant qu'une fenêtre dont la capture audio est morte ne redevienne
/// éligible au portage.
///
/// **Il finance le cas MAJORITAIRE** — une application, une fenêtre, donc
/// aucune voisine à promouvoir. Sans lui, le remède ne couvrirait que les
/// applications multi-fenêtres et le groupe resterait muet sans retour,
/// exactement comme avant D9. Réélire la même session construit une activation
/// *process loopback* NEUVE, ce qui est une chance réelle : les causes connues
/// d'un refus de lecture WASAPI — changement de périphérique, redémarrage du
/// service audio, changement de format — sont transitoires.
///
/// ⚠️ **NON CALIBRÉE.** Aucune mesure ne la fonde : elle rejoint `BPP_MIN`,
/// `FACTEUR_FOCUS`, `PART_DORMANTE_BPS`, `HYSTERESIS`, `REPIT_APRES_ECHEC` et
/// `TAILLE_MAX_SORTIE`.
pub const REPIT_REARMEMENT_AUDIO: Duration = Duration::from_secs(5);

/// Nombre de réarmements consécutifs avant abandon définitif.
///
/// Sans borne, un périphérique audio définitivement mort ferait tourner le
/// cycle « inapte → répit → réélue → morte » sans fin, et chaque tour coûte
/// une activation COM.
///
/// ⚠️ **NON CALIBRÉE**, comme la précédente. L'abandon définitif est
/// **journalisé**, jamais muet — c'est la contrainte que F3 de D7 a posée.
pub const REARMEMENTS_MAX: u32 = 5;

/// Ce qu'une fenêtre reçoit du registre global.
///
/// **Un seul canal pour les deux**, et non deux canaux parallèles : ce qu'un
/// canal unique garantit est l'ordre de LIVRAISON — deux canaux parallèles
/// laisseraient une part d'endormie doubler l'ordre de dormir qui la motive,
/// et la fenêtre serait momentanément décrite comme endormie alors qu'elle
/// encode encore.
///
/// ⚠️ **Il ne garantit PAS l'ordre de CALCUL, et la distinction n'est pas
/// théorique** (I2, revue finale de branche du sous-bloc D6). Les appelants
/// respectent bien « `distribuer` puis `distribuer_les_parts` », sauf un : le
/// chemin `rompus` de `distribuer_les_parts` envoie les parts d'abord, puis
/// retire du vivier les sessions dont le canal est rompu, puis seulement
/// relaie les ordres que ce retrait engendre. Une session réveillée par la
/// place ainsi libérée reçoit son `Reveiller` APRÈS une part d'endormie déjà
/// périmée, et ne reçoit sa part d'éveillée qu'au tour de roue suivant.
/// **Borne : `PERIODE_REARBITRAGE`, 250 ms au plancher `PART_DORMANTE_BPS`.**
/// Conséquence assumée : la recalculer sur place demanderait une seconde passe
/// de parts sous le même verrou, pour 250 ms de plancher sur un chemin qui ne
/// s'emprunte qu'à la mort inopinée d'un fil de fenêtre.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Message {
    Sommeil(Ordre),
    Part { bps: u32 },
    /// Ordre de porter le son, ou de se taire. Poussé **au changement
    /// seulement**, comme `Part`.
    ///
    /// Sur le même canal que les deux autres, et pour la même raison : au
    /// sein du canal d'UNE session, un canal unique garantit l'ordre de
    /// LIVRAISON entre `Sommeil`, `Part` et `Audio`. **Cela ne s'étend pas
    /// entre deux sessions** : l'ancienne porteuse et la nouvelle ont chacune
    /// leur propre canal, lu par son propre fil de fenêtre. `porteurs::
    /// distribuer_l_audio` envoie l'ordre de se taire avant celui de porter,
    /// ce qui RÉDUIT la fenêtre où les deux fenêtres d'un même processus
    /// seraient audibles ensemble — sans la fermer : la borne réelle est
    /// l'ordonnancement des deux fils, pas ce canal.
    Audio { actif: bool },
}

pub fn signaler(session: &str, visible: bool, focalisee: bool) {
    let mut garde = etat();
    if focalisee {
        garde.focalisee = Some(session.to_string());
        // Le rang du focus, et non un booléen : c'est lui qui fait tenir la
        // règle 3 de l'arbitrage — un groupe qui perd tout focus garde son son
        // sur la DERNIÈRE à l'avoir eu.
        garde.horloge += 1;
        let rang = garde.horloge;
        garde.derniers_focus.insert(session.to_string(), rang);
    } else if garde.focalisee.as_deref() == Some(session) {
        garde.focalisee = None;
    }
    let ordres = garde.vivier.signaler(session, visible, focalisee, Instant::now());
    distribuer(&mut garde, ordres);
    parts::distribuer_les_parts(&mut garde);
    porteurs::distribuer_l_audio(&mut garde);
}

/// Signale l'échec de la reconstruction du `WindowsSource` lors d'un réveil.
///
/// Appelée par la tâche 6 quand la reconstruction du `WindowsSource` échoue
/// après un `Ordre::Reveiller` : sans ce chemin de retour, le vivier croirait
/// la fenêtre éveillée pour toujours et ne la reproposerait jamais au tour de
/// roue.
pub fn echec_de_reveil(session: &str) {
    let mut garde = etat();
    let ordres = garde.vivier.echec_de_reveil(session, Instant::now());
    distribuer(&mut garde, ordres);
    parts::distribuer_les_parts(&mut garde);
    porteurs::distribuer_l_audio(&mut garde);
}

/// Une session signale que sa capture audio est morte.
///
/// **Ne répond rien, et c'est voulu** : l'arbitrage est global et la décision
/// peut concerner une AUTRE fenêtre. L'effet revient par
/// `DepuisCapteur::Audio`, poussé sur la connexion média de chaque fenêtre
/// concernée — exactement le patron de `signaler`.
pub fn audio_mort(session: &str) {
    let mut garde = etat();
    // Une session déjà inapte (répit en cours, ou abandon définitif) qui
    // signale À NOUVEAU une capture morte n'a pas échoué une seconde fois :
    // c'est le MÊME échec, redit — typiquement une reconnexion de canal sur
    // un capteur resté vivant (`SourceDistante::rattacher`, sous-bloc D9),
    // qui ne peut structurellement pas distinguer ce cas d'un vrai
    // redémarrage du capteur et remet donc `Session::audio_mort_signale` à
    // zéro dans les deux cas. Compter ce signal comme un échec CONSÉCUTIF de
    // plus rapprocherait l'abandon définitif de 24 h pour une raison
    // étrangère à l'état réel de la capture. **Journalisé, jamais tu** :
    // le silence est précisément le défaut que F3 de D7 a corrigé.
    if garde.inaptes.contains_key(session) {
        // `info!`, pas `debug!` : l'exploitation tourne en `RUST_LOG=info`
        // (voir `encode/arret.rs`), et un signal muet ici serait exactement
        // le défaut que ce commentaire vient d'expliquer comment éviter.
        tracing::info!(
            %session,
            "capture audio morte signalée à nouveau pour une session déjà \
             inapte : signal redondant, réarmement non recompté"
        );
        return;
    }
    let tours = garde.rearmements.entry(session.to_string()).or_insert(0);
    *tours += 1;
    if *tours > REARMEMENTS_MAX {
        // Abandon définitif, JOURNALISÉ. Un silence muet est précisément le
        // défaut que F3 de D7 a corrigé ; ne pas le réintroduire ici.
        tracing::warn!(
            %session,
            rearmements = *tours - 1,
            "capture audio morte et abandon définitif : le groupe de PID restera muet"
        );
        garde.inaptes.insert(session.to_string(), Instant::now() + Duration::from_secs(86_400));
    } else {
        tracing::info!(
            %session,
            rearmement = *tours,
            repit = ?REPIT_REARMEMENT_AUDIO,
            "capture audio morte, réarmement programmé"
        );
        garde.inaptes.insert(session.to_string(), Instant::now() + REPIT_REARMEMENT_AUDIO);
    }
    porteurs::distribuer_l_audio(&mut garde);
}

/// Retire du registre les inaptitudes dont le répit a expiré.
///
/// **Nommée et séparée pour être ÉPROUVABLE** : le tour de roue (250 ms) est
/// ce qui rend le répit effectif, et sans cette purge une inapte le resterait
/// jusqu'au prochain événement, qui peut ne jamais venir. Un `retain` en ligne
/// dans le tour de roue ne serait couvert par aucun test.
pub(super) fn purger_les_inaptitudes(inaptes: &mut HashMap<String, Instant>, maintenant: Instant) {
    inaptes.retain(|_, echeance| *echeance > maintenant);
}

/// Ce `retirer` est-il périmé, c'est-à-dire adressé à une instance déjà
/// remplacée par un rattachement ?
///
/// **Nommé et séparé pour être ÉPROUVABLE** : la logique en ligne dans
/// `retirer` ne serait couverte par aucun test, `retirer` touchant un état
/// global (`OnceLock<Mutex<Etat>>`) qu'un test d'hôte ne peut pas isoler.
pub(super) fn retirer_est_perime(
    generations: &HashMap<String, u64>,
    session: &str,
    generation: u64,
) -> bool {
    generations
        .get(session)
        .is_some_and(|courante| *courante > generation)
}

/// Le texte que le client recevra. **Stable** : il traverse deux protocoles et
/// s'affiche à l'utilisateur.
pub fn raison_en_texte(raison: Raison) -> &'static str {
    match raison {
        Raison::Masquee => "masquee",
        Raison::Evincee => "evincee",
    }
}

// Extrait dans un fichier voisin : cette suite portait `sommeil.rs` à 500
// lignes pour un plafond de projet à 500, marge nulle dès sa naissance. Voir
// l'en-tête de `sommeil/tests.rs`.
#[cfg(test)]
mod tests;
