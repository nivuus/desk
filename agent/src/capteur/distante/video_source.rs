//! `impl VideoSource for SourceDistante` — EXTRAIT VERBATIM de `distante.rs`.
//!
//! **Why this file exists**: `distante.rs` was at **472 lines** for
//! a project cap of 500, and sub-block A1 (the accent colour) had to
//! graft the `PressePapier` pattern onto it — a `Recu` variant, a
//! retention field, an arm and an accessor, some thirty lines. **The margin
//! of 28 was not enough.** The extraction is therefore done **BEFORE** the addition
//! that makes it necessary, in its own commit and without any other
//! change: that is the repository's doctrine, and it has five precedents (D9
//! task 6; D10 tasks 1 to 3; P3 tasks 3 and 4; G1 task 1). **Never a
//! compression** — D9 paid for it twice.
//!
//! ⚠️ **The `#[path]` declaring this module is OUTSIDE the convention of
//! `CLAUDE.md`** (§ "Child module convention"): that one only targets
//! modules extracted from a `#[cfg(windows)]` parent to compile on the host.
//! This is the **second** use of the same Rust mechanism — splitting a file
//! that is too long —, the one `distante/tests.rs` and `distante/tests_etats.rs`
//! already use in this same file, and it does not follow the prefix rule.
//!
//! ⚠️ **A trait `impl` in a child module is global: no call
//! site moved, and no re-export is needed.**

use anyhow::{bail, Result};

use super::{Rattachee, Recu, SourceDistante};
use crate::capteur::protocole::{DepuisCapteur, VersCapteur};
use crate::h264::AccessUnit;
use crate::source::VideoSource;
use std::sync::mpsc::TryRecvError;
use std::time::Instant;

