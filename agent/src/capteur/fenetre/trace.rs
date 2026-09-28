//! The periodic trace of the capture counters (`SOURCE_TRACE=1`).
//!
//! **Extracted from `fenetre.rs`** (review of task 11, D9) for the same reason
//! as `commandes.rs` and `transitions.rs`: the parent file has a narrow margin
//! under the project's 500-line cap — this would be the third
//! child module on the same pattern. Transposed as is: neither the values, nor
//! the order of operations changed, only the location.

use std::sync::OnceLock;

use crate::windows_source::WindowsSource;

/// `SOURCE_TRACE=1` (presence, any value) enables the periodic trace
/// of the capture counters (`Telemetrie`, see
/// `windows_source/telemetrie.rs`) — **presence ENABLES**, the reverse of the
/// `=0` DISABLES convention of `PLEIN_ECRAN`/`AUDIO`/`SUPERVISEUR`/`CAPTEUR`:
/// it is the convention already in force for this specific variable before its
/// move here (D9, task 11). It lived in `demarrage.rs`, on the
/// CHILD side, which has had no `WindowsSource` since D4: it only showed
/// zeros there, and nothing read the counters of the SENSOR, where they are
/// actually written — see the module doc of `windows_source/telemetrie.rs`.
///
/// `OnceLock`, same reason as `plein_ecran::actif`: this function is
/// consulted at every `PERIODE_COMPTEURS`, and the environment does not change during
/// the process.
fn trace_source_active() -> bool {
    static ACTIF: OnceLock<bool> = OnceLock::new();
    *ACTIF.get_or_init(|| std::env::var("SOURCE_TRACE").is_ok())
}

/// Under the `fenetre{session=…}` span set by D7: the trace is therefore
/// attributable without an extra field, which was impossible as long as
/// the counters lived in three process statics. `None`: the
/// window sleeps, the encoder and capture are released (see the
/// `source` field of `Fenetre`).
pub(super) fn tracer_les_compteurs(source: Option<&WindowsSource>) {
    if trace_source_active() {
        if let Some(source) = source {
            let (ticks, capturees, produites) = source.telemetrie.lire();
            tracing::info!(ticks, capturees, produites, "compteurs de capture");
        }
    }
}
