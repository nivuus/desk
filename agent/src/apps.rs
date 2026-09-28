//! Discovery and launch of the VM's applications.
//!
//! ⚠️ THIS MODULE IS DECLARED WITHOUT `cfg` in `main.rs`, and it is its Windows
//! children that carry theirs. That is what makes `apps::raccourci` and
//! `apps::reconciliation` exist on the Linux host, where their tests run — the
//! "Child module convention" of `CLAUDE.md` is therefore not invoked:
//! no module crosses a `#[cfg(windows)]` boundary here.

use std::time::Duration;

/// File associations — the PURE part here, the registry behind a
/// `#[cfg(windows)]` (sub-block G5, slice F).
pub mod associations;
pub mod icone;
pub mod installation;
pub mod raccourci;
pub mod reconciliation;
pub mod sha256;
pub mod surveillance;

#[cfg(windows)]
pub mod boucle;
#[cfg(windows)]
pub mod lancement;
#[cfg(windows)]
pub mod lecture;

/// Between two complete reads of the four roots.
///
/// **NOT CALIBRATED.** What allows it is measured: on this VM, enumerating the
/// four roots costs 31 ms and resolving the 218 shortcuts 82 ms, i.e.
/// 113 ms — 0.38 % of a period. What is NOT measured is the delay the
/// user feels between installing an application and its
/// appearance: it is up to thirty seconds, and that is the real trade-off.
///
/// ⚠️ THE PERIODIC RECONCILIATION IS THE SOURCE OF TRUTH, and remains so: the
/// notification through `ReadDirectoryChangesW` **IS** only an ACCELERATOR. A
/// missed notification must never be able to freeze a catalogue.
///
/// ✅ **THE FUTURE IS PAST** (sub-block G4): this sentence was written in the
/// future tense — "of a later sub-block will only be an accelerator" —, and
/// `apps::surveillance` exists. It is put in the present RATHER THAN REWRITTEN:
/// its load-bearing property is unchanged, and **G4 took nothing away from this
/// constant**. The only thing the watcher changes is WHEN a
/// reconciliation starts, never what it does nor what it relies on.
///
/// ⚠️ **AND ONE STATE DOES DISARM IT**: `APPS_SURVEILLANCE=seule`, a **BENCH
/// variable**, cuts this period in favour of notifications alone. It is the
/// RED of criterion ③ of G4, and the only setup that makes observable what
/// this very sentence promises.
pub const PERIODE_RECONCILIATION: Duration = Duration::from_secs(30);

/// 🔴 `APPS=0` DISARMS; MERE PRESENCE DOES NOT ENABLE.
///
/// Testing `is_ok()` — or `is_some()` here — WOULD ENABLE the mechanism when
/// `APPS=0` is written TO TURN IT OFF. It is the convention of `PLEIN_ECRAN`, `AUDIO` and
/// `PART_SONDAGE`, and `CLAUDE.md` writes it down for that exact reason.
///
/// The decision is isolated here BECAUSE `brancher` is `#[cfg(windows)]` and
/// no host test can therefore reach it: this predicate, on the other hand, is pure,
/// and it is the only part of the guard that can be seen turning red on the host.
pub fn desarme(value: Option<&str>) -> bool {
    value == Some("0")
}

