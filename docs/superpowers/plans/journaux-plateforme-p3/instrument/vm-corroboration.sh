#!/usr/bin/env bash
# TASK 23 — the corroboration on a real VM, OUTSIDE THE CRITERIA.
#
#     instrument/vm-corroboration.sh <repertoire-de-sortie> [duree-secondes]
#
# 🔴 EVERYTHING HAPPENS IN A SINGLE CALL, AND IT IS A CONSTRAINT, NOT A STYLE.
# A command put in the background by the harness DOES NOT SURVIVE the end of the
# turn of the agent that launched it — two acceptance runs of sub-block D10 lost
# a run each that way, and the symptom is a TRUNCATED log copied from a
# VM where the agent, for its part, keeps running. The platform service, the
# scripted shell page and the agent must therefore live and die in this very call.
#
# ⚠️ NO SECRET IS WRITTEN INTO THE FILED LOG. The enrolment secret
# drawn by `npm run admin:agent` is captured into a variable and NEVER
# printed: the log only carries its length. The token signing
# secret is fixed and worthless outside this throwaway run.
set -uo pipefail

SORTIE="${1:?usage : vm-corroboration.sh <repertoire-de-sortie> [duree]}"
DUREE="${2:-60}"
RACINE="$(cd "$(dirname "$0")/../../../../.." && pwd)"
INSTRU="$RACINE/docs/superpowers/plans/journaux-plateforme-p3/instrument"
cd "$RACINE"
mkdir -p "$SORTIE"

set -a; source "$RACINE/.env"; set +a

SECRET_JETON='***RETIRE-DE-L-HISTORIQUE***'
# ⚠️ 8081, AND NOT 8080. A platform service from an EARLIER acceptance run (block
# E1) had already been listening on 0.0.0.0:8080 for nearly five hours at the time
# of this run, with its own database and its own signing secret.
# Killing it would have been a gesture on someone else's work; plugging into it would have
# measured a service whose environment is not ours — it is the
# trap the TURN workstream paid for ("check the environment of the process
# THAT REALLY LISTENS"). We take a free port, and `SIGNALING_URL` follows.
PORT=8081
BASE_FICHIER="$SORTIE/plateforme-vm.sqlite"
rm -f "$BASE_FICHIER"

dire() { echo "[$(date +%H:%M:%S)] $*"; }

# --- 0. the VM's state, BEFORE anything else ------------------------------------
dire '=== 0. état de la VM ==='
virsh list --all 2>&1 | sed 's/^/    /'
if ! virsh list --state-running --name 2>/dev/null | grep -q '^Windows$'; then
    dire 'la VM est éteinte — démarrage'
    virsh start Windows 2>&1 | sed 's/^/    /'
fi
# ⚠️ WE WAIT FOR THE MOUNT, NOT THE PORT. `/media/vm` mounts AFTER WinRM
# answers, and its CIFS entry survives a powered-off VM: `mountpoint -q` would return
# true on a dead connection. A real ACCESS must be tested.
dire 'attente de WinRM puis du montage…'
for _ in $(seq 1 60); do timeout 3 bash -c 'echo > /dev/tcp/192.168.3.2/5985' 2>/dev/null && break; sleep 5; done
for _ in $(seq 1 60); do ls /media/vm/dev >/dev/null 2>&1 && break; sleep 5; done
dire "agent.exe sur la VM : $(stat -c '%s octets, %y' /media/vm/dev/target/release/agent.exe 2>&1)"
BRUT="$(node "$RACINE/scripts/winrm.js" '(Get-Process agent -ErrorAction SilentlyContinue).Count' 2>/dev/null)"
# ⚠️ We only accept the case where the command returned a short integer. A `tr -dc`
# on a network error message would return "192168325985…", that is, a
# number that is not zero and would trigger a useless `Stop-Process` on
# an unreachable VM — MEASURED at this script's first run.
if [[ "$BRUT" =~ ^[0-9]{1,3}$ ]]; then AGENTS="$BRUT"; else AGENTS='illisible'; fi
dire "Get-Process agent avant lancement : $AGENTS (brut : $(echo "$BRUT" | head -1))"
if [ "$AGENTS" != "0" ]; then
    dire '🔴 un agent tourne déjà — un superviseur vivant empêche le nouveau'
    dire "   d'ouvrir agent.log, et l'on relit alors le journal PÉRIMÉ. On tue."
    node "$RACINE/scripts/winrm.js" 'Stop-Process -Name agent -Force -ErrorAction SilentlyContinue; Start-Sleep -Seconds 2; (Get-Process agent -ErrorAction SilentlyContinue).Count' 2>/dev/null | sed 's/^/    /'
