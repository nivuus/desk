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
        annonces_audio_vivant: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
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

/// Capture morte : `capture_morte()` vrai, aucun paquet. C'est l'état dans
/// lequel le fil de `windows_audio.rs` laisse la source après son `return`
/// définitif.
struct SourceMorte;
impl SourceMorte {
    fn new() -> Self {
        Self
    }
}
impl AudioSource for SourceMorte {
    fn capture_morte(&self) -> bool {
        true
    }
    fn next_packet(&mut self) -> Option<AudioPacket> {
        None
    }
    fn set_actif(&mut self, _actif: bool) {}
}

/// Capture vivante, avec ou sans paquet en attente. `sans_paquet` sert à
/// distinguer « reconstruite » de « entendue » — c'est toute la
/// différence entre une décision et une preuve (leg 6).
///
/// `actif` observe les appels à `set_actif` — défaut RENDU OBSERVABLE en
/// recette VM (sous-bloc D10, après la tâche 12) : les trois constructeurs
/// historiques (`sans_paquet`/`avec_un_paquet`/`new`) lui donnent un `Arc`
/// frais que personne n'inspecte, comportement inchangé pour les tests
/// existants ; `observant_actif` en prend un fourni par l'appelant, pour les
/// tests qui vérifient précisément CET appel.
struct SourceVivante {
    paquets: u32,
    actif: std::sync::Arc<std::sync::Mutex<Option<bool>>>,
}
impl SourceVivante {
    fn new() -> Self {
        Self::avec_un_paquet()
    }
    fn sans_paquet() -> Self {
        Self { paquets: 0, actif: std::sync::Arc::new(std::sync::Mutex::new(None)) }
    }
    fn avec_un_paquet() -> Self {
        Self { paquets: 1, actif: std::sync::Arc::new(std::sync::Mutex::new(None)) }
    }
    /// `paquets = 0` : ce constructeur n'observe QUE l'appel `set_actif`,
    /// indépendamment de tout paquet — c'est un appel synchrone, fait avant
    /// le move dans `self.audio_source`, pas une conséquence d'un paquet
    /// émis.
    fn observant_actif(actif: std::sync::Arc<std::sync::Mutex<Option<bool>>>) -> Self {
        Self { paquets: 0, actif }
    }
}
impl AudioSource for SourceVivante {
    fn capture_morte(&self) -> bool {
        false
    }
    fn next_packet(&mut self) -> Option<AudioPacket> {
        (self.paquets > 0).then(|| {
            self.paquets -= 1;
            paquet_d_essai()
        })
    }
    fn set_actif(&mut self, actif: bool) {
        *self.actif.lock().unwrap() = Some(actif);
    }
}

/// Le fait réparé (D9 §4.3) : une capture morte n'était JAMAIS
/// reconstruite. `set_actif(true)` n'écrit qu'un booléen atomique que le
/// fil mort ne relit jamais, et réélire la même session ne fait rien.
#[test]
fn une_capture_morte_est_reconstruite_avant_tout_signalement() {
    let mut session = session_d_essai();
    session.set_audio_source(Box::new(SourceMorte::new()));
    let essais = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
    let compte = std::sync::Arc::clone(&essais);
    session.set_audio_reconstructeur(Box::new(move || {
        compte.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Ok(Box::new(SourceVivante::new()) as Box<dyn AudioSource + Send>)
    }));

    let t0 = std::time::Instant::now();
    assert!(!session.reconstruire_ou_signaler(t0), "rien à signaler : on reconstruit");
    assert_eq!(essais.load(std::sync::atomic::Ordering::Relaxed), 1);
    assert!(!session.capture_audio_morte(), "la source neuve est vivante");
}

/// Le budget épuisé fait retomber sur le signalement : c'est là que la
/// promotion d'une voisine par le capteur reprend son rôle — la seule
/// moitié de D9 qui fonctionnait.
#[test]
fn un_reconstructeur_qui_echoue_toujours_finit_par_signaler() {
    let mut session = session_d_essai();
    session.set_audio_source(Box::new(SourceMorte::new()));
    session.set_audio_reconstructeur(Box::new(|| anyhow::bail!("plus d'arbre de processus")));

    let mut t = std::time::Instant::now();
    for essai in 0..crate::audio::RECONSTRUCTIONS_MAX {
        assert!(!session.reconstruire_ou_signaler(t), "essai {essai} : budget restant");
        t += crate::audio::REPIT_RECONSTRUCTION;
    }
    assert!(session.reconstruire_ou_signaler(t), "budget épuisé : il faut signaler");
}

