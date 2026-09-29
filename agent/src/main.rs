mod accent;
mod apps;
mod audio;
// No `#[cfg(windows)]` here: the media channel protocol and the
// `SourceDistante` are pure logic, and must compile and be
// tested on Linux. The submodules that touch DXGI and the pipes are
// gated inside `capteur.rs`.
mod capteur;
// The READABLE rendering of an `anyhow::Error`'s chain of causes in a
// trace. Bare root: its name prefixes no top-level module.
mod cause;
mod clock;
mod congestion;
// Reading and validating the environment, extracted from this file on
// 20 August 2026: see the module comment of `configuration.rs`. `Config`
// is re-exported below, so that no call site has moved.
mod configuration;
mod cursor;
mod demarrage;
mod diagnostics;
mod disposition;
mod frames;
// No `#[cfg(windows)]` here: the pure logic of `gamepad` (task 9,
// ordering of states and limiting of vibrations) has nothing
// specific to Windows and must compile and be tested on Linux. The
// `probe` probe, for its part, stays gated inside the file itself
// (`agent/src/gamepad/probe.rs`), with its `vigem-client` /
// `anyhow::Context` dependencies specific to the probe.
mod gamepad;
mod geometry;
mod h264;
mod input;
mod micro;
mod mire;
mod moniteurs_virtuels;
mod opus;
#[cfg(windows)]
mod pointer_settings;
// The VM's clipboard, VM -> browser direction. No `#[cfg(windows)]`:
// the whole decision (normalisation, clamping, guards) is PURE and must
// compile and be tested on the Linux host; the two Win32 calls live in
// `presse_papier/win32.rs`, gated inside the module. At the bare root and
// not under `capteur/`: the owner is the sensor today, the child
// the day the single-window mode gets one — a name under `capteur/` would be
// wrong that day.
mod presse_papier;
// The client of the platform's `/agent` channel. No `#[cfg(windows)]`:
// the computation of the retry delay (`plateforme::repli`) is pure and must
// compile and be tested on the Linux host, and the socket itself has nothing
// specific to Windows.
mod plateforme;
mod pont;
// Bare root, not `pont/relance.rs`: the name describes nothing of the BRIDGE
// itself, only a supervision policy (spaced restart, stability
// threshold) — see the file's header for the full application of
// the naming convention. Extracted from `superviseur::boucle::
// surveillance_pont` (fix round 2) to compile and be tested on
// the host: the latter lives behind `superviseur::boucle::#![cfg(windows)]`.
mod rebuild;
mod relance_pont;
mod signaling;
// No `#[cfg(windows)]` here: it is the portable part of `capture.rs`
// (itself `#![cfg(windows)]` as a whole) — see the module comment
// of `sortie_dxgi.rs`. `superviseur::placement` (task 7) needs it
// to compile and be tested on Linux.
mod sortie_dxgi;
// Dominant frequency of a block of samples ("A-bis" fix). Pure,
// standalone name: bare root, like `geometry` and `sortie_dxgi` — see the
// child module convention of `docs/claude/module-conventions.md`.
mod spectre;
// No `#[cfg(windows)]` here: the PERSISTENCE predicate (task 2bis, D9,
// fix no. 14 of the review) is pure -- `Option<(u32, u32)> ×
// Option<(u32, u32)> -> &str` -- and must compile and be tested on Linux,
// even though it is only called from `diagnostics::multifenetre`, gated
// behind `#[cfg(windows)]` in `diagnostics.rs`.
mod survie_verdict;
// No `#[cfg(windows)]` here: the pure logic of `superviseur` (task 2)
// decides which windows deserve to exist on the browser side, and must
// compile and be tested on Linux without depending on the Windows API.
mod source;
mod superviseur;
mod transport;
mod turn;
// The region computation is pure and must be testable on the host: it is therefore
// declared independently of the rest of `windows_source`, which only compiles on
// Windows.
#[path = "windows_source/sortie.rs"]
mod windows_source_sortie;

