//! Le module ViGEmBus définitif : `VirtualPad` branche une manette Xbox 360
//! virtuelle et lui applique les états reçus, `spawn_rumble` relaie ses
//! notifications de vibration vers le client via le canal de contrôle.
//!
//! L'API réelle de `vigem-client` diffère de celle envisagée au brief de
//! cette tâche sur deux points, tous deux établis empiriquement par la sonde
//! `probe` (`gamepad/probe.rs` — voir aussi
//! `docs/superpowers/plans/2026-07-28-input-jeu-sondes.md`) :
//!
//! 1. Il n'existe pas de `notification.wait_timeout(Duration)`. L'usage
//!    prévu par la crate est `request_notification()` puis
//!    `spawn_thread(f)` : ce dernier consomme la requête et fait tourner la
//!    boucle requête/attente sur UN FIL DÉDIÉ créé par la crate elle-même,
//!    en rappelant `f` à chaque notification — sans délai réglable, il
//!    bloque tant qu'aucune notification n'arrive. On relaie donc chaque
//!    notification brute vers un `mpsc` propre à ce module, interrogé lui
//!    avec un délai (`recv_timeout`) pour retrouver un comportement
//!    d'attente bornée et pouvoir vérifier périodiquement l'arrêt demandé.
//! 2. `wait_ready()` ne garantit pas qu'un `update()` immédiat réussisse :
//!    le bus USB virtuel peut ne pas avoir fini son énumération PnP côté
//!    Windows, et le premier `update()` échoue alors avec `WinError(259)`
//!    (`ERROR_NO_MORE_ITEMS`), une variante que `vigem-client` ne traduit
//!    PAS en `Error::TargetNotReady` (seul `ERROR_DEV_NOT_EXIST` l'est). Une
//!    seule reprise a suffi lors de l'unique mesure de la sonde ; n'ayant
//!    caractérisé ce comportement qu'une fois, on garde ici la même marge
//!    que la sonde (jusqu'à 20 reprises, 250 ms chacune) plutôt que le
//!    minimum observé.

use super::{plus_recent, LimiteurVibration, PERIODE_MIN};
use anyhow::{Context, Result};
use proto::control::AgentControl;
use proto::input::GamepadState;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// Nombre maximal de REPRISES d'`update()` après `wait_ready()` avant
/// d'abandonner (donc 1 + `TENTATIVES_MAX` appels à `update()` au plus
/// au total) — même décompte et mêmes valeurs que la sonde de la tâche 1.
const TENTATIVES_MAX: u32 = 20;
const DELAI_TENTATIVE: Duration = Duration::from_millis(250);

/// Manette Xbox 360 virtuelle. Branchée à la PREMIÈRE réception d'un
/// état, jamais au démarrage : une manette présente en permanence
/// perturbe les applications qui réagissent à sa seule présence
/// (`demarrage.rs` réalise ce branchement paresseux).
pub struct VirtualPad {
    target: vigem_client::Xbox360Wired<vigem_client::Client>,
    derniere_seq: Option<u16>,
}

impl VirtualPad {
    /// Branche la cible et attend qu'elle accepte réellement un état,
    /// pas seulement que `wait_ready()` le prétende (voir le point 2 du
    /// commentaire de module). Renvoie une erreur si ViGEmBus est
    /// absent ou reste indisponible après toutes les reprises :
    /// `demarrage.rs` traite cet échec comme non bloquant pour la session.
    ///
    /// Peut bloquer jusqu'à `TENTATIVES_MAX * DELAI_TENTATIVE` (5 s avec
    /// les valeurs actuelles) : à appeler hors du fil qui pilote la
    /// session (voir `spawn_connect` plus bas), jamais directement
    /// depuis la boucle de `Session::run`.
    pub fn connect() -> Result<Self> {
        let client =
            vigem_client::Client::connect().context("connexion au pilote ViGEmBus")?;
        let mut target = vigem_client::Xbox360Wired::new(
            client,
            vigem_client::TargetId::XBOX360_WIRED,
        );
        target
            .plugin()
            .context("branchement de la manette virtuelle")?;
        target
            .wait_ready()
            .context("attente de disponibilité")?;

        // État neutre : cette première écriture ne sert qu'à confirmer
        // que la cible accepte réellement un `update()`, pas à refléter
        // un état de manette reçu — `derniere_seq` reste `None` après
        // cet appel, pour ne pas se substituer au premier vrai état.
        let neutre = vigem_client::XGamepad::default();
        let mut tentatives = 0u32;
        loop {
            match target.update(&neutre) {
                Ok(()) => break,
                // Uniquement les deux variantes documentées comme
                // « pas encore prêt » (voir le commentaire de module) :
                // une erreur différente (bus absent, permission refusée,
                // etc.) est définitive, retenter ne changerait rien et
                // ferait perdre jusqu'à 5 s pour rien.
                Err(
                    e @ (vigem_client::Error::WinError(259)
                    | vigem_client::Error::TargetNotReady),
                ) if tentatives < TENTATIVES_MAX => {
                    tentatives += 1;
                    tracing::warn!(
                        tentative = tentatives,
                        erreur = ?e,
                        "update() pas encore prêt, nouvelle tentative"
                    );
                    std::thread::sleep(DELAI_TENTATIVE);
                }
                Err(e) => {
                    return Err(e).context(
                        "premier envoi d'état à la manette virtuelle, après toutes les reprises",
                    )
                }
            }
        }
        if tentatives > 0 {
            tracing::info!(tentatives, "update() a fini par réussir après attente");
        }
        tracing::info!("manette virtuelle branchée");
        Ok(Self { target, derniere_seq: None })
    }

