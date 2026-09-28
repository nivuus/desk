//! A window's thread: it holds a complete `WindowsSource` and serves it to
//! the child through the pipe.
//!
//! **It rewrites NO capture or encoding code.** `WindowsSource` is
//! already exactly the `DesktopCapture` + `H264Encoder` pair behind the
//! `VideoSource` trait: this module only calls that trait and carries
//! its results. That is the central simplification of sub-block D4.
//!
//! **Since sub-block D5, the source is OPTIONAL.** The pool
//! (`capteur/vivier.rs`, wired to channels by `capteur/sommeil.rs`)
//! orders this thread to release its encoder and its duplication, then to
//! rebuild them. Both gestures happen HERE and nowhere else, for the
//! same reason as the loop exit: `Drop for H264Encoder` can freeze, and
//! on this thread a freeze would only cost this window.
//!
//! Five files, because sub-block D5 took this one from 336 to more than
//! 600 lines: the loop and the transport stay here, opening a
//! window at its attach (D10, review of task 8), the sleep
//! transitions, serving commands and the counters trace (D9, task 11)
//! live in the child modules.

#![cfg(windows)]

// `transitions` carries `dormir` and `reveiller` — the execution, for THIS
// window, of what `crate::capteur::sommeil` decides for all. It is
// deliberately NOT called `sommeil`: two modules of that name in the same
// subtree would be confused when reading, and the registry import would
// collide with the child.
mod commandes;
// `accent_fenetre` carries A1's accent round — fifth child module, on
// the same pattern as the other four, and its reason to be is written in its
// own header rather than copied here.
//
// ⚠️ **It carries a `#[path]` where its four siblings do not need one, and for
// the same reason `transitions` is not called `sommeil`**: a `mod accent;`
// would collide, when reading as well as in naming, with the
// `use crate::accent;` of this file. That `#[path]` is OUTSIDE the convention
// of `CLAUDE.md`, which only targets modules extracted from a
// `#[cfg(windows)]` parent to compile on the host.
#[path = "fenetre/accent.rs"]
mod accent_fenetre;
// `ouverture` carries `Fenetre::ouvrir` — fourth child module on the same
// pattern as the other three, extracted in review of task 8 of sub-block
// D10: tasks 8 and 9 had taken this file to 505 lines,
// above the cap of 500.
mod ouverture;
// The media connection's writer thread, and what it is handed.
mod media;
// `trace` carries the periodic trace of the capture counters
// (`SOURCE_TRACE=1`) — third child module on the same pattern as the
// two above, extracted in review of task 11 (D9) for the same
// size-cap reason.
mod trace;
mod transitions;

use std::io::Write;
use std::sync::mpsc::{sync_channel, Receiver, Sender, SyncSender};

use crate::capteur::sommeil::file::ReceveurSession;
use std::time::{Duration, Instant};

use anyhow::Result;
use windows::Win32::Foundation::HWND;

use crate::accent;
use crate::capteur::plein_ecran;
use crate::capteur::protocole::{DepuisCapteur, VersCapteur};
use crate::source::VideoSource;
use crate::windows_source::WindowsSource;

use self::commandes::{deposer, servir_les_commandes};
use self::media::{ecrire_le_media, AEcrire, CAPACITE_ECRITURES};
use self::trace::tracer_les_compteurs;

/// No sleep when the source has returned nothing.
///
/// 10 ms, the exact value of `FRAME_INTERVAL` on the transport side: this loop
/// takes the place of the polling the child used to do, and there is no
/// reason to change the polling cadence at the same time as the rest. The
/// comment in `transport/piste_video.rs` explains why 10 ms and not
/// 16: polling more often than the source produces lifts a bound without
/// costing anything when there is nothing to take.
const PAS_A_VIDE: Duration = Duration::from_millis(10);

/// Period of the counter lines. **Never a trace per frame**: the project
/// has already lost an entire session to a per-packet trace.
const PERIODE_COMPTEURS: Duration = Duration::from_secs(10);

