//! `WindowsSource::set_encode_size`: change the encoded resolution without
//! touching either the window or the capture.
//!
//! **CHILD module of `windows_source`**, like `redimensionnement` and for the
//! same reason: that is what gives it access to the private fields of
//! `WindowsSource` without any having to be opened as `pub(crate)` (see the
//! fields' comment in `windows_source.rs`). Extracted from that file
//! in sub-block D5, because it is in frozen size debt (`CLAUDE.md`) and
//! the remedy for defect C2 added some fifteen lines to it: the addition
//! comes with its extraction, as the rule requires.
//!
//! Nothing changed in the move apart from the remedy itself, described at the head of
//! the function.

use anyhow::{Context, Result};

use super::WindowsSource;
use crate::encode::H264Encoder;

impl WindowsSource {
    /// Rebuilds the encoder at a new output size, **without touching
    /// the capture or the window**.
    ///
    /// Media Foundation does not allow changing resolution on the
    /// way: a new encoder is needed (same constraint as `resize`, see
    /// its comment). But unlike `resize`, the DXGI capture stays
    /// alive — it is the converter's input, it has not changed. No
    /// DXGI duplication constraint here, hence no need for
    /// `rebuild_or_recover`.
    ///
    /// The timestamp is not reset: `last_pts_90k` is kept, the
    /// browser's decoder would reject going backwards.
    pub fn set_encode_size(&mut self, width: u32, height: u32) -> Result<()> {
        // The source is permanently exhausted: `capture` may be `None`
        // for good (see the field's comment), and `capture_mut()`
        // would panic. A panic here would cross `spawn_blocking` and
        // take down the whole process — the transport, for its part, knows what to do
        // with an error: it keeps the current rung and continues the session
        // until its normal closing.
        if self.fatal {
            anyhow::bail!("source épuisée : taille d'encodage inchangée");
        }

        // Upper bound added in the final branch review (C1): the
        // justification that made it useless until now ("the caller never
        // produces a size larger than the source") was wrong —
        // see the comment of `resize`. A net, not THE fix: it is
        // `changer_source` on the controller side that normally avoids aiming at a
        // size that is too large, but a future caller (or a calibration bug
        // of the ladder) must not be able to ask Media Foundation for an
        // output larger than its input.
        let (width, height) = (width.max(2) & !1, height.max(2) & !1);
        let (width, height) = (width.min(self.width), height.min(self.height));
        // Short-circuit BEFORE any destruction, and it is not cosmetic:
        // it is what avoids destroying then rebuilding an encoder for
        // a size it already serves — notably at the rung climb that
        // follows a wake-up, where the controller aims at the same value again.
        if (width, height) == self.encoder_mut()?.encode_size() {
            return Ok(());
        }

        let device = self.capture_mut().device().clone();

        // **Destroy BEFORE building**, and it is the remedy for the only defect
        // sub-block D4 had left open. Rebuilding while keeping
        // the old one alive required one more encoder for the duration of the
        // switch: at `vivier::PLAFOND_EVEIL` (8) live encoders, that
        // transient is refused at the NVIDIA MFT's `SetOutputType`
        // (`MF_E_UNSUPPORTED_D3D_TYPE`, `0xC00D6D76`) — **18 refusals out of 18**
        // noted in D4's second acceptance run at eight windows, against 3 successes
        // out of 3 at two windows. Adaptation through resolution was therefore
        // dead at the maximal rank, leaving only bitrate to respond to
        // congestion. That destroying really frees the slot is measured:
        // 10 successful recyclings out of 10, over four independent runs
        // (pivot measurement of sub-block D5).
        //
        // ⚠️ **The price is real and accepted**: if building the new one
        // fails, the old one is no longer there. The old behaviour then kept
        // the current rung and the session kept broadcasting; this one
        // makes it lose its video. Hence the `self.fatal` below, which makes
        // the session close through its normal path rather than leaving a
        // silent source — and above all, never a panic: it would cross
        // `spawn_blocking` and take down the whole process, hence all the
        // other windows with it.
        drop(self.encoder.take());
        let neuf = H264Encoder::new(
            &device,
            (self.width, self.height),
            (width, height),
            self.fps,
            self.bitrate,
        );
        let mut encoder = match neuf {
            Ok(encoder) => encoder,
            Err(erreur) => {
                self.fatal = true;
                return Err(erreur).context(
                    "encodeur neuf refusé après destruction de l'ancien : source épuisée",
                );
            }
        };
        // A new encoder must start with a keyframe: without it, the
        // browser's decoder has no entry point into the new
        // stream and renders a grey screen until the next one. A failure here leaves
        // `self.encoder` at `None` if not caught: same treatment
        // as above, and for the same reason.
        if let Err(erreur) = encoder.request_keyframe() {
            self.fatal = true;
            return Err(erreur).context("image clé refusée par l'encodeur neuf : source épuisée");
        }

        self.encoder = Some(encoder);
        // The new encoder has produced nothing: the startup polling budget
        // must start again, as after `resize`.
        self.encoder_warmed_up = false;
        tracing::info!(
            width,
            height,
            "taille d'encodage changée sans toucher à la fenêtre"
        );
        Ok(())
    }
}
