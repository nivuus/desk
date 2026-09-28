//! The accent colour of a window — sub-block **A1**.
//!
//! **PURE, no `cfg`**: this whole file compiles and is tested on the Linux host.
//! The Windows half — reading the icon of an `hwnd` — lives in `accent/win32.rs`,
//! which makes **no decision**: it returns bytes, and it is here that the
//! decision is made.
//!
//! **Module convention** (`CLAUDE.md`, § "Child module convention"):
//! `accent` prefixes no existing top-level module, so it lives at the
//! **bare root** — an ordinary `mod accent;` in `main.rs`. Its child
//! `win32` is declared by an ordinary `mod` **inside** it: it never
//! needs to leave its parent's tree, so the `#[path]` rule
//! is **out of scope**. This is what `presse_papier` already does.
//!
//! ⚠️ **NONE of the five constants of `dominante` is CALIBRATED**, nor is
//! `PERIODE_ACCENT`. They join the list this repository has kept since
//! `BPP_MIN`: no visual judgement has been made on any of them, and sub-block
//! A1 makes none either.

use std::sync::OnceLock;
use std::time::Duration;

/// The Win32 reading of the icon — **no decision lives there**.
#[cfg(windows)]
pub mod win32;

#[cfg(test)]
mod tests;

/// The icon re-read step, on the sensor's **window thread**.
///
/// ⚠️ **This is NOT the wheel tick.** Spec D9 prescribed re-reading
/// the icon on the same tick as the clipboard; **the wheel tick does not have the
/// `hwnd`** (`capteur/sommeil/registre.rs`: none of its fifteen fields
/// carries it, and `inscrire(session, pid)` does not take it). The clipboard lives there
/// because it is **global to the window station** — one resource, one poller;
/// the accent is **per window**, and that is precisely the property D9
/// claims. See D-A1-1 of the plan.
///
/// ⚠️ **NOT CALIBRATED.**
pub const PERIODE_ACCENT: Duration = Duration::from_secs(5);

/// Below this opacity, a pixel does not count.
///
/// An icon is **mostly transparent**: counting its empty pixels
/// would drown any hue. ⚠️ **NOT CALIBRATED.**
const ALPHA_MIN: u8 = 128;

/// Below this per-pixel `max − min` spread, the colour is **achromatic**
/// and does not count: this is what keeps a flat grey from winning.
/// ⚠️ **NOT CALIBRATED.**
const SATURATION_MIN: u8 = 32;

/// Outside this luminance band, a pixel does not count: this is what
/// keeps a **dominant dark outline** from beating the hue of
/// the icon. ⚠️ **NOT CALIBRATED.**
const LUMA_MIN: u8 = 32;
/// Voir [`LUMA_MIN`].
const LUMA_MAX: u8 = 224;

/// The quantisation step, per channel: the retained pixels are sorted into
/// buckets of `PAS³`, and the most populated bucket decides.
/// ⚠️ **NOT CALIBRATED.**
const PAS: u16 = 32;

/// Perceived luminance, as an integer, on the ITU-R BT.601 coefficients.
fn luma(r: u8, g: u8, b: u8) -> u8 {
    ((r as u32 * 299 + g as u32 * 587 + b as u32 * 114) / 1000) as u8
}

