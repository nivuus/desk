//! Audio track types and the transfer buffer between threads.
//!
//! This module never references the `windows` crate: it compiles and is tested
//! on Linux.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// A timestamped Opus packet, ready to be written to the audio track.
#[derive(Debug, Clone)]
pub struct AudioPacket {
    /// Encoded Opus frame.
    pub data: Vec<u8>,
    /// Presentation timestamp, in units of 1/48000 s — that is, a
    /// count of samples since the session's clock origin.
    ///
    /// Unlike video, this value is not a clock reading
    /// but an exact count: audio cannot drift by itself.
    pub pts_48k: u64,
    /// Real instant at which these samples were captured.
    ///
    /// It is this value that goes into `Writer::write` as `wallclock`, and
    /// hence into the RTCP Sender Reports: it carries all the A/V sync.
    pub captured_at: Instant,
}

/// Producteur de paquets Opus, pendant audio de `VideoSource`.
pub trait AudioSource {
    /// Next packet, or `None` if none is ready this round.
    ///
    /// `None` is the common case: the transport loop runs much
    /// faster than the 100 packets per second the capture produces.
    fn next_packet(&mut self) -> Option<AudioPacket>;

    /// Declares the observed loss rate, so that Opus in-band FEC
    /// actually produces redundancy. No effect by default.
    fn set_packet_loss_perc(&mut self, _perc: i32) -> anyhow::Result<()> {
        Ok(())
    }

    /// Carries the sound, or goes silent, on the sensor's arbitration order.
    ///
    /// No effect by default: a source that is not arbitrated always
    /// emits.
    fn set_actif(&mut self, _actif: bool) {}

    /// True when the capture has definitively stopped and no order will
    /// make it start again.
    ///
    /// **Exists so that the traces stop lying.** `set_actif` always
    /// succeeds — it only writes an atomic —, and without this witness
    /// `appliquer_audio` would log `actif=true` for a window that will
    /// never produce a packet again. False by default: a source that has
    /// no capture thread has nothing that can die.
    fn capture_morte(&self) -> bool {
        false
    }
}

/// What is needed to rebuild an audio source after its capture died.
///
/// **The remedy for D9's hand-over 1.** When `capture.read()` fails more than
/// `LECTURES_ECHOUEES_MAX` times in a row, the `windows_audio.rs` thread sets
/// `capture_morte` and executes a DEFINITIVE `return`. Nothing, until D10,
/// rebuilt the source: a window alone in its PID group — the
/// MAJORITY case, one application one window — lost its sound for the rest
/// of the session, and the sensor's "re-election after a respite" only
/// wrote a boolean this dead thread never re-read.
///
/// A closure rather than a trait: `transport/` must know nothing about
/// Windows, and it is `demarrage/audio.rs` — sole holder of `Config` and of the
/// `clock_origin` — that knows how to redo the right mode choice.
pub type Reconstructeur = Box<dyn Fn() -> anyhow::Result<Box<dyn AudioSource + Send>> + Send>;

/// Number of rebuilds attempted before giving up and reporting.
///
/// ⚠️ **NOT CALIBRATED** — it joins `BPP_MIN`, `FACTEUR_FOCUS`,
/// `PART_DORMANTE_BPS`, `HYSTERESIS`, `REPIT_APRES_ECHEC`, `MAX_OUTPUT_SIZE`,
/// `REPIT_REARMEMENT_AUDIO` and `REARMEMENTS_MAX` in the list of constants
/// no listening judgement has assessed.
pub const RECONSTRUCTIONS_MAX: u32 = 3;

/// Delay between two rebuild attempts.
///
/// ⚠️ **Do NOT reuse `temporisation_de_reprise`**: it paces the
/// re-reads INSIDE the capture thread, not the source
/// rebuilds. Two durations with different meanings that would silently diverge the day
/// one of them changed.
///
/// ⚠️ **NOT CALIBRATED** either.
pub const REPIT_RECONSTRUCTION: std::time::Duration = std::time::Duration::from_secs(2);

