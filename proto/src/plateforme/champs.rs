//! The three field readers that `serde` calls through `deserialize_with`, and
//! what each one refuses.
//!
//! 🔴 **EXTRACTED BEFORE THE ADDITION, NEVER AFTER.** Sub-block G3 adds five
//! variants to the protocol; `plateforme.rs` was at 468 lines, margin 32, and
//! would have crossed 500 with them. The doctrine of `CLAUDE.md` is to give back the
//! margin through an **extraction carried out in advance**, never through a compression —
//! and sub-block G2 paid, a few hours earlier, for not having seen it
//! coming on this very file (it crossed it at 588 then extracted `apps`).
//!
//! ⚠️ It is NOT the "Child module convention" of `CLAUDE.md`, which targets
//! modules extracted from a `#[cfg(windows)]` parent: it is the same mechanism
//! used for the other reason — the 500-line rule.
//!
//! 🔴 **NO LINE OF BEHAVIOUR HAS CHANGED**, and the three functions are
//! transposed word for word. The parent re-imports them through a `use`, which means
//! the `deserialize_with = "verifie_version"` attributes of the structures
//! **have not moved by one character**: `serde` resolves the path in the scope
//! of the module carrying the attribute, and a `use` is enough to put it back there.

use serde::Deserialize;

use super::PLATEFORME_VERSION;

// Note: no `default` on the `v` field — a message without a `v` field must be
// rejected (mandatory field), not silently filled in with the current
// version. `default` would short-circuit `deserialize_with` when the field is
// absent, which would break the check.
pub(super) fn verifie_version<'de, D>(deserializer: D) -> Result<u8, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let v = u8::deserialize(deserializer)?;
    if v != PLATEFORME_VERSION {
        return Err(serde::de::Error::custom(format!(
            "version de plateforme non supportée : {v}"
        )));
    }
    Ok(v)
}

/// Reads the `v` field of a REFUSAL **without checking it** — see clause 1 of
/// the header of this module.
///
/// 🔴 IT IS NOT "WITHOUT `v`": the field stays mandatory and stays an
/// integer. Making it optional would reopen the hole that
/// [`verifie_version`] refuses — a `v: null`, or an absent `v`, would become
/// acceptable — and would deprive the log of the only information that says
/// WHICH version refuses us.
pub(super) fn version_toleree<'de, D>(deserializer: D) -> Result<u8, D::Error>
where
    D: serde::Deserializer<'de>,
{
    u8::deserialize(deserializer)
}

/// Reads an `Option<String>` while keeping it **MANDATORY on the wire**.
///
/// 🔴 WITHOUT THIS FUNCTION, THE FIELD WOULD BE SILENTLY OPTIONAL, AND THE
/// PLANNING OF G2 WAS WRONG ON THIS PRECISE POINT. `serde_derive` treats
/// any field of type `Option<T>` as carrying an IMPLICIT
/// `#[serde(default)]`: an absent field becomes `None` without any `default` having
/// been written, and `deny_unknown_fields` changes nothing about it — it looks at the
/// EXTRA fields, never at the missing ones.
///
/// **Measured on 20 August 2026**, two structures identical except for this detail,
/// `deny_unknown_fields` on both:
/// ```text
/// bare Option<String>                         -> `{"a":1}` ACCEPTED
/// Option<String> + deserialize_with           -> `{"a":1}` REFUSED
/// Option<String> + deserialize_with, `b:null` -> Ok(None), and serializes `"b":null`
/// ```
/// The only thing `deserialize_with` changes is therefore the implicit: it
/// cuts the default, and the field becomes required again.
///
/// **What would be lost without it**: the catalogue of a v2 agent — six fields,
/// without `icone` — would be accepted by a v3 platform, with an icon
/// silently absent. It is exactly the disguise that the version bump
/// exists to prevent, and the rule this module already imposes on itself for
/// the `v` field: **an absent field is refused, it is not completed**.
pub(super) fn icone_obligatoire<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    // ⚠️ DELEGATES TO THE GENERIC FORM BELOW since sub-block G3, which
    // needed it for an `Option<i32>` and for a second `Option<String>`.
    // The original name is KEPT because it is cited by the
    // `deserialize_with = "icone_obligatoire"` attribute of the field it guards, and
    // renaming it would have been an addition of risk for a gain in style.
    option_obligatoire(deserializer)
}

/// The GENERIC form of the function above: makes a field of type
/// `Option<T>` **mandatory on the wire**, whatever `T`.
///
/// 🔴 WITHOUT IT, THE FIELD WOULD BE SILENTLY OPTIONAL. The whole reasoning
/// is written above, for `icone` — it does not depend on the type at all, and
/// it holds word for word for the `motif` and the `code_sortie` of sub-block G3:
/// a `termine` without `motif` would be accepted with an absent reason, which is
/// exactly the disguise that the version bump exists to prevent.
///
/// ⚠️ `null` STAYS ACCEPTED, AND IT IS INTENDED: "the field is there and it carries
/// nothing" is one fact, "the field is missing" is another. It is that
/// distinction alone that this function restores.
pub(super) fn option_obligatoire<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer)
}