    /// Applique un état reçu du client à la manette virtuelle.
    ///
    /// Rejette les états périmés AVANT d'atteindre le pilote : le canal
    /// qui transporte les `GamepadState` n'est pas ordonné, un état plus
    /// ancien arrivé après un plus récent le rétablirait sinon à tort.
    pub fn apply(&mut self, state: &GamepadState) -> Result<()> {
        if let Some(courante) = self.derniere_seq {
            if !plus_recent(state.seq, courante) {
                return Ok(());
            }
        }

        let gamepad = vigem_client::XGamepad {
            buttons: vigem_client::XButtons(state.buttons),
            left_trigger: state.left_trigger,
            right_trigger: state.right_trigger,
            thumb_lx: state.thumb_lx,
            thumb_ly: state.thumb_ly,
            thumb_rx: state.thumb_rx,
            thumb_ry: state.thumb_ry,
        };
        self.target
            .update(&gamepad)
            .context("application de l'état de manette")?;
        // Enregistré seulement après succès : un `update()` en échec ne
        // doit pas marquer cette séquence comme traitée, sous peine de
        // ne plus jamais pouvoir la réappliquer (`plus_recent` la
        // rejetterait alors comme périmée).
        self.derniere_seq = Some(state.seq);
        Ok(())
    }
}

/// Débranche explicitement la cible.
///
/// `Xbox360Wired::drop` le fait déjà tout seul (vérifié dans le code
/// source de la crate, `x360.rs`) : cet appel explicite est donc
/// redondant en pratique, mais rend l'intention lisible sans dépendre
/// d'un comportement de `Drop` qu'on ne voit pas au site d'appel.
///
/// **Pas de `join()` du fil interne de la crate ici** (contrairement à
/// une version antérieure de ce code) : `request()`, côté crate,
/// ignore le code de retour de son `DeviceIoControl` (`bus.rs`). Si le
/// débranchement tombe entre le retour d'un `poll()` et l'appel suivant
/// à `request()`, aucune E/S n'est en attente et `poll(true)` (qui
/// attend un `GetOverlappedResult` bloquant) ne reçoit jamais
/// `ERROR_OPERATION_ABORTED` : joindre ce fil pourrait alors bloquer
/// indéfiniment, sans délai de garde ni trace — figeant l'ouvrier
/// bloquant qui porte toute la session. Ce fil possède de toute façon
/// son propre `Client` dupliqué (`request_notification` appelle
/// `try_clone`) : il ne référence rien dans `VirtualPad`, et ne pas le
/// joindre ne fuit donc rien au-delà de la vie du processus. Seule la
/// sonde de la tâche 1 (cas nominal, un seul essai) a vérifié que le
/// débranchement seul suffit à débloquer ce fil ; ce commentaire
/// documente pourquoi on ne va pas plus loin.
impl Drop for VirtualPad {
    fn drop(&mut self) {
        let _ = self.target.unplug();
    }
}

