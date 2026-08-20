//! **L'AIGUILLAGE du micro** : lequel des puits reçoit le flux montant, et
//! pourquoi il n'y en a jamais deux.
//!
//! Trois issues, et une seule règle (Décision 10 du plan E2) — voir
//! [`choisir_puits`] :
//!
//! | `MICRO` | `MICRO_MESURE` | Puits |
//! | --- | --- | --- |
//! | ≠ `0` | `1` | **Mesure** — l'instrument de banc PREND LE PAS |
//! | ≠ `0` | autre | **Câble** — le puits nominal depuis le bloc E2 |
//! | `0` | *n'importe* | **Aucun** |
//!
//! 🔴 **Deux puits ne peuvent pas consommer un même `LecteurMicro`** : chacun
//! draine ce que l'autre attend, et le symptôme serait un micro qui hoquette
//! sans qu'aucune ligne ne le dise. D'où une règle **pure et testée sur
//! l'hôte**, plutôt que deux `if` posés l'un après l'autre.
//!
//! Le puits de MESURE lui-même (`MICRO_MESURE=1`) vit dans `micro/mesure.rs` :
//! un consommateur qui joue le rôle du câble et rend au journal ce qu'il a
//! entendu.
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

/// Quel puits reçoit le flux montant. Voir la table en tête de module.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Puits {
    /// Le câble virtuel (`windows_micro`) : le puits nominal.
    Cable,
    /// L'instrument de banc (`MICRO_MESURE=1`).
    Mesure,
    /// Aucun : `MICRO=0`. `micro_disponible()` reste faux et `ready` porte
    /// `mic: false`.
    Aucun,
}

/// **PUR, et testé sur l'hôte.** L'arbitrage entre les deux puits.
///
/// ⚠️ **Il prend deux booléens et non `&Config`, contrairement à la lettre du
/// plan E2 (tâche 10, step 2).** Deux raisons : c'est tout ce dont la règle a
/// besoin — un `Config` porte dix-huit champs dont seize sont hors sujet —, et
/// surtout il n'existe aucun constructeur de `Config` pour les tests, si bien
/// qu'exiger le type entier aurait rendu la règle éprouvable seulement au prix
/// d'un montage. Une règle qu'on n'éprouve pas parce qu'elle coûte trop cher à
/// monter est une règle non éprouvée.
pub(crate) fn choisir_puits(micro: bool, micro_mesure: bool) -> Puits {
    if !micro {
        return Puits::Aucun;
    }
    // ⚠️ **L'instrument de banc PREND LE PAS**, il ne s'ajoute pas : on l'arme
    // pour observer *au lieu* du câble.
    if micro_mesure {
        return Puits::Mesure;
    }
    Puits::Cable
}

/// `MICRO` laisse-t-elle le micro armé ? **Tout sauf `0`.**
///
/// ⚠️ **Convention INVERSE de celle de `MICRO_MESURE` juste en dessous, et à
/// dessein** : on désarme sur `=0` ce qui est LIVRÉ, on arme sur `=1` ce qui ne
/// l'est pas. Le micro est livré depuis le bloc E2 ; le puits de mesure ne le
/// sera jamais.
///
/// **Ne jamais tester `is_ok()`** : quelqu'un qui écrirait `MICRO=0` pour être
/// sûr de le couper l'allumerait. Un test garde ce prédicat.
pub(crate) fn arme_micro(valeur: Option<&str>) -> bool {
    valeur != Some("0")
}

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