/// Converts **BGRA to RGBA in place**, by SWAPPING the red and
/// blue channels and NOT touching alpha.
///
/// 🔴 IT EXISTS TO BE TESTED, AND THAT IS ITS WHOLE PURPOSE. The conversion
/// lived identically in `accent/win32.rs`, behind a `#[cfg(windows)]`,
/// and **was covered by nothing** — that is legacy RA1-6 of sub-project ①.
/// Sub-block G5 needed it a SECOND time, for the icon of an
/// application: **writing a second copy would have duplicated a rule that
/// nobody checked**. It is therefore extracted rather than copied, and the
/// original site CALLS it.
///
/// ⚠️ GETTING THE DIRECTION WRONG WOULD SWAP RED AND BLUE — a
/// **plausible and silent** defect, which would live behind the `#[cfg(windows)]` where
/// no host test would see it. That is precisely why the rule
/// moves down here.
///
/// ⚠️ ALPHA IS NOT TOUCHED: it is what the `ALPHA_MIN` filter of
/// [`dominante`] consumes, and swapping it with a colour channel would make that
/// filter absurd without any test saying so.
///
/// A slice whose length is not a multiple of 4 has its remainder
/// **left as is** — `as_chunks_mut` returns it apart and it is ignored. This is not a
/// convenient silence: a badly sized buffer is refused further on by [`dominante`],
/// which compares the length with the product `largeur × hauteur × 4`.
pub fn bgra_en_rgba(tampon: &mut [u8]) {
    let (pixels, _reste) = tampon.as_chunks_mut::<4>();
    for pixel in pixels {
        pixel.swap(0, 2);
    }
}

/// La couleur dominante d'une tranche **RGBA**, ou `None`.
///
/// ⚠️ **RGBA, not BGRA.** `GetDIBits` returns **BGRA**: the conversion
/// belongs to `accent/win32.rs`, never here. Getting the direction wrong would swap
/// red and blue — a **plausible and silent** defect, which no
/// host test would see since it would live behind the `#[cfg(windows)]`.
///
/// The five clauses, and each one is a test:
/// 1. **too transparent** pixels do not count (`ALPHA_MIN`);
/// 2. **non-chromatic** pixels do not count — too little saturated
///    (`SATURATION_MIN`), too dark or too light (`LUMA_MIN`/`LUMA_MAX`);
/// 3. the survivors are **quantised** into buckets of `PAS`;
/// 4. the **AVERAGE of the pixels of the most populated bucket** is returned — never the
///    centre of the bucket: the average gives a hue **really in the image**, the
///    centre gives a hue **of the grid**;
/// 5. `None` if no pixel survives. **`None` is NOT an error**: it means
///    "no accent", and no announcement goes out. It must remain
///    **reachable**, otherwise clause 2 would be an ornament.
///
/// On equal population, the bucket with the smallest key wins: the
/// result is **deterministic**, which a test requires.
pub fn dominante(rgba: &[u8], largeur: u32, hauteur: u32) -> Option<[u8; 3]> {
    let attendu = (largeur as usize)
        .checked_mul(hauteur as usize)?
        .checked_mul(4)?;
    if attendu == 0 || rgba.len() != attendu {
        return None;
    }

    // bucket key -> (sum_r, sum_g, sum_b, count)
    type Cle = (u16, u16, u16);
    type Sommes = (u64, u64, u64, u64);
    let mut seaux: std::collections::BTreeMap<Cle, Sommes> = std::collections::BTreeMap::new();

    let (pixels, _) = rgba.as_chunks::<4>();
    for pixel in pixels {
        let (r, g, b, a) = (pixel[0], pixel[1], pixel[2], pixel[3]);
        if a < ALPHA_MIN {
            continue;
        }
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        if max - min < SATURATION_MIN {
            continue;
        }
        let l = luma(r, g, b);
        if !(LUMA_MIN..=LUMA_MAX).contains(&l) {
            continue;
        }
        let cle = (r as u16 / PAS, g as u16 / PAS, b as u16 / PAS);
        let seau = seaux.entry(cle).or_insert((0, 0, 0, 0));
        seau.0 += r as u64;
        seau.1 += g as u64;
        seau.2 += b as u64;
        seau.3 += 1;
    }

    let (_, (sr, sg, sb, n)) = seaux.iter().max_by_key(|(cle, (_, _, _, n))| {
        // `max_by_key` returns the LAST maximum: the key is inverted so that the
        // bucket with the smallest key wins ties.
        (*n, std::cmp::Reverse(**cle))
    })?;
    if *n == 0 {
        return None;
    }
    Some([(sr / n) as u8, (sg / n) as u8, (sb / n) as u8])
}

