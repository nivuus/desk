mod accent;
mod apps;
mod audio;
// Pas de `#[cfg(windows)]` ici : le protocole du canal média et la
// `SourceDistante` sont de la logique pure, et doivent se compiler et se
// tester sur Linux. Les sous-modules qui touchent DXGI et les tubes sont
// gatés à l'intérieur de `capteur.rs`.
mod capteur;
// Le rendu LISIBLE de la chaine de causes d'une `anyhow::Error` dans une
// trace. Racine nue : son nom ne prefixe aucun module de premier niveau.
mod cause;
mod clock;
mod congestion;
// La lecture et la validation de l'environnement, extraites de ce fichier le
// 20 août 2026 : voir le commentaire de module de `configuration.rs`. `Config`
// est réexporté ci-dessous, de sorte qu'aucun site d'appel n'ait bougé.
mod configuration;
mod cursor;
mod demarrage;
mod diagnostics;
mod disposition;
mod frames;
// Pas de `#[cfg(windows)]` ici : les logiques pures de `gamepad` (tâche 9,
// ordonnancement des états et limitation des vibrations) n'ont rien de
// spécifique à Windows et doivent compiler et se tester sur Linux. La sonde
// `probe`, elle, reste gated à l'intérieur même du fichier
// (`agent/src/gamepad/probe.rs`), avec ses dépendances `vigem-client` /
// `anyhow::Context` propres à la sonde.
mod gamepad;
mod geometry;
mod mire;
mod moniteurs_virtuels;
mod h264;
mod input;
mod micro;
mod opus;
#[cfg(windows)]
mod pointer_settings;
// Le presse-papier de la VM, sens VM -> navigateur. Pas de `#[cfg(windows)]` :
// toute la décision (normalisation, bornage, gardes) est PURE et doit se
// compiler et se tester sur l'hôte Linux ; les deux appels Win32 vivent dans
// `presse_papier/win32.rs`, gaté à l'intérieur du module. À la racine nue et
// non sous `capteur/` : le propriétaire est le capteur aujourd'hui, l'enfant
// le jour où le mode mono-fenêtre en aura un — un nom sous `capteur/` serait
// faux ce jour-là.
mod presse_papier;
// Le client du canal `/agent` de la plateforme. Pas de `#[cfg(windows)]` :
// le calcul du délai de reprise (`plateforme::repli`) est pur et doit se
// compiler et se tester sur l'hôte Linux, et le socket lui-même n'a rien de
// spécifique à Windows.
mod plateforme;
mod pont;
// Racine nue, pas `pont/relance.rs` : le nom ne décrit rien du PONT
// lui-même, seulement une politique de supervision (relance espacée, seuil
// de stabilité) — voir l'en-tête du fichier pour l'application complète de
// la convention de nommage. Extrait de `superviseur::boucle::
// surveillance_pont` (round de correction 2) pour compiler et se tester sur
// l'hôte : ce dernier vit derrière `superviseur::boucle::#![cfg(windows)]`.
mod relance_pont;
mod rebuild;
mod signaling;
// Pas de `#[cfg(windows)]` ici : c'est la part portable de `capture.rs`
// (lui-même `#![cfg(windows)]` dans son ensemble) — voir le commentaire de
// module de `sortie_dxgi.rs`. `superviseur::placement` (tâche 7) en a besoin
// pour se compiler et se tester sur Linux.
mod sortie_dxgi;
// Fréquence dominante d'un bloc d'échantillons (correction « A-bis »). Pur,
// nom autonome : racine nue, comme `geometry` et `sortie_dxgi` — voir la
// convention de module enfant de `CLAUDE.md`.
mod spectre;
// Pas de `#[cfg(windows)]` ici : le prédicat de PERSISTANCE (tâche 2bis, D9,
// correction n°14 de la revue) est pur -- `Option<(u32, u32)> ×
// Option<(u32, u32)> -> &str` -- et doit se compiler et se tester sur Linux,
// même s'il n'est appelé que depuis `diagnostics::multifenetre`, gated
// derrière `#[cfg(windows)]` dans `diagnostics.rs`.
mod survie_verdict;
// Pas de `#[cfg(windows)]` ici : la logique pure de `superviseur` (tâche 2)
// décide quelles fenêtres méritent d'exister côté navigateur, et doit se
// compiler et se tester sur Linux sans dépendance à l'API Windows.
mod superviseur;
mod source;
mod transport;
mod turn;
// Le calcul de région est pur et doit être testable sur l'hôte : il est donc
// déclaré indépendamment du reste de `windows_source`, qui ne compile que sur
// Windows.
#[path = "windows_source/sortie.rs"]
mod windows_source_sortie;

