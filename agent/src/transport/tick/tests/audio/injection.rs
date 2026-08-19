//! Les tests de l'injection de fautes de RECONSTRUCTION audio, et le verrou
//! qui sérialise tout test exerçant ce chemin.

use super::*;

// ---------------------------------------------------------------------------
// L'injection de fautes de RECONSTRUCTION (`AUDIO_FAUTE_RECONSTRUCTION`)
// ---------------------------------------------------------------------------
//
// ⚠️ **Le budget d'injection est GLOBAL AU PROCESSUS, donc PARTAGÉ par tous
// les tests de ce binaire**, et `cargo test` est multi-fils par défaut. Deux
// règles, toutes deux obligatoires :
//
// 1. les tests d'injection ne touchent JAMAIS l'environnement — ils seedent
//    directement l'atomique, ce qui court-circuite le `OnceLock` déjà
//    initialisé (il ne relirait de toute façon plus `std::env`) ;
// 2. **TOUT test qui exerce la reconstruction prend ce verrou**, pas seulement
//    ceux qui injectent.
//
// ⚠️ **Le point 2 a été payé, et le plan de D11 ne le prescrivait pas** : il
// ne sérialisait que les tests d'injection entre eux. Insuffisant — un test
// PRÉEXISTANT qui reconstruit (`audio_vivant_n_est_annonce_qu_apres_un_paquet_reel`)
// a tourné pendant qu'un budget était armé, a CONSOMMÉ une des fautes
// injectées, et les deux tests ont échoué : celui-là parce que sa
// reconstruction a été refusée, celui-ci parce qu'il n'a plus trouvé son
// budget. Un état global partagé se sérialise du côté de TOUS ses lecteurs,
// jamais du côté de ses seuls écrivains.
pub(super) fn verrou_injection() -> std::sync::MutexGuard<'static, ()> {
    static VERROU: std::sync::Mutex<()> = std::sync::Mutex::new(());
    // Tolérant à l'empoisonnement : l'échec d'UN test ne doit pas transformer
    // les douze autres en `PoisonError`, qui masquerait la cause réelle
    // derrière une cascade — c'est exactement ce qui s'est produit au premier
    // tirage.
    let garde = VERROU.lock().unwrap_or_else(|e| e.into_inner());
    // Auto-cicatrisant : chaque section critique DÉBUTE budget à zéro, donc
    // l'état nominal (variable absente, injection désarmée). Un test
    // d'injection seede APRÈS avoir pris le verrou ; aucun ne peut donc
    // hériter du reliquat d'un autre.
    crate::transport::piste_audio::injection::budget_faute_reconstruction()
        .store(0, std::sync::atomic::Ordering::Relaxed);
    garde
}

/// Une faute armée fait REFUSER la reconstruction : la source morte n'est pas
/// remplacée, le budget de reconstruction a décru, et la faute est consommée.
#[test]
fn une_faute_injectee_fait_refuser_la_reconstruction() {
    use std::sync::atomic::Ordering;

    let _verrou = verrou_injection();
    crate::transport::piste_audio::injection::budget_faute_reconstruction().store(1, Ordering::Relaxed);

    let mut session = session_d_essai();
    session.set_audio_source(Box::new(SourceMorte::new()));
    let actif_recu = std::sync::Arc::new(std::sync::Mutex::new(None));
    let observe = actif_recu.clone();
    session.set_audio_reconstructeur(Box::new(move || {
        Ok(Box::new(SourceVivante::observant_actif(observe.clone()))
            as Box<dyn AudioSource + Send>)
    }));

    let signale = session.reconstruire_ou_signaler(std::time::Instant::now());

    assert!(!signale, "un refus n'est pas encore un AudioMort : il reste du budget");
    assert!(
        session.capture_audio_morte(),
        "la reconstruction a REUSSI alors qu'une faute etait armee"
    );
    assert_eq!(
        *actif_recu.lock().unwrap(),
        None,
        "le reconstructeur ne doit pas avoir ete invoque"
    );
    assert_eq!(
        crate::transport::piste_audio::injection::budget_faute_reconstruction().load(Ordering::Relaxed),
        0,
        "la faute doit avoir ete CONSOMMEE, pas seulement lue"
    );
}

/// Budget à zéro : le binaire se comporte exactement comme sans injection.
/// C'est ce qui établit que la variable est bien **absente = désarmée**.
#[test]
fn le_budget_epuise_laisse_la_reconstruction_reussir() {
    use std::sync::atomic::Ordering;

    let _verrou = verrou_injection();
    crate::transport::piste_audio::injection::budget_faute_reconstruction().store(0, Ordering::Relaxed);

    let mut session = session_d_essai();
    session.set_audio_source(Box::new(SourceMorte::new()));
    let actif_recu = std::sync::Arc::new(std::sync::Mutex::new(None));
    let observe = actif_recu.clone();
    session.set_audio_reconstructeur(Box::new(move || {
        Ok(Box::new(SourceVivante::observant_actif(observe.clone()))
            as Box<dyn AudioSource + Send>)
    }));

    let signale = session.reconstruire_ou_signaler(std::time::Instant::now());

    assert!(!signale);
    assert!(!session.capture_audio_morte(), "la reconstruction devait REUSSIR");
    assert_eq!(*actif_recu.lock().unwrap(), Some(false));
}

/// Un budget plus grand que `RECONSTRUCTIONS_MAX` épuise le budget de
/// reconstruction et mène à `AudioMort` — c'est le chemin que la recette ②
/// emprunte pour éprouver le repli sur la promotion d'une voisine, et que le
/// sous-bloc D10 n'avait AUCUN moyen d'atteindre.
#[test]
#[allow(non_snake_case)]
fn un_budget_superieur_a_RECONSTRUCTIONS_MAX_mene_a_AudioMort() {
    use std::sync::atomic::Ordering;

    let _verrou = verrou_injection();
    crate::transport::piste_audio::injection::budget_faute_reconstruction()
        .store(crate::audio::RECONSTRUCTIONS_MAX + 2, Ordering::Relaxed);

    let mut session = session_d_essai();
    session.set_audio_source(Box::new(SourceMorte::new()));
    session.set_audio_reconstructeur(Box::new(move || {
        Ok(Box::new(SourceVivante::new()) as Box<dyn AudioSource + Send>)
    }));

    // Chaque tentative est refusée par une faute injectée ; le répit est
    // franchi en avançant l'horloge fournie, jamais en dormant.
    let mut maintenant = std::time::Instant::now();
    for tour in 0..crate::audio::RECONSTRUCTIONS_MAX {
        assert!(
            !session.reconstruire_ou_signaler(maintenant),
            "tour {tour} : il reste du budget, AudioMort serait premature"
        );
        maintenant += crate::audio::REPIT_RECONSTRUCTION;
    }

    assert!(
        session.reconstruire_ou_signaler(maintenant),
        "budget de reconstruction epuise : AudioMort doit etre signale"
    );
    assert!(session.capture_audio_morte(), "la source doit etre restee morte");
}
