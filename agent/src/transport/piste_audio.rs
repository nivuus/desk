//! The audio track: negotiation of the Opus payload type, and writing
//! packets to str0m. Audio comes BEFORE video in `tick`'s priority
//! list — a sound dropout is heard, an image 10 ms
//! late is not seen.

use std::time::Duration;

use str0m::format::Codec;
use str0m::media::{Frequency, MediaTime, Mid, Pt};

use super::tick::Tick;
use super::Session;
use crate::audio::{AudioPacket, AudioSource};

/// Rebuild fault injection (bench variable
/// `AUDIO_FAUTE_RECONSTRUCTION`), extracted here because adding it took
/// `piste_audio.rs` to 513 lines — above the repository's 500 ceiling. The
/// rule has no exception: **extraction, never compression**.
///
/// No `#[cfg(windows)]` boundary here, so `docs/claude/module-conventions.md`'s `#[path]` convention
/// does not apply: an ordinary `mod` is enough.
pub(in crate::transport) mod injection;

/// Wait ceiling when an audio track is negotiated.
///
/// Audio packets arrive from an independent capture thread: this loop
/// has no way to predict their arrival instant, it can only
/// wake up often enough not to let them age. 2 ms for a
/// 10 ms frame cadence — a fifth of a frame of delay at worst.
pub(super) const AUDIO_POLL_INTERVAL: Duration = Duration::from_millis(2);

impl Session {
    /// Provides the audio source. Without a call, the session stays silent and
    /// video works normally.
    pub fn set_audio_source(&mut self, source: Box<dyn AudioSource + Send>) {
        self.audio_source = Some(source);
    }

    /// Wait ceiling of branch `c`: only when a source AND
    /// an audio track exist, otherwise nothing justifies waking up more
    /// often.
    pub(super) fn audio_wait_cap(&self) -> Option<Duration> {
        (self.audio_source.is_some() && self.audio_mid.is_some() && !self.ending)
            .then_some(AUDIO_POLL_INTERVAL)
    }

    /// Branch `a3` of the priority list (see `tick`): emits an audio
    /// packet if the track is negotiated and a packet is waiting.
    ///
    /// No deadline to watch here: the capture thread deposits into a
    /// buffer, it is enough to look whether there is something. The regular
    /// wake-up comes from `AUDIO_POLL_INTERVAL`, applied in branch `c`.
    ///
    /// Returns `Some(Tick::Continue)` when it has concluded the round — a written
    /// packet is a mutation of `Rtc`, which must be followed by the deferred
    /// drain of branch `a0`. Returns `None` when there was nothing to
    /// emit, and has then mutated nothing: the priority list can move on to
    /// the next branch without breaking the drain invariant.
    pub(super) fn brancher_audio(&mut self) -> Option<Tick> {
        let (Some(mid), false) = (self.audio_mid, self.ending) else {
            return None;
        };
        let paquet = self
            .audio_source
            .as_mut()
            .and_then(|source| source.next_packet())?;
        // D9's legacy 6: resetting the re-arming counter happens
        // on a PROOF of sound — this very packet —, never on the arbitration
        // decision which, for its part, cannot bite in the majority case
        // (`sommeil/porteurs.rs`, a window alone in its PID group
        // automatically becomes carrier again when leaving respite).
        if self.audio_reconstruit_sans_preuve {
            self.audio_reconstruit_sans_preuve = false;
            self.audio_vivant_a_annoncer = true;
        }
        if self.write_audio(mid, paquet) {
            self.audio_write_pending_drain = true;
        }
        Some(Tick::Continue)
    }

    /// Selects the Opus payload type negotiated for `mid`.
    ///
    /// A call separate from `write_audio` so that the borrow on `self` through
    /// `Rtc::writer` ends before any later `&mut self` call — same
    /// reason as `select_negotiated_h264_pt`.
    fn select_negotiated_opus_pt(&mut self, mid: Mid) -> Option<Pt> {
        let writer = self.rtc.writer(mid)?;
        // Bound to a variable rather than returned directly: the anonymous type
        // returned by `payload_params()` (capturing the lifetime of
        // `writer`, see its signature) would otherwise remain a temporary living
        // until the end of the block, after `writer` is dropped — rejected
        // by the borrow checker ("`writer` does not live long enough") whereas
        // the final value (`Option<Pt>`, `Copy`) no longer borrows anything.
        let pt = writer
            .payload_params()
            .find(|p| p.spec().codec == Codec::Opus)
            .map(|p| p.pt());
        pt
    }