// Same set-up, and for the same reason: the classification of acquisition
// failures and the retry budget are pure and must be tested on the
// host, whereas `capture.rs` is `#![cfg(windows)]` as a whole.
#[path = "capture/reprise.rs"]
mod capture_reprise;

// Same set-up again (D9, task 11): `Telemetrie` is pure — three
// atomic counters per SESSION, replacing the PROCESS statics
// `TICKS`/`CAPTURED`/`PRODUCED` of `windows_source.rs`, dead on both sides
// since D4 (record no. 1 of D6). `mod telemetrie;` INSIDE `windows_source`
// would not be enough: that file is itself `#![cfg(windows)]`, and
// `mod windows_source;` below is too — on Linux, its whole
// subtree would be absent from compilation, including this module, which
// would therefore no longer be "testable on the host".
#[path = "windows_source/telemetrie.rs"]
mod windows_source_telemetrie;

// Same set-up again ("A-bis" fix, 19 August 2026): the rule that
// elects the render audio endpoint to capture is pure — it takes
// a list of names and identifiers and returns an elected one or a refusal reason —
// whereas `wasapi.rs` is `#![cfg(windows)]` as a whole. A
// `mod peripherique;` INSIDE `wasapi` would make it absent from the host
// compilation, hence untestable, exactly as for `telemetrie` above.
#[path = "wasapi/peripherique.rs"]
mod wasapi_peripherique;

// Same set-up, a third time (block E2, 20 August 2026): the mix
// format that writing the microphone to the cable accepts — and above all those
// it REFUSES while naming them — is a rule without a single byte of COM. It
// lives with `wasapi` because it is WASAPI it talks about, and it is
// hoisted here because `wasapi.rs` is `#![cfg(windows)]`: plan E2
// asks for this rule "PURE and tested on the host" and elsewhere houses
// `wasapi/ecriture.rs` under that same `cfg`. The two cannot fit in
// the same file; they fit in the same directory.
#[path = "wasapi/format.rs"]
mod wasapi_format;

/// The PURE part of the NVENC path (batch 31): choice of path, translation of
/// settings, version arithmetic of structures.
///
/// ⚠️ **Hoisted here for the same reason as `wasapi_format` just above**:
/// `encode.rs` is `#![cfg(windows)]`, and this logic must be tested on
/// the host. The name carries the `encode_` prefix of an existing top-level
/// module, so the convention places it IN its parent, through `#[path]` —
/// and not at the bare root. See the file's header.
#[path = "encode/nvenc.rs"]
mod encode_nvenc;

#[cfg(windows)]
mod appartenance;
#[cfg(windows)]
mod capture;
#[cfg(windows)]
mod encode;
mod entrees;
#[cfg(windows)]
mod wasapi;
mod window;
#[cfg(windows)]
mod windows_audio;
/// Writing the microphone to the virtual cable (block E2).
///
/// ⚠️ **Bare root, and it is checked against the convention** (§ "Child
/// module convention", head of `docs/claude/module-conventions.md`): `windows_micro` carries the
/// `<parent>_` prefix of no existing top-level module — `window` would require
/// `window_`, and there is no `mod windows;`. It joins `windows_audio`
/// and `windows_source`, at the bare root for the same reason. **No `#[path]`.**
#[cfg(windows)]
mod windows_micro;
#[cfg(windows)]
mod windows_source;

pub(crate) use configuration::Config;
use configuration::{config, variable_non_vide};

