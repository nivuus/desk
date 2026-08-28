#!/usr/bin/env bash
# Une phase de recette A-bis.
#   $1 etiquette  $2 valeur d'AUDIO_PERIPHERIQUE ('' = absente)
#   $3 prefixe waveOut ou jouer ('' = rien)  $4 frequence Hz
set -uo pipefail
ROOT="$(git rev-parse --show-toplevel)" || { echo "🔴 hors du depot git : impossible de deriver ROOT (git rev-parse a echoue)" >&2; exit 1; }
cd "$ROOT"
set -a && source .env && set +a
ETIQ="$1"; VAR="$2"; JOUER="$3"; HZ="${4:-440}"
JOURNAUX="$ROOT/docs/superpowers/plans/journaux-micro"
USER_NAME="${WINDOWS_ADMIN_USERNAME:-Administrateur}"

node scripts/winrm.js 'Stop-Process -Name agent -Force -ErrorAction SilentlyContinue' >/dev/null 2>&1

# La tonalite doit jouer dans la SESSION INTERACTIVE, comme l'agent : un
# rendu waveOut lance depuis WinRM (session 0) n'atteint aucun point de
# terminaison de la session 1 — mesure de la phase R2-temoin, echantillons=0.
if [ -n "$JOUER" ]; then
  node scripts/winrm.js "schtasks /delete /tn abis-tonalite /f 2>\$null; \
     schtasks /create /tn abis-tonalite /f /it /ru '$USER_NAME' /rp '$WINDOWS_ADMIN_PASSWORD' /sc once /st 00:00 \
       /tr 'powershell -WindowStyle Hidden -NoProfile -ExecutionPolicy Bypass -File C:\\dev\\abis-tonalite.ps1 -Prefixe \"$JOUER\" -Hz $HZ'; \
     schtasks /run /tn abis-tonalite" >/dev/null 2>&1
  sleep 5
fi

export AUDIO_PROBE=1 AUDIO_PROBE_SECS=12
if [ -n "$VAR" ]; then export AUDIO_PERIPHERIQUE="$VAR"; else unset AUDIO_PERIPHERIQUE; fi
scripts/run-agent.sh >/dev/null 2>&1
sleep 22

sed 's/\x1b\[[0-9;]*m//g' /media/vm/dev/agent.log > "$JOURNAUX/abis-$ETIQ.log" 2>/dev/null
cp /media/vm/dev/abis-tonalite.log "$JOURNAUX/abis-$ETIQ-tonalite.log" 2>/dev/null || true
node scripts/winrm.js 'Stop-Process -Name agent -Force -ErrorAction SilentlyContinue; schtasks /end /tn abis-tonalite 2>$null' >/dev/null 2>&1 || true
echo "=========== PHASE $ETIQ (AUDIO_PERIPHERIQUE=${VAR:-<absente>} ; ${HZ}Hz sur ${JOUER:-<rien>}) ==========="
[ -n "$JOUER" ] && grep -a "CIBLE\\|LECTURE \\|METRE" "$JOURNAUX/abis-$ETIQ-tonalite.log" 2>/dev/null | sed "s/^/  tonalite: /"
grep -aE "de rendu retenu|sonde audio terminée|loopback ouvert|WARN" "$JOURNAUX/abis-$ETIQ.log" | sed 's/^/  /'
