//! Manette virtuelle : ce fichier porte deux choses de nature différente.
//!
//! - Les logiques pures d'ordonnancement des états reçus (`plus_recent`) et
//!   de limitation du débit des vibrations (`LimiteurVibration`). Rien n'y
//!   est spécifique à Windows : elles vivent hors de tout `#[cfg(windows)]`
//!   et se testent sur cette machine Linux.
//! - La sonde du chantier B (`probe`, sous `#[cfg(windows)]` plus bas) :
//!   ViGEmBus est-il utilisable, et son rappel de vibration restitue-t-il
//!   les magnitudes ? Ce code est temporaire — la tâche 10 le remplace par
//!   le module ViGEmBus définitif — mais reste nécessaire ici : elle est
//!   encore appelée par `agent/src/main.rs` (`VIGEM_PROBE`), et la tâche 16
//!   (recette) prévoit explicitement de la réutiliser.

use std::time::{Duration, Instant};

/// Débit maximal des messages de vibration vers le client. Le canal de
/// contrôle est FIABLE : l'inonder lui ferait accumuler du retard exactement
/// quand le jeu produit le plus de vibrations.
pub const PERIODE_MIN: Duration = Duration::from_millis(20);

/// Vrai si `nouveau` succède à `courant` dans l'espace des séquences.
///
/// La soustraction en `u16` puis la relecture en `i16` traite le bouclage
/// sans cas particulier : 0 succède bien à 65535.
pub fn plus_recent(nouveau: u16, courant: u16) -> bool {
    (nouveau.wrapping_sub(courant)) as i16 > 0
}

/// Limite le débit des vibrations sans jamais perdre l'état courant.
///
/// `observer` rend l'état à émettre immédiatement, ou `None` s'il est
/// mémorisé. `echu`, appelée périodiquement, rend l'état mémorisé une fois le
/// délai écoulé. Un état mémorisé écrase le précédent : seul le dernier
/// décrit ce que le jeu demande.
pub struct LimiteurVibration {
    dernier_emis: Option<(u8, u8)>,
    dernier_envoi: Option<Instant>,
    en_attente: Option<(u8, u8)>,
}

impl Default for LimiteurVibration {
    fn default() -> Self {
        Self::new()
    }
}

impl LimiteurVibration {
    pub fn new() -> Self {
        Self {
            dernier_emis: None,
            dernier_envoi: None,
            en_attente: None,
        }
    }

    pub fn observer(&mut self, maintenant: Instant, etat: (u8, u8)) -> Option<(u8, u8)> {
        if self.dernier_emis == Some(etat) && self.en_attente.is_none() {
            return None;
        }
        let assez_tot = self
            .dernier_envoi
            .is_none_or(|precedent| maintenant.duration_since(precedent) >= PERIODE_MIN);
        if assez_tot {
            self.emettre(maintenant, etat)
        } else {
            self.en_attente = Some(etat);
            None
        }
    }

    pub fn echu(&mut self, maintenant: Instant) -> Option<(u8, u8)> {
        let etat = self.en_attente?;
        let assez_tot = self
            .dernier_envoi
            .is_none_or(|precedent| maintenant.duration_since(precedent) >= PERIODE_MIN);
        if !assez_tot {
            return None;
        }
        self.en_attente = None;
        if self.dernier_emis == Some(etat) {
            return None;
        }
        self.emettre(maintenant, etat)
    }

