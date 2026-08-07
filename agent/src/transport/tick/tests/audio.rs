use super::*;
use crate::transport::fixtures;

/// Remède à la réserve I1 de la revue de la tâche 9 (sous-bloc D9) : la
/// branche a1sexies n'avait aucun test — exactement le risque que le
/// commentaire d'`une_part_en_attente_est_appliquee_par_act_on_timeout`
/// nomme pour a1quater (« supprimer tout le bloc laissait les autres tests
/// verts »). Séquence complète, vue ROUGE avant remède (voir le rapport de
/// tâche) : `capture_morte()` devient vraie → `AudioMort` part UNE fois →
/// elle NE repart PAS au tour suivant (le verrou `audio_mort_signale`) →
/// `rattachement_survenu()` devient vraie → elle REPART.
#[test]
fn une_capture_audio_morte_est_signalee_une_fois_puis_de_nouveau_apres_un_rattachement() {
    let inner = fixtures::video_test_source();
    let signalements = std::sync::Arc::new(std::sync::Mutex::new(0u32));
    let rattachement = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let source = Box::new(SourceAvecAudioMort {
        inner,
        signalements: signalements.clone(),
        rattachement_prepare: rattachement.clone(),
    });
    let mut session = Session::new(source, fixtures::local_ip(), Instant::now(), 12_000_000)
        .expect("session");

    let morte = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    session.set_audio_source(Box::new(AudioSourceMortelle { morte: morte.clone() }));

    // Capture vivante : rien à signaler.
    session
        .act_on_timeout(Instant::now())
        .expect("un tour sans capture morte ne doit jamais faire échouer la session");
    assert_eq!(
        *signalements.lock().unwrap(),
        0,
        "aucun signalement tant que la capture est vivante"
    );

    // La capture meurt : le tour SUIVANT doit signaler exactement une fois.
    morte.store(true, std::sync::atomic::Ordering::Relaxed);
    session
        .act_on_timeout(Instant::now())
        .expect("signaler une capture morte ne doit jamais faire échouer la session");
    assert_eq!(
        *signalements.lock().unwrap(),
        1,
        "la capture morte doit être signalée exactement une fois"
    );

    // Tour suivant, capture toujours morte, AUCUN rattachement : le verrou
    // doit empêcher toute réémission.
    session
        .act_on_timeout(Instant::now())
        .expect("un tour sous verrou ne doit jamais faire échouer la session");
    assert_eq!(
        *signalements.lock().unwrap(),
        1,
        "le verrou audio_mort_signale doit empêcher une réémission tant qu'aucun \
         rattachement n'a eu lieu"
    );

    // Un rattachement survient (capteur relancé, ou reconnexion de canal) :
    // le verrou retombe, et la capture toujours morte doit repartir.
    rattachement.store(true, std::sync::atomic::Ordering::Relaxed);
    session
        .act_on_timeout(Instant::now())
        .expect("re-signaler après un rattachement ne doit jamais faire échouer la session");
    assert_eq!(
        *signalements.lock().unwrap(),
        2,
        "un rattachement doit remettre le verrou à zéro et permettre un nouveau signalement"
    );
}
