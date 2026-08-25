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
// `file` porte la RÈGLE PURE de coalescence du canal d'une session : aucun
// verrou, aucun `cfg`, aucune API Windows — c'est ce qui la rend éprouvable
// par `cargo test --workspace` sur l'hôte Linux, alors que ce fichier entier
// ne l'est pas. Le verrou et le réveil restent chez cet appelant (tâche
// ultérieure) ; ce module ne décide QUE si un dépôt s'empile, coalesce, ou
// est refusé.
pub(crate) mod file;
mod parts;
mod porteurs;
// `presse_papier` porte la DISTRIBUTION du presse-papier de la VM, extraite
// au même endroit que `parts` et `porteurs` — mais PAS pour la même raison, et
// il faut le dire : `parts` a été extrait d'un fichier qui avait FRANCHI 500
// lignes, alors que celui-ci en fait 352 et garde 148 de marge. Ce qui la
// justifie est que le module inliné, ses trois tests compris, aurait porté
// `sommeil.rs` au-delà du plafond. ⚠️ La rédaction initiale disait « ce
// fichier-ci est proche de son plafond » : c'était faux à l'écriture (328
// lignes alors), et corrigé par la revue transverse du 20 août 2026 — **le
// geste était bon, sa raison écrite ne l'était pas**.
// Il ne s'appelle pas comme le module racine
// `crate::presse_papier` par confusion — celui-là porte la RÈGLE pure, celui-ci
// sa seule branche sur ce registre.
mod presse_papier;

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
// `ReceveurSession`, `Mutex` et `MutexGuard` : plus employés par le code de
// PRODUCTION de ce fichier depuis l'extraction ci-dessus — seul
// `sommeil::tests` s'en sert encore (`premier_ordre`, `VERROU_TESTS`), via
// `use super::*`. Gater sur `cfg(test)` évite un `unused_imports` en dehors de
// la compilation de test, sans toucher `tests.rs`.
//
// ⚠️ `Receiver` jusqu'au 25 août 2026 : le canal du registre n'est plus un
// `std::sync::mpsc` non borné (voir `file.rs`).
#[cfg(test)]
use file::ReceveurSession;
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
/// ❌ **CE MÉCANISME EST INERTE POUR LE CAS MAJORITAIRE, et une rédaction
/// antérieure de ce commentaire affirmait le contraire à tort** (constaté
/// par la recette VM de la tâche 15, sous-bloc D9, revue finale). Elle
/// disait : « réélire la même session construit une activation *process
/// loopback* NEUVE ». **C'est faux, vérifié sur le code** :
/// `WindowsAudioSource` n'est construite QU'UNE FOIS, au démarrage de
/// l'enfant (`demarrage/audio.rs::brancher`, appelé une seule fois, sans
/// boucle) ; le fil de capture (`windows_audio.rs`), une fois
/// `capture_morte` posé, exécute un `return` DÉFINITIF et ne relit plus
/// jamais rien ; et réélire la MÊME session ne fait que pousser
/// `Audio { actif: true }`, qui aboutit à `AudioSource::set_actif(true)`
/// (`windows_audio.rs::set_actif`) — **lequel n'écrit qu'un booléen atomique
/// que ce fil mort ne lira plus jamais**. Rien, nulle part, ne reconstruit
/// la source.
///
/// ✅ **La branche PROMOTION, elle, reste valide** : une voisine du même
/// groupe de PID a SA PROPRE `WindowsAudioSource`, construite à SON PROPRE
/// démarrage, sur un fil de capture qui n'a jamais échoué — l'élire lui
/// donne réellement le son. C'est le répit lui-même — la RÉÉLECTION DE LA
/// MÊME SESSION SANS VOISINE — qui ne restaure rien.
///
/// **Conséquence assumée** : le cas MAJORITAIRE — une application, une
/// fenêtre, donc aucune voisine à promouvoir — reste SANS REMÈDE. Le répit
/// fait taire puis reparler la bonne session au niveau du REGISTRE (le
/// capteur cesse de la croire inapte, lui renvoie `Audio { actif: true }`),
/// mais aucun son ne sort réellement côté enfant tant que sa capture n'a pas
/// été reconstruite — ce que rien ne fait. **Ceci reste une dette, pas un
/// remède partiel** : le chemin de reconstruction n'existe pas, et devrait
/// vivre à l'intersection de trois fichiers déjà nommés dans ce commentaire —
/// `demarrage/audio.rs` (qui construit la source aujourd'hui, une fois),
/// `transport/piste_audio.rs` (qui la porte via `appliquer_audio`), et
/// `windows_audio.rs` (qui tient le fil et le témoin `capture_morte`) —
/// légué au sous-bloc suivant.
///
/// ✅ **CE CHEMIN EXISTE DÉSORMAIS, câblé de bout en bout (sous-bloc D10,
/// tâches 11 et 12) : le cas MAJORITAIRE N'EST PLUS SANS REMÈDE.**
/// `Session::reconstruire_ou_signaler` (`transport/piste_audio.rs`) tente
/// D'ABORD de refabriquer la source — exactement à l'intersection nommée
/// ci-dessus, `demarrage/audio.rs` fournissant le reconstructeur — et
/// n'appelle `audio_mort` (donc ce répit et cette promotion) qu'en REPLI :
/// quand son propre budget de tentatives (`crate::audio::RECONSTRUCTIONS_MAX`)
/// est épuisé, ou qu'il n'existe aucun reconstructeur (`AUDIO=0`, ou toute
/// session dont l'ouverture audio initiale a échoué : là, le comportement
/// d'avant D10 — signaler immédiatement — reste exactement conservé).
///
/// ❌ **« Chemin mono-fenêtre » figurait dans cette liste, et c'est faux :
/// `demarrage/audio.rs::brancher` pose un reconstructeur dans les DEUX
/// modes** (revue transverse de fin de branche). Le mono-fenêtre reconstruit
/// donc lui aussi, `RECONSTRUCTIONS_MAX` fois, avant de signaler. ⚠️ **Le
/// remède y a été INERTE pour une autre raison** : la source reconstruite y
/// était réarmée sur `audio_porteuse`, qu'aucun ordre de capteur ne venait
/// jamais poser en l'absence de capteur. ✅ **« Léguée et non corrigée » ne
/// l'est plus — leg n°4 de D10, CORRIGÉ au sous-bloc D11** : le branchement
/// mono-fenêtre pose lui-même `audio_porteuse` (`demarrage/audio.rs`), et la
/// recette ① mesure le son revenu (441 Hz contre la sentinelle au rouge) —
/// voir `Session::reconstruire_ou_signaler` (`transport/piste_audio.rs`). Ce que
/// CE mécanisme-ci (le répit et la
/// promotion) continue de faire, inchangé : donner sa chance à une voisine du
/// même groupe de PID, et éviter qu'un périphérique définitivement mort ne
/// fasse tourner le cycle sans fin. Et la preuve que la reconstruction a
/// réellement rendu du son — pas seulement réussi à s'ouvrir — referme le
/// cycle de réarmements : voir `signaler_audio_vivant` plus bas, et le leg 6
/// de D9 qu'il ferme (`capteur/sommeil/porteurs.rs`).
///
/// ⚠️ **Précision apportée en REVUE de la tâche 12 : le rôle de la
/// réélection ne s'arrête pas à « donner sa chance à une voisine ».** Le
/// budget de tentatives (`crate::audio::RECONSTRUCTIONS_MAX`) n'est posé
/// qu'UNE FOIS à la construction de la `Session`, et sans réapprovisionnement
/// le cycle décrit ci-dessus ne pouvait tourner qu'une seule fois PAR
/// SESSION : après le premier `AudioMort`, le verrou `audio_mort_signale` (qui
/// ne retombe qu'à un rattachement) empêchait toute nouvelle tentative,
/// quelle que soit la durée de vie restante de la session. C'est la
/// réélection ELLE-MÊME — la transition vers `Audio { actif: true }`,
/// `Session::appliquer_audio`, `transport/piste_audio.rs` — qui
/// réapprovisionne ce budget et lève ce verrou. Sans ce second rôle,
/// `REARMEMENTS_MAX` juste en dessous n'aurait jamais compté qu'un seul échec
/// par session, jamais plusieurs échecs CONSÉCUTIFS — l'inverse de son
/// intention.
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
    /// Le presse-papier de la VM a changé (sous-bloc P1). Poussé **au
    /// changement seulement**, comme `Part` et `Audio` — c'est
    /// `crate::presse_papier::Sondeur` qui porte les gardes, pas ce registre.
    ///
    /// `texte` vaut `None` sur un REFUS de taille, `octets` portant alors la
    /// taille refusée : le contenu est refusé, jamais tronqué, et le refus est
    /// DIT à l'utilisateur (D4). Un `None` n'est donc pas « rien à annoncer ».
    ///
    /// ⚠️ **C'est de très loin le plus gros message de ce canal** : jusqu'à
    /// `crate::presse_papier::PRESSE_PAPIER_MAX` (64 Kio), là où `Sommeil`,
    /// `Part` et `Audio` pèsent quelques octets — **quatre** ordres de
    /// grandeur, et non « deux » comme cette phrase l'annonçait d'abord. Le
    /// « deux ordres de grandeur » de `protocole.rs`, sa voisine, est exact
    /// (8 Mio contre 64 Kio) : les deux phrases n'employaient pas la même
    /// échelle (revue transverse, 20 août 2026).
    ///
    /// 🔴 ~~Le canal du registre est NON BORNÉ (`std::sync::mpsc::channel`),
    /// donc `send` ne bloque jamais — mais un fil de fenêtre bloqué
    /// accumulerait ces messages. Borné en pratique par le fait qu'on n'émet
    /// qu'au changement et que le garde d'égalité de contenu supprime les
    /// répétitions ; nommé ici plutôt que découvert, et à surveiller si P3
    /// mesure une fenêtre lente.~~ **FAUX DEPUIS LE 25 AOÛT 2026 : le canal
    /// est BORNÉ**, par `capteur/sommeil/file.rs` — `PROFONDEUR_MAX` (64
    /// messages), et un dépôt au-delà est **REFUSÉ et COMPTÉ**, jamais
    /// bloquant ni tronqué. Le « borné en pratique » ci-dessus était un
    /// raisonnement de bonne foi sur le régime NORMAL : c'est le régime
    /// ANORMAL — un fil de fenêtre qui cesse de lire — qu'il ne bornait pas,
    /// et c'est celui-là qui coûtait la mémoire. ⚠️ **`PressePapier` n'est PAS
    /// coalescable** (elle porte la donnée de l'utilisateur), donc c'est la
    /// variante par laquelle la borne se heurte réellement : 64 messages ×
    /// 64 Kio, soit 4 Mio au pire pour une fenêtre bloquée.
    PressePapier { texte: Option<String>, octets: u32 },
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