/// Le répit est respecté : sans lui, la boucle de tick tenterait une
/// ouverture WASAPI à chaque tour.
#[test]
fn le_repit_espace_les_tentatives() {
    let mut session = session_d_essai();
    session.set_audio_source(Box::new(SourceMorte::new()));
    let essais = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
    let compte = std::sync::Arc::clone(&essais);
    session.set_audio_reconstructeur(Box::new(move || {
        compte.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        anyhow::bail!("pas encore")
    }));

    let t0 = std::time::Instant::now();
    session.reconstruire_ou_signaler(t0);
    session.reconstruire_ou_signaler(t0);
    assert_eq!(
        essais.load(std::sync::atomic::Ordering::Relaxed),
        1,
        "deux appels dans le même instant ne font qu'une tentative"
    );
}

/// Sans reconstructeur — `AUDIO=0`, `TEST_FILE`, ou un échec d'ouverture
/// audio initiale —, le comportement d'avant D10 doit être exactement
/// conservé.
///
/// ❌ **Ce doc-comment annonçait « chemin mono-fenêtre » parmi ces cas, et
/// c'est FAUX** (revue transverse de fin de branche, second tour) :
/// `demarrage/audio.rs::brancher` pose un reconstructeur
/// INCONDITIONNELLEMENT dans son bras `Ok`, branche `None` comprise. **Ce
/// test ne couvre donc PAS le mono-fenêtre**, contrairement à ce qu'il
/// annonçait — il couvre l'absence de reconstructeur, qui est autre chose.
/// Le mono-fenêtre a bien un reconstructeur, et son défaut propre (la
/// source reconstruite est réarmée à `false`) n'est couvert par aucun test :
/// voir le legs n°4 de D10.
#[test]
fn sans_reconstructeur_on_signale_immediatement() {
    let mut session = session_d_essai();
    session.set_audio_source(Box::new(SourceMorte::new()));
    assert!(session.reconstruire_ou_signaler(std::time::Instant::now()));
}

