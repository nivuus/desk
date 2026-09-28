#!/usr/bin/env bash
# Runs PowerShell in the VM's INTERACTIVE SESSION (session 1),
# through an /IT scheduled task — a mechanism established by task 6.
# Usage: vm-it.sh <task-name> '<powershell>'
set -euo pipefail
ROOT="$(git rev-parse --show-toplevel)" || { echo "🔴 hors du depot git : impossible de deriver ROOT (git rev-parse a echoue)" >&2; exit 1; }
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
