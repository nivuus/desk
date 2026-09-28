#!/usr/bin/env bash
# Chains the work item D probes, ONE PER RUN of the binary.
#
# It is not a convenience: `captureservice.dll` crashed with 0xc0000005 at
# milestone 1, and a crash of that kind takes the process down. Exercising the
# four paths in one run would lose the other three with the
# first. Each path thus runs alone, and its log is collected before
# the next one.

# ── DEAD PATH, 29 August 2026 — see scripts/voie-morte.sh ───────────────────
. "$(dirname "$0")/voie-morte.sh"
voie_morte "enchaînait les sondes du chantier D, une par exécution, à travers /media/vm" \
"     Rien ne le remplace tel quel. Les sondes se lancent aujourd'hui en posant
     leur variable (MULTIFENETRE_*) dans C:\nivuus\agent\run-agent.ps1 —
     voir le successeur de scripts/run-agent.sh."
# ─── Below, the original body, kept as a historical record. ──────

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
JOURNAUX="$ROOT/docs/superpowers/plans/journaux-sonde-multifenetre"
mkdir -p "$JOURNAUX"

# `run-agent.sh` requires the Windows credentials; loading them here avoids
# imposing a `set -a && source .env` at every invocation.
if [ -f "$ROOT/.env" ]; then
    set -a
    # shellcheck disable=SC1091
    . "$ROOT/.env"
    set +a
fi

executer() {
    local nom="$1"; shift
    # Read on EVERY call: the phase 2 bench raises SONDE_SECS for its
    # own passes, without the short phase 1 probes inheriting it.
    local secs="${SONDE_SECS:-25}"
    echo "── sonde : $nom ─────────────────────────────"
    rm -f /media/vm/dev/agent.log
    env "$@" "$ROOT/scripts/run-agent.sh"
    sleep "$secs"
    cp /media/vm/dev/agent.log "$JOURNAUX/$nom.log" 2>/dev/null \
        || echo "AUCUN JOURNAL — la sonde a-t-elle planté au démarrage ?"
    # The log comes out of PowerShell in UTF-16LE: without this decoding, each
    # ASCII character is displayed spaced by a null byte ("t e x t" instead of
    # "text"), making `tail` unreadable over the whole log, not only
    # on accented letters.
    iconv -f UTF-16LE -t UTF-8 "$JOURNAUX/$nom.log" 2>/dev/null | tail -30 || true
}

executer dxgi MULTIFENETRE_DXGI=1
executer wgc MULTIFENETRE_WGC=1
executer replis MULTIFENETRE_REPLIS=1
executer nvenc MULTIFENETRE_NVENC=1

# Phase 2: the bench, on the paths declared surviving by phase 1.
# `VOIES` is set by hand from the verdicts — the script infers nothing.
for voie in ${VOIES:-}; do
    for n in 1 2 4 8; do
        SONDE_SECS=45 executer "banc-$voie-$n" "MULTIFENETRE_BANC=$voie" "MULTIFENETRE_N=$n"
    done
done
