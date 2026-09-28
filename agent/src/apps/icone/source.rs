//! Where the icon of a shortcut comes from: its `IconLocation`, or its target.
//!
//! **PURE, with no `cfg`.** It splits a string and looks at an extension;
//! it opens nothing.
//!
//! 🔴 THE RULE "EMPTY PATH ⇒ THE TARGET CARRIES THE ICON" IS NOT A
//! DETAIL. Measured on 20 August 2026: **92 of the 153 retained shortcuts of the
//! development VM** carry an `IconLocation` WITHOUT a path — the specification
//! notes 135 of 218 before filtering. Treating them as "no icon"
//! would lose the icon of **more than one application in two, silently**.
//!
//! That is exactly where the icons of the legacy product got lost:
//! `convertToLinuxPath('')` returns the empty string (`src/lnkParser.js:172`, cited
//! by the specification).

/// Where to read the icon directory from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Provenance {
    /// Un module PE — `.exe`, `.dll`, `.mun` : sa ressource `RT_GROUP_ICON`.
    Module(String),
    /// A standalone `.ico`: its `ICONDIR`, that is its first bytes.
    Ico(String),
    /// 🔴 NOTHING READABLE, AND IT IS NOT AN ERROR. A type
    /// association, a Shell namespace, an MSI installer `ProductIcon` without
    /// extension. The image will be extracted anyway — the Shell can render it —
    /// but its PROVENANCE will stay `SourceMax::NonMesuree`. **Measured: 37 of the
    /// 153 applications of this VM.**
    Absent,
}

/// ⚠️ THE LAST COMMA, NEVER THE FIRST.
///
/// A Windows path may contain one — `C:\Program Files\Machin, Inc\a.exe`
/// is perfectly legal — and splitting on the first would return
/// `C:\Program Files\Machin` as the path and ` Inc\a.exe,0` as the index. The
/// format is `<path>,<index>`: it is the LAST comma that separates.
fn couper(icon_location: &str) -> (&str, Option<&str>) {
    match icon_location.rfind(',') {
        Some(i) => (&icon_location[..i], Some(&icon_location[i + 1..])),
        None => (icon_location, None),
    }
}

/// Where the icon comes from, given the `IconLocation` of the `.lnk` and the target.
pub fn provenance(icon_location: &str, cible: &str) -> Provenance {
    let (chemin, _) = couper(icon_location);
    // 🔴 THE 92 OUT OF 153: an empty path — including the entirely empty string,
    // and a lone `,0` — points back to the TARGET.
    let chemin = if chemin.trim().is_empty() {
        cible
    } else {
        chemin
    };
    let chemin = chemin.trim();
    if chemin.is_empty() {
        return Provenance::Absent;
    }
    match extension(chemin).as_deref() {
        Some("ico") => Provenance::Ico(chemin.to_string()),
        // `.mun` is the resource container Windows 10+ uses for
        // system icons (`imageres.dll` points to it).
        Some("exe" | "dll" | "mun" | "cpl" | "scr" | "ocx") => {
            Provenance::Module(chemin.to_string())
        }
        // ⚠️ WITHOUT AN EXTENSION, WE CANNOT READ — and saying so is better than
        // guessing. `C:\Windows\Installer\{1BEA…}\ProductIcon` is the most
        // frequent case of this corpus.
        _ => Provenance::Absent,
    }
}

/// The extension in lowercase, or `None` — never that of a parent directory.
fn extension(chemin: &str) -> Option<String> {
    let last = chemin.rsplit(['\\', '/']).next()?;
    let point = last.rfind('.')?;
    if point + 1 >= last.len() {
        return None;
    }
    Some(last[point + 1..].to_ascii_lowercase())
}

/// The index of the icon in the module, `0` by default.
///
/// ⚠️ IT MAY BE NEGATIVE: a negative index designates a resource by its
/// IDENTIFIER and not by its rank, and it is a common usage of
/// `imageres.dll`. Hence the `i32`, never a `u32`.
pub fn index(icon_location: &str) -> i32 {
    match couper(icon_location) {
        (_, Some(i)) => i.trim().parse().unwrap_or(0),
        (_, None) => 0,
    }
}

#[cfg(test)]
#[path = "source/tests.rs"]
mod tests;