impl VideoSource for SourceDistante {
    fn next_frame(&mut self) -> Option<AccessUnit> {
        loop {
            match self.images.try_recv() {
                Ok(Recu::Image(unite)) => {
                    self.fenetre.succes();
                    return Some(unite);
                }
                Ok(Recu::Etat {
                    vivante,
                    epuisee,
                    largeur,
                    hauteur,
                }) => {
                    self.vivante = vivante;
                    self.epuisee = epuisee;
                    self.size.poser(largeur, hauteur);
                }
                Ok(Recu::Sommeil { endormie, raison }) => {
                    // Two writes, two lifetimes: the current state, which
                    // survives its read, and the announcement, which does not.
                    self.endormie = endormie;
                    self.sommeil = Some((endormie, raison));
                }
                Ok(Recu::Part { bps }) => {
                    self.part = Some(bps);
                }
                Ok(Recu::Audio { actif }) => {
                    self.audio = Some(actif);
                }
                Ok(Recu::PleinEcran { actif }) => {
                    self.plein_ecran = Some(actif);
                }
                Ok(Recu::PressePapier { texte, octets }) => {
                    self.presse_papier = Some((texte, octets));
                }
                Ok(Recu::Accent { couleur }) => {
                    self.accent = Some(couleur);
                }
                // The COMMON and normal case: nothing new this round. The
                // transport loop polls at 100 Hz a source that
                // produces at ~90 fps.
                Err(TryRecvError::Empty) => {
                    self.fenetre.succes();
                    return None;
                }
                // The channel is broken. The resumption window bounds how
                // long the source stays alive while waiting for a re-attachment.
                Err(TryRecvError::Disconnected) => {
                    // **An AUTHORITATIVE exhaustion is not retried** (I2,
                    // final branch review of sub-block D4). At every
                    // NORMAL close of a window, the sensor pushes an
                    // `Etat { epuisee: true }` then closes the pipe: the child
                    // consumes that state, loops again, and sees `Disconnected` in
                    // the same round. Without this guard it would re-attach
                    // at once — a new command pipe, a new media
                    // pipe, and on the sensor side a new DXGI duplication and
                    // `H264Encoder` on an output the supervisor
                    // is precisely destroying. Worse, a
                    // re-attachment that succeeded would reset `epuisee` to false
                    // (below): the session that was meant to close would not
                    // close, and the outcome would depend on a race.
                    if self.epuisee {
                        return None;
                    }
                    let maintenant = Instant::now();
                    if self.fenetre.rupture(maintenant) {
                        // The window has expired: exhaustion is settled.
                        self.epuisee = true;
                        return None;
                    }
                    if self.fenetre.can_retry(maintenant) {
                        match self.canal.rattacher() {
                            Ok(Rattachee {
                                images,
                                largeur,
                                hauteur,
                            }) => {
                                tracing::info!(largeur, hauteur, "channel attached to the sensor");
                                self.images = images;
                                self.size.poser(largeur, hauteur);
                                self.vivante = true;
                                self.epuisee = false;
                                // A re-attachment goes through `VersCapteur::Attache`,
                                // hence through a NEW `Fenetre` on the sensor side — and
                                // a window is born asleep. Keeping here the state
                                // from before the break would make the floor
                                // share of this rebirth apply as an encoding
                                // ceiling, for the whole time between
                                // the attach and the first `Ordre::Reveiller`.
                                self.endormie = true;
                                // ⚠️ **`Some(false)`, NOT `None`: on
                                // re-attachment, the child BECOMES SILENT AGAIN**
                                // (design §4.4; F2, final branch
                                // review of sub-block D7). `None` does not
                                // mean "silent", it means "nothing to
                                // change": the `emet` flag of the
                                // `WindowsAudioSource` then kept its value
                                // from BEFORE the break, and a window that
                                // carried the sound went on carrying it.
                                //
                                // The case that bites: two windows A and B of the
                                // same PID, A the carrier, the sensor restarts.
                                // B re-attaches first, its group is
                                // empty on the sensor side — so it is elected and
                                // starts. A re-attaches a few tens to
                                // a few hundred milliseconds
                                // later (D4 measured 538 to 689 ms for the
                                // re-attachment alone, and nothing synchronises the two
                                // children) while STILL emitting: both
                                // play the same mix of the same PID, out of sync
                                // — an audible echo — until the
                                // `Audio { actif: false }` meant for A arrives.
                                //
                                // The reverse defect does not exist: a
                                // re-attachment goes through `VersCapteur::Attache`,
                                // hence through a NEW `Fenetre` on the sensor side,
                                // whose `sommeil::inscrire` purges
                                // `derniers_audio` — a new order therefore
                                // ALWAYS arrives, and without the wheel-round bound.
                                // `inscrire` calls `distribuer_l_audio`
                                // SYNCHRONOUSLY, before `boucler` even
                                // starts its loop (`fenetre.rs`, `servir`),
                                // which polls `ordres` at the head of every
                                // round, without delay. It is NOT the residue of
                                // `sommeil/porteurs.rs` (an order deferred to the
                                // NEXT WHEEL ROUND, bounded by
                                // `PERIODE_REARBITRAGE` = 250 ms): that
                                // residue only plays when the channel of a
                                // NEIGHBOURING window breaks during the SAME arbitration
                                // pass. The silence of a carrier that
                                // re-attaches is therefore bounded by the delivery of the
                                // message on the wire, and that is the trade-off
                                // the design chose: a brief gap rather
                                // than an echo.
                                self.audio = Some(false);
                                // Consumed by `rattachement_survenu`, to
                                // reset `Session::audio_mort_signale`
                                // : an `AudioMort` already reported before the
                                // break is not guaranteed to be known by the sensor on
                                // the other side of THIS re-attachment (the targeted case
                                // is the restarted sensor, whose registry
                                // of unfitness starts empty in memory again).
                                self.rattache = true;
                                self.fenetre.succes();
                            }
                            // Logged at `debug!` and not `info!`: at a step
                            // of 250 ms over a 15 s window, a sensor
                            // absent for long would produce 60 lines per
                            // window and per session.
                            // `cause::chain`: same reason as elsewhere on
                            // this path — `anyhow`'s plain `Display` only
                            // renders the outer layer. See `crate::cause`.
                            Err(error) => tracing::debug!(
                                error = %crate::cause::chain(&error),
                                "reattachment refused"
                            ),
                        }
                    }
                    return None;
                }
            }
        }
    }

    fn dimensions(&self) -> (u32, u32) {
        self.size.lire()
    }

    fn is_exhausted(&self) -> bool {
        self.epuisee
    }

    fn is_alive(&self) -> bool {
        self.vivante
    }

    fn resize(&mut self, width: u32, height: u32) -> Result<()> {
        match self.canal.commander(VersCapteur::Redimensionner {
            largeur: width,
            hauteur: height,
        })? {
            // The size KEPT is the one obtained, never the one requested: the
            // driver quantises, and a Windows window imposes even
            // dimensions. Same rule as in single-window mode.
            DepuisCapteur::Size { largeur, hauteur } => {
                self.size.poser(largeur, hauteur);
                Ok(())
            }
            DepuisCapteur::Error { motif } => bail!("the sensor refused: {motif}"),
            autre => bail!("unexpected answer from the sensor: {autre:?}"),
        }
    }