/// Wires up discovery **AND INSTALLATION**, or returns `None` while SAYING
/// which of the three reasons.
///
/// ❌ **THIS LINE ONLY SAID "DISCOVERY" UNTIL SUB-BLOCK G3**, and
/// it was true until then. It now returns TWO handles: the COM discovery
/// thread, and the `tokio` installation task. Both are disarmed
/// together by `APPS=0` — declared, not discovered.
///
/// ⚠️ "NO CHANNEL" IS NOT "`APPS=0`", AND A SILENCE WOULD CONFUSE THEM.
/// Without a channel there is nowhere to send a catalogue; it is a state
/// DIFFERENT from an explicit disarm, and it must read as such in the
/// log.
///
/// ⚠️ NOTHING IS WIRED IN SENSOR MODE, and it is not an oversight: the
/// `CAPTEUR` mode returns BEFORE enrolment in `main.rs`, so a sensor has
/// no channel, no token, no prefix, and cannot carry any catalogue.
///
/// ⚠️ **NOTHING IS WIRED EITHER IN THE BRIDGE OR IN A WINDOW CHILD,
/// AND THAT HAS BECOME THE ORDINARY CASE** (fix of 20 August 2026). They hold
/// their token from the supervisor through `AGENT_JETON` and open no channel, so
/// that a single socket per VM exists. **It was even one of the consequences of the
/// defect that was fixed**: before, the bridge enrolled, hence reconciled too,
/// and the COM/Shell work was done TWICE every thirty seconds —
/// measured, 6 reconciliations in 64 s against 3 after the fix.
///
/// 🔴 HENCE THE WORDING OF THE `warn!` BELOW, WHICH NAMES BOTH CAUSES WITHOUT
/// CLAIMING TO TELL THEM APART: this function only receives an `Option`, it cannot
/// know whether the channel is missing because the identity is inherited (normal)
/// or because `AGENT_VM`/`AGENT_SECRET` are missing (failure). Naming both
/// is better than a curt "inactive" that reads as a failure in the
/// case that has become the most frequent. The process that INHERITS writes its own
/// line just before (`main.rs`), and the two are read together.
///
/// It takes the `Option` rather than the channel so that `main.rs` only receives
/// one call: it is the 500-line rule applied where it bites, the
/// file already being above the 450 gate.
/// The handles that `brancher` returns, and that `main.rs` merely BINDS.
///
/// 🔴 A STRUCTURE RATHER THAN A SECOND RETURN VALUE, AND IT IS A SCOPE
/// COMMITMENT: `main.rs` writes `let _apps = apps::brancher(…)`, and this line
/// **does not move** — four work streams run concurrently in this
/// tree, and `main.rs` is the file they all touch.
///
/// ⚠️ BOTH HANDLES ARE KEPT WITHOUT BEING AWAITED: dropping them
/// would end the threads. That is why `main.rs` binds the return value
/// instead of ignoring it, and it now applies to two threads instead of one.
pub struct Poignees {
    /// The COM discovery thread.
    _decouverte: Option<std::thread::JoinHandle<()>>,
    /// The tokio installation task.
    _installation: Option<tokio::task::JoinHandle<()>>,
    /// The watcher thread of the four roots (sub-block G4).
    ///
    /// ⚠️ **THIRD HANDLE, AND THE `main.rs` LINE STILL DOES NOT MOVE**
    /// — that is the whole purpose of the structure: `let _apps = apps::brancher(…)`
    /// is in the file that four concurrent work streams all touch.
    _surveillance: Option<std::thread::JoinHandle<()>>,
}

pub fn brancher(canal: Option<&mut crate::plateforme::Canal>) -> Option<Poignees> {
    let Some(canal) = canal else {
        tracing::warn!(
            "application discovery inactive: this process has no /agent channel \
             (identity inherited from the supervisor, or AGENT_VM/AGENT_SECRET absent)"
        );
        return None;
    };
    // 🔴 `APPS=0` ALSO DISARMS INSTALLATION, and it is DECLARED rather than
    // discovered: `start` returns before anything, so neither of the two halves
    // gets wired. No separate variable is added to disarm
    // installation alone, for lack of a demonstrated need — and one more variable
    // that would serve nobody is a variable one will forget to
    // pass through `scripts/run-agent.sh`, a trap this repository has paid for five
    // times.
    let partage = installation::partage::Partage::neuf();
    // 🔴 THE MODE IS READ HERE, AND ITS `info!` IS UNCONDITIONAL. No
    // run can then be misattributed: an acceptance run that reads a
    // verdict knows under which mode it was produced.
    //
    // ⚠️ **THE TRACE PROVES THE VARIABLE REACHED THE PROCESS; IT DOES NOT
    // PROVE THE MECHANISM IS OFF** — a lesson sub-block P1
    // paid for on `PRESSE_PAPIER=0`. What discriminates is the COUNT of
    // `catalogue reconcilie` lines, never the presence of this one.
    let brut = std::env::var("APPS_SURVEILLANCE").ok();
    let (mode, inconnue) = surveillance::mode::Mode::lire(brut.as_deref());
    if let Some(value) = inconnue {
        // 🔴 AN UNKNOWN VALUE IS NAMED, AND THE SHIPPED BEHAVIOUR IS
        // KEPT. Without this `warn!`, a typo in a red (`seul` for
        // `seule`) would run the GREEN behaviour under the name of the RED, and
        // the acceptance run would read a false verdict — "a check that cannot
        // fail", in a new form.
        tracing::warn!(
            value,
            "APPS_SURVEILLANCE: unknown value, the SHIPPED behaviour is kept \
             (expected: 0, sans-rebond, seule, or the variable absent)"
        );
    }
    tracing::info!(mode = ?mode, "watch mode retained");
    // 🔴 `APPS=0` **ALSO** DISARMS THE WATCHER, AND IT IS DECLARED RATHER THAN
    // DISCOVERED — the wording G3 used for installation. Without
    // this line, `APPS=0` would cut discovery and installation, and
    // leave a watcher thread running to feed counters
    // that **nobody polls any more**: four handles, 256 KiB of NON-PAGED
    // pool and a thread, serving nothing.
    //
    // ⚠️ The read is done HERE, in addition to the two `start` that already do it
    // each for themselves: `desarme` is pure and reading it costs nothing,
    // whereas deducing the state from an `Option<JoinHandle>` returned by another stage
    // would couple two mechanisms through a value.
    let mode = if desarme(std::env::var("APPS").ok().as_deref()) {
        surveillance::mode::Mode::Desarmee
    } else {
        mode
    };
    let (veille, surveillance) = surveillance::start(mode);
    let decouverte = start(canal, partage.clone(), veille, mode);
    let installation = start_installation(canal, partage);
    if decouverte.is_none() && installation.is_none() {
        return None;
    }
    Some(Poignees {
        _decouverte: decouverte,
        _installation: installation,
        _surveillance: surveillance,
    })
}

