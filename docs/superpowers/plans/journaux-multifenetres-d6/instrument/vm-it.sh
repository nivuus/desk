#!/usr/bin/env bash
# Exécute un script PowerShell dans la SESSION INTERACTIVE de la VM (session 1),
# par tâche planifiée /IT — même mécanisme que scripts/run-agent.sh.
# WinRM tourne en session 0 : une fenêtre ouverte depuis là ne serait pas
# visible de la session de l'utilisateur, et le superviseur ne la verrait pas.
set -euo pipefail
NOM="$1"
PS="$2"
ROOT="$(git rev-parse --show-toplevel)" || { echo "🔴 hors du depot git : impossible de deriver ROOT (git rev-parse a echoue)" >&2; exit 1; }
USER_NAME="${WINDOWS_ADMIN_USERNAME:-Administrateur}"
printf '%s\n' "$PS" > "/media/vm/dev/it-$NOM.ps1"
node "$ROOT/scripts/winrm.js" \
  "schtasks /delete /tn it-$NOM /f 2>\$null; \
   schtasks /create /tn it-$NOM /f /it /ru '$USER_NAME' /rp '$WINDOWS_ADMIN_PASSWORD' \
     /sc once /st 00:00 \
     /tr 'powershell -WindowStyle Hidden -NoProfile -ExecutionPolicy Bypass -File C:\\dev\\it-$NOM.ps1'; \
   schtasks /run /tn it-$NOM" > /dev/null 2>&1