    fn set_bitrate(&mut self, bitrate: u32) -> Result<()> {
        self.commander_simple(VersCapteur::Debit { bps: bitrate })
    }

    fn set_encode_size(&mut self, width: u32, height: u32) -> Result<()> {
        self.commander_simple(VersCapteur::EncodeSize {
            largeur: width,
            hauteur: height,
        })
    }

    fn request_keyframe(&mut self) -> Result<()> {
        self.commander_simple(VersCapteur::ImageCle)
    }

    fn set_awake(&mut self, visible: bool, focalisee: bool) -> Result<()> {
        self.commander_simple(VersCapteur::Visibilite { visible, focalisee })
    }

    /// Has the VM's clipboard written by the SENSOR, its sole
    /// owner (D1), and **waits for its reply**.
    ///
    /// 🔴 **Synchronous on purpose, and that is D6's whole ordering**:
    /// `commander_simple` blocks until the sensor's `Fait`, so that
    /// the caller (`transport/tick.rs`, `a1octies` branch) can arm
    /// the `Ctrl+V` injection only after a write has ACTUALLY happened.
    /// No channel scheduling enters into it.
    ///
    /// ⚠️ **The cost is real and it is named**: this call blocks the
    /// transport loop for the time of a pipe round trip. The precedent exists and it
    /// is exercised — `set_awake` just above commands the sensor from
    /// this same loop, and D4 measured an attach at 34 µs. But the
    /// channel's bound is **12 s**, and a dead sensor would make the loop wait
    /// that long. **That path has never run** (the 12 s bound has been
    /// declared "code never run" since D4): it is a hand-over from P2, not a
    /// remedy.
    ///
    /// `commander_simple` — and not bare `commander` — because it alone translates
    /// `DepuisCapteur::Error` into `Err`: a refusal to open the clipboard
    /// by another application (a NORMAL case under Windows) must prevent
    /// the injection, not let it through.
    fn write_clipboard(&mut self, texte: &str) -> Result<()> {
        self.commander_simple(VersCapteur::ClipboardWrite {
            texte: texte.to_owned(),
        })
    }

    /// Returns the pending sleep change, and consumes it.
    ///
    /// **An announcement is not repeated**: the transport loop polls it at
    /// every round, and re-emitting the same message would flood the control
    /// channel.
    fn sommeil_a_annoncer(&mut self) -> Option<(bool, String)> {
        self.sommeil.take()
    }

    /// Returns the pending share, and consumes it.
    fn part_a_appliquer(&mut self) -> Option<u32> {
        self.part.take()
    }

    /// Returns the pending audio order, and consumes it.
    fn audio_a_appliquer(&mut self) -> Option<bool> {
        self.audio.take()
    }

    /// Returns the pending fullscreen change, and consumes it.
    fn plein_ecran_a_annoncer(&mut self) -> Option<bool> {
        self.plein_ecran.take()
    }

    /// Returns the clipboard pending announcement, and consumes it.
    fn presse_papier_a_annoncer(&mut self) -> Option<(Option<String>, u32)> {
        self.presse_papier.take()
    }

    /// Returns the accent colour pending announcement, and consumes it.
    fn accent_a_annoncer(&mut self) -> Option<String> {
        self.accent.take()
    }

    /// Returns the current sleep state, without consuming it.
    fn est_endormie(&self) -> bool {
        self.endormie
    }

    fn signaler_audio_mort(&mut self) {
        if let Err(error) = self.commander_simple(VersCapteur::AudioMort) {
            tracing::warn!(%error, "dead audio capture signal not delivered");
        }
    }

    /// See the trait: tells the sensor that a real packet has proven the
    /// audio capture resumed — the PROOF, not merely the rebuild
    /// decision (sub-block D10).
    fn signaler_audio_vivant(&mut self) {
        if let Err(error) = self.commander_simple(VersCapteur::AudioVivant) {
            tracing::warn!(%error, "live audio capture signal not delivered");
        }
    }

    fn rattachement_survenu(&mut self) -> bool {
        std::mem::take(&mut self.rattache)
    }
}
