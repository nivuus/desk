// The protocol version of the `/agent` channel, and nothing else.
//
// 🔴 THIS MODULE EXISTS TO BREAK A CYCLE, NOT OUT OF A TASTE FOR SPLITTING. The
// sub-block G3 extracted the installation types and encoders to
// `plateforme-installation.ts` to hold the 500-line rule; those
// encoders need the constant, and the constant lived in
// `plateforme.ts`, which already imports that module. The cycle would have been a cycle
// of VALUES — not of types, which TypeScript erases —, hence a real runtime
// cycle, the kind that makes a constant `undefined` depending on the
// module evaluation order. Moving it out here removes it, instead of betting on
// that order.
//
// ⚠️ IT REMAINS RE-EXPORTED BY `plateforme.ts`: none of the consumers, in
// any package, had to move.

/**
 * The protocol version.
 *
 * 🔴 A BUMP IS A BREAK, AND IT IS DEPLOYED AT BOTH ENDS IN THE SAME COMMIT.
 * An agent from an older version READS the refusal that tells it so — the refusal
 * is the only variant outside versioning — and GIVES UP; it no longer loops.
 * The break remains a break, it has only become diagnosable.
 *
 * v4 is that of sub-block G3: it adds `installer` (downstream),
 * `progression` and `termine` (upstream), and the two enums `Phase` and
 * `Issue` they carry.
 *
 * v5 is that of sub-block G5 (slice F): `Application` gains `accent` — the
 * dominant colour of its icon, `null` when it has none — and
 * `associations`, the extensions it opens. Both serve the per-application PWA
 * manifest: `theme_color` and `file_handlers`.
 *
 * ⚠️ `Application` carries `#[serde(deny_unknown_fields)]` on the Rust side, and the
 * two new fields are MANDATORY on both sides: **any field added to
 * this structure is breaking**, and that is deliberate — an incomplete catalogue
 * accepted silently is the failure mode this versioning exists to
 * prevent.
 */
export const PLATEFORME_VERSION = 5;