/// Écrit `texte` dans le presse-papier de la VM (sens navigateur → VM).
///
/// **Le capteur est le seul propriétaire du presse-papier** (D1) : c'est
/// pourquoi cette porte existe, et pourquoi aucun enfant n'écrit lui-même.
///
/// Rend `Err` quand l'écriture a échoué — refus d'ouverture par une autre
/// application (cas NORMAL sous Windows), ou mécanisme désarmé. **L'appelant
/// n'injecte alors PAS `Ctrl+V`** : la touche est perdue, pas reportée (D6).
pub fn ecrire_le_presse_papier(texte: &str) -> anyhow::Result<()> {
    presse_papier::ecrire(texte)
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

/// Une session apporte la PREUVE que sa capture audio est repartie : un
/// paquet réel, pas seulement une reconstruction qui a rendu `Ok` (sous-bloc
/// D10, ferme le leg 6 de D9).
///
/// **Ne répond rien, et ne ré-arbitre rien** : contrairement à `audio_mort`,
/// cette preuve ne concerne jamais qu'une seule session — la sienne — donc
/// rien à distribuer à une voisine. Elle referme seulement le cycle de
/// réarmements sur une PREUVE plutôt que sur la seule décision d'arbitrage
/// (voir `sommeil/porteurs.rs`, dont la remise à zéro vivait ici jusqu'à ce
/// signal).
pub fn signaler_audio_vivant(session: &str) {
    etat().rearmements.remove(session);
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
