//! Sonde du chantier B : ViGEmBus est-il utilisable, et son rappel de
//! vibration restitue-t-il les magnitudes ?
//!
//! Ce fichier est temporaire : la tâche 10 le remplace par le module
//! définitif. Il n'existe que pour figer l'API réelle de `vigem-client`.
//!
//! **Écart avec la forme envisagée au départ** : il n'existe pas de
//! `notification.wait_timeout(Duration)`. L'API réelle (vérifiée sur le
//! code source publié de la crate, docs.rs 0.1.4) est :
//!
//! - `target.request_notification() -> Result<XRequestNotification, Error>`,
//!   disponible uniquement avec la fonctionnalité de crate
//!   `unstable_xtarget_notification` (sans elle la méthode n'existe pas du
//!   tout — pas une erreur à l'exécution, une absence à la compilation).
//! - `XRequestNotification` n'est pas directement pollable : ses méthodes
//!   bas niveau (`request`, `poll`) exigent un `Pin<&mut Self>` parce que la
//!   structure contient un `PhantomPinned`. L'usage prévu par la crate
//!   elle-même est sa méthode `spawn_thread(self, f)`, qui fait tourner la
//!   boucle requête/attente dans un fil dédié et rappelle `f` à chaque
//!   notification reçue — pas de délai réglable non plus, elle bloque tant
//!   qu'aucune notification n'arrive.
//! - La structure reçue est `XNotification { large_motor: u8, small_motor:
//!   u8, led_number: u8 }` — les noms de champs supposés dans le brief
//!   étaient corrects.
//!
//! On relaie donc les notifications du fil de `spawn_thread` vers ce fil-ci
//! par un canal `mpsc`, et c'est CE canal qu'on interroge avec un délai
//! (`recv_timeout`) pour retrouver le comportement « attends jusqu'à N
//! secondes » que la sonde doit avoir.

use anyhow::{Context, Result};
use std::sync::mpsc;
use std::time::{Duration, Instant};

pub fn probe(secondes: u64) -> Result<String> {
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
