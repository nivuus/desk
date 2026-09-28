//! The agent's configuration, read from the environment.
//!
//! **Extracted from `main.rs` VERBATIM on 20 August 2026**, before the additions of
//! clipboard sub-block P1, because `main.rs` was at **513 lines** —
//! above the 500 cap of `CLAUDE.md`, and absent from its debt
//! table. Commit `264c275` ("a single `/agent` channel per VM") had taken it
//! from 470 to 505 without declaring it, and clipboard task 4 had added
//! 8 more lines to it. The repository's rule is to **extract before adding**,
//! never to compress a comment to get back under the line.
//!
//! The split follows responsibility: `main.rs` only keeps the declaration
//! of the modules and the routing of the modes (sensor, bridge, supervisor,
//! single-window); reading and validating the environment live here.
//! No call site moved — `Config` stays `crate::Config`, re-exported by
//! `main.rs`. The only change to the moved text is **visibility**:
//! `pub(crate)` on the type, its fields and the two functions `main.rs`
//! still calls, otherwise they would be invisible from their own
//! crate.

use std::net::IpAddr;
use std::path::PathBuf;

use anyhow::{Context, Result};

// Imported rather than qualified `crate::demarrage::…` at its call site: the
// moved text thus stays VERBATIM, visibility aside.
use crate::demarrage;

/// Agent configuration, read from the environment.
pub(crate) struct Config {
    pub(crate) signaling_url: String,
    pub(crate) session_id: String,
    pub(crate) local_ip: IpAddr,
    pub(crate) test_file: Option<PathBuf>,
    /// True in supervisor mode: this process captures nothing, it detects the
    /// windows and launches one child per window.
    pub(crate) superviseur: bool,
    /// `HWND` of the window to capture, in decimal or hexadecimal prefixed
    /// with `0x`. Set by the supervisor on its children; absent, the agent
    /// falls back on the search by title (`WINDOW_TITLE`), that is on
    /// the single-window behaviour from before this sub-block.
    ///
    /// The four fields that follow are only read by the Windows branch of
    /// `demarrage`: on the Linux host they are dead by construction, and
    /// the `allow` says so rather than letting a warning settle in.
    #[cfg_attr(not(windows), allow(dead_code))]
    pub(crate) fenetre_hwnd: Option<u64>,
    /// DXGI output to capture, designated by its name (`\\.\DISPLAYn`). Absent,
    /// the agent captures the desktop and crops the window.
    #[cfg_attr(not(windows), allow(dead_code))]
    pub(crate) sortie_dxgi: Option<String>,
    /// The KEPT size (`superviseur::placement::taille_retenue`) at
    /// which the supervisor placed this window — set by it alone
    /// (`lanceur.rs`), never by an operator. Absent — the
    /// single-window path, where `sortie_dxgi` is too —, the size requested from the
    /// sensor at attach stays `(u32::MAX, u32::MAX)`
    /// (`capteur::tube::connecter`), and `taille_retenue` brings it back as
    /// is to the output size: the behaviour from before this
    /// sub-block, unchanged.
    #[cfg_attr(not(windows), allow(dead_code))]
    pub(crate) taille_fenetre: Option<(u32, u32)>,
    /// False when `AUDIO=0` mutes this agent's sound.
    ///
    /// **GLOBAL switch, no longer a per-window instruction.** Until
    /// sub-block D7, the supervisor set `AUDIO=0` on all children but
    /// one, because a single session loopback would have been captured eight times.
    /// Each child now captures the sound of its OWN process, and it is
    /// the sensor that arbitrates between the windows of the same process: there is
    /// nothing left to reserve.
    ///
    /// **An agent launched by hand mutes its sound via `AUDIO=0
    /// scripts/run-agent.sh`**, which passes the variable as `$env:AUDIO` in
    /// the PowerShell bootstrap script (`schtasks` does not pass
    /// the environment directly). This transport was missing for a while: removing
    /// the supervisor's setter first left `AUDIO` with no path at all
    /// to the child, `run-agent.sh` not carrying it — fixed in the
    /// same task 9, before this comment was read by anyone.
    #[cfg_attr(not(windows), allow(dead_code))]
    pub(crate) audio: bool,
    /// False when `MICRO=0` mutes this agent's microphone.
    ///
    /// ⚠️ **Convention of `AUDIO`, `SUPERVISEUR`, `PLEIN_ECRAN` and `CAPTEUR` —
    /// and therefore the REVERSE of that of `micro_mesure` just below, on
    /// purpose**: we disarm on `=0` what is SHIPPED, we arm on `=1` what
    /// is not. The mic has been shipped since block E2; the measurement sink
    /// never will be.
    ///
    /// The decision lives in `demarrage::micro::arme_micro`, where a test
    /// guards it: **never `is_ok()`**, otherwise someone writing
    /// `MICRO=0` to be sure to turn it off would turn it on.
    pub(crate) micro: bool,
    /// True when `MICRO_MESURE=1` arms the mic measurement sink
    /// (work stream E, `demarrage/micro.rs`).
    ///
    /// ⚠️ **REVERSE CONVENTION of `AUDIO`, `SUPERVISEUR`, `PLEIN_ECRAN` and
    /// `CAPTEUR`, on purpose**: we disarm on `=0` what is SHIPPED, we
    /// **arm on `=1`** what is not. This sink is a bench
    /// instrument — it consumes the upstream flow to log it, it plays it
    /// nowhere —, and a mere presence of the variable is therefore not enough:
    /// the value `1` is required. It is the convention
    /// `PLEIN_ECRAN_MODE_SORTIE` had, a variable removed by sub-block D9 and which it
    /// is therefore pointless to look for in the code.
    pub(crate) micro_mesure: bool,
    /// The name of the VM enrolled with the platform, and its enrolment
    /// secret (sub-block P3). **Both or neither**: it is the pair
    /// the `/agent` channel presents.
    ///
    /// ⚠️ **ABSENT = no agent token, hence NO session.** Since P3 the
    /// platform's guard refuses an anonymous `{"role":"agent"}`, and there
    /// is no permissive switch. Their absence is therefore NOT a fallback
    /// mode: it is a failure, announced by a `warn!` that names it, and not
    /// a start-up failure — the `diagnostics` probes (`MULTIFENETRE_*`)
    /// open no signaling and must keep running without them.
    pub(crate) agent_vm: Option<String>,
    pub(crate) agent_secret: Option<String>,
    /// The session prefix delivered at enrolment, and the agent token that
    /// opens both handshakes. Filled by `main` AFTER
    /// start-up, never by `config()`: obtaining them requires a socket.
    pub(crate) prefixe: String,
    pub(crate) jeton: Option<String>,
}