/// Le leg 6 : `REARMEMENTS_MAX` doit se remettre à zéro sur une PREUVE de
/// son, pas sur une décision d'arbitrage. La preuve est le premier paquet
/// qui repart après une reconstruction.
///
/// Construite directement via `Session::new`, PAS via `session_d_essai()` :
/// celle-ci pose une `FileSource` muette sur `signaler_audio_vivant` (défaut
/// inerte du trait), qui ne permettrait d'observer aucun appel. Seule une
/// source vidéo FACTICE — `SourceAvecAudioMort`, étendue pour ce test plutôt
/// que dupliquée — peut porter l'`Arc<AtomicBool>` que ce test lit, sans
/// downcast sur `Box<dyn VideoSource>`.
#[test]
fn audio_vivant_n_est_annonce_qu_apres_un_paquet_reel() {
    let annonces = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let source = Box::new(SourceAvecAudioMort {
        inner: fixtures::video_test_source(),
        signalements: std::sync::Arc::new(std::sync::Mutex::new(0)),
        rattachement_prepare: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        annonces_audio_vivant: annonces.clone(),
    });
    let mut session = Session::new(source, fixtures::local_ip(), Instant::now(), 12_000_000)
        .expect("session");

    session.set_audio_source(Box::new(SourceMorte::new()));
    session.set_audio_reconstructeur(Box::new(|| {
        Ok(Box::new(SourceVivante::sans_paquet()) as Box<dyn AudioSource + Send>)
    }));
    session.reconstruire_ou_signaler(std::time::Instant::now());

    // Un `mid` factice fait passer la garde de négociation de `brancher_audio`
    // : `write_audio` échouera derrière (`rtc.writer` ne connaît pas ce mid,
    // aucune vraie négociation SDP n'a eu lieu ici), mais `next_packet()` aura
    // déjà été appelé AVANT cet échec — c'est lui, et lui seul, qui porte la
    // preuve que ce test vérifie (voir le commentaire de `brancher_audio`).
    // Posé ICI, AVANT le premier tour de boucle : voir le commentaire
    // ci-dessous sur pourquoi ce tour doit avoir lieu avant l'assertion qui
    // suit.
    session.audio_mid = Some(str0m::media::Mid::new());

    // ⚠️ **Trouvé en revue de la tâche 12** : sans CE tour de boucle,
    // l'assertion qui suit s'exécuterait avant tout appel à `act_on_timeout`
    // et serait donc vraie PAR CONSTRUCTION, quelle que soit l'implémentation
    // — y compris une implémentation buguée qui poserait
    // `audio_vivant_a_annoncer` dans le bras `Ok` de `reconstruire_ou_signaler`
    // (le défaut exact que ce test existe pour attraper). Un tour SANS PAQUET
    // disponible (`sans_paquet()` rend `None` à `next_packet()`) donne au
    // mécanisme une occasion réelle de se manifester, et à l'assertion une
    // chance réelle d'échouer si le drapeau était posé au mauvais endroit.
    session
        .act_on_timeout(std::time::Instant::now())
        .expect("un tour sans paquet ne doit jamais faire échouer la session");
    assert!(
        !annonces.load(std::sync::atomic::Ordering::Relaxed),
        "reconstruite n'est pas entendue : aucune preuve encore (sans_paquet ne produit rien)"
    );

    session.set_audio_source(Box::new(SourceVivante::avec_un_paquet()));
    session.brancher_audio(); // le paquet qui repart EST la preuve
    session
        .act_on_timeout(std::time::Instant::now())
        .expect("annoncer une reprise audio ne doit jamais faire échouer la session");
    assert!(annonces.load(std::sync::atomic::Ordering::Relaxed));
}

/// Le remède à la revue de la tâche 12 (sous-bloc D10) : sans
/// réapprovisionnement, `reconstructions_restantes` — posé UNE FOIS à la
/// construction de la `Session`, décrémenté seulement — épuise le cycle pour
/// TOUJOURS après le premier `AudioMort`, et le verrou `audio_mort_signale`
/// (qui ne retombe qu'à un rattachement) empêche même de retenter. Une
/// RÉÉLECTION — `appliquer_audio(true)` — doit réapprovisionner le budget ET
/// lever ce verrou, pour que le cycle puisse tourner une SECONDE fois. Ce
/// test le fait tourner deux fois, explicitement : c'est le seul moyen de
/// savoir que la chaîne est réellement refermée.
#[test]
fn une_reelection_reapprovisionne_le_budget_et_leve_le_verrou() {
    let mut session = session_d_essai();
    session.set_audio_source(Box::new(SourceMorte::new()));
    session.set_audio_reconstructeur(Box::new(|| anyhow::bail!("jamais")));

    // Premier cycle : `RECONSTRUCTIONS_MAX` tentatives, toutes en échec,
    // espacées de `REPIT_RECONSTRUCTION` (horloge avancée à la main, comme
    // `un_reconstructeur_qui_echoue_toujours_finit_par_signaler`).
    let mut t = std::time::Instant::now();
    for essai in 0..crate::audio::RECONSTRUCTIONS_MAX {
        assert!(!session.reconstruire_ou_signaler(t), "premier cycle, essai {essai}");
        t += crate::audio::REPIT_RECONSTRUCTION;
    }
    assert!(session.reconstruire_ou_signaler(t), "premier épuisement : il faut signaler");
    // C'est ce que fait `act_on_timeout` (branche a1sexies) au moment de
    // signaler `AudioMort` — reproduit ici pour ne pas dépendre du reste de
    // la liste de priorités, non pertinente pour ce test.
    session.audio_mort_signale = true;

    // Le capteur réélit cette session (répit expiré, plus aucune voisine à
    // préférer) : sans le remède, ceci ne changerait rien.
    session.appliquer_audio(true);
    assert!(
        !session.audio_mort_signale,
        "une réélection doit lever le verrou, sinon a1sexies ne rappelle plus jamais \
         reconstruire_ou_signaler"
    );

    // Second cycle : le budget doit être de nouveau plein.
    for essai in 0..crate::audio::RECONSTRUCTIONS_MAX {
        assert!(
            !session.reconstruire_ou_signaler(t),
            "second cycle, essai {essai} : le budget devait avoir été réapprovisionné"
        );
        t += crate::audio::REPIT_RECONSTRUCTION;
    }
    assert!(
        session.reconstruire_ou_signaler(t),
        "second épuisement : le cycle a bien tourné une seconde fois"
    );
}