fi

# --- 1. le service ------------------------------------------------------------
dire '=== 1. démarrage du service de plateforme ==='
PLATEFORME_HOTE=192.168.3.1 \
PLATEFORME_PORT="$PORT" \
PLATEFORME_BASE=sqlite \
PLATEFORME_BASE_URL="$BASE_FICHIER" \
PLATEFORME_SECRET_JETON="$SECRET_JETON" \
    "$RACINE/plateforme/node_modules/.bin/tsx" "$RACINE/plateforme/src/index.ts" \
    > "$SORTIE/service.log" 2>&1 &
PID_SERVICE=$!
arreter_service() { kill "$PID_SERVICE" 2>/dev/null; }
trap arreter_service EXIT

for _ in $(seq 1 60); do
    grep -q 'le port ' "$SORTIE/service.log" 2>/dev/null && break
    sleep 0.5
done
grep 'le port ' "$SORTIE/service.log" | sed 's/^/    /' || { dire '🔴 le service n’a pas démarré'; cat "$SORTIE/service.log"; exit 1; }

# --- 2. enrolment ----------------------------------------------------------
dire '=== 2. enrôlement de la VM (npm run admin:agent) ==='
ENROLEMENT="$(cd "$RACINE/plateforme" && \
    PLATEFORME_HOTE=192.168.3.1 PLATEFORME_BASE=sqlite PLATEFORME_BASE_URL="$BASE_FICHIER" \
    PLATEFORME_SECRET_JETON="$SECRET_JETON" \
    npm run --silent admin:agent -- --vm w1 --adresse 192.168.3.2 2>/dev/null)"
AGENT_VM="$(echo "$ENROLEMENT" | sed -n 's/^vm_id=//p')"
PREFIXE="$(echo "$ENROLEMENT" | sed -n 's/^prefixe=//p')"
AGENT_SECRET="$(echo "$ENROLEMENT" | sed -n 's/^AGENT_SECRET=//p')"
dire "    vm_id   = $AGENT_VM"
dire "    prefixe = $PREFIXE"
dire "    secret  = <non imprimé — ${#AGENT_SECRET} caractères>"
[ -n "$AGENT_VM" ] && [ -n "$PREFIXE" ] && [ -n "$AGENT_SECRET" ] || { dire '🔴 enrôlement en échec'; echo "$ENROLEMENT"; exit 1; }

# --- 3. a window to capture ------------------------------------------------
dire '=== 3. ouverture d’une fenêtre éligible dans la session interactive ==='
cat > /media/vm/dev/p3-fenetre.ps1 <<'PS1'
Get-Process notepad -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Process notepad
Start-Sleep -Seconds 2
(Get-Process notepad | Select-Object -First 1).MainWindowTitle
PS1
node "$RACINE/scripts/winrm.js" \
  "schtasks /delete /tn p3-fenetre /f 2>\$null; \
   schtasks /create /tn p3-fenetre /f /it /ru '${WINDOWS_ADMIN_USERNAME:-Administrateur}' /rp '$WINDOWS_ADMIN_PASSWORD' \
     /sc once /st 00:00 \
     /tr 'powershell -WindowStyle Normal -NoProfile -ExecutionPolicy Bypass -File C:\\dev\\p3-fenetre.ps1'; \
   schtasks /run /tn p3-fenetre" 2>&1 | tail -2 | sed 's/^/    /'
