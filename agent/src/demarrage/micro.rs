//! Le PUITS DE MESURE du micro (`MICRO_MESURE=1`) : un consommateur qui joue le
//! rôle qu'un vrai câble audio tiendra au bloc E2, et qui rend au journal ce
//! qu'il a entendu.
//!
//! ⚠️ **INSTRUMENT DE BANC, JAMAIS UNE CONFIGURATION LIVRÉE.** D'où la
//! convention `MICRO_MESURE=1` qui **ARME** — et non `=0` qui désarmerait,
//! comme le font `AUDIO`, `PLEIN_ECRAN`, `SUPERVISEUR` et `CAPTEUR`. Une simple
//! présence ne suffit pas non plus : il faut la valeur `1`. La règle générale du
//! dépôt reste « on désarme sur `=0` ce qui est livré, on arme sur `=1` ce qui
//! ne l'est pas ».
//!
//! **PUR : aucun `cfg`, aucun objet COM, aucun périphérique.** Ce fichier
//! compile et se teste sous Linux, contrairement à son voisin
//! `demarrage/audio.rs`. C'est ce qui permet à la partie qui peut réellement se
//! tromper — le désentrelacement, la fenêtre d'une seconde, le sens de la crête
//! — d'être éprouvée sans VM.

use std::sync::{Arc, Mutex};

use crate::micro::LecteurMicro;
use crate::transport::Session;
use crate::Config;

use mesure::{consommer, PuitsDeMesure};

/// Le puits de mesure lui-même, extrait au titre de la règle des 500 lignes.
/// **Ce fichier ne garde que l'aiguillage.**
mod mesure;

/// `MICRO_MESURE` arme-t-elle le puits ? **`1`, et rien d'autre.**
///
/// ⚠️ **Ce prédicat existe pour être TESTÉ**, et le test existe pour empêcher
/// une « simplification » future en `is_ok()`. La convention est l'inverse de
/// celle d'`AUDIO`/`SUPERVISEUR`/`PLEIN_ECRAN`/`CAPTEUR`, qui désarment sur
/// `=0` : ici on arme sur `=1`, parce qu'un instrument de banc ne doit pas
/// s'allumer par la simple présence d'une variable — quelqu'un qui écrirait
/// `MICRO_MESURE=0` pour être sûr de le couper l'allumerait.
pub(crate) fn arme(valeur: Option<&str>) -> bool {
    valeur == Some("1")
}

/// Installe le puits de mesure sur `session`, si `MICRO_MESURE=1`.
///
/// Son absence ne compromet jamais la session : sans lui, `micro_disponible()`
/// reste faux, `ready` porte `mic: false`, et le bouton du navigateur ne paraît
/// pas — exactement le comportement voulu tant que le vrai câble (bloc E2)
/// n'existe pas.
pub(super) fn brancher(config: &Config, session: &mut Session) {
    if !config.micro_mesure {
        return;
    }

    let lecteur = match LecteurMicro::new() {
        Ok(l) => Arc::new(Mutex::new(l)),
        Err(e) => {
            tracing::warn!(erreur = %e, "puits de mesure du micro indisponible, la session continue sans");
            return;
        }
    };

    session.set_puits_micro(Box::new(PuitsDeMesure { lecteur: Arc::clone(&lecteur) }));

    // ⚠️ ÉMISE AU BRANCHEMENT, PAS AU PREMIER PAQUET, et c'est délibéré. D6 a
    // écrit un contrôle « la variable est-elle arrivée ? » qui rendait vide aux
    // sept exécutions parce qu'il courait avant l'initialisation qu'il
    // observait : il aurait masqué une variable réellement manquante. Ici, la
    // ligne sort dès que le puits est posé — donc avant toute session WebRTC —
    // et son ABSENCE prouve que `MICRO_MESURE` n'a pas atteint le processus.
    tracing::info!(
        session = %config.session_id,
        "micro de mesure ARME (MICRO_MESURE=1) : instrument de banc, jamais une configuration livree"
    );

    let session_id = config.session_id.clone();
    std::thread::spawn(move || consommer(lecteur, session_id));
}
