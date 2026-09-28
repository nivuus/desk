#!/usr/bin/env bash

# ── DEAD PATH, 29 August 2026 — see scripts/voie-morte.sh ───────────────────
. "$(dirname "$0")/voie-morte.sh"
voie_morte "stopped the agent through scripts/winrm.js, whose Basic transport is refused" \
"     source docs/superpowers/plans/journaux-lot3/instrument/harnais-appliance.sh
     then  agent_arreter   (Stop-ScheduledTask then Stop-Process by the PID read)."
# ─── Below, the original body, kept as a historical record. ──────

set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
node "$ROOT/scripts/winrm.js" \
    "schtasks /end /tn guacamole-agent 2>\$null; \
     Stop-Process -Name agent -Force -ErrorAction SilentlyContinue; 'stopped'"