pub(crate) fn config() -> Result<Config> {
    Ok(Config {
        signaling_url: std::env::var("SIGNALING_URL")
            .unwrap_or_else(|_| "ws://127.0.0.1:8080".into()),
        session_id: std::env::var("SESSION_ID").unwrap_or_else(|_| "demo".into()),
        local_ip: std::env::var("LOCAL_IP")
            .unwrap_or_else(|_| "127.0.0.1".into())
            .parse()
            .context("LOCAL_IP n'est pas une adresse IP valide")?,
        test_file: std::env::var("TEST_FILE").ok().map(PathBuf::from),
        // `SUPERVISEUR=0` DISABLES the mode, as `AUDIO=0` disables sound.
        // A mere presence (`is_ok()`) would make writing `SUPERVISEUR=0`
        // to turn it off enable it — an operational trap all the more
        // certain since the neighbouring variable is indeed read that way.
        superviseur: matches!(std::env::var("SUPERVISEUR").as_deref(), Ok(v) if v != "0"),
        // ABSENT: legitimate single-window mode, no noise. PRESENT BUT
        // MALFORMED: start-up failure, never a silent fallback — see
        // `analyser_hwnd`.
        fenetre_hwnd: match std::env::var("FENETRE_HWND") {
            Ok(brut) => Some(analyser_hwnd(&brut)?),
            Err(_) => None,
        },
        // ABSENT: legitimate single-window mode. PRESENT BUT EMPTY: start-up
        // failure, never a silent fallback — same rule as `FENETRE_HWND`.
        // No more `adaptateur:sortie` parsing: it is a DXGI output NAME
        // (`\\.\DISPLAYn`), stable where indices are positional.
        sortie_dxgi: match std::env::var("SORTIE_DXGI") {
            Ok(brut) => {
                let nom = brut.trim().to_string();
                anyhow::ensure!(!nom.is_empty(), "SORTIE_DXGI est vide");
                Some(nom)
            }
            Err(_) => None,
        },
        // ABSENT: legitimate single-window mode, no noise — same case as
        // `SORTIE_DXGI`. PRESENT BUT MALFORMED: start-up failure, same
        // rule as `FENETRE_HWND` and `SORTIE_DXGI` — this variable is only
        // set by the supervisor (`lanceur.rs`, `WxH` form), so an
        // unreadable value signals a supervisor bug, not an operator
        // input to tolerate silently.
        taille_fenetre: match std::env::var("TAILLE_FENETRE") {
            Ok(brut) => {
                let (l, h) = brut
                    .split_once('x')
                    .with_context(|| format!("TAILLE_FENETRE « {brut} » : format attendu LxH"))?;
                let largeur: u32 = l
                    .parse()
                    .with_context(|| format!("TAILLE_FENETRE « {brut} » : largeur illisible"))?;
                let hauteur: u32 = h
                    .parse()
                    .with_context(|| format!("TAILLE_FENETRE « {brut} » : hauteur illisible"))?;
                Some((largeur, hauteur))
            }
            Err(_) => None,
        },
        // Sound is on by default: an agent launched by hand must
        // get this behaviour back. `AUDIO=0` DISABLES, like
        // `SUPERVISEUR=0`: a mere presence (`is_ok()`) would enable sound
        // when writing `AUDIO=0` to mute it — same trap as documented
        // above for `SUPERVISEUR`. Since task 9 of sub-block D7, the
        // supervisor no longer sets this variable on its children: each one
        // captures the sound of ITS OWN process (task 7), and it is the sensor
        // that arbitrates between the windows that share one.
        audio: std::env::var("AUDIO").as_deref() != Ok("0"),
        // The decision lives in `demarrage::micro::arme`, where a test guards it:
        // a variable set to `0`, empty, or anything else
        // leaves the sink DISARMED, like its absence.
        // The mic has been on by default since block E2: an agent launched by
        // hand must get the shipped behaviour back. `MICRO=0` DISABLES,
        // like `AUDIO=0` — and the predicate lives in `demarrage::micro`, where a
        // test forbids the `is_ok()` that would invert the variable's meaning.
        micro: demarrage::micro::arme_micro(std::env::var("MICRO").ok().as_deref()),
        micro_mesure: demarrage::micro::arme(std::env::var("MICRO_MESURE").ok().as_deref()),
        // An empty value counts as absence: `run-agent.sh` only writes the line
        // if the variable is defined, but an `AGENT_VM=` set by hand
        // would otherwise give an enrolment with an empty name, which the platform
        // would refuse without anyone knowing why.
        agent_vm: variable_non_vide("AGENT_VM"),
        agent_secret: variable_non_vide("AGENT_SECRET"),
        prefixe: String::new(),
        jeton: None,
    })
}

