#!/usr/bin/env bash
# Syncs the Rust sources to C:\dev (mounted on /media/vm) for
# building on Windows.

# ── DEAD PATH, 29 August 2026 — see scripts/voie-morte.sh ───────────────────
. "$(dirname "$0")/voie-morte.sh"
voie_morte "rsynced the Rust sources to C:\\dev, through the CIFS mount /media/vm" \
"     Nothing replaces it, and nothing needs to: the appliance no longer builds.
     The binary is built on the host (scripts/build-agent-croise.sh) and
     dropped over HTTP. See the successor of scripts/build-agent.sh."
# ─── Below, the original body, kept as a historical record. ──────

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DEST="/media/vm/dev"

if ! mountpoint -q /media/vm; then
    echo "error: /media/vm is not mounted" >&2
    exit 1
fi

# We only copy what git tracks under the paths useful to the Rust
# build: an inclusion list targeting what must cross over to the VM,
# rather than an enumeration of what must not. This choice is
# deliberately robust to future additions in the tree — `node_modules/`,
# `target/`, `dist/`, `.git/` — since none is ever tracked by git
# (the first through .gitignore, the others by nature) and thus never listed
# by `git ls-files`.
#
# History: the previous version rsync-ed the whole of `proto/`, including
# `proto/node_modules/` (JS tooling of the shared vector tests). The
# CIFS mount to the VM cannot set timestamps on the
# symbolic links of `node_modules/.bin/` (`Operation not supported`, errno 95),
# which made rsync exit with code 23 and interrupted this script before
# it even reached the build step.
mkdir -p "$DEST"

# We start again from a clean tree on the VM side for the synced directories:
# that removes any leftover from a previous run (like the old
# `proto/node_modules/`) without depending on the subtleties of `rsync --delete`
# combined with `--files-from`.
rm -rf "$DEST/agent" "$DEST/proto"

cd "$ROOT"

# Safeguard: a file created under agent/ or proto/ then never `git add`ed
# does not appear in `git ls-files` and would thus be silently absent from
# the VM — we would build a state of the code different from the one in front
# of us, without the slightest warning. We warn rather than block: an
# unstaged draft may be tested on purpose, but not unknowingly.
untracked="$(git ls-files --others --exclude-standard -- agent proto)"
if [ -n "$untracked" ]; then
    echo "warning: files not tracked by git under agent/ or proto/ — they will NOT be synced to the VM:" >&2
    echo "$untracked" | sed 's/^/  /' >&2
    echo "  (run \"git add\" if these files must be part of the Windows build)" >&2
fi

git ls-files -z -- Cargo.toml Cargo.lock rust-toolchain.toml agent proto |
    rsync -a --from0 --files-from=- "$ROOT/" "$DEST/"

echo "sources synced to $DEST"