/// Whether to continue the window loop, or close it — and why.
enum Fin {
    Continuer,
    Terminer(&'static str),
}

/// What command serving needs to know besides the source.
///
/// **Grouped because these four always travel together**:
/// `servir_les_commandes` has the commands executed, and `deposer` calls it at
/// every round of its back-pressure. Passing them one by one lengthened both
/// signatures by four parameters.
///
/// **Borrows nothing from `Fenetre`** — `taille` is copied — so that a
/// live context never prevents a method call on `&mut self`.
struct Contexte<'a> {
    /// The session, for the sleep registry: it is through it that the
    /// visibility received here is arbitrated globally.
    session: &'a str,
    /// KEPT dimensions of the window. The only possible reply to a
    /// resize received during sleep, when there is no longer a source to
    /// query.
    taille: (u32, u32),
    commandes: &'a Receiver<VersCapteur>,
    reponses: &'a Sender<DepuisCapteur>,
}

/// What is needed to rebuild the source identically after a sleep.
///
/// **`clock_origin` is kept, never recomputed**: it is the origin of the
/// video track's timestamps, and redoing it on wake-up would shift the stream by
/// the gap between the two origins — the same trap the attach solves through
/// `origine_qpc`.
struct Parametres {
    hwnd: HWND,
    sortie: String,
    fps: u32,
    debit: u32,
    clock_origin: Instant,
}

/// A window served by the sensor: its source, and what is needed to name it.
///
/// **The `WindowsSource` never leaves the thread that built it.** It carries
/// COM objects and is not `Sync`: `ouvrir` and `servir` are called on
/// the window thread alone, and commands reach it through `mpsc` from
/// the thread holding the command connection.
pub struct Fenetre {
    /// `None` when the window sleeps: the encoder and the DXGI duplication are
    /// then released, and that is the whole purpose of sub-block D5. The virtual
    /// output, for its part, is never touched — that is what avoids inflicting a
    /// mutex abandonment on the neighbouring windows at every fall-asleep.
    source: Option<WindowsSource>,
    parametres: Parametres,
    session: String,
    largeur: u32,
    hauteur: u32,
    /// PID of the process owning the Windows window, derived from the `hwnd` at
    /// attach time. It is through it that `capteur::audio::arbitrer` groups the
    /// windows of the same application.
    pid: u32,
}

impl Fenetre {
    pub fn dimensions(&self) -> (u32, u32) {
        (self.largeur, self.hauteur)
    }

