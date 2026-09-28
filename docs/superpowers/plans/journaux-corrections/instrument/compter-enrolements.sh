#!/usr/bin/env bash
# Counts the enrolments and evictions of a VM's /agent channel, over a
# bounded window, with the binary actually deployed.
#
# 🔴 IT IS THE RED RUN OF DEFECT 1, AND IT COUNTS REAL ENROLMENTS — not a
# code inspection. The supervisor and the file bridge both enrol
# under the same `vm_id` as long as `lancer_pont` does not remove `AGENT_VM` and
# `AGENT_SECRET`; G1's registry ("the last registered wins") then closes
# the other's socket, which reconnects, and so on without end.
#
# Usage : compter-enrolements.sh <etiquette> <duree_s>
#
# Prerequisite: `.env` sourced by the caller (WINDOWS_ADMIN_PASSWORD), VM powered on.
set -euo pipefail

ETIQUETTE="${1:?étiquette du relevé}"
DUREE="${2:-64}"
ROOT="$(cd "$(dirname "$0")/../../../../.." && pwd)"
SORTIE="$ROOT/docs/superpowers/plans/journaux-corrections"
TRAVAIL="$ROOT/.corrections-tmp"
PORT=8090
BASE="$TRAVAIL/plateforme-$ETIQUETTE.sqlite"

mkdir -p "$TRAVAIL"
rm -f "$BASE"

# A service left by a previous report would make the next one fail on an
# EADDRINUSE — and, worse, we would otherwise measure against a platform whose
# version we do not know. Killed BY PID read from the port, never through `pkill -f`:
# this repository has already seen a `pkill -f <pattern>` kill the shell that launched it.
RESIDU="$(ss -ltnp 2>/dev/null | grep ":$PORT " | grep -o 'pid=[0-9]*' | cut -d= -f2 | head -1 || true)"
if [ -n "${RESIDU:-}" ]; then
    echo "== 0. plateforme résiduelle sur le port $PORT (pid $RESIDU) : arrêtée"
    kill "$RESIDU" 2>/dev/null || true
    sleep 2
fi

export PLATEFORME_HOTE=0.0.0.0
export PLATEFORME_PORT=$PORT
export PLATEFORME_BASE=sqlite
export PLATEFORME_BASE_URL="$BASE"
export PLATEFORME_SECRET_JETON='***RETIRE-DE-L-HISTORIQUE***'

echo "== 1. enrôlement d'une VM neuve"
ENR="$(cd "$ROOT/plateforme" && npx tsx src/admin/enroler-agent.ts --vm "essai-$ETIQUETTE" --adresse 192.168.3.2 2>/dev/null)"
VM_ID="$(echo "$ENR" | sed -n 's/^vm_id=//p')"
SECRET="$(echo "$ENR" | sed -n 's/^AGENT_SECRET=//p')"
PREFIXE="$(echo "$ENR" | sed -n 's/^prefixe=//p')"
[ -n "$VM_ID" ] && [ -n "$SECRET" ] || { echo '🔴 enrôlement en échec'; echo "$ENR"; exit 1; }
echo "   vm_id=$VM_ID prefixe=$PREFIXE secret=<${#SECRET} caractères>"

echo "== 2. démarrage de la plateforme sur le port $PORT"
(cd "$ROOT/plateforme" && nohup npx tsx src/index.ts > "$SORTIE/plateforme-$ETIQUETTE.log" 2>&1 & echo $! > "$TRAVAIL/plateforme.pid")
PID_PLAT="$(cat "$TRAVAIL/plateforme.pid")"
# The retained PID is that of `npx`, which is not necessarily the one listening:
# we kill both, the second read from the port at clean-up time.
nettoyer() {
    kill "$PID_PLAT" 2>/dev/null || true
    local ecoutant
    ecoutant="$(ss -ltnp 2>/dev/null | grep ":$PORT " | grep -o 'pid=[0-9]*' | cut -d= -f2 | head -1 || true)"
    [ -n "${ecoutant:-}" ] && kill "$ecoutant" 2>/dev/null || true
}
trap nettoyer EXIT
for _ in $(seq 1 60); do
    grep -q "le port $PORT" "$SORTIE/plateforme-$ETIQUETTE.log" && break
    sleep 1
done
grep -q "le port $PORT" "$SORTIE/plateforme-$ETIQUETTE.log" || { echo '🔴 plateforme non démarrée'; cat "$SORTIE/plateforme-$ETIQUETTE.log"; exit 1; }

echo "== 3. tuer tout agent résiduel sur la VM"
node "$ROOT/scripts/winrm.js" 'Get-Process agent -ErrorAction SilentlyContinue | Stop-Process -Force; schtasks /end /tn guacamole-agent 2>$null; Start-Sleep -Seconds 2; (Get-Process agent -ErrorAction SilentlyContinue | Measure-Object).Count' || true

echo "== 4. lancement du superviseur"
SIGNALING_URL="ws://192.168.3.1:$PORT" \
SUPERVISEUR=1 \
AGENT_VM="$VM_ID" \
AGENT_SECRET="$SECRET" \
    "$ROOT/scripts/run-agent.sh"

echo "== 5. fenêtre d'observation : $DUREE s"
DEBUT="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
python3 -c "import time; time.sleep($DUREE)"
FIN="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

echo "== 6. arrêt de l'agent, PUIS copie du journal"
node "$ROOT/scripts/winrm.js" 'schtasks /end /tn guacamole-agent 2>$null; Get-Process agent -ErrorAction SilentlyContinue | Stop-Process -Force; Start-Sleep -Seconds 3; (Get-Process agent -ErrorAction SilentlyContinue | Measure-Object).Count' || true
cp /media/vm/dev/agent.log "$SORTIE/agent-$ETIQUETTE.log"
sed 's/\x1b\[[0-9;]*m//g' "$SORTIE/agent-$ETIQUETTE.log" > "$SORTIE/agent-$ETIQUETTE-plat.log"

echo "== 7. comptage sur $SORTIE/agent-$ETIQUETTE-plat.log (fenêtre $DEBUT -> $FIN)"
PLAT="$SORTIE/agent-$ETIQUETTE-plat.log"
ENROLEMENTS=$(grep -ac 'agent enrôlé auprès de la plateforme' "$PLAT" || true)
EVICTIONS=$(grep -a 'canal /agent fermé par la plateforme' "$PLAT" | grep -ac 'remplace' || true)
REPRISES=$(grep -ac 'reprise du canal /agent' "$PLAT" || true)
CATALOGUES=$(grep -ac 'catalogue' "$PLAT" || true)
echo "etiquette=$ETIQUETTE duree=${DUREE}s enrolements=$ENROLEMENTS evictions=$EVICTIONS reprises=$REPRISES"
{
  echo "etiquette=$ETIQUETTE"
  echo "duree_s=$DUREE"
  echo "debut_utc=$DEBUT"
  echo "fin_utc=$FIN"
  echo "enrolements=$ENROLEMENTS"
  echo "evictions_remplace=$EVICTIONS"
  echo "reprises_canal=$REPRISES"
  echo "lignes_catalogue=$CATALOGUES"
  echo "binaire_octets=$(stat -c %s /media/vm/dev/target/release/agent.exe)"
} > "$SORTIE/compte-$ETIQUETTE.txt"
cat "$SORTIE/compte-$ETIQUETTE.txt"