/// Number of consecutive read errors the capture thread tolerates
/// before giving up for good.
///
/// **An isolated error must not doom a whole PID group.** The capture
/// thread is the only sound producer of its window, and ~~its death is
/// final: the sensor keeps treating this session as the sound carrier of
/// its group, so its neighbour stays silent and is never promoted~~. Yet the
/// known causes of a WASAPI read refusal — device change,
/// audio service restart, format change — are **transient**.
/// We therefore retry, with the growing backoff below, and
/// give up only after `LECTURES_ECHOUEES_MAX` failures in a row.
///
/// ✅ **The struck-out clause above was refuted by sub-block D10, at
/// both ends at once** (found by the cross-cutting review; the following
/// paragraph, `Reconstructeur`, already correctly stated the opposite, without
/// this one being updated). The thread's death is no longer final:
/// `Session::reconstruire_ou_signaler` rebuilds the source, up to
/// `RECONSTRUCTIONS_MAX` times, the budget being replenished at each
/// re-election. And even in fallback, `AudioMort` marks the session unfit on the
/// sensor side, which does promote a neighbour of the same PID group.
/// **The underlying argument holds unchanged**: the known causes
/// of a read refusal are transient, and retrying costs less than
/// condemning.
pub const LECTURES_ECHOUEES_MAX: u32 = 10;

/// Bounds of the backoff applied between two read attempts.
///
/// The base equals the capture thread's polling interval: at the first failure,
/// retrying costs no more than a normal loop round. The cap
/// prevents a burst of errors returned *immediately* — the case that matters,
/// `read()` then not waiting for its delay — from spinning in a tight loop.
const REPRISE_LECTURE_BASE: std::time::Duration = std::time::Duration::from_millis(5);
const REPRISE_LECTURE_MAX: std::time::Duration = std::time::Duration::from_millis(200);

/// Backoff to observe after `consecutives` read errors in a row:
/// bounded exponential growth (5, 10, 20, ... up to
/// `REPRISE_LECTURE_MAX`).
///
/// Same shape as `transport::socket::recv_error_backoff`, its
/// precedent — except that this function lives in a module without
/// `cfg`, hence testable on the host, where its caller
/// (`windows_audio.rs`) is not.
///
/// `consecutives = 0` makes no sense (no error, hence no wait) and
/// returns the base, like `consecutives = 1`.
pub fn temporisation_de_reprise(consecutives: u32) -> std::time::Duration {
    // Bound the exponent BEFORE the shift: `1u32 << 32` would overflow
    // silently, and `saturating_mul` only operates on the `Duration`, not
    // on the integer operand passed to it.
    let exposant = consecutives.saturating_sub(1).min(31);
    REPRISE_LECTURE_BASE
        .saturating_mul(1u32 << exposant)
        .min(REPRISE_LECTURE_MAX)
}

/// Bounded circular buffer, shared between the capture thread and the transport
/// loop.
///
/// **Why not a `std::sync::mpsc::sync_channel`**: when full, its
/// `try_send` fails, hence rejects the **new** packet. On a real-time
/// track that is the wrong end — the fresh packet is the one with value,
/// the stale one has none left. Here, it is the **oldest** that goes.
///
/// Pushing never blocks: a blocking push would make the transport
/// loop the constraint of the capture thread, and a block on the transport side
/// would freeze the WASAPI capture, whose internal buffer would overflow in turn.
#[derive(Debug, Clone)]
pub struct PacketRing {
    file: Arc<Mutex<VecDeque<AudioPacket>>>,
    capacite: usize,
    rejetes: Arc<AtomicU64>,
}

