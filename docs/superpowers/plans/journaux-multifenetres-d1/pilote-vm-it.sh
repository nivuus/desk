#!/usr/bin/env bash
# Exécute du PowerShell dans la SESSION INTERACTIVE (session 1) de la VM,
# via une tâche planifiée /IT — mécanisme établi par la tâche 6.
# Usage : vm-it.sh <nom-tache> '<powershell>'
set -euo pipefail
ROOT=/home/mallanic/Projects/Guacamole
NOM="$1"; shift
SCRIPT="$*"
USER_NAME="${WINDOWS_ADMIN_USERNAME:-Administrateur}"
printf '%s\n' "$SCRIPT" > "/media/vm/dev/it-$NOM.ps1"
node "$ROOT/scripts/winrm.js" \
  "schtasks /delete /tn $NOM /f 2>\$null; \
   schtasks /create /tn $NOM /f /it /ru '$USER_NAME' /rp '$WINDOWS_ADMIN_PASSWORD' \
     /sc once /st 00:00 \
     /tr 'powershell -WindowStyle Hidden -NoProfile -ExecutionPolicy Bypass -File C:\\dev\\it-$NOM.ps1'; \
   schtasks /run /tn $NOM" >/dev/null