    /// Writes an Opus packet on the audio track.
    ///
    /// Returns `true` if `writer.write()` actually pushed the packet — hence
    /// that a deferred drain is needed.
    ///
    /// Unlike `write_frame`, a write failure does **not** close the
    /// session: an audio defect must never kill a video session that
    /// works.
    pub(super) fn write_audio(&mut self, mid: Mid, packet: AudioPacket) -> bool {
        let Some(pt) = self.select_negotiated_opus_pt(mid) else {
            self.warn_audio_negotiation_once();
            return false;
        };
        let Some(writer) = self.rtc.writer(mid) else {
            self.warn_audio_negotiation_once();
            return false;
        };

        // `captured_at` is the real instant matching `pts_48k`: it is
        // what goes into the RTCP Sender Reports and carries A/V sync.
        let rtp_time = MediaTime::new(packet.pts_48k, Frequency::FORTY_EIGHT_KHZ);
        match writer.write(pt, packet.captured_at, rtp_time, packet.data) {
            Ok(()) => true,
            Err(e) => {
                tracing::warn!(error = %e, "audio write failure, packet dropped");
                false
            }
        }
    }

    /// Applies the sensor's audio arbitration: carry the sound, or stay silent.
    ///
    /// **Two effects, and the second is easy to forget**: the source stops
    /// emitting, AND the audio budget retained by the congestion controller
    /// drops to zero. Without the second, a silent window would keep cutting
    /// 128 kb/s off its video budget for a track that emits nothing — that is the
    /// pre-existing defect D7 fixes (spec §5).
    ///
    /// ⚠️ **The budget depends on the EXISTENCE of a source, not on the order
    /// alone** (F4, final branch review of sub-block D7). `actif` alone
    /// missed the two paths where the session has no audio source while
    /// the sensor elects it carrier: failure to open the *process loopback*,
    /// which spec §6 explicitly makes a silent fallback, and `AUDIO=0` —
    /// where, at one window per PID, **all** windows are carriers and the
    /// pre-existing defect came back intact.
    pub(super) fn appliquer_audio(&mut self, actif: bool) {
        // Fix made during the review of task 12 (sub-block D10): the
        // rebuild budget (`reconstructions_restantes`) was only set
        // ONCE, at the construction of the `Session`, and never
        // replenished — the dead → rebuilt → proven cycle could
        // therefore only turn once per session (see the doc of the field
        // `audio_porteuse`, `transport.rs`).
        //
        // A RE-ELECTION — a TRANSITION to `actif: true` — is
        // literally the sensor saying "retry": it is the only
        // replenishment point retained. **A transition, not the mere
        // presence of an `actif: true` order**: the sensor already only re-emits
        // on change (`sommeil::porteurs::distribuer_l_audio`), but relying
        // on that alone would shift this guarantee onto a remote module, over
        // which this file has no hold; `audio_porteuse` makes it local
        // and checkable here, without depending on that remote discipline. Without
        // this restriction to the transition alone, a stream of identical `actif:
        // true` orders would make the budget infinite.
        if actif && !self.audio_porteuse {
            self.reconstructions_restantes = crate::audio::RECONSTRUCTIONS_MAX;
            // ⚠️ **Residue found in re-review (sub-block D10), documented and not
            // fixed: this reset to `None` CANCELS the
            // `REPIT_RECONSTRUCTION` spacing at each re-election.** For a session
            // already latched dead, each `false → true` transition therefore
            // buys an IMMEDIATE rebuild attempt at the next round
            // — that is, a blocking *process loopback* opening on
            // the `Session::run` thread, exactly the cost the respite
            // exists to space out. `REARMEMENTS_MAX` does not bound it: it only
            // counts cycles that reach `AudioMort`, never the
            // re-elections themselves — a group toggling between two
            // windows of the same PID can therefore re-elect faster than a budget
            // runs out. **It is not a regression**: the rate stays
            // bounded by `PERIODE_REARBITRAGE` (250 ms, `capteur/sommeil.rs`),
            // which bounds how often the registry can toggle
            // `actif`. But it is a NEW coupling between the
            // back-and-forth of audio arbitration and blocking work on the
            // drain thread, which nothing prevented before this reset
            // existed.
            self.prochaine_reconstruction = None;
            // Lifts the latch that would otherwise prevent `act_on_timeout`
            // (branch a1sexies) from calling `reconstruire_ou_signaler` again:
            // without this line, replenishing the budget above
            // would have no effect, since the entry door would stay closed.
            self.audio_mort_signale = false;
        }
        self.audio_porteuse = actif;

        let mut capture_morte = false;
        if let Some(source) = self.audio_source.as_mut() {
            source.set_actif(actif);
            capture_morte = source.capture_morte();
        }
        self.congestion
            .changer_audio_bps(if actif && self.audio_source.is_some() {
                crate::opus::BITRATE_BPS as u32
            } else {
                0
            });
        // `session`: without this field the trace is NOT attributable — all
        // children inherit the same `agent.log` since D4. Same reason and same
        // field as "budget share applied".
        //
        // `capture_morte`: without it this line WOULD LIE (F3). A capture
        // thread that has given up for good lets `set_actif` succeed —
        // it only writes an atomic no one reads any more —, and the trace
        // then announced `actif=true` for a window that will never
        // produce a packet again. ~~It is the only place in the product where this state
        // becomes observable; the sensor, for its part, does not see it.~~
        //
        // ❌ **The two struck-out clauses have been wrong since sub-block
        // D10** (cross-cutting review). `capture_morte` is reread at EVERY round by
        // `capture_audio_morte` → `reconstruire_ou_signaler` (branch
        // a1sexies), which logs "audio capture rebuilt" or
        // "audio capture rebuild refused" and pushes `AudioMort`
        // as a fallback — the sensor therefore sees it, records it in its `inaptes` and
        // logs it in turn. This trace is no longer either the only
        // observatory or the only path; it stays useful for what it
        // is, a state read at the point where the order is applied. The reference to
        // the "abandonment comment in `windows_audio.rs`" has moreover followed
        // task 3's extraction: it lives in `windows_audio/fil.rs`.
        tracing::info!(
            session = %self.session_id,
            actif,
            capture_morte,
            "ordre audio applique"
        );
    }

