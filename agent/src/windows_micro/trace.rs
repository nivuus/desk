//! The periodic trace of the mic render thread (`windows_micro.rs`), in its
//! own file for the 500-lines-per-file limit.

#![cfg(windows)]

use crate::micro::CompteursMicro;
use crate::wasapi::ecriture::Reveil;

/// The periodic trace.
///
/// ⚠️ **`session` is MANDATORY**: `agent.log` mixes the supervisor and all
/// its children since D4, and D6 had to re-attribute two traces in the middle of acceptance
/// for lack of this field. A trace without it is a number in an anonymous
/// multiset.
#[allow(clippy::too_many_arguments)]
pub(super) fn tracer(
    session: &str,
    ecrites: u64,
    silence: u64,
    retards: u64,
    reveil: Reveil,
    compteurs: &CompteursMicro,
    precedents: &CompteursMicro,
    occupation: std::time::Duration,
) {
    // The counters are DELTAS of the elapsed second, not cumulative totals: a
    // cumulative total would drag a single incident along for the rest of the session.
    let d = |maintenant: u64, before: u64| maintenant.saturating_sub(before);
    tracing::info!(
        session = %session,
        ecrites,
        // ⚠️ `ecrites` and `silence` SIDE BY SIDE: `remplir` fills with silence
        // without ever saying so, so "48,000 frames written" is written
        // exactly the same for a microphone that speaks and for a microphone that is
        // silent. Without the second, this trace proves nothing.
        silence,
        retards,
        reveil = reveil.libelle(),
        deposees = d(compteurs.deposees, precedents.deposees),
        sauts = d(compteurs.sauts, precedents.sauts),
        insertions = d(compteurs.insertions, precedents.insertions),
        plc = d(compteurs.plc, precedents.plc),
        // ⚠️ **`plc` and `plc_plafonnees` SIDE BY SIDE**, and it is the heart of
        // this pair: both arise from a missing frame, and without the
        // second one cannot distinguish "concealment is working" from "the
        // ceiling has bitten and the sink is silent". See `micro/dissimulation.rs`.
        plc_plafonnees = d(compteurs.plc_plafonnees, precedents.plc_plafonnees),
        // ── Bloc E3 : le legs n°7 de E2 ────────────────────────────────────
        //
        // 🔴 **These two quantities ALREADY EXISTED and were read by
        // NOBODY on the production path.** `LecteurMicro::occupation()`
        // has been public since E1 and `famines` has been a field of `CompteursMicro`
        // since E1; both only lived in the trace of the MEASUREMENT SINK
        // (`MICRO_MESURE=1`), which **cannot coexist with the cable** — the
        // measurement sink consumes the buffer, the cable too. E2 noted it and
        // could not fix it within its scope. There was nothing to
        // compute, only to trace.
        //
        // 🔴 **`occupation_ms` is a SNAPSHOT; `famines` is a DELTA.**
        // All the other counters of this trace are deltas *on purpose*
        // — "a cumulative total would drag a single incident along for the rest of the
        // session". Mixing the two without saying so would make the line
        // unreadable: one would read "120" for an occupation and "3" for
        // starvations believing both comparable over the same second, whereas
        // the first describes the trace's instant and the second the second
        // preceding it.
        //
        // ⚠️ **`famines` and `occupation_ms` SIDE BY SIDE**, on the pattern of the
        // two preceding pairs: a starvation is an emptied buffer, hence an
        // occupation that touched zero. The second alone would not say how many
        // times; the first alone would not say how far we are from the edge.
        //
        // ⚠️ **What this does NOT give**: end-to-end latency, which
        // NOTHING in this repository has measured since D1. It is the SECOND of the two
        // components of the latency added by the agent, of which E2 only had the
        // first (`retards`). The third criterion of spec §13 stays NOT
        // JUDGED.
        famines = d(compteurs.famines, precedents.famines),
        occupation_ms = occupation.as_millis(),
        "mic writes to the cable"
    );
}