/// `#rrggbb`, **six lowercase hexadecimal digits**, and nothing else.
///
/// 🔴 **The format is a constraint of the DESIGN SYSTEM, not of the protocol**, and a
/// successor who ignored it would widen it without knowing:
/// `client/src/design/contraste.ts::luminanceRelative` only accepts `#rgb`,
/// `#rgba`, `#rrggbb` and `#rrggbbaa`, and **THROWS** on everything else. The client
/// defends itself (`client/src/accent.ts` refuses any other shape **before** calling
/// `rapportDeContraste`), but the agent has no reason to send it a
/// shape it will have to throw away.
pub fn en_hexa(rgb: [u8; 3]) -> String {
    format!("#{:02x}{:02x}{:02x}", rgb[0], rgb[1], rgb[2])
}

/// Tracks the accent colour of a window and only announces CHANGES —
/// **its first reading included**.
///
/// 🔴 **This is the OPPOSITE of `plein_ecran::SuiviBordure`, and it is deliberate.**
/// That one is built from **the state read at opening**, precisely
/// so as to announce **nothing** on the first tick: D8 wanted "an application
/// born without a border to announce nothing". The accent has the **opposite** need — the
/// browser must receive the initial colour, otherwise `--accent-fenetre` is
/// never set during the session.
///
/// ⚠️ **This is what makes criterion ④ judgeable**: "no message as long as
/// the icon does not change" is counted **AFTER** the first announcement, and the survey
/// must show *exactly one* announcement in steady state. **A criterion that
/// required zero messages would be met by an entirely dead mechanism.**
///
/// ⚠️ **And this is what makes replaying at registration useless**: a
/// re-attachment recreates the window thread on the sensor side, hence a new `SuiviAccent`,
/// hence a first announcement. Legacy no. 3 of P1 — the current state at
/// attach time, which cost P3 two tasks — **has no equivalent here**.
#[derive(Default)]
pub struct SuiviAccent {
    derniere: Option<String>,
}

impl SuiviAccent {
    /// A new tracker has seen **nothing**: its first successful reading is announced.
    pub fn neuf() -> Self {
        Self::default()
    }

    /// Returns `Some(couleur)` on a change — **first reading included** —,
    /// `None` otherwise.
    pub fn observer(&mut self, couleur: &str) -> Option<String> {
        if self.derniere.as_deref() == Some(couleur) {
            return None;
        }
        self.derniere = Some(couleur.to_string());
        Some(couleur.to_string())
    }
}

/// `ACCENT=0` disarms the **WHOLE** mechanism.
///
/// ⚠️ **`=0` DISABLES; mere PRESENCE does not enable** — the convention of
/// `PLEIN_ECRAN`, `AUDIO`, `SUPERVISEUR`, `CAPTEUR`, `PART_SONDAGE`,
/// `PRESSE_PAPIER` and `APPS`, **and for the same reason**: testing `is_ok()`
/// would arm the mechanism when `ACCENT=0` is written to turn it off.
///
/// 🔴 **It is tested BEFORE any Win32 read**, as `Sondeur::tour` does
/// for the clipboard: `ACCENT=0` must prevent even the
/// `SendMessageTimeout`, not only the send.
///
/// `OnceLock` rather than a read per call: the re-read runs at 0.2 Hz, and
/// the environment does not change during the process.
pub fn actif() -> bool {
    static ACTIF: OnceLock<bool> = OnceLock::new();
    *ACTIF.get_or_init(|| {
        let actif = std::env::var("ACCENT").as_deref() != Ok("0");
        if !actif {
            tracing::warn!(
                "window accent DISARMED (ACCENT=0): the icon colour \
                 is no longer pushed to the browser"
            );
        }
        actif
    })
}