    fn emettre(&mut self, maintenant: Instant, etat: (u8, u8)) -> Option<(u8, u8)> {
        self.dernier_emis = Some(etat);
        self.dernier_envoi = Some(maintenant);
        self.en_attente = None;
        Some(etat)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn un_etat_plus_recent_est_accepte() {
        assert!(plus_recent(11, 10));
        assert!(plus_recent(1000, 1));
    }

    #[test]
    fn un_etat_plus_ancien_est_rejete() {
        assert!(!plus_recent(9, 10));
        assert!(!plus_recent(1, 1000));
    }

    #[test]
    fn un_etat_identique_est_rejete() {
        assert!(!plus_recent(10, 10));
    }

    #[test]
    fn le_bouclage_de_la_sequence_est_franchi_correctement() {
        // Le point du champ `seq` : à 250 Hz, l'u16 boucle toutes les
        // 4 minutes. Une comparaison naïve `nouveau > courant` rejetterait
        // alors tous les états pendant une demi-boucle — soit deux minutes
        // de manette figée.
        assert!(plus_recent(0, 65535));
        assert!(plus_recent(3, 65533));
        assert!(!plus_recent(65535, 0));
    }

    #[test]
    fn la_premiere_vibration_passe_immediatement() {
        let t0 = Instant::now();
        let mut limiteur = LimiteurVibration::new();
        assert_eq!(limiteur.observer(t0, (200, 100)), Some((200, 100)));
    }

    #[test]
    fn un_etat_identique_n_est_pas_reemis() {
        let t0 = Instant::now();
        let mut limiteur = LimiteurVibration::new();
        limiteur.observer(t0, (200, 100));
        assert_eq!(limiteur.observer(t0 + PERIODE_MIN * 2, (200, 100)), None);
    }

    #[test]
    fn un_changement_trop_rapproche_est_differe_puis_emis() {
        let t0 = Instant::now();
        let mut limiteur = LimiteurVibration::new();
        limiteur.observer(t0, (10, 0));
        assert_eq!(limiteur.observer(t0 + Duration::from_millis(5), (20, 0)), None);
        assert_eq!(limiteur.echu(t0 + Duration::from_millis(10)), None);
        assert_eq!(limiteur.echu(t0 + PERIODE_MIN), Some((20, 0)));
    }

    #[test]
    fn seul_le_dernier_etat_differe_est_emis() {
        // Une rafale de vibrations pendant la fenêtre de limitation ne doit
        // pas produire une file d'états périmés : c'est le DERNIER qui décrit
        // ce que le jeu demande maintenant.
        let t0 = Instant::now();
        let mut limiteur = LimiteurVibration::new();
        limiteur.observer(t0, (10, 0));
        limiteur.observer(t0 + Duration::from_millis(2), (20, 0));
        limiteur.observer(t0 + Duration::from_millis(4), (30, 0));
        limiteur.observer(t0 + Duration::from_millis(6), (40, 0));
        assert_eq!(limiteur.echu(t0 + PERIODE_MIN), Some((40, 0)));
        assert_eq!(limiteur.echu(t0 + PERIODE_MIN * 3), None);
    }
}

/// Sonde du chantier B : ViGEmBus est-il utilisable, et son rappel de
/// vibration restitue-t-il les magnitudes ?
///
/// Ce fichier est temporaire : la tâche 10 le remplace par le module
/// définitif. Il n'existe que pour figer l'API réelle de `vigem-client`.
///
/// **Écart avec la forme envisagée au départ** : il n'existe pas de
/// `notification.wait_timeout(Duration)`. L'API réelle (vérifiée sur le
/// code source publié de la crate, docs.rs 0.1.4) est :
///
/// - `target.request_notification() -> Result<XRequestNotification, Error>`,
///   disponible uniquement avec la fonctionnalité de crate
///   `unstable_xtarget_notification` (sans elle la méthode n'existe pas du
///   tout — pas une erreur à l'exécution, une absence à la compilation).
/// - `XRequestNotification` n'est pas directement pollable : ses méthodes
///   bas niveau (`request`, `poll`) exigent un `Pin<&mut Self>` parce que la
///   structure contient un `PhantomPinned`. L'usage prévu par la crate
///   elle-même est sa méthode `spawn_thread(self, f)`, qui fait tourner la
///   boucle requête/attente dans un fil dédié et rappelle `f` à chaque
///   notification reçue — pas de délai réglable non plus, elle bloque tant
///   qu'aucune notification n'arrive.
/// - La structure reçue est `XNotification { large_motor: u8, small_motor:
///   u8, led_number: u8 }` — les noms de champs supposés dans le brief
///   étaient corrects.
///
/// On relaie donc les notifications du fil de `spawn_thread` vers ce fil-ci
/// par un canal `mpsc`, et c'est CE canal qu'on interroge avec un délai
/// (`recv_timeout`) pour retrouver le comportement « attends jusqu'à N
/// secondes » que la sonde doit avoir.
#[cfg(windows)]
pub fn probe(secondes: u64) -> anyhow::Result<String> {
    use anyhow::Context;
    use std::sync::mpsc;

    let client = vigem_client::Client::connect().context("connexion au pilote ViGEmBus")?;
    let id = vigem_client::TargetId::XBOX360_WIRED;
    let mut target = vigem_client::Xbox360Wired::new(client, id);
    target
        .plugin()
        .context("branchement de la manette virtuelle")?;
    target
        .wait_ready()
        .context("attente de disponibilité")?;

    // Un état non neutre : si un outil Windows (joy.cpl) est ouvert sur la
    // VM, il doit le montrer.
    let etat = vigem_client::XGamepad {
        buttons: vigem_client::XButtons!(A),
        left_trigger: 128,
        right_trigger: 0,
        thumb_lx: 16384,
        thumb_ly: 0,
        thumb_rx: 0,
        thumb_ry: 0,
    };
    // Constat de cette sonde : `wait_ready()` peut rendre `Ok(())` alors que
    // le bus USB virtuel n'a pas fini son énumération PnP côté Windows — le
    // premier `update()` échoue alors avec `WinError(259)`
    // (`ERROR_NO_MORE_ITEMS`), une variante que `vigem-client` ne traduit
    // PAS en `Error::TargetNotReady` (seul `ERROR_DEV_NOT_EXIST` l'est).
    // `wait_ready` n'est donc pas une garantie suffisante avant le premier
    // envoi d'état : il faut réessayer avec un court repli. Documenté ici
    // pour la tâche 10, qui devra faire de même dans le module définitif.
    //
    // Décompte exact : le premier appel à `update()` ci-dessous compte comme
    // tentative n°1 et n'est pas soumis à la garde `tentatives < 20` (elle ne
    // s'évalue qu'après un premier échec) ; en cas d'échecs répétés, la
    // boucle en fait donc au plus 21 au total (1 initial + 20 reprises), pas
    // 20 — c'est bien ce que `tentatives` compte à la fin (nombre de
    // REPRISES, pas d'appels totaux).
    let mut tentatives = 0u32;
    loop {
        match target.update(&etat) {
            Ok(()) => break,
            Err(e) if tentatives < 20 => {
                tentatives += 1;
                tracing::warn!(
                    tentative = tentatives,
                    erreur = ?e,
                    "update() pas encore prêt, nouvelle tentative"
                );
                std::thread::sleep(Duration::from_millis(250));
            }
            Err(e) => {
                tracing::warn!(erreur = ?e, "update() a échoué — variante brute, abandon");
                return Err(e).context("application d'un état");
            }
        }
    }
    if tentatives > 0 {
        tracing::info!(tentatives, "update() a fini par réussir après attente");
    }

    // `request_notification()` puis `spawn_thread` : voir le commentaire de
    // module ci-dessus pour pourquoi ce détour est nécessaire plutôt qu'un
    // hypothétique `wait_timeout`.
    let (tx, rx) = mpsc::channel::<vigem_client::XNotification>();
    let requete = target
        .request_notification()
        .context("requête de notification (nécessite la fonctionnalité unstable_xtarget_notification)")?;
    let fil = requete.spawn_thread(move |_requete, notification| {
        let _ = tx.send(notification);
    });

    let mut recu = 0u32;
    let echeance = Instant::now() + Duration::from_secs(secondes);
    loop {
        let restant = echeance.saturating_duration_since(Instant::now());
        if restant.is_zero() {
            break;
        }
        match rx.recv_timeout(restant.min(Duration::from_millis(500))) {
            Ok(vibration) => {
                recu += 1;
                tracing::info!(
                    grand = vibration.large_motor,
                    petit = vibration.small_motor,
                    "vibration reçue"
                );
            }
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }

    // Débrancher la manette avant de joindre le fil : `poll` y est bloqué en
    // attente d'une notification, et seul le débranchement (qui annule la
    // requête en cours côté pilote) le fait sortir de sa boucle.
    drop(target);
    let _ = fil.join();

    Ok(format!(
        "branchement OK, {recu} notification(s) de vibration reçue(s)"
    ))
}