/// Installe sur `session` le puits que [`choisir_puits`] désigne — le câble,
/// l'instrument de banc, ou aucun.
///
/// ⚠️ **Cette ligne disait « le puits de mesure, si `MICRO_MESURE=1` », et
/// concluait « tant que le vrai câble (bloc E2) n'existe pas ».** Le câble
/// existe depuis la tâche 9 de ce même bloc : la phrase était devenue fausse
/// dans la branche qui la livrait, ce qui est la classe de défaut exacte que
/// la revue transverse de fin de branche cherche. Corrigée ici plutôt que
/// laissée à trouver.
///
/// **Aucune de ces trois issues ne compromet la session** : quand aucun puits
/// n'est posé, `micro_disponible()` reste faux, `ready` porte `mic: false`, et
/// le bouton du navigateur ne paraît pas.
pub(super) fn brancher(config: &Config, session: &mut Session) {
    match choisir_puits(config.micro, config.micro_mesure) {
        Puits::Aucun => {
            // ⚠️ ÉMISE AU BRANCHEMENT, comme celle du puits de mesure : son
            // absence est ce qui prouve que `MICRO` n'a pas atteint le
            // processus.
            tracing::info!(
                session = %config.session_id,
                "micro DESARME (MICRO=0)"
            );
        }
        Puits::Mesure => {
            tracing::warn!(
                session = %config.session_id,
                "micro : l'instrument de banc PREND LE PAS sur le cable (MICRO_MESURE=1). Deux \
                 puits ne peuvent pas consommer le meme flux montant — chacun mangerait ce que \
                 l'autre attend"
            );
            brancher_mesure(config, session);
        }
        Puits::Cable => brancher_cable(config, session),
    }
}

/// Le puits NOMINAL : l'écriture sur le câble virtuel.
///
/// Son échec ne compromet jamais la session — `micro_disponible()` reste faux,
/// `ready` porte `mic: false`, et le bouton du navigateur ne paraît pas. C'est
/// le comportement d'avant le bloc E2, et il reste atteignable pour toutes les
/// raisons que `windows_micro::ouvrir` sait nommer : câble introuvable ou
/// ambigu, format refusé, boucle locale.
#[cfg(windows)]
fn brancher_cable(config: &Config, session: &mut Session) {
    match crate::windows_micro::ouvrir(config) {
        Ok(puits) => session.set_puits_micro(Box::new(puits)),
        Err(e) => tracing::warn!(
            session = %config.session_id,
            erreur = %e,
            "micro indisponible, la session continue sans"
        ),
    }
}

/// Sur l'hôte Linux il n'y a pas de câble, et il n'y a rien à dire : ce mode
/// n'existe que pour que le crate compile et que les règles pures s'éprouvent.
#[cfg(not(windows))]
fn brancher_cable(_config: &Config, _session: &mut Session) {}

/// L'instrument de banc.
fn brancher_mesure(config: &Config, session: &mut Session) {
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

#[cfg(test)]
mod tests {
    use super::*;

    /// 🔴 **LE test de la Décision 10, et le seul de ce fichier qui protège
    /// contre une panne silencieuse.** Deux puits ne peuvent pas consommer un
    /// même `LecteurMicro` : chacun draine ce que l'autre attend, et le
    /// symptôme serait un micro qui hoquette sans qu'aucune ligne ne le dise.
    /// `MICRO_MESURE=1` gagne — on arme un instrument de banc pour observer
    /// *au lieu* du câble, jamais en plus.
    #[test]
    fn le_puits_de_mesure_PREND_LE_PAS_sur_le_cable() {
        assert_eq!(choisir_puits(true, true), Puits::Mesure);
    }

    #[test]
    fn sans_instrument_de_banc_le_puits_est_le_cable() {
        assert_eq!(choisir_puits(true, false), Puits::Cable);
    }

    /// `MICRO=0` désarme tout, y compris l'instrument de banc : la variable
    /// dit « pas de micro », pas « pas de câble ».
    #[test]
    fn micro_desarme_ne_pose_aucun_puits() {
        assert_eq!(choisir_puits(false, false), Puits::Aucun);
        assert_eq!(choisir_puits(false, true), Puits::Aucun);
    }

    /// ⚠️ **`MICRO` DÉSARME sur `=0` ; une simple présence n'arme pas.**
    /// Convention d'`AUDIO`, `PLEIN_ECRAN`, `SUPERVISEUR` et `CAPTEUR` : on
    /// désarme sur `=0` ce qui est LIVRÉ. Ce test existe pour empêcher une
    /// « simplification » future en `is_ok()`, qui allumerait le micro chez
    /// quelqu'un qui écrit `MICRO=0` pour être sûr de le couper.
    #[test]
    fn seule_la_valeur_zero_desarme_le_micro() {
        assert!(!arme_micro(Some("0")));
        assert!(arme_micro(None));
        assert!(arme_micro(Some("")));
        assert!(arme_micro(Some("1")));
        assert!(arme_micro(Some("nimporte quoi")));
    }
}