/// Défaut trouvé en recette VM (deux exécutions, `capture audio reconstruite`
/// = 2, `compteurs_audio_actif_true` = 0 aux deux) : une source reconstruite
/// par `WindowsAudioSource::pour_processus` NAÎT MUETTE
/// (`windows_audio.rs::demarrer`, `emet = Arc::new(AtomicBool::new(false))`)
/// — contrairement au mode mono-fenêtre `new()`, qui s'émet lui-même. Rien,
/// avant ce correctif, ne réarmait la source reconstruite :
/// `reconstruire_ou_signaler` la posait dans `self.audio_source` sans jamais
/// appeler `set_actif`. Chaîne complète, refermée sur elle-même : muette →
/// aucun paquet → aucune PREUVE (`audio_vivant_a_annoncer`) → aucune
/// réélection → muette pour toujours. **Un état ABSORBANT, pas un retard.**
///
/// Aucun test d'hôte antérieur ne pouvait voir ce défaut : `SourceMorte` et
/// (avant ce correctif) `SourceVivante` avaient toutes deux un `set_actif`
/// no-op.
#[test]
fn une_session_porteuse_reconstruite_recoit_set_actif_true() {
    let mut session = session_d_essai();
    session.set_audio_source(Box::new(SourceMorte::new()));
    let actif_recu = std::sync::Arc::new(std::sync::Mutex::new(None));
    let observe = actif_recu.clone();
    session.set_audio_reconstructeur(Box::new(move || {
        Ok(Box::new(SourceVivante::observant_actif(observe.clone()))
            as Box<dyn AudioSource + Send>)
    }));
    // Cette session PORTE le son au moment où sa capture meurt — le cas
    // majoritaire (une application, une fenêtre), miroir local du dernier
    // ordre `Audio { actif: true }` reçu du capteur.
    session.audio_porteuse = true;

    session.reconstruire_ou_signaler(std::time::Instant::now());

    assert_eq!(
        *actif_recu.lock().unwrap(),
        Some(true),
        "une session porteuse dont la capture est reconstruite doit être \
         réémise IMMÉDIATEMENT (avant tout paquet) : sans quoi elle reste \
         muette pour toujours (aucun paquet -> aucune preuve -> aucune \
         réélection -> muette)"
    );
}

/// Cas symétrique, demandé en revue : une session qui NE PORTE PAS le son au
/// moment où sa capture est reconstruite ne doit pas être rallumée par sa
/// propre reconstruction — `audio_porteuse` le donne gratuitement (le
/// correctif appelle `set_actif(self.audio_porteuse)` sans condition), mais
/// ce test le PROUVE plutôt que de le supposer.
#[test]
fn une_session_non_porteuse_reconstruite_reste_muette() {
    let mut session = session_d_essai();
    session.set_audio_source(Box::new(SourceMorte::new()));
    let actif_recu = std::sync::Arc::new(std::sync::Mutex::new(None));
    let observe = actif_recu.clone();
    session.set_audio_reconstructeur(Box::new(move || {
        Ok(Box::new(SourceVivante::observant_actif(observe.clone()))
            as Box<dyn AudioSource + Send>)
    }));
    // `audio_porteuse` reste à son défaut de construction : `false`.
    assert!(!session.audio_porteuse, "précondition : cette session ne porte pas le son");

    session.reconstruire_ou_signaler(std::time::Instant::now());

    assert_eq!(
        *actif_recu.lock().unwrap(),
        Some(false),
        "une session qui ne porte pas le son ne doit pas être rallumée par sa \
         propre reconstruction"
    );
}
