// THE REDIRECTION OF THE OLD SHELL PAGE — a rule, pure and tested.
//
// 🔴 WHY `shell.html` SURVIVES AS A REDIRECT RATHER THAN BEING
// DELETED (owner's decision, August 31st, 2026).
//
// ⚠️ FIXED ON AUGUST 31st, 2026 (fix round 1 of this same task): this sentence
// named `start_url` — true when it was written, WRONG since
// this task migrated `start_url` to the root (`hub/manifeste.ts`,
// commit `a100e46`). It is now **`id`**, NOT `start_url`, that carries
// `shell.html?app=<id>` (`hub/manifeste.ts:265`) — `id` is the frozen IDENTITY
// of the installed application, never updated (see its comment).
//
// The manifests of **ALREADY installed** PWAs therefore still carry, in their
// `id`, `shell.html?app=<id>` — and since the manifest is published as `blob:`
// (`hub/manifeste.ts`, declared G5 legacy), such a PWA **will never reread its
// manifest**: it is its `start_url` (frozen too, at the moment of
// installation) that decides where it opens, and for an installation made
// BEFORE this batch, it was still `shell.html?app=<id>`. It will therefore open
// that address forever, and it is for THOSE installations — the
// installations already made, not future ones — that the redirect exists.
// An installation made from now on carries the `start_url` migrated
// to the root and will never again reach `shell.html` through this path.
// Deleting the file would therefore break the former for good. It is not
// a transition: it is the permanent path of those installations.

/// The address to redirect to, query string KEPT.
///
/// ⚠️ `?app=` IS THE POINT: losing it would break PWAs as surely as
/// deleting the file.
export function cibleDeRedirection(search: string): string {
    return `/${search}`;
}