/// The installation thread, on `tokio` — never on the COM thread.
///
/// ⚠️ IT IS NOT WIRED IF THE QUEUE HAS ALREADY BEEN TAKEN: `installations()` returns
/// `None` on the second call, exactly like `ordres()`, and for the same
/// reason — two consumers would steal orders from each other.
#[cfg(windows)]
fn start_installation(
    canal: &mut crate::plateforme::Canal,
    partage: installation::partage::Partage,
) -> Option<tokio::task::JoinHandle<()>> {
    if desarme(std::env::var("APPS").ok().as_deref()) {
        return None;
    }
    let installations = canal.installations()?;
    let base = canal.url_signaling().to_string();
    let identite = canal.veille_identite();
    let emetteur = canal.emetteur();
    Some(tokio::spawn(installation::fil::tourner(
        installations,
        move |message| emetteur.emettre(message),
        identite,
        base,
        partage,
    )))
}

/// The non-Windows variant: nothing to install, and **nothing to log** —
/// same reason as `start`.
#[cfg(not(windows))]
fn start_installation(
    _canal: &mut crate::plateforme::Canal,
    _partage: installation::partage::Partage,
) -> Option<tokio::task::JoinHandle<()>> {
    None
}

#[cfg(windows)]
fn start(
    canal: &mut crate::plateforme::Canal,
    partage: installation::partage::Partage,
    veille: surveillance::partage::Veille,
    mode: surveillance::mode::Mode,
) -> Option<std::thread::JoinHandle<()>> {
    if desarme(std::env::var("APPS").ok().as_deref()) {
        tracing::warn!("application discovery DISARMED (APPS=0)");
        return None;
    }
    let ordres = canal.ordres()?;
    // The HTTP address of the icon upload is DERIVED from the channel's:
    // both live on the same service, and two variables would diverge.
    let base = canal.url_signaling().to_string();
    let identite = canal.veille_identite();
    let emetteur = canal.emetteur();
    // 🔴 A REAL THREAD, NOT A `spawn_blocking`. The COM apartment belongs to
    // ITS thread: `IShellLinkW` and `ShellExecuteExW` must both run
    // on the one that called `CoInitializeEx`, and a tokio pool thread may
    // serve other tasks between two ticks.
    std::thread::Builder::new()
        .name("app-discovery".into())
        .spawn(move || {
            boucle::tourner(
                move |message| emetteur.emettre(message),
                ordres,
                identite,
                boucle::Reglages {
                    base_plateforme: base,
                    periode: PERIODE_RECONCILIATION,
                    partage,
                    veille,
                    mode,
                },
            )
        })
        .map_err(|error| {
            tracing::error!(%error, "application discovery thread not started");
        })
        .ok()
}

/// The non-Windows variant: there is no shortcut to read.
///
/// It exists so that `main.rs` does not have to carry one more `cfg`, and
/// **it logs nothing**: a Linux agent has no business complaining that it does
/// not discover Windows applications, and confusing it with `APPS=0` — which,
/// itself, SAYS it is disarmed — would blur two distinct states.
#[cfg(not(windows))]
fn start(
    _canal: &mut crate::plateforme::Canal,
    _partage: installation::partage::Partage,
    _veille: surveillance::partage::Veille,
    _mode: surveillance::mode::Mode,
) -> Option<std::thread::JoinHandle<()>> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_value_zero_disarms_discovery() {
        // 🔴 THE RED: a `value.is_some()`. It would return `true` for `"1"`,
        // for `""` and for anything — that is, writing `APPS=0`
        // TO TURN OFF discovery would turn it on, and writing `APPS=1` TO
        // TURN IT ON would turn it off. The two errors cancel out so well that
        // nobody would see them without this test.
        assert!(desarme(Some("0")));
        for value in [
            None,
            Some(""),
            Some("1"),
            Some("00"),
            Some("0 "),
            Some("false"),
        ] {
            assert!(!desarme(value), "{value:?} must NOT disarm");
        }
    }

    #[test]
    fn la_periode_de_reconciliation_laisse_la_place_a_son_propre_cout() {
        // The 113 ms measured on this VM (31 ms of enumeration + 82 ms of
        // COM resolution) must remain a negligible fraction of the
        // period, otherwise the loop would spend its time reading itself.
        assert!(PERIODE_RECONCILIATION >= Duration::from_secs(5));
    }
}
