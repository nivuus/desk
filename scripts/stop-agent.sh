#!/usr/bin/env bash

# ── VOIE MORTE, 29 août 2026 — voir scripts/voie-morte.sh ───────────────────
. "$(dirname "$0")/voie-morte.sh"
voie_morte "arrêtait l'agent en passant par scripts/winrm.js, dont le transport Basic est refusé" \
"     source docs/superpowers/plans/journaux-lot3/instrument/harnais-appliance.sh
     puis  agent_arreter   (Stop-ScheduledTask puis Stop-Process par PID relevé)."
# ─── Ci-dessous, le corps d'origine, conservé comme relevé historique. ──────

set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
node "$ROOT/scripts/winrm.js" \
    "schtasks /end /tn guacamole-agent 2>\$null; \
     Stop-Process -Name agent -Force -ErrorAction SilentlyContinue; 'arrêté'"