    /// Serves the window until the end of its life.
    ///
    /// `ecrivain` is the **media** connection, and it is handed to a dedicated
    /// WRITER thread: this thread no longer touches any file object. The
    /// command replies go out through `reponses`, to the thread holding the
    /// command connection, which writes them itself. No file object
    /// therefore ever carries a concurrent read and write.
    ///
    /// **Why a writer thread rather than a direct write.** A
    /// blocking write here blocked the window thread *after* its polling
    /// of commands, hence without serving any of them — and the child, which waits for its
    /// reply without a timeout since task 10, could never resume
    /// its media read again: a six-link deadlock, found by the review
    /// of task 10. The only wait this thread can still suffer is that
    /// of `deposer`, **which serves the commands at every round**.
    pub fn servir<E: Write + Send + 'static>(
        mut self,
        commandes: Receiver<VersCapteur>,
        reponses: Sender<DepuisCapteur>,
        ecrivain: E,
    ) -> Result<()> {
        // Copied once: the traces cite it at every round, and the loop
        // borrows `self` mutably all that time.
        let session = self.session.clone();

        // D6's record no. 2, carried here: all children and the
        // sensor write to the SAME `agent.log` (stdout inherited since D4).
        // A trace without `session` there is a number in an anonymous
        // multiset, and D6 had to add this field to two traces IN THE MIDDLE OF AN ACCEPTANCE RUN.
        // A span set once on the window thread gives it to everything
        // emitted below, including the `warn!` of the modules called.
        let _span = tracing::info_span!("fenetre", session = %session).entered();

        // 🔴 **`pid` IS THE REPOSITORY'S ONLY session ↔ WINDOWS window
        // ATTRIBUTION, AND IT MUST BE SAID SO THAT A SUCCESSOR DOES NOT REMOVE IT
        // AS NOISE** (D-P3-7, sub-block P3). `enfant lancé`
        // (`superviseur/enfants.rs`) does carry a `pid`, but it is that of the
        // AGENT CHILD process; no other trace associates a `session`
        // with the `hwnd` or the PID of the Windows APPLICATION it streams.
        //
        // Without it, an acceptance run writing "B's text arrived in B's
        // window" is not ATTRIBUTABLE — and a non-attributable reading
        // is not a verdict. The "paste a nonce and see which
        // Notepad grew" route is CIRCULAR: it would establish attribution
        // through the very mechanism the acceptance run measures, a defect D8 paid for on
        // `resoudreIdentite` and that its own report describes as
        // "partially circular".
        //
        // The field ALREADY lives on `Fenetre` and is ALREADY passed to `inscrire`: nothing
        // had to be brought up. The driver then resolves it into
        // `MainWindowHandle` through `Get-Process -Id`, then reads through `WM_GETTEXT`.
        tracing::info!(
            %session,
            pid = self.pid,
            sortie = %self.parametres.sortie,
            largeur = self.largeur,
            hauteur = self.hauteur,
            "fenêtre attachée au capteur"
        );

        let (ecritures, a_ecrire) = sync_channel::<AEcrire>(CAPACITE_ECRITURES);
        let session_ecrivain = session.clone();
        std::thread::spawn(move || ecrire_le_media(ecrivain, a_ecrire, &session_ecrivain));

        // Registration in the pool. A window is born ASLEEP on both sides — in
        // the pool AND here, `source` being `None` since `ouvrir`: that is what
        // makes the pool see the truth from the very first second, and that
        // at most eight encoders exist whatever the number of attached
        // windows. The client's first visibility signal will wake it up.
        //
        // ⚠️ **Corollary: a window whose client NEVER announces its
        // visibility never wakes up, and its page stays black.** That is the
        // intended behaviour — no encoder must be taken for a
        // window nobody declares they are watching —, but it is the new
        // way a session can stay empty without any error being
        // logged.
        //
        // `inscrire` stamps and returns the GENERATION of this registration (D9,
        // F5 of D7): kept locally — never on `self`, it only makes sense
        // between this call and the `retirer` at the end of the function, both
        // on this same thread — and handed back as is to `retirer`, the only way
        // for the registry to recognise a `retirer` already made stale by a
        // re-attachment that happened in between.
        let (ordres, generation) = crate::capteur::sommeil::inscrire(&session, self.pid);

        let resultat = self.boucler(&session, &ordres, &ecritures, &commandes, &reponses);

        // **SINGLE passage point of all exits from the loop**,
        // including its error exits: a session that exited without
        // withdrawing would keep its place in the pool for the whole life of the process.
        // A panic on this thread would nonetheless short-circuit these two lines —
        // the safety net is then the drop of `ordres` during stack
        // unwinding, which the registry's wheel round sees as a broken channel and
        // removes by itself. This path is the deterministic one.
        //
        // The source is released BEFORE the withdrawal, and explicitly rather than
        // through the drop of `self` at the end of the function, so that this order does not
        // depend on the position of a `return`: `retirer` returns a place
        // the pool can immediately assign to a sleeping window, which
        // would ask the hardware for one more encoder if ours were still
        // alive. The release stays on this thread, as everywhere else.
        drop(self.source.take());
        crate::capteur::sommeil::retirer(&session, generation);
        resultat
    }

    /// The service loop. **Extracted from `servir` so that releasing
    /// the source and withdrawing from the pool have only one passage point**,
    /// whatever the exit path.
    ///
    /// ⚠️ **No borrow of `self.source` survives a statement.**
    /// That is the structuring constraint of this function since the source
    /// became optional: `appliquer_les_ordres` needs the whole `&mut self`
    /// (it releases and rebuilds the source), which is incompatible with the
    /// `let source = &mut self.source;` this loop used to hold from
    /// end to end. Each usage point therefore takes a fresh borrow through
    /// `self.source.as_mut()`, in a statement that ends. **Do not
    /// reintroduce a long borrow**: the compiler would refuse it, but the
    /// temptation to work around it by moving sleep off this thread would
    /// break the invariant of releasing on this thread.
    fn boucler(
        &mut self,
        session: &str,
        ordres: &ReceveurSession,
        ecritures: &SyncSender<AEcrire>,
        commandes: &Receiver<VersCapteur>,
        reponses: &Sender<DepuisCapteur>,
    ) -> Result<()> {
        // Initialised on the size RESOLVED by `ouvrir` — the very one that
        // went out in `Attachee` —, never on zero nor on a guessed value.
        // That is what makes the comparison in point 3 a real safety net: if the
        // texture returned at the first wake-up is not the announced size (a
        // DPI-scaled output announces less than it renders, see
        // `capture::ouverture::taille_de_sortie`), the gap becomes an `Etat` the
        // child applies. Starting from zero would have produced a useless `Etat` at
        // every session; starting from a guess would have hidden the gap.
        let mut dernier_etat = (true, false, self.largeur, self.hauteur);
        let mut images = 0u64;
        let mut dernier_compte = Instant::now();
        // D8: the reference state is the one read when the window is OPENED, not
        // an arbitrary default value — it is the guard that prevents an
        // application born borderless from putting its browser window into
        // fullscreen for no reason (see `plein_ecran::SuiviBordure`).
        let mut suivi_bordure = plein_ecran::SuiviBordure::nouveau(
            plein_ecran::lire_style(self.parametres.hwnd).unwrap_or(0),
        );
        let mut dernier_style = Instant::now();
        // A1: `SuiviAccent` starts from `None` and ANNOUNCES ITS FIRST READING —
        // it is the REVERSE of `SuiviBordure` just above, and the why lives
        // in the doc of `accent::SuiviAccent`.
        let mut suivi_accent = accent::SuiviAccent::neuf();
        // `Instant::now()` and not "a long time ago": the first reading
        // waits for `PERIODE_ACCENT`, which lets the session settle. ⚠️ If the
        // acceptance run finds it too late, it is `PERIODE_ACCENT` that must be
        // tuned, not this line.
        let mut dernier_accent = Instant::now();

        let motif = loop {
            // Redone at every round: the kept size may change on wake-up.
            // Contains only copies and borrows external to `self`,
            // so hinders no `&mut self`.
            let ctx = Contexte {
                session,
                taille: (self.largeur, self.hauteur),
                commandes,
                reponses,
            };

            // 0. The pool's orders. Before everything else: sleeping frees
            //    resources, and there is no reason to encode one more
            //    frame when the order is already there.
            if let Fin::Terminer(motif) = self.appliquer_les_ordres(ordres, ecritures, &ctx) {
                break motif;
            }

            // 1. The pending commands, if any. They are rare, and
            //    they are served EVEN WHEN ASLEEP: refusing everything during
            //    sleep would make the child's network adaptation fail and
            //    close the session through a path foreign to sleep.
            if let Fin::Terminer(motif) = servir_les_commandes(self.source.as_mut(), &ctx) {
                break motif;
            }

            // 2. A frame, if there is one — and there never is when the
            //    window sleeps. Extracted by a statement that ends,
            //    so that the borrow dies with it: `deposer` takes a
            //    fresh one right after.
            let unite = match self.source.as_mut() {
                Some(source) => source.next_frame(),
                None => None,
            };
            match unite {
                Some(unite) => {
                    images += 1;
                    // The bounded queue IS the back-pressure: if the child no longer
                    // reads, this thread ends up waiting — and it only waits
                    // for ITS window, without ever ceasing to serve the
                    // commands. An access unit cannot be thrown away without
                    // corrupting the stream (P frames reference the
                    // previous ones), hence waiting rather than dropping.
                    if let Fin::Terminer(motif) =
                        deposer(AEcrire::Image(unite), ecritures, self.source.as_mut(), &ctx)
                    {
                        break motif;
                    }
                }
                // Nothing to send: either the desktop has not changed, or the
                // window sleeps. In both cases, take a breath.
                None => std::thread::sleep(PAS_A_VIDE),
            }

            // 3. The state, on CHANGE only. A sleeping window has none
            //    to report: its last `Etat` stays true — the output and
            //    the geometry do not move during sleep — and it is
            //    `Sommeil` that tells the client what is happening to it.
            let etat = self.source.as_ref().map(|source| {
                let (largeur, hauteur) = source.dimensions();
                (source.is_alive(), source.is_exhausted(), largeur, hauteur)
            });
            if let Some(etat) = etat {
                if etat != dernier_etat {
                    dernier_etat = etat;
                    // 🔴 **THE KEPT SIZE FOLLOWS, AND IT IS NEW IN BATCH 33.**
                    // `reveiller` rebuilds the source on `self.dimensions()`
                    // and the asleep arm of `servir_les_commandes` replies
                    // `ctx.taille`: as long as `resize` was a `no-op` in
                    // `SortieEntiere`, these two fields could not
                    // drift, and `transitions.rs` relied on it in so many
                    // words. Since cropping follows the viewport, they
                    // can — a window resized then put to sleep would
                    // wake up at ITS OPENING SIZE, erasing the
                    // resize without a trace.
                    self.largeur = etat.2;
                    self.hauteur = etat.3;
                    let message = DepuisCapteur::Etat {
                        vivante: etat.0,
                        epuisee: etat.1,
                        largeur: etat.2,
                        hauteur: etat.3,
                    };
                    if let Fin::Terminer(motif) = deposer(
                        AEcrire::Etat(message),
                        ecritures,
                        self.source.as_mut(),
                        &ctx,
                    ) {
                        break motif;
                    }
                    if !etat.0 || etat.1 {
                        tracing::info!(%session, vivante = etat.0, epuisee = etat.1, "source close");
                        break "la source est morte ou épuisée";
                    }
                }
            }

            // 4. D8: the window's style says whether the application has gone
            //    fullscreen. Throttled by its own timer — see
            //    `plein_ecran::PERIODE_STYLE`.
            //
            //    `plein_ecran::actif()` FIRST: `PLEIN_ECRAN=0` disarms
            //    detection — re-reading the style and the resulting `PleinEcran`
            //    announcement. That is all this mechanism does
            //    now: sub-block D9 removed the other half, the
            //    virtual output's mode change (see
            //    `plein_ecran::actif` for the measurement finding).
            if plein_ecran::actif() && dernier_style.elapsed() >= plein_ecran::PERIODE_STYLE {
                dernier_style = Instant::now();
                if let Some(style) = plein_ecran::lire_style(self.parametres.hwnd) {
                    if let Some(actif) = suivi_bordure.observer(style) {
                        tracing::info!(%session, actif, "plein ecran de la fenetre Windows");
                        let message = DepuisCapteur::PleinEcran { actif };
                        if let Fin::Terminer(motif) = deposer(
                            AEcrire::Etat(message),
                            ecritures,
                            self.source.as_mut(),
                            &ctx,
                        ) {
                            break motif;
                        }
                    }
                }
            }

            // 5. A1: the dominant tint of the window's icon.
            //    **The body lives in `fenetre/accent.rs`**: the addition would have
            //    taken this file to EXACTLY 500 lines, hence to zero margin,
            //    and the repository's rule is "extraction, never compression".
            //    It has already crossed 500 twice (508 in D9, 505 in D10).
            #[cfg(windows)]
            if let Some(Fin::Terminer(motif)) = accent_fenetre::tour(
                &mut suivi_accent,
                &mut dernier_accent,
                self.parametres.hwnd,
                ecritures,
                self.source.as_mut(),
                &ctx,
            ) {
                break motif;
            }

            if dernier_compte.elapsed() >= PERIODE_COMPTEURS {
                let ecoule = dernier_compte.elapsed().as_secs_f64();
                tracing::info!(
                    %session,
                    images,
                    endormie = self.source.is_none(),
                    cadence = format!("{:.1}", images as f64 / ecoule),
                    "cadence du capteur"
                );
                tracer_les_compteurs(self.source.as_ref());
                images = 0;
                dernier_compte = Instant::now();
            }
        };

        tracing::info!(%session, images, motif, "fin de la fenêtre côté capteur");
        Ok(())
    }
}