impl PacketRing {
    pub fn new(capacite: usize) -> Self {
        Self {
            file: Arc::new(Mutex::new(VecDeque::with_capacity(capacite.max(1)))),
            capacite: capacite.max(1),
            rejetes: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Pushes a packet. Never blocks. When full, the oldest packet
    /// is dropped and the reject counter incremented.
    pub fn push(&self, packet: AudioPacket) {
        let mut file = self.verrou();
        while file.len() >= self.capacite {
            file.pop_front();
            self.rejetes.fetch_add(1, Ordering::Relaxed);
        }
        file.push_back(packet);
    }

    /// Removes the oldest packet, or `None` if the buffer is empty.
    pub fn pop(&self) -> Option<AudioPacket> {
        self.verrou().pop_front()
    }

    /// Cumulative number of packets dropped for lack of room.
    pub fn rejetes(&self) -> u64 {
        self.rejetes.load(Ordering::Relaxed)
    }

    /// Poison-tolerant lock.
    ///
    /// An `unwrap()` here would make the transport loop panic because another
    /// thread panicked elsewhere — a perfectly healthy video session
    /// would die from an audio incident, which the global constraints
    /// forbid. The queue remains usable in every case: at worst a
    /// packet is incomplete, and an incomplete audio packet breaks nothing.
    fn verrou(&self) -> std::sync::MutexGuard<'_, VecDeque<AudioPacket>> {
        self.file.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// Is the read fault injection still armed?
///
/// `fenetre = None`: unlimited, hence always armed — that is the behaviour
/// of sub-block D10, **strictly preserved** when `AUDIO_FAUTE_LECTURE_MS`
/// is absent.
///
/// **Why bound the arming in time.** Each window is its own
/// process and **inherits the environment** of the supervisor, which removes only
/// `SUPERVISEUR`, `TEST_FILE`, `WINDOW_TITLE` and `CAPTEUR`
/// (`superviseur/lanceur.rs`): each child therefore receives a **fresh**
/// `AUDIO_FAUTE_LECTURE` budget. Yet only the CARRIER window consumes it —
/// the `if !emettait { … continue; }` guard in `windows_audio/fil.rs` means
/// a silent source never calls `read()`. The neighbour therefore stays
/// intact as long as it is silent, **and dies in ≈ 50 ms** (`LECTURES_ECHOUEES_MAX`
/// = 10 × `POLL_INTERVAL` = 5 ms) as soon as it is promoted, on its budget still
/// full — that is **1/600th** of `REPORT_INTERVAL`. The criterion "the fallback onto
/// promotion" would then remain undemonstrable: the promoted neighbour would die
/// before being able to prove anything.
///
/// Bounding the arming closes that **without naming any session**: the carrier
/// consumes in the first milliseconds, the window closes, and the
/// neighbour — promoted at the earliest `RECONSTRUCTIONS_MAX × REPIT_RECONSTRUCTION`
/// = 6 s later, plus `PERIODE_REARBITRAGE` — reads for real. No
/// prior recognition, no extra Win32 call.
///
/// **Bound EXCLUDED**: at `depuis == fenetre`, disarmed. Explicit choice, and
/// `la_borne_de_la_fenetre_est_incluse_ou_exclue_mais_dite` pins it.
///
/// Pure and tested on the host, on purpose: `windows_audio/fil.rs` is
/// `#[cfg(windows)]` and nothing living there can be tested here.
pub fn injection_encore_armee(
    depuis: std::time::Duration,
    fenetre: Option<std::time::Duration>,
) -> bool {
    match fenetre {
        None => true,
        Some(f) => depuis < f,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- read resumption backoff --------------------------------

    #[test]
    fn la_temporisation_de_reprise_croit_puis_se_borne() {
        // Without growth, a burst of errors returned immediately
        // would spin in a tight loop; without a bound, the tenth attempt
        // would arrive seconds too late — and it is the one that decides the
        // thread's death.
        let un = temporisation_de_reprise(1);
        let deux = temporisation_de_reprise(2);
        let trois = temporisation_de_reprise(3);
        assert!(un < deux && deux < trois, "{un:?} {deux:?} {trois:?}");
        assert_eq!(un, REPRISE_LECTURE_BASE);
        assert_eq!(temporisation_de_reprise(0), REPRISE_LECTURE_BASE);

        assert_eq!(temporisation_de_reprise(u32::MAX), REPRISE_LECTURE_MAX);
        assert!(temporisation_de_reprise(LECTURES_ECHOUEES_MAX) <= REPRISE_LECTURE_MAX);
    }

    #[test]
    fn la_tolerance_totale_reste_de_l_ordre_de_la_seconde() {
        // The bound that matters is not the number of attempts but the time
        // they take: too short, an audio service restart kills
        // the window; too long, a dead capture stays announced as alive.
        let totale: std::time::Duration = (1..=LECTURES_ECHOUEES_MAX)
            .map(temporisation_de_reprise)
            .sum();
        assert!(
            totale >= std::time::Duration::from_millis(500)
                && totale <= std::time::Duration::from_secs(3),
            "tolérance totale hors bornes : {totale:?}"
        );
    }

    fn paquet(pts_48k: u64) -> AudioPacket {
        AudioPacket {
            data: vec![0xAA],
            pts_48k,
            captured_at: Instant::now(),
        }
    }

    #[test]
    fn returns_packets_in_deposit_order() {
        let ring = PacketRing::new(4);
        ring.push(paquet(0));
        ring.push(paquet(480));
        assert_eq!(ring.pop().unwrap().pts_48k, 0);
        assert_eq!(ring.pop().unwrap().pts_48k, 480);
        assert!(ring.pop().is_none());
    }

    #[test]
    fn a_saturation_jette_le_plus_ancien_pas_le_plus_recent() {
        // It is the trade-off of §5 of the spec, and reversing it would go
        // unnoticed without this test: both behaviours "lose a
        // packet", but one makes latency grow and the other does not.
        let ring = PacketRing::new(2);
        ring.push(paquet(0));
        ring.push(paquet(480));
        ring.push(paquet(960));

        assert_eq!(
            ring.pop().unwrap().pts_48k,
            480,
            "le paquet le plus ancien (pts 0) doit avoir été jeté"
        );
        assert_eq!(ring.pop().unwrap().pts_48k, 960);
        assert!(ring.pop().is_none());
    }

    #[test]
    fn compte_les_paquets_rejetes() {
        let ring = PacketRing::new(1);
        assert_eq!(ring.rejetes(), 0);
        ring.push(paquet(0));
        assert_eq!(ring.rejetes(), 0);
        ring.push(paquet(480));
        ring.push(paquet(960));
        assert_eq!(ring.rejetes(), 2);
    }

    #[test]
    fn le_depot_ne_bloque_jamais_meme_saturee() {
        // A blocking push would make the transport loop the constraint of the
        // capture thread; the WASAPI capture would overflow in turn. This test
        // would fail by hitting cargo test's global timeout if `push`
        // were to block.
        let ring = PacketRing::new(2);
        for i in 0..1000 {
            ring.push(paquet(i * 480));
        }
        assert_eq!(ring.rejetes(), 998);
    }

    #[test]
    fn une_copie_partage_le_meme_tampon() {
        // The capture thread and the transport loop each hold
        // a copy: they must see the same queue, not two
        // independent queues.
        let ring = PacketRing::new(4);
        let copie = ring.clone();
        ring.push(paquet(0));
        assert_eq!(copie.pop().unwrap().pts_48k, 0);
    }

    // -- the time window of the read fault injection ------------
    use std::time::Duration;

    #[test]
    fn sans_fenetre_l_injection_reste_armee_indefiniment() {
        assert!(injection_encore_armee(Duration::from_secs(3600), None));
    }

    #[test]
    fn within_the_window_the_injection_is_armed() {
        assert!(injection_encore_armee(
            Duration::from_millis(500),
            Some(Duration::from_secs(3))
        ));
    }

    #[test]
    fn passe_la_fenetre_l_injection_est_desarmee() {
        assert!(!injection_encore_armee(
            Duration::from_secs(6),
            Some(Duration::from_secs(3))
        ));
    }

    #[test]
    fn la_borne_de_la_fenetre_est_incluse_ou_exclue_mais_dite() {
        // Explicit choice: `depuis < fenetre`. At the EXACT bound, DISARMED.
        assert!(!injection_encore_armee(
            Duration::from_secs(3),
            Some(Duration::from_secs(3))
        ));
    }
}