// Même montage, et pour la même raison : la classification des échecs
// d'acquisition et le budget de reprises sont purs et doivent se tester sur
// l'hôte, alors que `capture.rs` est `#![cfg(windows)]` dans son ensemble.
#[path = "capture/reprise.rs"]
mod capture_reprise;

// Même montage encore (D9, tâche 11) : `Telemetrie` est pure — trois
// compteurs atomiques par SESSION, remplaçant les statiques de PROCESSUS
// `TICKS`/`CAPTURED`/`PRODUCED` de `windows_source.rs`, mortes des deux côtés
// depuis D4 (consignation n°1 de D6). `mod telemetrie;` DANS `windows_source`
// ne suffirait pas : ce fichier est lui-même `#![cfg(windows)]`, et
// `mod windows_source;` ci-dessous l'est aussi — sur Linux, tout son
// sous-arbre serait absent de la compilation, y compris ce module, qui ne
// serait donc plus « éprouvable sur l'hôte ».
#[path = "windows_source/telemetrie.rs"]
mod windows_source_telemetrie;

// Même montage encore (correction « A-bis », 19 août 2026) : la règle qui
// élit le point de terminaison audio de rendu à capter est pure — elle prend
// une liste de noms et d'identifiants et rend un élu ou une raison de refus —
// alors que `wasapi.rs` est `#![cfg(windows)]` dans son ensemble. Un
// `mod peripherique;` DANS `wasapi` la rendrait absente de la compilation
// hôte, donc inéprouvable, exactement comme pour `telemetrie` ci-dessus.
#[path = "wasapi/peripherique.rs"]
mod wasapi_peripherique;

// Même montage, une troisième fois (bloc E2, 20 août 2026) : le format de
// mixage que l'écriture du micro sur le câble accepte — et surtout ceux
// qu'elle REFUSE en les nommant — est une règle sans un octet de COM. Elle
// vit chez `wasapi` parce que c'est de WASAPI qu'elle parle, et elle est
// hissée ici parce que `wasapi.rs` est `#![cfg(windows)]` : le plan E2
// demande cette règle « PURE et testée sur l'hôte » et loge par ailleurs
// `wasapi/ecriture.rs` sous ce même `cfg`. Les deux ne peuvent pas tenir dans
// le même fichier ; elles tiennent dans le même répertoire.
#[path = "wasapi/format.rs"]
mod wasapi_format;

/// La part PURE du chemin NVENC (lot 31) : choix de la voie, traduction des
/// réglages, arithmétique de version des structures.
///
/// ⚠️ **Hissée ici pour la même raison que `wasapi_format` juste au-dessus** :
/// `encode.rs` est `#![cfg(windows)]`, et cette logique-ci doit se tester sur
/// l'hôte. Le nom porte le préfixe `encode_` d'un module de premier niveau
/// existant, donc la convention la range CHEZ son parent, par `#[path]` —
/// et non à la racine nue. Voir l'en-tête du fichier.
#[path = "encode/nvenc.rs"]
mod encode_nvenc;

#[cfg(windows)]
mod capture;
#[cfg(windows)]
mod encode;
#[cfg(windows)]
mod appartenance;
mod window;
#[cfg(windows)]
mod wasapi;
#[cfg(windows)]
mod windows_audio;
/// L'ecriture du micro sur le cable virtuel (bloc E2).
///
/// ⚠️ **Racine nue, et c'est vérifié contre la convention** (§ « Convention de
/// module enfant », tête de `CLAUDE.md`) : `windows_micro` ne porte le préfixe
/// `<parent>_` d'aucun module de premier niveau existant — `window` exigerait
/// `window_`, et il n'existe aucun `mod windows;`. Il rejoint `windows_audio`
/// et `windows_source`, à la racine nue pour la même raison. **Aucun `#[path]`.**
#[cfg(windows)]
mod windows_micro;
#[cfg(windows)]
mod windows_source;

pub(crate) use configuration::Config;
use configuration::{config, variable_non_vide};