use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    // Diagnostic (parallel duplications work stream, task 2bis): set BEFORE
    // everything else, so that no fault can occur before it.
    // Inert without `AGENT_TRACE_EXCEPTIONS`, and the handler only runs
    // at the moment of an access violation — never on the nominal path.
    #[cfg(windows)]
    diagnostics::exceptions::installer();

    // At the very beginning, before any possibility of input injection (the
    // diagnostic modes of `diagnostics::aiguiller` inject none, but the
    // normal session further down does): neutralise the pointer acceleration and
    // sensitivity of the Windows session. Never makes start-up
    // fail — degraded aiming is better than no session.
    //
    // `INPUT_LINEARITY_NEUTRALISER=0` ALSO skips this call, and not
    // only the one specific to the linearity probe, carried by
    // `diagnostics::entree`.
    // Fix of review round 1: the SPI setting that
    // `neutraliser()` sets applies to the whole Windows SESSION, not to the
    // process that set it (see `pointer_settings.rs`) — skipping
    // only the probe's call was therefore not enough, since this
    // call, higher up and unconditional, had already neutralised
    // acceleration before the probe even read its own variable.
    // The reference measurement got a zero gap whatever the real
    // state of the VM: an artefact guaranteed by construction, not a measurement.
    #[cfg(windows)]
    if std::env::var("INPUT_LINEARITY_NEUTRALISER").as_deref() != Ok("0") {
        match pointer_settings::neutraliser() {
            Ok(rapport) => tracing::info!(rapport, "pointer acceleration neutralised"),
            Err(e) => {
                tracing::warn!(error = %e, "pointer acceleration neutralisation failed")
            }
        }
    } else {
        tracing::warn!(
            "neutralisation SKIPPED at startup (INPUT_LINEARITY_NEUTRALISER=0, reference measurement)"
        );
    }

    let mut config = config()?;

    // After the neutralisation above, and before any session assembly:
    // it is this order that the linearity probe assumes (see `diagnostics`).
    if diagnostics::aiguiller()? {
        return Ok(());
    }

    // The sensor mode detects nothing and launches no one: it holds the N
    // DXGI duplications and the N encoders, and serves the media to the
    // supervisor's children through a named pipe. `CAPTEUR=0` DISABLES the mode, like
    // `SUPERVISEUR=0` and `AUDIO=0` — same operational trap, same safeguard.
    if matches!(std::env::var("CAPTEUR").as_deref(), Ok(v) if v != "0") {
        return capteur::executer();
    }

    // ENROLMENT PRECEDES ANY SIGNALING (sub-block P3). The two handshakes
    // that follow — the supervisor's on its control session,
    // the child's on its media session — present the token this
    // channel delivers, and the prefix it returns names the sessions.
    //
    // The channel stays open for the whole life of the process: it carries the
    // heartbeat, hence `vu_a`, hence the ready/unreachable state
    // the platform reads. Binding it to a variable and not to `_` is not a
    // lint nicety — it is what keeps it alive.
    //
    // The sensor mode, for its part, has already gone off above: it talks to no
    // signaling and therefore has no identity to present.
    //
    // 🔴 **A SINGLE PROCESS PER VM OPENS THIS CHANNEL, AND IT IS THE FIX OF
    // 20 August 2026.** The file bridge and the window children inherited
    // `AGENT_VM`/`AGENT_SECRET` and enrolled under the SAME identity as the
    // supervisor; the platform's registry admitting only one socket per
    // VM, they evicted one another endlessly — 95 enrolments and
    // 94 evictions in 64 s, measured. They now receive `AGENT_JETON` from the
    // supervisor (`superviseur/lanceur.rs`) and open no channel. The
    // rule that decides is PURE and tested on the host: `plateforme::identite`.
    let mut _canal_plateforme = match plateforme::identite::source(
        variable_non_vide("AGENT_JETON").as_deref(),
        config.agent_vm.as_deref(),
        config.agent_secret.as_deref(),
    ) {
        plateforme::identite::SourceIdentite::Heritee(jeton) => {
            // ⚠️ NO PREFIX IS INHERITED, and it is not an oversight:
            // `config.prefixe` is only read by the supervisor mode, which
            // composes its control session with it. A child receives its session
            // ready-made in `SESSION_ID`, and the bridge its own — both
            // composed by the supervisor, which knows the prefix. Inheriting
            // an unused prefix would only invite using it.
            tracing::info!(
                "identity inherited from the supervisor (AGENT_JETON): this process opens \
                 no /agent channel, a single socket per VM"
            );
            config.jeton = Some(jeton);
            None
        }
        plateforme::identite::SourceIdentite::Enrolement { vm, secret } => {
            let mut canal = plateforme::ouvrir(&config.signaling_url, vm, secret);
            // UNBOUNDED wait, and it is deliberate: without an identity, no
            // session can be established, and the retry loop logs
            // each of its attempts. It only returns `None` if it has
            // GIVEN UP — a version refusal —, and there is then nothing to
            // wait for.
            let Some(identite) = canal.attendre_identite().await else {
                anyhow::bail!("enrolment abandoned by the platform: see the /agent channel log");
            };
            config.prefixe = identite.prefixe;
            config.jeton = Some(identite.jeton);
            Some(canal)
        }
        plateforme::identite::SourceIdentite::Absent => {
            tracing::warn!(
                "AGENT_VM or AGENT_SECRET missing, and no inherited AGENT_JETON: no \
                 agent token. The platform WILL REFUSE the handshake and no \
                 session will be established (sub-block P3, without a permissive switch)."
            );
            None
        }
    };

    // Application discovery lives HERE, between enrolment and the
    // routings: it needs the channel, and must run in BOTH modes
    // that have one — supervisor and single-window. The sensor, for its part, has already
    // gone off much higher, BEFORE enrolment. Bound like `_canal_plateforme`
    // and FOR THE SAME REASON: dropping it would end the discovery thread.
    let _apps = apps::brancher(_canal_plateforme.as_mut());

    // The bridge mode captures nothing and launches no one: it holds the ProjFS
    // virtualisation root and serves it from the directory the
    // shell page opened. `PONT=0` DISABLES the mode, like `CAPTEUR=0` and
    // `SUPERVISEUR=0` — same operational trap, same safeguard: testing
    // `is_ok()` would mean that writing `PONT=0` to TURN OFF the bridge would turn it on.
    //
    // Placed AFTER `CAPTEUR` — a bridge that inherited `CAPTEUR` would become
    // a sensor, hence the `env_remove("CAPTEUR")` of `lancer_pont` — and BEFORE
    // the supervisor branch, hence the `env_remove("PONT")` of `lancer`.
    //
    // ⚠️ **But AFTER THE ENROLMENT above, and it is an assumed DIVERGENCE
    // from F1's plan**, which wrote "after `CAPTEUR`, before
    // `superviseur`" at a time when these two branches were adjacent. Sub-block
    // P3 inserted enrolment between them, and the bridge needs
    // it: it opens its OWN `PeerConnection` to the shell page, so it
    // presents a token, exactly like a child. Placing it before
    // enrolment would have left it `config.jeton = None`, the platform
    // would have refused the handshake, and **no session would have been
    // established** — without anything linking the failure to the placement of an `if`.
    //
    // The sensor, for its part, does go off BEFORE enrolment, and it is consistent:
    // it talks to no signaling.
    if matches!(std::env::var("PONT").as_deref(), Ok(v) if v != "0") {
        return pont::executer(config).await;
    }

    // The supervisor mode captures nothing: it detects windows and launches
    // one child per window. Its children NEVER inherit `SUPERVISEUR`
    // (see `superviseur::lanceur`), otherwise each would take itself for a
    // supervisor and launch its own, indefinitely.
    if config.superviseur {
        // 🔴 THE IDENTITY WATCH, AND NOT `config.jeton`, IS WHAT GOES TO THE
        // LAUNCHER. A supervisor lives for hours; the agent token, for its part, lasts
        // ten minutes and is renewed at each heartbeat. Passing the start-up
        // snapshot would mean that a window opened an hour later
        // would receive a dead token, which the platform's guard would refuse —
        // and no session would be established, without any trace linking
        // the failure to the age of a variable.
        let veille = _canal_plateforme
            .as_ref()
            .map(plateforme::Canal::veille_identite);
        return superviseur::executer(config, veille).await;
    }

    demarrage::executer(config).await
}
