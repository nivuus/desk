#!/usr/bin/env bash
# Joue un .ps1 dans la SESSION INTERACTIVE de la VM (schtasks /it), et attend
# que son journal soit ecrit.
#
# WinRM s'execute en session 0 : un relevé WinRM est celui de la session 0,
# jamais de la session interactive. Les racines de raccourcis, le presse-papier
# et le rendu audio en dependent tous.
#
# Usage : g4-tache.sh <fichier.ps1 local> <journal distant sans C:\dev\> [secondes]
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
# UNE TACHE PLANIFIEE DEMARRE DANS UN ENVIRONNEMENT NEUF : `schtasks /run` ne
# transporte RIEN de l'appelant. Tout parametre passe donc par un FICHIER, et
# jamais par une variable -- piege paye trois executions durant, ou N valait
# 1000 alors qu'on demandait 5000 puis 20000.
[ -n "${G4_RAFALE_N:-}" ] && printf '%s' "$G4_RAFALE_N" > /media/vm/dev/g4-rafale-n.txt
[ -n "${G4_PARAMS:-}" ] && printf '%s\n' "$G4_PARAMS" | tr ' ' '\n' > /media/vm/dev/g4-p.txt
rm -f "/media/vm/dev/$JOURNAL" 2>/dev/null || true

node "$ROOT/scripts/winrm.js" \
  "schtasks /delete /tn $TASK /f 2>\$null; \
   schtasks /create /tn $TASK /f /it /ru '$USER_NAME' /rp '$WINDOWS_ADMIN_PASSWORD' \
     /sc once /st 00:00 \
     /tr 'powershell -WindowStyle Hidden -NoProfile -ExecutionPolicy Bypass -File C:\\dev\\$BASE'; \
   schtasks /run /tn $TASK" >/dev/null

# ATTENDRE LE FAIT, JAMAIS UNE DUREE : on guette la ligne de fin du journal.
for _ in $(seq 1 "$ATTENTE"); do
  if [ -f "/media/vm/dev/$JOURNAL" ] && grep -aq '=== FIN\|^ERREUR ' "/media/vm/dev/$JOURNAL" 2>/dev/null; then
    exit 0
  fi
  sleep 1
done
echo "EXPIRE apres ${ATTENTE}s -- journal partiel :" >&2
exit 2