use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    // Diagnostic (chantier duplications-parallèles, tâche 2bis) : posé AVANT
    // tout le reste, pour qu'aucune faute ne puisse survenir avant lui.
    // Inerte sans `AGENT_TRACE_EXCEPTIONS`, et le gestionnaire ne s'exécute
    // qu'au moment d'une violation d'accès — jamais sur le chemin nominal.
    #[cfg(windows)]
    diagnostics::exceptions::installer();

    // Au tout début, avant toute possibilité d'injection d'entrée (les modes
    // diagnostic de `diagnostics::aiguiller` n'en injectent pas, mais la
    // session normale plus bas le fait) : neutraliser l'accélération et la
    // sensibilité pointeur de la session Windows. Ne fait jamais échouer le
    // démarrage — une visée dégradée vaut mieux que pas de session.
    //
    // `INPUT_LINEARITY_NEUTRALISER=0` saute AUSSI cet appel-ci, et pas
    // seulement celui, propre à la sonde de linéarité, que porte
    // `diagnostics::entree`.
    // Correction de la ronde de revue 1 : le réglage SPI que pose
    // `neutraliser()` vaut pour la SESSION Windows entière, pas pour le
    // processus qui l'a posé (voir `pointer_settings.rs`) — sauter
    // uniquement l'appel de la sonde ne suffisait donc pas, puisque cet
    // appel-ci, plus haut et inconditionnel, avait déjà neutralisé
    // l'accélération avant même que la sonde ne lise sa propre variable.
    // La mesure de référence obtenait un écart nul quel que soit l'état
    // réel de la VM : un artefact garanti par construction, pas une mesure.
    #[cfg(windows)]
    if std::env::var("INPUT_LINEARITY_NEUTRALISER").as_deref() != Ok("0") {
        match pointer_settings::neutraliser() {
            Ok(rapport) => tracing::info!(rapport, "accélération pointeur neutralisée"),
            Err(e) => tracing::warn!(erreur = %e, "neutralisation de l'accélération pointeur échouée"),
        }
    } else {
        tracing::warn!(
            "neutralisation SAUTÉE au démarrage (INPUT_LINEARITY_NEUTRALISER=0, mesure de référence)"
        );
    }

    let mut config = config()?;

    // Après la neutralisation ci-dessus, et avant tout assemblage de session :
    // c'est cet ordre que la sonde de linéarité suppose (voir `diagnostics`).
    if diagnostics::aiguiller()? {
        return Ok(());
    }

    // Le mode capteur ne détecte rien et ne lance personne : il tient les N
    // duplications DXGI et les N encodeurs, et sert le média aux enfants du
    // superviseur par tube nommé. `CAPTEUR=0` DÉSACTIVE le mode, comme
    // `SUPERVISEUR=0` et `AUDIO=0` — même piège d'exploitation, même parade.
    if matches!(std::env::var("CAPTEUR").as_deref(), Ok(v) if v != "0") {
        return capteur::executer();
    }

    // L'ENRÔLEMENT PRÉCÈDE TOUT SIGNALING (sous-bloc P3). Les deux poignées
    // de main qui suivent — celle du superviseur sur sa session de contrôle,
    // celle de l'enfant sur sa session média — présentent le jeton que ce
    // canal délivre, et le préfixe qu'il rend nomme les sessions.
    //
    // Le canal reste ouvert pour toute la vie du processus : il porte le
    // battement de cœur, donc `vu_a`, donc l'état `prête`/`injoignable` que
    // la plateforme lit. Le lier à une variable et non à `_` n'est pas une
    // coquetterie de lint — c'est ce qui le garde vivant.
    //
    // Le mode capteur, lui, est déjà reparti plus haut : il ne parle à aucun
    // signaling et n'a donc aucune identité à présenter.
    //
    // 🔴 **UN SEUL PROCESSUS PAR VM OUVRE CE CANAL, ET C'EST LE CORRECTIF DU
    // 20 août 2026.** Le pont fichiers et les enfants de fenêtre héritaient de
    // `AGENT_VM`/`AGENT_SECRET` et s'enrôlaient sous la MÊME identité que le
    // superviseur ; le registre de la plateforme n'admettant qu'un socket par
    // VM, ils s'évinçaient l'un l'autre sans terme — 95 enrôlements et
    // 94 évictions en 64 s, mesurés. Ils reçoivent désormais `AGENT_JETON` du
    // superviseur (`superviseur/lanceur.rs`) et n'ouvrent aucun canal. La
    // règle qui tranche est PURE et testée sur l'hôte : `plateforme::identite`.
    let mut _canal_plateforme = match plateforme::identite::source(
        variable_non_vide("AGENT_JETON").as_deref(),
        config.agent_vm.as_deref(),
        config.agent_secret.as_deref(),
    ) {
        plateforme::identite::SourceIdentite::Heritee(jeton) => {
            // ⚠️ AUCUN PRÉFIXE N'EST HÉRITÉ, et ce n'est pas un oubli :
            // `config.prefixe` n'est lu que par le mode superviseur, qui
            // compose sa session de contrôle avec. Un enfant reçoit sa session
            // toute faite dans `SESSION_ID`, et le pont la sienne — toutes deux
            // composées par le superviseur, qui connaît le préfixe. Hériter
            // d'un préfixe inutilisé ne ferait qu'inviter à s'en servir.
            tracing::info!(
                "identité héritée du superviseur (AGENT_JETON) : ce processus n'ouvre \
                 aucun canal /agent, un seul socket par VM"
            );
            config.jeton = Some(jeton);
            None
        }
        plateforme::identite::SourceIdentite::Enrolement { vm, secret } => {
            let mut canal = plateforme::ouvrir(&config.signaling_url, vm, secret);
            // Attente NON bornée, et c'est délibéré : sans identité, aucune
            // session ne peut s'établir, et la boucle de reprise journalise
            // chacune de ses tentatives. Elle ne rend `None` que si elle a
            // RENONCÉ — un refus de version —, et il n'y a alors rien à
            // attendre.
            let Some(identite) = canal.attendre_identite().await else {
                anyhow::bail!(
                    "enrôlement abandonné par la plateforme : voir le journal du canal /agent"
                );
            };
            config.prefixe = identite.prefixe;
            config.jeton = Some(identite.jeton);
            Some(canal)
        }
        plateforme::identite::SourceIdentite::Aucune => {
            tracing::warn!(
                "AGENT_VM ou AGENT_SECRET absent, et aucun AGENT_JETON hérité : aucun \
                 jeton d'agent. La plateforme REFUSERA la poignée de main et aucune \
                 session ne s'établira (sous-bloc P3, sans interrupteur permissif)."
            );
            None
        }
    };

    // La découverte d'applications vit ICI, entre l'enrôlement et les
    // aiguillages : elle a besoin du canal, et doit courir dans les DEUX modes
    // qui en ont un — superviseur et mono-fenêtre. Le capteur, lui, est déjà
    // reparti bien plus haut, AVANT l'enrôlement. Lié comme `_canal_plateforme`
    // et POUR LA MÊME RAISON : le lâcher terminerait le fil de découverte.
    let _apps = apps::brancher(_canal_plateforme.as_mut());

    // Le mode pont ne capture rien et ne lance personne : il tient la racine
    // de virtualisation ProjFS et la sert depuis le répertoire que la
    // page-shell a ouvert. `PONT=0` DÉSACTIVE le mode, comme `CAPTEUR=0` et
    // `SUPERVISEUR=0` — même piège d'exploitation, même parade : tester
    // `is_ok()` ferait qu'écrire `PONT=0` pour COUPER le pont l'allumerait.
    //
    // Placée APRÈS `CAPTEUR` — un pont qui hériterait de `CAPTEUR` deviendrait
    // un capteur, d'où l'`env_remove("CAPTEUR")` de `lancer_pont` — et AVANT
    // la branche superviseur, d'où l'`env_remove("PONT")` de `lancer`.
    //
    // ⚠️ **Mais APRÈS L'ENRÔLEMENT ci-dessus, et c'est une DIVERGENCE assumée
    // d'avec le plan de F1**, qui écrivait « après `CAPTEUR`, avant
    // `superviseur` » à une date où ces deux branches se touchaient. Le
    // sous-bloc P3 a intercalé l'enrôlement entre elles, et le pont en a
    // besoin : il ouvre sa PROPRE `PeerConnection` vers la page-shell, donc il
    // présente un jeton, exactement comme un enfant. Le placer avant
    // l'enrôlement lui aurait laissé `config.jeton = None`, la plateforme
    // aurait refusé la poignée de main, et **aucune session ne se serait
    // établie** — sans que rien ne rattache la panne au placement d'un `if`.
    //
    // Le capteur, lui, est bien reparti AVANT l'enrôlement, et c'est cohérent :
    // il ne parle à aucun signaling.
    if matches!(std::env::var("PONT").as_deref(), Ok(v) if v != "0") {
        return pont::executer(config).await;
    }

    // Le mode superviseur ne capture rien : il détecte les fenêtres et lance
    // un enfant par fenêtre. Ses enfants n'héritent JAMAIS de `SUPERVISEUR`
    // (voir `superviseur::lanceur`), sans quoi chacun se prendrait pour un
    // superviseur et lancerait les siens, indéfiniment.
    if config.superviseur {
        // 🔴 LA VEILLE D'IDENTITÉ, ET NON `config.jeton`, EST CE QUI PART AU
        // LANCEUR. Un superviseur vit des heures ; le jeton d'agent, lui, dure
        // dix minutes et se renouvelle à chaque battement. Passer l'instantané
        // du démarrage ferait qu'une fenêtre ouverte une heure plus tard
        // recevrait un jeton mort, que la garde de la plateforme refuserait —
        // et aucune session ne s'établirait, sans qu'aucune trace ne rattache
        // la panne à l'âge d'une variable.
        let veille = _canal_plateforme.as_ref().map(plateforme::Canal::veille_identite);
        return superviseur::executer(config, veille).await;
    }

    demarrage::executer(config).await
}