sleep 6

# --- 4. the agent, WITH its identity --------------------------------------------
dire '=== 4. lancement de l’agent, AVEC AGENT_VM et AGENT_SECRET ==='
SIGNALING_URL="ws://192.168.3.1:$PORT" \
SUPERVISEUR=1 \
RUST_LOG=info \
AGENT_VM="$AGENT_VM" \
AGENT_SECRET="$AGENT_SECRET" \
    "$RACINE/scripts/run-agent.sh" 2>&1 | sed 's/^/    /'

# --- 5. the scripted shell page -------------------------------------------------
dire "=== 5. page-shell scriptée, $DUREE s ==="
"$RACINE/plateforme/node_modules/.bin/tsx" "$INSTRU/shell-scripte.ts" \
    "ws://192.168.3.1:$PORT/" "$PREFIXE" "$SECRET_JETON" "$DUREE" \
    > "$SORTIE/shell.log" 2>&1
dire "    shell : $(grep -c '' "$SORTIE/shell.log") lignes"

# --- 6. the stop, and the evidence --------------------------------------------------
dire '=== 6. arrêt de l’agent et copie des pièces ==='
node "$RACINE/scripts/winrm.js" 'Stop-Process -Name agent -Force -ErrorAction SilentlyContinue; Start-Sleep -Seconds 2; (Get-Process agent -ErrorAction SilentlyContinue).Count' 2>/dev/null | sed 's/^/    agents restants : /'
sleep 2
cp /media/vm/dev/agent.log "$SORTIE/agent-avec-identite.log"
sed 's/\x1b\[[0-9;]*m//g' "$SORTIE/agent-avec-identite.log" > "$SORTIE/agent-avec-identite-plat.log"
dire "    agent.log : $(grep -c '' "$SORTIE/agent-avec-identite-plat.log") lignes"

dire '=== 7. la table session, telle que la plateforme l’a écrite ==='
node -e '
const { DatabaseSync } = require("node:sqlite");
const db = new DatabaseSync(process.argv[1]);
for (const l of db.prepare("SELECT id, utilisateur_id, vm_id, ouverte_a, fermee_a FROM session ORDER BY ouverte_a").all()) {
    console.log("    " + JSON.stringify(l));
}
' "$BASE_FICHIER" 2>&1 | sed 's/^/    /'

# --- 8. task 20's RED run: WITHOUT the variables ---------------------------
dire '=== 8. 🔴 la même chose SANS AGENT_VM ni AGENT_SECRET ==='
SIGNALING_URL="ws://192.168.3.1:$PORT" \
SUPERVISEUR=1 \
RUST_LOG=info \
    "$RACINE/scripts/run-agent.sh" 2>&1 | sed 's/^/    /'
sleep 25
node "$RACINE/scripts/winrm.js" 'Stop-Process -Name agent -Force -ErrorAction SilentlyContinue' 2>/dev/null
sleep 2
cp /media/vm/dev/agent.log "$SORTIE/agent-sans-identite.log"
sed 's/\x1b\[[0-9;]*m//g' "$SORTIE/agent-sans-identite.log" > "$SORTIE/agent-sans-identite-plat.log"
dire "    agent.log : $(grep -c '' "$SORTIE/agent-sans-identite-plat.log") lignes"

dire '=== 9. état final de la VM ==='
virsh list --all 2>&1 | sed 's/^/    /'
node "$RACINE/scripts/winrm.js" '(Get-Process agent -ErrorAction SilentlyContinue).Count' 2>/dev/null | sed 's/^/    agents restants : /'
echo
dire 'terminé'