/// Reads an environment variable, treating the empty string as
/// absence.
pub(crate) fn variable_non_vide(nom: &str) -> Option<String> {
    let valeur = std::env::var(nom).ok()?;
    let valeur = valeur.trim().to_string();
    (!valeur.is_empty()).then_some(valeur)
}

/// Parses an `HWND` as the supervisor sets it on its children:
/// hexadecimal prefixed with `0x` (the form `lanceur.rs` produces), or decimal.
///
/// **Fails loudly rather than returning `None`**, and it is fix I5
/// of the final review. The previous version chained `.ok().and_then(…)`:
/// any malformed value — a forgotten `0x`, a space, an overflow —
/// became indistinguishable from an absent variable, and `demarrage::source`
/// then switched to capturing the whole desktop with cropping, without a word.
/// A supervisor child would thus stream the VM's desktop believing it was
/// showing its window. An ABSENT variable keeps its meaning (single-window mode,
/// search by title); a PRESENT variable must be honoured or refused.
fn analyser_hwnd(brut: &str) -> Result<u64> {
    let texte = brut.trim();
    let valeur = match texte
        .strip_prefix("0x")
        .or_else(|| texte.strip_prefix("0X"))
    {
        Some(hexa) => u64::from_str_radix(hexa, 16)
            .with_context(|| format!("FENETRE_HWND « {texte} » : hexadécimal illisible"))?,
        None => texte
            .parse()
            .with_context(|| format!("FENETRE_HWND « {texte} » : décimal illisible"))?,
    };
    anyhow::ensure!(
        valeur != 0,
        "FENETRE_HWND vaut 0 : aucune fenêtre ne porte ce handle"
    );
    Ok(valeur)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn un_hwnd_hexadecimal_ou_decimal_est_accepte() {
        assert_eq!(analyser_hwnd("0x1a2b").unwrap(), 0x1a2b);
        assert_eq!(analyser_hwnd(" 0x1A2B ").unwrap(), 0x1a2b);
        assert_eq!(analyser_hwnd("6699").unwrap(), 6699);
    }

    /// The heart of I5: each of these values returned `None` — hence "no
    /// imposed window", hence the silent fallback to capturing the desktop.
    #[test]
    fn un_hwnd_mal_forme_fait_echouer_le_demarrage() {
        assert!(analyser_hwnd("").is_err(), "vide");
        assert!(analyser_hwnd("0x").is_err(), "préfixe seul");
        assert!(analyser_hwnd("0xzz").is_err(), "pas de l'hexadécimal");
        assert!(analyser_hwnd("1a2b").is_err(), "hexadécimal sans préfixe");
        assert!(analyser_hwnd("-1").is_err(), "négatif");
        assert!(analyser_hwnd("0").is_err(), "handle nul");
    }
}
