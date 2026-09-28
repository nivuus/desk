//! The APPLICATION MANAGEMENT types of the platform <-> agent channel:
//! the application itself, the provenance of its icon, and the outcome of a launch
//! order.
//!
//! 🔴 EXTRACTED FROM `proto/src/plateforme.rs` VERBATIM (sub-block G2), BECAUSE
//! THE 500-LINE CEILING WAS CROSSED — 588 — AND THE REPOSITORY'S DOCTRINE
//! IS TO CATCH UP THROUGH AN EXTRACTION, NEVER THROUGH A COMPRESSION.
//!
//! ⚠️ **THE EXTRACTION SHOULD HAVE PRECEDED THE ADDITION, AND IT DID NOT.**
//! The G2 plan had named three extractions to carry out in advance — the two
//! test files of `proto/` and `routes-applications.test.ts` — and all
//! three were indeed carried out BEFORE their addition. This one was not planned:
//! the file was announced at 433 lines for "+1 enum, +1 variant, +2
//! fields", and the documentation of those additions took it to 588. **The
//! crossing is DECLARED rather than hidden**, as this repository requires of
//! its three crossings of D10 and its two of D9.
//!
//! The boundary is the SAME as that of the two test files, and it is
//! no accident: the protocol already carries this cut — the lifecycle
//! on one side, app management on the other.

use serde::{Deserialize, Serialize};

/// Where the image comes from: the largest entry actually PRESENT in the
/// icon directory of the source (`GRPICONDIR` of a PE module, `ICONDIR`
/// of an `.ico`).
///
/// 🔴 IT IS NOT THE RENDERED SIZE, AND THE TWO MUST NEVER BE
/// CONFLATED. Measured on 20 August 2026 on two crafted witnesses — filed
/// since, in `agent/testdata/g2-temoin-{48,256}.ico`: an `.ico`
/// containing ONLY a 48×48 entry, queried at 256, renders **256×256 32bpp** —
/// through `IShellItemImageFactory::GetImage` as through `PrivateExtractIconsW`,
/// without `SIIGBF_SCALEUP` and **EVEN with `SIIGBF_BIGGERSIZEOK`**, that is
/// explicitly telling the Shell that a larger size would do.
/// **The four render lines of the two witnesses are identical; only the
/// `ICONDIR` line differs.** A criterion that compared the rendered size to
/// 256 THEREFORE CANNOT FAIL.
///
/// ✅ **TESTED ON THE PRODUCT on 21 August 2026**: the two witnesses entered in the
/// real catalogue both render a **256×256** PNG, and `source_max` is
/// `{"pixels":48}` for one and `{"pixels":256}` for the other.
///
/// 🔵 **`NonMesuree` IS WRITTEN IN TWO WORDS, AND IT IS NO ACCIDENT.** The
/// comment on [`IssueLancement`] records a coverage gap: its
/// four variants being single-word, `rename_all` is UNOBSERVABLE there and
/// no test can turn red if the convention changes. `non-mesuree` against
/// `non_mesuree` makes it observable **for this enum**; ⚠️ it stays
/// OPEN for `IssueLancement`, which no G2 task touches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SourceMax {
    /// The largest entry of the icon directory, in pixels.
    ///
    /// ⚠️ `bWidth == 0` MEANS 256 in the format: the field is one byte, and
    /// 256 does not fit in it. The conversion happens in the PURE module
    /// `agent::apps::icone::ressource`, never here.
    Pixels(u16),
    /// 🔴 A VALUE DISTINCT FROM 256, AND IT IS FORBIDDEN TO CONFLATE THEM.
    /// The provenance is neither a PE module nor a readable `.ico`: type
    /// association, Shell namespace, or unreadable resource. **Measured on the
    /// product on 21 August 2026: 71 of the 154 applications of this VM.**
    ///
    /// On the wire, serde makes it the string `"non-mesuree"`: it cannot
    /// structurally be a number, and that is what acceptance criterion ④
    /// asks for.
    NonMesuree,
}