/// Démarre le relais de vibration vers le client, sur son propre fil.
///
/// Prend `&mut VirtualPad` (et pas `&VirtualPad` comme envisagé au
/// brief) : `request_notification()` exige un accès mutable à la cible
/// — conséquence directe de la vraie signature de la crate, pas un choix
/// arbitraire.
///
/// Deux fils distincts collaborent ici :
/// - celui que la crate crée elle-même via `spawn_thread` : il tourne
///   côté pilote et ne fait QUE relayer chaque notification brute dans
///   un canal — jamais de logique dessus, pour ne jamais retarder le
///   rappel du pilote.
/// - celui rendu par cette fonction : il lit ce canal avec un délai
///   borné (`recv_timeout`), applique `LimiteurVibration` pour ne pas
///   inonder le canal de contrôle FIABLE vers la session (l'inonder lui
///   ferait accumuler du retard exactement quand le jeu vibre le plus),
///   et vérifie `arret` à chaque réveil pour pouvoir s'arrêter même sans
///   notification.
///
/// Arrêt propre : ce fil se termine quand `arret` passe à vrai, quand le
/// canal se ferme (ce qui arrive typiquement peu après que `VirtualPad`
/// est abandonné — `Drop`, ci-dessus, débranche la cible, ce qui fait
/// SOUVENT sortir le fil interne de la crate de son attente et donc
/// fermer ce canal, mais pas garanti à coup sûr selon où ce fil se
/// trouve dans sa boucle — voir le commentaire de `Drop`), ou quand
/// l'envoi vers `tx` échoue (session déjà terminée côté récepteur).
/// Sur toute sortie autre qu'un échec d'envoi, un dernier
/// `AgentControl::rumble(0, 0)` est tenté : le client ne doit jamais
/// rester bloqué à faire vibrer une manette physique parce que la fin
/// de session est arrivée entre deux notifications.
pub fn spawn_rumble(
    pad: &mut VirtualPad,
    tx: Sender<AgentControl>,
    arret: Arc<AtomicBool>,
) -> Result<JoinHandle<()>> {
    let requete = pad
        .target
        .request_notification()
        .context("abonnement aux notifications de vibration")?;

    let (notif_tx, notif_rx) = mpsc::channel::<vigem_client::XNotification>();
    // Fil créé et possédé par la crate, sur son propre `Client` dupliqué
    // (`request_notification` appelle `try_clone` en interne) : simple
    // relais, aucune logique dessus. Son `JoinHandle` est délibérément
    // abandonné (pas stocké, pas joint) : voir le commentaire de `Drop`
    // ci-dessus pour pourquoi le joindre serait risqué (blocage
    // indéfini possible) pour un bénéfice nul (ce fil ne référence rien
    // dans `VirtualPad`, ne pas le joindre ne fuit rien au-delà de la
    // vie du processus).
    let _fil_vigem = requete.spawn_thread(move |_requete, notification| {
        let _ = notif_tx.send(notification);
    });

    Ok(std::thread::spawn(move || {
        let mut limiteur = LimiteurVibration::new();
        'relais: while !arret.load(Ordering::Relaxed) {
            match notif_rx.recv_timeout(PERIODE_MIN) {
                Ok(vibration) => {
                    let etat = (vibration.large_motor, vibration.small_motor);
                    if let Some((gauche, droite)) = limiteur.observer(Instant::now(), etat) {
                        if tx.send(AgentControl::rumble(gauche, droite)).is_err() {
                            return;
                        }
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    // Rien reçu dans la fenêtre : occasion de vider un
                    // état différé par la limitation de débit.
                    if let Some((gauche, droite)) = limiteur.echu(Instant::now()) {
                        if tx.send(AgentControl::rumble(gauche, droite)).is_err() {
                            return;
                        }
                    }
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => break 'relais,
            }
        }
        // Best-effort : le canal de contrôle peut déjà être fermé côté
        // session (fin normale), auquel cas il n'y a de toute façon plus
        // personne pour lire ce message.
        let _ = tx.send(AgentControl::rumble(0, 0));
    }))
}

/// Lance `VirtualPad::connect()` sur un fil dédié et renvoie un
/// récepteur non bloquant.
///
/// `connect()` peut dormir jusqu'à 5 s (voir sa documentation) en cas
/// d'énumération PnP lente côté Windows. L'appeler directement depuis la
/// boucle de `Session::run` figerait vidéo ET audio pendant ce délai :
/// cette boucle est dimensionnée sur la cadence vidéo et les échéances
/// RTCP, pas sur la latence d'un pilote tiers (voir le commentaire sur
/// `spawn_blocking` dans `demarrage.rs`). `demarrage.rs` sonde ce récepteur avec
/// `try_recv()` à chaque état de manette reçu, sans jamais bloquer
/// dessus ; les états reçus pendant que la connexion est en cours sont
/// perdus sans conséquence — pas parce que le client sonde à 250 Hz
/// (cadence sous charge jamais mesurée, voir `client/src/gamepad.ts`),
/// mais parce qu'il réémet un état complet toutes les 100 ms même sans
/// changement (`RAFRAICHISSEMENT_MS`) : c'est ce rafraîchissement
/// périodique, indépendant de la fréquence de sondage, qui porte la
/// garantie d'auto-réparation.
pub fn spawn_connect() -> mpsc::Receiver<Result<VirtualPad>> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(VirtualPad::connect());
    });
    rx
}
