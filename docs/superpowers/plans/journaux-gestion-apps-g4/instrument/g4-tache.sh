#!/usr/bin/env bash
# Plays a .ps1 in the VM's INTERACTIVE SESSION (schtasks /it), and waits
# for its log to be written.
#
# WinRM runs in session 0: a WinRM reading is that of session 0,
# never of the interactive session. The shortcut roots, the clipboard
# and audio rendering all depend on it.
#
# Usage: g4-tache.sh <local file.ps1> <remote log without C:\dev\> [seconds]
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../../../../.." && pwd)"
PS1_LOCAL="$1"
JOURNAL="$2"
ATTENTE="${3:-180}"
TASK="g4-sonde"
USER_NAME="${WINDOWS_ADMIN_USERNAME:-Administrateur}"
: "${WINDOWS_ADMIN_PASSWORD:?WINDOWS_ADMIN_PASSWORD non defini}"

BASE="$(basename "$PS1_LOCAL")"
cp "$PS1_LOCAL" "/media/vm/dev/$BASE"
# A SCHEDULED TASK STARTS IN A FRESH ENVIRONMENT: `schtasks /run`
# carries NOTHING from the caller. Every parameter therefore goes through a FILE, and
# never through a variable -- a trap paid for over three runs, where N was
# 1000 while 5000 then 20000 were requested.
[ -n "${G4_RAFALE_N:-}" ] && printf '%s' "$G4_RAFALE_N" > /media/vm/dev/g4-rafale-n.txt
[ -n "${G4_PARAMS:-}" ] && printf '%s\n' "$G4_PARAMS" | tr ' ' '\n' > /media/vm/dev/g4-p.txt
rm -f "/media/vm/dev/$JOURNAL" 2>/dev/null || true

node "$ROOT/scripts/winrm.js" \
  "schtasks /delete /tn $TASK /f 2>\$null; \
   schtasks /create /tn $TASK /f /it /ru '$USER_NAME' /rp '$WINDOWS_ADMIN_PASSWORD' \
     /sc once /st 00:00 \
     /tr 'powershell -WindowStyle Hidden -NoProfile -ExecutionPolicy Bypass -File C:\\dev\\$BASE'; \
   schtasks /run /tn $TASK" >/dev/null

# WAIT FOR THE FACT, NEVER A DURATION: we watch for the log's final line.
for _ in $(seq 1 "$ATTENTE"); do
  if [ -f "/media/vm/dev/$JOURNAL" ] && grep -aq '=== FIN\|^ERREUR ' "/media/vm/dev/$JOURNAL" 2>/dev/null; then
    exit 0
  fi
  sleep 1
done
echo "EXPIRE apres ${ATTENTE}s -- journal partiel :" >&2
exit 2