    /// Declares that this session carries the sound **without any sensor
    /// telling it so**. Reserved for SINGLE-WINDOW mode.
    ///
    /// ⚠️ **NEVER call from a session served by a sensor.** The
    /// sensor arbitrates who carries the sound between the windows of the same PID
    /// group, and `appliquer_audio` is the only legitimate path in that mode.
    /// Setting `true` here on an arbitrated session would make a window speak that
    /// must stay silent — two windows would then play the same mix
    /// out of sync, the audible echo that defect F2 of sub-block D7 describes.
    /// `a_rebuilt_non_carrier_session_stays_silent` keeps it red.
    ///
    /// Its only production caller is `demarrage/audio.rs::brancher`,
    /// in its ONLY branch `config.fenetre_hwnd == None`.
    pub fn set_audio_porteuse(&mut self, porteuse: bool) {
        self.audio_porteuse = porteuse;
    }

    /// Hands over what is needed to rebuild the audio source after its capture died.
    pub fn set_audio_reconstructeur(&mut self, r: crate::audio::Reconstructeur) {
        self.audio_reconstructeur = Some(r);
    }

    /// Returns `true` if `AudioMort` must be reported to the sensor — that is,
    /// when there is nothing left to rebuild.
    ///
    /// **Rebuilding comes BEFORE reporting**, and it is the inversion
    /// D10 brings: the signal to the sensor stops being the first gesture
    /// and becomes the fallback. Promoting a neighbour (the only half of
    /// D9 that worked) then keeps its exact role — that of the case where
    /// the process tree has really disappeared.
    ///
    /// ⚠️ **A successful rebuild ALSO RE-ARMS the source, on
    /// `audio_porteuse`** (defect found during VM acceptance, fixed in the body
    /// below): `WindowsAudioSource::pour_processus` is always born
    /// SILENT, and without this re-arming a carrier session whose capture
    /// has just been rebuilt would never produce any packet again, hence
    /// no PROOF, hence no re-election: an ABSORBING state.
    ///
    /// ❌ **"The only path a rebuilder takes" was written here,
    /// and it is WRONG — found by the cross-cutting end-of-branch review, and
    /// it is the heaviest defect it found, because it has a
    /// behavioural consequence.** `demarrage/audio.rs::brancher` sets a
    /// rebuilder in BOTH modes: its `None` branch
    /// (`config.fenetre_hwnd` absent — the SINGLE-WINDOW path) calls
    /// `WindowsAudioSource::new`, which enables its own emission.
    ///
    /// ✅ **The consequence D10 left as legacy here — "in single-window mode, the rebuild
    /// remedy is INERT" — is FIXED (sub-block D11, legacy 4).**
    /// It came from `audio_porteuse` being born `false` (`transport.rs`)
    /// with `appliquer_audio` as sole writer, that is, an `Audio` order
    /// from the sensor that a single-window agent never receives: a
    /// rebuilt capture there had its emission enabled at `true` by `new()`, then
    /// **reset to `false`** by the re-arming line below.
    /// `demarrage/audio.rs::brancher` now calls `set_audio_porteuse`
    /// in its single-window branch only, and the field therefore has a second
    /// writer — outside this module.
    ///
    /// ⚠️ **The remedy does NOT force `true` without arbitration**, and it is the only
    /// safe form: forcing unconditionally here would reintroduce the
    /// WORSE defect that going through `audio_porteuse` avoids in multi-window (a
    /// sound leak to a window that must stay silent), and
    /// `a_rebuilt_non_carrier_session_stays_silent` keeps it red.
    /// The fix distinguishes the two modes **at wiring time**, where the mode
    /// is known, never here where it is not.
    ///
    /// ⚠️ **This method runs on the `Session::run` thread**, and opening a
    /// WASAPI source there is a blocking call of unbounded duration. Hence the
    /// respite: at most one attempt per `REPIT_RECONSTRUCTION`. If measurement
    /// shows it delays draining, it will move to a thread — same
    /// risk `Drop for H264Encoder` already carries on this thread.
    pub(super) fn reconstruire_ou_signaler(&mut self, maintenant: std::time::Instant) -> bool {
        if !self.capture_audio_morte() {
            return false;
        }
        let Some(reconstructeur) = self.audio_reconstructeur.as_ref() else {
            return true;
        };
        if self.reconstructions_restantes == 0 {
            return true;
        }
        if self
            .prochaine_reconstruction
            .is_some_and(|t| maintenant < t)
        {
            return false;
        }
        self.reconstructions_restantes -= 1;
        self.prochaine_reconstruction = Some(maintenant + crate::audio::REPIT_RECONSTRUCTION);
        // The bench fault injection is interposed in front of the
        // rebuilder — see `injection`, which carries the budget, its reason
        // for being process-global, and the contract of this call site.
        //
        // ⚠️ The `Err` arm that follows is LEGACY 6's (`{error:#}`): the
        // injected fault goes through the same `warn!`, and the acceptance log
        // therefore carries the injected-fault error naming `AUDIO_FAUTE_RECONSTRUCTION`.
        // It is the REACHABILITY check of this legacy — if that string
        // does not appear while injection is armed, it is the format that
        // does not work, not the cause that is missing.
        let tentative = injection::intercepter(reconstructeur);
        match tentative {
            Ok(mut source) => {
                tracing::info!(
                    restantes = self.reconstructions_restantes,
                    "capture audio reconstruite"
                );
                // Found during VM acceptance (two runs: "audio capture
                // rebuilt" = 2, `compteurs_audio_actif_true` = 0 in
                // both): a source rebuilt by
                // `WindowsAudioSource::pour_processus` IS BORN SILENT
                // (`windows_audio.rs::start`) — unlike the
                // single-window `new()`, which enables its own emission. Without this
                // line, NOTHING re-arms the rebuilt source: it
                // produces no packet, hence no PROOF
                // (`audio_vivant_a_annoncer`), hence no re-election —
                // silent forever. An ABSORBING state, not a delay.
                //
                // `audio_porteuse` — never the `actif` order of the last call
                // to `appliquer_audio`, captured BEFORE this function could
                // have changed it — is the LOCAL mirror of the last order received
                // from the sensor (see `appliquer_audio`): the guarantee thus depends
                // on no remote discipline. Applied WITHOUT
                // condition, for a carrier session (`true`,
                // which re-arms) as well as for a silent session (`false`, which
                // explicitly confirms the silence rather than
                // assuming it) — see
                // `a_rebuilt_non_carrier_session_stays_silent`.
                source.set_actif(self.audio_porteuse);
                self.audio_source = Some(source);
                self.audio_reconstruit_sans_preuve = true;
                false
            }
            Err(error) => {
                // `{error:#}` and not `%error`: `anyhow`'s plain `Display`
                // only renders the OUTERMOST context, and
                // `windows_audio.rs` sets precisely one
                // (the process loopback opening for the PID …). The HRESULT —
                // the only datum that answers D10's legacy 6, "the cause of the
                // rebuild refusal is not identified" — therefore stayed
                // in the causes, thrown away at write time. The evidence is
                // filed: `journaux-multifenetres-d10/agent-critere-2-1-plat.log`
                // l. 97 carries only that outer context (process loopback opening for PID
                // 27544) and nothing else. Same doctrine as
                // `diagnostics/multifenetre/plafond/sonde.rs`, which explains it
                // word for word about an HRESULT lost the same way.
                tracing::warn!(
                    error = format!("{error:#}"),
                    restantes = self.reconstructions_restantes,
                    "audio capture rebuild refused"
                );
                false
            }
        }
    }

