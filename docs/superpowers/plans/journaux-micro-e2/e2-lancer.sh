#!/usr/bin/env bash
# Launch sequence of one E2 acceptance run: KILL, check, launch.
#
# 🔴 TWO TRAPS ARE WIRED HERE BECAUSE THEY WERE PAID FOR IN THIS BLOCK.
#
# 1. A surviving agent holds C:\dev\agent.log: the new StreamWriter cannot
#    open it, and one then rereads the log of the PREVIOUS attempt
#    believing one reads one's own. Hence `e2-tuer.ps1` first, unconditionally.
# 2. `scripts/run-agent.sh` requires `.env` (WINDOWS_ADMIN_PASSWORD), and `.env`
#    carries SIGNALING_URL=…:8080 — the platform of ANOTHER workstream. A
#    `set -a; source .env` after the acceptance settings OVERWRITES them
#    silently, and the agent will talk to the wrong service. The order below is
#    therefore: .env first, acceptance settings AFTERWARDS.
#
# Usage : SESSION_ID=… MICRO=… [AUDIO_PERIPHERIQUE=…] ./e2-lancer.sh <agent.env>
set -euo pipefail
RACINE="$(git rev-parse --show-toplevel)" || { echo "🔴 hors du depot git : impossible de deriver RACINE (git rev-parse a echoue)" >&2; exit 1; }
ENVRECETTE="${1:?chemin du fichier agent.env attendu}"
# The acceptance settings, remembered BEFORE .env can overwrite them.
GARDE_SESSION="${SESSION_ID:-}"; GARDE_MICRO="${MICRO:-}"
GARDE_AUDIOP="${AUDIO_PERIPHERIQUE:-}"; GARDE_MICROP="${MICRO_PERIPHERIQUE:-}"
GARDE_FAUTE="${MICRO_FAUTE_ECRITURE:-}"; GARDE_MESURE="${MICRO_MESURE:-}"
cd "$RACINE"
set -a; source .env; source "$ENVRECETTE"; set +a
export SESSION_ID="$GARDE_SESSION"
[ -n "$GARDE_MICRO" ] && export MICRO="$GARDE_MICRO" || true
[ -n "$GARDE_AUDIOP" ] && export AUDIO_PERIPHERIQUE="$GARDE_AUDIOP" || unset AUDIO_PERIPHERIQUE
[ -n "$GARDE_MICROP" ] && export MICRO_PERIPHERIQUE="$GARDE_MICROP" || true
[ -n "$GARDE_FAUTE" ] && export MICRO_FAUTE_ECRITURE="$GARDE_FAUTE" || true
[ -n "$GARDE_MESURE" ] && export MICRO_MESURE="$GARDE_MESURE" || true
echo "SIGNALING_URL=$SIGNALING_URL  SESSION_ID=$SESSION_ID  MICRO=${MICRO:-<absente>}  AUDIO_PERIPHERIQUE=${AUDIO_PERIPHERIQUE:-<absente>}"
node scripts/winrm.js 'powershell -NoProfile -ExecutionPolicy Bypass -File C:\dev\e2-tuer.ps1' 2>&1 | tail -1
rm -f /media/vm/dev/agent.log
scripts/run-agent.sh >/dev/null 2>&1
echo "AGENT LANCE"
