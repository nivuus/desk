//! The share rule: who receives which fraction of the session budget.
//!
//! **No `#[cfg(windows)]`, no COM object, no channel.** This module only
//! decides; the application lives in `capteur/sommeil.rs` and
//! `capteur/fenetre.rs`. It is the pattern set by D4 for `capteur/protocole.rs`
//! and by D5 for `capteur/vivier.rs`: what decides is tested on the host.
//!
//! **The defect this module exists to fix.** Since sub-block D1,
//! each window is a process carrying its own `PeerConnection`, hence its
//! own BWE, and each inherits `BITRATE` as is. With eight windows, eight
//! `set_desired_bitrate` aim at 96 Mb/s cumulated on a single link: nobody
//! arbitrates, and each window encodes as if it were alone.
//!
//! ⚠️ **It is NOT link congestion, and the sub-block's original premise
//! said the opposite — the acceptance run refuted it.** The bridge carries
//! **≥ 1.44 Gb/s** and `packetsLost` is **0 on all eleven runs**: 96 Mb/s
//! cumulated are 15 to 27 times less than what the path carries, and
//! no window ever read another's probing as congestion.
//! **The measured bottleneck is the browser's DECODER**, and what relieves it
//! is the number of pixels: cutting bits without crossing a rung
//! threshold saves nothing (23.08 % of frames dropped at constant area), whereas
//! going from 1280×720 to 852×480 brings the rate down from 18.03 % to 3.94 %.
//! Sharing therefore remains the right mechanism, but **it acts through the RESOLUTION**:
//! a smaller share moves the ladder of `congestion/echelle.rs` down
//! one rung, and it is that rung that relieves the decoder. See
//! `docs/superpowers/plans/2026-08-03-multifenetres-partage-capacite-resultats.md`, policy: allow-fr (file path)
//! §1 and §3.6.

/// Boost granted to the window the user is looking at.
///
/// ⚠️ **NOT CALIBRATED.** The reasoning behind it is not a measurement:
/// two neighbouring rungs of the ladder are in a pixel ratio of
/// 1.25² ≈ 1.56 (`DIVISEURS` in `congestion/echelle.rs`), so a factor of 2
/// guarantees more than one rung of difference in favour of the watched window. It is
/// criterion ② of the acceptance run that will judge it, not this intuition.
pub const FACTEUR_FOCUS: u32 = 2;

/// Share left to a sleeping window.
///
/// **Never zero**: it no longer encodes anything (D5 released its encoder) but
/// its `PeerConnection` lives, and `set_desired_bitrate(0)` is not a setting
/// str0m is meant to receive.
///
/// ⚠️ **NOT CALIBRATED**, and the hypothesis motivating it is not verified: we
/// do not know whether str0m actually emits probing padding when no media
/// goes out. If it does, this floor saves sleeping windows from emitting
/// traffic for nothing; if not, it only costs its line. **It was never
/// a saturation question anyway**: the link carries ≥ 1.44 Gb/s and
/// has never lost a packet (see the head doc). The order of magnitude
/// covers audio (`opus::BITRATE_BPS`, 128 kb/s) and leaves margin.
///
/// ⚠️ **This value must NEVER reach `Controleur::changer_plafond`.**
/// It is far below the lowest rung of the ladder (691,200 bps at
/// 1280×720/60 with `BPP_MIN`): applied as an encoding ceiling, it
/// sets `video_bitrate_bps = 256_000` through the `min` of `changer_plafond`, and
/// **nothing raises it again on wake-up** — a sleeping window emits nothing, so str0m
/// emits no `MediaEgressStats` for it, so `Controleur::observer`,
/// the only possible repair, is never called. It is defect I1 of the
/// final branch review; the remedy lives in `Session::appliquer_part`, which
/// applies a sleeping window's share only to probing. **A sleeping window has
/// released its encoder (D5): there is nothing to bound on the encoding side.**
pub const PART_DORMANTE_BPS: u32 = 256_000;

/// A window's state, as the distributor needs to know it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fenetre {
    pub session: String,
    pub eveillee: bool,
    pub focalisee: bool,
}

/// Splits `budget_bps` among the windows.
///
/// Each sleeping window receives `PART_DORMANTE_BPS`; the rest is divided among the
/// awake ones, the focused one receiving `FACTEUR_FOCUS` shares instead of one.
///
/// **Inviolable guarantees**: no panic, no zero share, total
/// function.
///
/// **Three regimes** depending on `budget` and `diviseur` (number of awake windows,
/// increased by `FACTEUR_FOCUS - 1` if there is an awake focused one):
///
/// **Regime 1**: `budget ≥ endormies × PART_DORMANTE_BPS` AND `reste ≥ diviseur`.
/// The sum never exceeds the budget, and the focus boost is applied.
/// It is the only no-overrun guarantee.
///
/// **Regime 2**: `budget ≥ endormies × PART_DORMANTE_BPS` BUT `reste < diviseur`.
/// The `.max(1)` applied to each final share makes `part_base = 0`. Each
/// awake window receives 1 bps, the focus boost disappears, and the sum exceeds
/// the budget by at most `diviseur − reste` bps. The sleeping windows' floors are
/// paid in full.
///
/// **Regime 3**: `budget < endormies × PART_DORMANTE_BPS`. The sleeping windows'
/// floors are paid anyway (no zero share), and the sum exceeds the
/// budget by `(endormies × PART_DORMANTE_BPS − budget) + eveillees` bps, not bounded
/// by the number of awake windows alone. This behaviour is accepted: a
/// `set_desired_bitrate(0)` would be worse than an overrun on a link nothing
/// can satisfy anyway. This case is described in detail in the comment
/// on the computation of `reste` in the body.
///
/// **No work-conserving**: a window that does not use its share does not
/// give it back to the others. That would be a second feedback loop whose
/// stability would have to be tested — out of D6's scope.
pub fn repartir(budget_bps: u32, fenetres: &[Fenetre]) -> Vec<(String, u32)> {
    let endormies = fenetres.iter().filter(|f| !f.eveillee).count() as u32;
    let eveillees = fenetres.iter().filter(|f| f.eveillee).count() as u32;

    // `saturating_sub`: a budget below the total of the floors yields a
    // zero remainder, never an overflow. The sleeping windows then keep their
    // floor and the awake ones receive the minimum of one share, which makes it
    // cross the budget — accepted degenerate case, but it does not panic.
    let reste = budget_bps.saturating_sub(endormies.saturating_mul(PART_DORMANTE_BPS));

    // A single boost, to the FIRST awake focused window encountered:
    // several focused windows do not last (the client emits `blur`), but
    // granting two would break the budget invariant.
    let indice_focalisee = fenetres.iter().position(|f| f.eveillee && f.focalisee);

    let diviseur = match indice_focalisee {
        Some(_) => eveillees.saturating_sub(1) + FACTEUR_FOCUS,
        None => eveillees,
    };
    // `max(1)`: without an awake window, the divisor is 0 and the division would panic.
    // The value then serves nobody — no window is awake.
    let part_base = reste / diviseur.max(1);

    fenetres
        .iter()
        .enumerate()
        .map(|(i, f)| {
            let bps = if !f.eveillee {
                PART_DORMANTE_BPS
            } else if Some(i) == indice_focalisee {
                part_base.saturating_mul(FACTEUR_FOCUS)
            } else {
                part_base
            };
            // No zero share: a derisory budget must not produce a
            // `set_desired_bitrate(0)`.
            (f.session.clone(), bps.max(1))
        })
        .collect()
}

#[cfg(test)]
mod tests;
