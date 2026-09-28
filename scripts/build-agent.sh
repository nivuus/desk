#!/usr/bin/env bash
# Synchronise puis compile l'agent sur la VM Windows.

# ── DEAD PATH, 29 August 2026 — see scripts/voie-morte.sh ───────────────────
. "$(dirname "$0")/voie-morte.sh"
voie_morte "synchronisait les sources Rust vers C:\\dev via /media/vm, puis les compilait SUR la VM" \
"     scripts/build-agent-croise.sh <destination>
       bâtit agent.exe en croisé (mingw, x86_64-pc-windows-gnu) SUR L'HÔTE,
       sans la VM. Le déposer ensuite sur l'invité par un serveur HTTP local
       et Invoke-WebRequest, en COMPARANT LES DEUX sha256 — l'idiome est dans
       docs/superpowers/plans/journaux-lot32t/instrument/fenetre-e1.sh."
# ─── Below, the original body, kept as a historical record. ──────

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
"$ROOT/scripts/sync-agent.sh"

# The WinRM per-shell quota (MaxMemoryPerShellMB) defaults to 1024 MB on
# Windows Server. That is too tight for rustc's parallel LLVM codegen
# when compiling the proc-macro crates (proc-macro2, parking_lot_core
# in particular): the build then fails with an error that never mentions
# memory — `STATUS_STACK_BUFFER_OVERRUN (0xc0000409)`, which looks like
# stack corruption but actually comes from the overly
# constrained WinRM Job Object. We raise the quota once and for all, idempotently:
# we read the current value and only change it if it is insufficient,
# never downwards.
MIN_SHELL_MB=4096
CURRENT_SHELL_MB="$(node "$ROOT/scripts/winrm.js" \
    '(Get-Item WSMan:\localhost\Shell\MaxMemoryPerShellMB).Value' 2>/dev/null | tr -dc '0-9')"
if ! [[ "$CURRENT_SHELL_MB" =~ ^[0-9]+$ ]] || [ "$CURRENT_SHELL_MB" -lt "$MIN_SHELL_MB" ]; then
    echo "quota WinRM MaxMemoryPerShellMB insuffisant (${CURRENT_SHELL_MB:-inconnu} Mo) : relèvement à ${MIN_SHELL_MB} Mo" >&2
    node "$ROOT/scripts/winrm.js" \
        "Set-Item -Path WSMan:\\localhost\\Shell\\MaxMemoryPerShellMB -Value $MIN_SHELL_MB" >/dev/null
else
    echo "quota WinRM MaxMemoryPerShellMB déjà suffisant (${CURRENT_SHELL_MB} Mo)" >&2
fi

# `release` by default: the milestone 1 measurements (throughput ~30 fps, median
# latency 276 ms) were all taken on a `debug` binary, without that
# being a choice — it was simply this script's default value, and
# `run-agent.sh` launched the `target\debug` path hardcoded. Passing `debug` as
# first argument remains possible for debugging.
PROFILE="${1:-release}"
FLAG=""
[ "$PROFILE" = "release" ] && FLAG="--release"

# cmake is required by `audiopus_sys`, which builds libopus from the C source
# vendored in the crate. It is not in the default PATH of the
# WinRM session after a winget install.
#
# CMAKE_POLICY_VERSION_MINIMUM=3.5: the cmake installed by winget (4.0.2) has
# removed compatibility with `cmake_minimum_required` older than 3.5
# and flatly refuses to configure without this safety net — exactly the
# CMakeLists.txt vendored by `audiopus_sys` (`cmake_minimum_required(VERSION
# 3.1)`). Without this variable, configuration fails with "Compatibility
# with CMake < 3.5 has been removed from CMake", even though cmake is
# indeed found and on the PATH.
#
# Scope: this environment variable applies to the WHOLE `cargo
# build` call that follows, hence to every `-sys` crate in the graph that invokes cmake —
# not only `audiopus_sys`. Today, `aws-lc-sys` (a dependency of
# `str0m`, unrelated to audio) is the only other crate in the graph to
# invoke cmake; it declares `cmake_minimum_required(VERSION 3.5..3.31)`
# (range syntax) in its vendored CMakeLists.txt, which is not
# affected by the compatibility net and thus ignores this variable with no
# side effect. If a future dependency adds a vendored CMakeLists.txt
# with a different version floor, check that it also tolerates
# CMAKE_POLICY_VERSION_MINIMUM=3.5 before assuming it harmless.
node "$ROOT/scripts/winrm.js" \
    "\$env:Path += ';C:\\Users\\Administrateur\\.cargo\\bin;C:\\Program Files\\CMake\\bin'; \$env:CMAKE_POLICY_VERSION_MINIMUM = '3.5'; Set-Location C:\\dev; cargo build $FLAG 2>&1 | Out-String"