/// An application as the agent discovers it on the VM's disk.
///
/// ⚠️ `arguments` is RAW and CASE-SENSITIVE, unlike `cible` and
/// `repertoire` which are normalised (case folded). It is spec D4: two
/// shortcuts that differ only by the case of a Windows path designate
/// the same file, whereas two command lines that differ only by
/// the case of an argument are two distinct invocations — folding them
/// would merge `-Mode admin` and `-mode Admin`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Application {
    /// Fingerprint of the triplet `(cible, arguments, repertoire)` — the identity.
    pub cle: String,
    /// The name of the `.lnk`, without its extension.
    pub nom: String,
    /// The path of the `.lnk` ITSELF, and it is what gets launched (spec D6).
    pub chemin: String,
    /// The path of the target, normalised.
    pub cible: String,
    /// The arguments, RAW (see above). Empty = `""`, never absent.
    pub arguments: String,
    /// The working directory, normalised.
    pub repertoire: String,
    /// The SHA-256 fingerprint of the icon PNG, in lowercase hexadecimal — or
    /// `None` when the extraction failed.
    ///
    /// ⚠️ AN APPLICATION WITHOUT AN ICON IS BETTER THAN AN ABSENT APPLICATION
    /// (spec §7). `None` is not an error, and is emitted as `"icone":null`.
    ///
    /// ⚠️ **No `#[serde(default)]`, no `skip_serializing_if`**: it is
    /// the rule this module already imposes on itself for the `v` field — an
    /// ABSENT field must be rejected, not silently completed. A `default`
    /// would make a v2 agent catalogue accepted without anything saying so.
    ///
    /// 🔴 AND THAT IS WHY THIS FIELD CARRIES A `deserialize_with` THAT DOES
    /// NOTHING BUT DELEGATE: without it, serde would make the field
    /// optional ALL BY ITSELF, because it is of type `Option`. See
    /// [`super::icone_obligatoire`] and the measurement transcribed there.
    #[serde(deserialize_with = "super::icone_obligatoire")]
    pub icone: Option<String>,
    /// Always present. Is [`SourceMax::NonMesuree`] when `icone` is
    /// `None`, and may also be so when `icone` exists — an icon whose
    /// provenance is not readable.
    ///
    /// ⚠️ The reverse combination — `icone` null and a measured size — is
    /// FORBIDDEN, and no path writes it.
    pub source_max: SourceMax,
    /// The DOMINANT colour of the icon, as `#rrggbb`, or `None`.
    ///
    /// 🔴 IT IS THE "ACCENT COLOUR" THAT THE DESIGN OF ④ ASKS FOR IN §G5,
    /// and it is **PER APPLICATION** — not to be confused with that of
    /// sub-project ①, which is **per WINDOW**, arrives mid-session on
    /// the WebRTC control channel, and does not describe the same thing. Both
    /// are computed by the same pure rule (`agent::accent::dominante`);
    /// it is their SUBJECT that differs.
    ///
    /// ⚠️ `None` IS NOT AN ERROR: an icon too pale, too dark or
    /// too transparent has no dominant, and `dominante` returns `None` by
    /// construction (its clause 5). The manifest then OMITS `theme_color`
    /// rather than inventing one.
    ///
    /// 🔴 SAME `deserialize_with` AS `icone`, AND FOR THE SAME REASON: without
    /// it, serde would make the field optional ALL BY ITSELF because it is of
    /// type `Option`, and an agent catalogue of another version would pass
    /// without anything saying so.
    #[serde(deserialize_with = "super::icone_obligatoire")]
    pub accent: Option<String>,
    /// The extensions this application opens — lowercase, **with** the
    /// dot, sorted and deduplicated.
    ///
    /// 🔴 EXTENSIONS, NEVER MIME TYPES (decision D13 of the G5 plan).
    /// Shipping the MIME would double the table — Rust **and** TypeScript —
    /// for a datum that is **not a property of the VM**: it is a
    /// Web convention. The extension → MIME map lives **only once**,
    /// on the platform side, at the place that writes the manifest.
    ///
    /// ⚠️ EMPTY IS A NORMAL STATE, NOT A FAILURE: most applications
    /// open no file type. The field stays PRESENT on the wire.
    ///
    /// ⚠️ THE ORDER IS IMPOSED BY THE AGENT (`apps::associations::ranger`) and it
    /// is not decorative: the platform compares the received catalogue to the one
    /// it knows, and two IDENTICAL sets in a different order would
    /// make it write on every round.
    pub associations: Vec<String>,
}

/// What a launch order really did.
///
/// 🔴 `Raccourci` AGAINST `Cible` IS WHAT MAKES THE ACCEPTANCE CRITERION
/// DECIDABLE: launching through the rebuilt target instead of the `.lnk` would pass a
/// criterion that would only say "something launched". Naming the path
/// taken tells the two apart without having to be clever.
///
/// ⚠️ IT IS NOT A [`super::MotifCanal`], and reusing it would be a defect:
/// two values of `MotifCanal` CLOSE the socket, and a failed launch must
/// close no channel.
///
/// ⚠️ **COVERAGE GAP, RECORDED RATHER THAN ENDURED (G1 acceptance run, 20 August
/// 2026).** The `rename_all` below is UNOBSERVABLE on this enum: its
/// four variants are SINGLE-WORD, so `kebab-case`, `snake_case`,
/// `lowercase` and `camelCase` all four produce the same strings.
/// **No test can therefore turn red if the naming convention changes here**
/// — checked by mutation: replacing `kebab-case` by `snake_case` on this
/// enum leaves `cargo test -p proto` at **75 passed, 0 failed**.
///
/// **The contrast is measured on the same module**: the same mutation applied
/// to the enum that carries `BattementRecu` — two words, so `battement-recu` against
/// `battement_recu` — makes `conformance_to_the_shared_vectors` FAIL. The
/// protection thus does exist for compound variants, and not for
/// these ones.
///
/// ✅ **AND SUB-BLOCK G2 ADDS A SECOND WITNESS**: [`SourceMax`] carries
/// `NonMesuree`, two words, whose case is tested. **The gap of THIS
/// enum stays whole** — the first variant of `IssueLancement` written in
/// two words will close it by itself, and until then any change to this
/// line must be reread by hand.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum IssueLancement {
    /// The `.lnk` itself was executed. It is the nominal path.
    Raccourci,
    /// The `.lnk` failed (gone, unreadable) and the recorded target took
    /// over.
    Cible,
    /// The key is in no catalogue of the agent. Returned by the caller,
    /// which alone knows the current catalogue.
    Inconnue,
    /// The shortcut AND the target failed. Both attempts are
    /// logged: a typed outcome, never a silence.
    Echec,
}