    /// True if this window's audio capture has given up for good
    /// (`AudioSource::capture_morte`, set after
    /// `crate::audio::LECTURES_ECHOUEES_MAX` consecutive WASAPI read
    /// errors, `windows_audio.rs`).
    ///
    /// Without a source (no audio track for this session, or `AUDIO=0`),
    /// never dead: there is nothing to report.
    pub(super) fn capture_audio_morte(&self) -> bool {
        self.audio_source
            .as_deref()
            .is_some_and(|source| source.capture_morte())
    }

    fn warn_audio_negotiation_once(&mut self) {
        if !self.warned_audio_negotiation {
            self.warned_audio_negotiation = true;
            tracing::warn!(
                "no Opus payload type negotiated: audio packets dropped (single warning)"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- horloge RTP audio --------------------------------------------------
    //
    // `write_audio` builds `MediaTime::new(pts_48k, Frequency::FORTY_EIGHT_KHZ)`:
    // the RTP clock rate of the Opus payload type is hardcoded
    // here, separately from `opus::SAMPLE_RATE_HZ`, which nevertheless explicitly
    // documents being "the RTP clock rate of the Opus payload
    // type". Nothing ties these two constants: changing one without
    // the other would compile without warning and produce RTP timestamps
    // wrong by a constant factor — a silent synchronisation
    // defect. This test fails if they diverge.
    #[test]
    fn the_audio_rtp_frequency_matches_the_opus_sample_rate() {
        assert_eq!(
            Frequency::FORTY_EIGHT_KHZ.get(),
            crate::opus::SAMPLE_RATE_HZ
        );
    }

    #[test]
    fn bounds_the_wait_when_audio_is_negotiated() {
        // Without this ceiling, the waiting branch would sleep until the deadline
        // `Rtc` asks for — up to a whole second — and would thus
        // go through a hundred or so due audio packets. It is the same defect as
        // C1 on the video side, transposed.
        use std::time::Instant;

        use crate::transport::socket::bounded_wait;

        let maintenant = Instant::now();
        let echeance_rtc = maintenant + Duration::from_secs(1);

        let sans_audio = bounded_wait(maintenant, echeance_rtc, None, None);
        assert_eq!(sans_audio, Duration::from_secs(1));

        let with_audio = bounded_wait(maintenant, echeance_rtc, None, Some(AUDIO_POLL_INTERVAL));
        assert_eq!(with_audio, AUDIO_POLL_INTERVAL);

        // The ceiling must never LENGTHEN an already shorter wait.
        let echeance_proche = maintenant + Duration::from_micros(200);
        let court = bounded_wait(maintenant, echeance_proche, None, Some(AUDIO_POLL_INTERVAL));
        assert_eq!(court, Duration::from_micros(200));
    }

    /// Exercises the FORMAT, not the call site: `{error:#}` renders the chain of
    /// causes where `{error}` only renders the outermost context. The site
    /// itself is not observable on the host (it is a `tracing`
    /// `warn!`); its proof is the acceptance log, not this test.
    #[test]
    fn the_alternate_format_renders_the_cause_chain() {
        use anyhow::Context;

        let cause = anyhow::anyhow!("0x88890004");
        let e = Err::<(), _>(cause)
            .context("opening the process loopback of PID 42")
            .unwrap_err();
        assert!(
            !format!("{e}").contains("0x88890004"),
            "the plain Display loses the cause"
        );
        assert!(
            format!("{e:#}").contains("0x88890004"),
            "{{:#}} must return it"
        );
    }
}
