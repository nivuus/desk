// The payload types of APP MANAGEMENT, on the TypeScript side.
//
// 🔴 EXTRACTED BEFORE THE ADDITION, NEVER AFTER, and it is the exact mirror of what
// sub-block G2 did on the Rust side (`proto/src/plateforme/apps.rs`) — except
// that it did so AFTER crossing 500 lines, and declared it.
// `plateforme.ts` was at 475 lines, margin 25, and sub-block G3 adds
// three messages, two enums and their encoders: it would have crossed.
// The doctrine of `CLAUDE.md` is to restore the margin through an extraction played
// AHEAD, never through compression.
//
// 🔴 NO LINE OF BEHAVIOUR HAS CHANGED. The three types are transposed
// word for word, and `plateforme.ts` RE-EXPORTS them — so that the ten
// importers found in `plateforme/` and `proto/` did not have to move
// by one character. It is the same figure as the `pub use apps::{…}` of the Rust
// parent.
//
// ⚠️ THIS FILE MUST IMPORT NEITHER `node:` NOR ANY DOM: it is loaded by the
// service AND by the browser.

/**
 * An application as the agent discovers it on the VM disk.
 *
 * ⚠️ `arguments` is RAW and CASE-SENSITIVE, unlike `cible` and
 * `repertoire` which are normalised. Two Windows paths that differ only
 * by case designate the same file; two command lines that differ
 * only by the case of an argument are two distinct invocations.
 */
export interface Application {
    /** Fingerprint of the `(cible, arguments, repertoire)` triple — the identity. */
    cle: string;
    /** The name of the `.lnk`, without its extension. */
    nom: string;
    /** The path of the `.lnk` ITSELF, and that is what gets launched. */
    chemin: string;
    cible: string;
    /** BRUTS (voir ci-dessus). Vide = `''`, jamais absent. */
    arguments: string;
    repertoire: string;
    /**
     * The SHA-256 fingerprint of the icon PNG, in lowercase hexadecimal — or
     * `null` when extraction failed.
     *
     * ⚠️ AN APPLICATION WITHOUT AN ICON IS BETTER THAN A MISSING APPLICATION.
     * `null` is not an error, and the field stays PRESENT on the wire.
     */
    icone: string | null;
    /** Always present. Is `'non-mesuree'` when `icone` is `null`. */
    source_max: SourceMax;
    /**
     * The DOMINANT colour of the icon, as `#rrggbb`, or `null`.
     *
     * 🔴 It is the "accent colour" the design of ④ asks for in §G5,
     * and it is **PER APPLICATION** — not to be confused with that of
     * sub-project ①, which is **per WINDOW** and arrives on the WebRTC channel.
     * Both are computed by the same pure rule; it is their SUBJECT that
     * differs.
     *
     * ⚠️ `null` IS NOT AN ERROR: an icon too pale, too dark or
     * too transparent has no dominant colour. The manifest then OMITS
     * `theme_color` rather than inventing one.
     */
    accent: string | null;
    /**
     * The extensions this application opens — lowercase, **with** the
     * dot, sorted and deduplicated by the agent.
     *
     * 🔴 EXTENSIONS, NEVER MIME TYPES (decision D13 of the G5 plan):
     * a MIME type is not a property of the VM, it is a convention of the Web.
     * The extension → MIME map lives **only once**, on the platform side, at
     * the place that writes the manifest — making it travel would duplicate a table
     * in Rust **and** in TypeScript.
     *
     * ⚠️ EMPTY IS A NORMAL STATE: most applications open no
     * file type. The field stays PRESENT on the wire.
     */
    associations: string[];
}

/**
 * Where the image comes from: the largest entry actually PRESENT in the
 * icon directory of the source.
 *
 * 🔴 IT IS NOT THE RENDERED SIZE. Measured on 20 August 2026 on two crafted
 * witnesses (`agent/testdata/g2-temoin-{48,256}.ico`): an `.ico` holding
 * ONLY a 48×48 entry, queried at 256, renders 256×256 32bpp — through
 * `IShellItemImageFactory` as through `PrivateExtractIconsW`, without
 * `SIIGBF_SCALEUP` and EVEN with `SIIGBF_BIGGERSIZEOK`. A criterion that
 * compared the rendered size to 256 CANNOT FAIL.
 *
 * 🔵 `'non-mesuree'` is written with a HYPHEN, never an underscore: it is the
 * Rust `rename_all = "kebab-case"` on a TWO-WORD variant, hence the
 * only one in the module whose convention is observable.
 */
export type SourceMax = { pixels: number } | 'non-mesuree';


/**
 * What a launch order actually did.
 *
 * 🔴 `raccourci` VERSUS `cible` IS WHAT MAKES THE ACCEPTANCE CRITERION
 * DECIDABLE: launching through the rebuilt target instead of the `.lnk` would pass a
 * criterion that would only say "something launched".
 *
 * ⚠️ IT IS NOT A `MotifCanal`: two values of `MotifCanal` CLOSE the
 * socket, and a failed launch must close no channel.
 */
export type IssueLancement = 'raccourci' | 'cible' | 'inconnue' | 'echec';

