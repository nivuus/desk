//! Le corps du fil de capture WASAPI.
//!
//! Extrait de `windows_audio.rs` à la tâche 3 du sous-bloc D10, au point de
//! chute que `CLAUDE.md` nomme depuis le sous-bloc D7. L'extraction précède
//! l'addition de la tâche 13 (`AUDIO_FAUTE_LECTURE`).
//!
//! `#![cfg(windows)]` comme son parent : aucun test d'hôte ne peut le couvrir,
//! et c'est précisément pourquoi la tâche 13 lui donne une injection de faute.

#![cfg(windows)]

use super::*;

/// Corps du fil de capture audio, lancé par `WindowsAudioSource::demarrer`.
///
/// Toutes les valeurs sont passées explicitement — aucune n'est plus
/// capturée par une fermeture — et leur nom reprend celui des variables
/// `_fil` de l'appelant, pour que la correspondance reste lisible d'un
/// fichier à l'autre.
pub(super) fn tourner(
    mut capture: Capture,
    mut encodeur: OpusEncoder,
    origin: Instant,
    ring_fil: PacketRing,
    arret_fil: Arc<AtomicBool>,
    perte_desiree_fil: Arc<AtomicI32>,
    emet_fil: Arc<AtomicBool>,
    capture_morte_fil: Arc<AtomicBool>,
    pid_fil: Option<u32>,
) {
    // Ce fil appelle lui-même des méthodes COM — `read()` à chaque
    // tour, et `Stop()` via le `Drop` de `LoopbackCapture` en
    // sortant — alors que `open()` a initialisé COM sur le fil
    // APPELANT, pas sur celui-ci. Microsoft exige que tout fil
    // invoquant des méthodes COM ait d'abord rejoint un
    // appartement.
    //
    // Résultat volontairement ignoré ICI — contrairement à
    // `wasapi::open`, qui lui **vérifie** son `HRESULT` et refuse
    // `RPC_E_CHANGED_MODE` (voir son commentaire, dont dépend
    // `unsafe impl Send for LoopbackCapture`) : ce fil-ci vient
    // d'être créé par `thread::Builder::spawn` juste au-dessus,
    // il n'a donc encore rejoint aucun appartement COM, et
    // `CoInitializeEx` y rend nécessairement `S_OK`. `open()`,
    // lui, s'exécute sur un fil quelconque — potentiellement
    // recyclé, potentiellement déjà lié à une STA — d'où la
    // vérification qui n'a pas lieu d'être répétée ici.
    //
    // Symétriquement, PAS de `CoUninitialize` : voir le motif
    // détaillé dans `wasapi.rs`.
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
    }

    let mut assembleur = FrameAssembler::new(origin);
    let mut dernier_rapport = Instant::now();
    // Dernière valeur effectivement posée sur l'encodeur. Un
    // appel CTL par trame de 10 ms serait du gaspillage sur ce
    // chemin chaud : on ne réécrit que lorsque la cible a changé.
    let mut derniere_perte: i32 = 0;
    // Dernier ordre pour lequel une bascule a été TENTÉE (que
    // `capture.emettre` ait réussi ou non). Sert uniquement à ne
    // pas rappeler `Start()`/`Stop()` à chaque tour à ~200 Hz : ce
    // n'est PAS l'état réel du flux, voir `emettait`.
    let mut voulu_applique = false;
    // État RÉEL du flux : vrai seulement quand `Start()` a
    // effectivement réussi, écrit UNIQUEMENT dans la branche
    // `Ok` ci-dessous. Gouverne à la fois le gate de
    // lecture/encodage plus bas et la trace `actif` de
    // « compteurs audio » — la seule fenêtre sur un arbitrage
    // figé. Si un refus de `Start()` faisait mentir cette valeur
    // (comme le ferait `emettait = veut_emettre` inconditionnel),
    // la trace annoncerait une fenêtre audible qui ne capture
    // rien, exactement le mode de défaillance silencieux que
    // cette trace existe pour révéler.
    let mut emettait = false;
    // Erreurs de lecture consécutives. Remis à zéro par toute
    // lecture qui aboutit — y compris `Ok(None)`, qui est le cas
    // courant : le flux n'a simplement rien de neuf à rendre.
    let mut lectures_echouees: u32 = 0;

    while !arret_fil.load(Ordering::Relaxed) {
        let veut_emettre = emet_fil.load(Ordering::Relaxed);
        if veut_emettre != voulu_applique {
            voulu_applique = veut_emettre;
            match capture.emettre(veut_emettre) {
                Ok(()) => {
                    // Reprise réelle (Start() a réussi après une
                    // coupure, ou premier démarrage) : réancrer
                    // l'assembleur AVANT qu'il ne rejoue toute la
                    // coupure en une rafale de silence (voir
                    // `FrameAssembler::reancrer`).
                    if veut_emettre && !emettait {
                        assembleur.reancrer();
                    }
                    emettait = veut_emettre;
                }
                Err(e) => {
                    // Un refus ne tue pas la session : on
                    // journalise et on retentera au prochain
                    // changement d'ordre plutôt qu'à chaque tour
                    // (grâce à `voulu_applique`, mis à jour ci-
                    // dessus). `emettait` NE BOUGE PAS : c'est
                    // l'état réel du flux, et il n'a pas changé.
                    tracing::warn!(
                        erreur = %e,
                        actif = veut_emettre,
                        "bascule d'emission audio refusee"
                    );
                }
            }
        }
        // ⚠️ **CE BLOC EST AVANT LE GATE `!emettait`, ET C'EST
        // TOUT SON INTÉRÊT** (F1, revue finale de branche du
        // sous-bloc D7). Il vivait en fin de corps de boucle,
        // c'est-à-dire APRÈS le `continue` de la branche muette :
        // la trace n'était donc atteignable que quand `emettait`
        // valait vrai, et son champ `actif` valait
        // structurellement `true` — le contrôle d'entrée de D8
        // (`grep -c 'actif=true'` opposé au compte total) était
        // **insatisfiable**, et le défaut qu'il existe pour
        // révéler — plus aucune fenêtre ne porte le son — rendait
        // 0 et 0, que ces mêmes documents classaient comme bénin.
        // Une fenêtre muette rapporte désormais elle aussi, toutes
        // les `REPORT_INTERVAL`.
        //
        // Les trois compteurs restent lisibles en muette : ils
        // vivent sur `ring_fil` et `assembleur`, que ce fil
        // possède, et leurs accesseurs ne prennent que `&self`.
        if dernier_rapport.elapsed() >= REPORT_INTERVAL {
            dernier_rapport = Instant::now();
            // `info!`, pas `debug!` : le filtre par défaut
            // (`agent/src/main.rs`, `EnvFilter` replié sur
            // `"info"` en l'absence de `RUST_LOG`) n'émet jamais
            // les journaux `debug!` en exploitation normale. La
            // spec (§5, §9) promet des compteurs « journalisés
            // périodiquement et jamais silencieux » — un
            // enregistrement toutes les `REPORT_INTERVAL` (30 s)
            // n'est pas du bruit, et un compteur de rejets muet
            // est exactement ce qui rendrait une dégradation
            // audio invisible en recette.
            //
            // `pid` et `actif` sont le seul moyen d'observer un
            // arbitrage figé : si aucune fenêtre ne portait plus
            // jamais le son, toutes rapporteraient `actif=false`
            // — le symptôme serait sinon le silence total, sans un
            // `WARN`, sans une erreur. C'est le `grep` d'entrée du
            // sous-bloc suivant (spec §6).
            tracing::info!(
                pid = pid_fil,
                actif = emettait,
                rejetes = ring_fil.rejetes(),
                complements = assembleur.complements(),
                echantillons_jetes = assembleur.echantillons_jetes(),
                "compteurs audio"
            );
        }

        if !emettait {
            // Muette : ne rien lire, ne rien encoder, ne rien
            // déposer. Une trame de silence encodée coûterait
            // quelques octets grâce au DTX, mais elle arriverait
            // au navigateur — et deux fenêtres d'un même processus
            // s'entendraient toutes les deux.
            std::thread::sleep(POLL_INTERVAL);
            continue;
        }

        match capture.read() {
            Ok(Some(bloc)) => {
                lectures_echouees = 0;
                assembleur.push(&bloc);
            }
            Ok(None) => lectures_echouees = 0,
            Err(e) => {
                // **Une erreur ISOLÉE ne condamne pas tout un
                // groupe de PID** (F3, revue finale de branche du
                // sous-bloc D7) : on retente, et l'on n'abandonne
                // qu'après `LECTURES_ECHOUEES_MAX` échecs d'affilée.
                // Le motif complet et la temporisation vivent sur
                // `audio::LECTURES_ECHOUEES_MAX` et
                // `audio::temporisation_de_reprise`, éprouvés sur
                // l'hôte.
                lectures_echouees = lectures_echouees.saturating_add(1);
                if lectures_echouees < LECTURES_ECHOUEES_MAX {
                    tracing::warn!(
                        erreur = %e,
                        consecutives = lectures_echouees,
                        "lecture audio échouée, nouvelle tentative"
                    );
                    std::thread::sleep(temporisation_de_reprise(lectures_echouees));
                    continue;
                }
                // Abandon définitif. Le témoin est posé AVANT la
                // trace, pour qu'aucun ordre traité entre les deux
                // ne puisse se déclarer appliqué à une capture
                // déjà morte.
                capture_morte_fil.store(true, Ordering::Relaxed);
                // Les compteurs sont inclus ici parce que c'est la
                // dernière ligne de log de ce fil : sans eux, une
                // capture morte en cours de session serait
                // indiscernable d'un simple silence —
                // `next_packet` continuerait à rendre `None` comme
                // dans le cas nominal.
                tracing::warn!(
                    erreur = %e,
                    consecutives = lectures_echouees,
                    rejetes = ring_fil.rejetes(),
                    complements = assembleur.complements(),
                    echantillons_jetes = assembleur.echantillons_jetes(),
                    "lecture audio échouée, capture arrêtée définitivement"
                );
                // ⚠️ **HORS PÉRIMÈTRE, et nommé pour le sous-bloc
                // suivant** : le capteur ne peut PAS observer ce
                // témoin — il vit dans l'enfant. La fenêtre reste
                // donc porteuse de son groupe aux yeux de
                // `capteur/audio.rs`, et sa voisine n'est jamais
                // promue. Fermer ce trou demande un signal
                // enfant→capteur (un `VersCapteur` neuf, puis un
                // réarbitrage), c'est-à-dire un changement de
                // protocole : à cadrer, pas à improviser ici.
                return;
            }
        }

        for trame in assembleur.drain_due(Instant::now()) {
            let voulue = perte_desiree_fil.load(Ordering::Relaxed);
            if voulue != derniere_perte {
                match encodeur.set_packet_loss_perc(voulue) {
                    Ok(()) => derniere_perte = voulue,
                    Err(e) => {
                        // Refus de l'encodeur : on retentera au
                        // prochain changement de cible plutôt que
                        // de rejouer cet appel à chaque trame.
                        derniere_perte = voulue;
                        tracing::warn!(
                            erreur = %e,
                            valeur = voulue,
                            "réglage du taux de perte Opus refusé"
                        );
                    }
                }
            }
            match encodeur.encode(&trame.pcm) {
                Ok(data) => ring_fil.push(AudioPacket {
                    data,
                    pts_48k: trame.pts_48k,
                    captured_at: trame.captured_at,
                }),
                Err(e) => {
                    // Même raisonnement que pour l'erreur de
                    // lecture ci-dessus : dernière ligne de log
                    // de ce fil, donc dernière chance de rendre
                    // les compteurs accumulés exploitables — et
                    // même témoin, pour la même raison (F3).
                    //
                    // **Pas de tolérance ici**, contrairement à la
                    // lecture : un refus de l'encodeur Opus sur
                    // une trame bien formée ne relève d'aucune
                    // cause transitoire connue, là où un refus
                    // WASAPI en a plusieurs.
                    capture_morte_fil.store(true, Ordering::Relaxed);
                    tracing::warn!(
                        erreur = %e,
                        rejetes = ring_fil.rejetes(),
                        complements = assembleur.complements(),
                        echantillons_jetes = assembleur.echantillons_jetes(),
                        "encodage Opus échoué, capture arrêtée définitivement"
                    );
                    return;
                }
            }
        }

        std::thread::sleep(POLL_INTERVAL);
    }
}
