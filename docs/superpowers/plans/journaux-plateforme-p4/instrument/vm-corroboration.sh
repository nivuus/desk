#!/usr/bin/env bash
# TASK 16 — the corroboration on a real VM, OUTSIDE THE CRITERIA and CONDITIONAL.
#
#     instrument/vm-corroboration.sh <repertoire-de-sortie> [duree-secondes]
#
# 🔴 WHAT IT ADDS TO `corroboration-navigateur.log`, AND NOTHING ELSE. The
# browser corroboration of task 14 already tested the sign-in screen,
# the cross-origin policy and the vault write against a REAL
# service — but with the agent's state SEEDED IN THE DATABASE. What existed nowhere
# is the complete UNSEEDED chain: a real Windows agent beats -> `vu_a`
# advances -> `agents/fraicheur.ts` deduces `prete` from it -> `POST /session` returns the
# prefix -> the shell page opens `<prefix>:bureau` on that very agent. That is
# what, and only what, this script establishes.
#
# 🔴 EVERYTHING HAPPENS IN A SINGLE CALL, AND IT IS A CONSTRAINT, NOT A STYLE.
# A command put in the background by the harness DOES NOT SURVIVE the end of the
# turn of the agent that launched it (two acceptance runs of D10 lost a
# run each that way, symptom: a TRUNCATED log copied from a VM where
# the agent keeps running). The service, the agent and the shell page live and
# die in this very call. Structure taken from
# `journaux-plateforme-p3/instrument/vm-corroboration.sh`, from which it also inherits
# the traps already paid for (wait for the MOUNT and not the port; only accept
# a short integer from `Get-Process`).
#
# 🔴 NO REBUILD OF THE AGENT, AND IT IS A REASONED DECISION. P4 touches
# neither `agent/` nor `proto/`: it needs no new binary. And at the
# time this is played, a neighbouring workstream (G1, app management) has
# UNCOMMITTED changes in `proto/` — `scripts/build-agent.sh`
# rsyncs the sources, so a build from here would push its half-done work
# onto the VM, and would perhaps give it a `PLATEFORME_VERSION = 2`
# that this service, which speaks 1, would refuse. The present binary is therefore
# used AS IS, and its size and date are recorded: they are
# attributable to NO commit of P4, and the log says so.
#
# ⚠️ NO SECRET IS WRITTEN INTO THE FILED LOG: the enrolment secret
# only appears through its length, and the trial account's password is a
# throwaway string specific to this run.
set -uo pipefail

SORTIE="${1:?usage : vm-corroboration.sh <repertoire-de-sortie> [duree]}"
DUREE="${2:-45}"
RACINE="$(cd "$(dirname "$0")/../../../../.." && pwd)"
INSTRU_P3="$RACINE/docs/superpowers/plans/journaux-plateforme-p3/instrument"
cd "$RACINE"
mkdir -p "$SORTIE"

set -a; source "$RACINE/.env"; set +a

SECRET_JETON='***RETIRE-DE-L-HISTORIQUE***'
MOTDEPASSE='p4-corroboration-jetable'
EMAIL='p4@essai.local'
# ⚠️ 8082: neither 8080 (the default, which another acceptance run's service may hold),
# nor 8081 (P3's). The TURN workstream's trap — "check the environment
# of the process THAT REALLY LISTENS" — is paid for by plugging into a service
# one did not launch. We take a port of our own, and `SIGNALING_URL` follows.
PORT=8090
BASE_FICHIER="$SORTIE/plateforme-vm.sqlite"
rm -f "$BASE_FICHIER"

dire() { echo "[$(date +%H:%M:%S)] $*"; }
# Reads `vu_a` IN THE DATABASE, without going through the service: it is the column the
# heartbeat advances, and reading it here avoids judging freshness on the output of the
# very module we want to test.
vu_a() {
    node -e '
const { DatabaseSync } = require("node:sqlite");
const db = new DatabaseSync(process.argv[1]);
const l = db.prepare("SELECT vu_a FROM agent_enrole WHERE vm_id = ?").all(process.argv[2]);
console.log(l.length === 0 ? "aucune ligne agent_enrole" : String(l[0].vu_a));
' "$BASE_FICHIER" "$1" 2>/dev/null
}
# `curl` rather than `fetch`: we test the HTTP surface as it is
# served, without the slightest code of ours between it and the reading.
appel() { curl -s -o "$2" -w '%{http_code}' "${@:3}"; }

echo "# CORROBORATION SUR VM RÉELLE — TÂCHE 16, HORS CRITÈRE"
echo "# Jouée le $(date -Is), commit $(git -C "$RACINE" rev-parse --short HEAD)"
echo "# Durée de la phase page-shell : ${DUREE} s"
echo

# --- 0. the VM's state, BEFORE anything else ------------------------------------
dire '=== 0. état de la VM et du binaire ==='
virsh list --all 2>&1 | sed 's/^/    /'
if ! virsh list --state-running --name 2>/dev/null | grep -q '^Windows$'; then
    dire 'la VM est éteinte — démarrage'
    virsh start Windows 2>&1 | sed 's/^/    /'
fi
dire 'attente de WinRM puis du MONTAGE (pas du port : /media/vm se monte après)…'
for _ in $(seq 1 60); do timeout 3 bash -c 'echo > /dev/tcp/192.168.3.2/5985' 2>/dev/null && break; sleep 5; done
for _ in $(seq 1 60); do ls /media/vm/dev >/dev/null 2>&1 && break; sleep 5; done
dire "agent.exe : $(stat -c '%s octets, modifié %y' /media/vm/dev/target/release/agent.exe 2>&1)"
dire '   ⚠️ CE BINAIRE N’EST ATTRIBUABLE À AUCUN COMMIT DE P4 : il vient d’une'
dire '      compilation d’un chantier voisin. P4 ne touche aucun fichier Rust,'
dire '      donc il n’en exige aucun — mais la provenance est DITE, pas tue.'
BRUT="$(node "$RACINE/scripts/winrm.js" '(Get-Process agent -ErrorAction SilentlyContinue).Count' 2>/dev/null)"
if [[ "$BRUT" =~ ^[0-9]{1,3}$ ]]; then AGENTS="$BRUT"; else AGENTS='illisible'; fi
dire "Get-Process agent avant lancement : $AGENTS"
if [ "$AGENTS" != "0" ]; then
    dire '🔴 un agent tourne déjà — un superviseur vivant empêche le nouveau d’ouvrir'
    dire '   agent.log, et l’on relit alors le journal PÉRIMÉ. On tue.'
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
for _ in $(seq 1 60); do grep -q 'le port ' "$SORTIE/service.log" 2>/dev/null && break; sleep 0.5; done
grep 'le port ' "$SORTIE/service.log" | sed 's/^/    /' || { dire '🔴 le service n’a pas démarré'; cat "$SORTIE/service.log"; exit 1; }

ADMIN_ENV=(PLATEFORME_HOTE=192.168.3.1 PLATEFORME_BASE=sqlite
           PLATEFORME_BASE_URL="$BASE_FICHIER" PLATEFORME_SECRET_JETON="$SECRET_JETON")

# --- 2. enrolment, account, and the state BEFORE assignment -----------------------
dire '=== 2. npm run admin:agent ==='
ENROLEMENT="$(cd "$RACINE/plateforme" && env "${ADMIN_ENV[@]}" npm run --silent admin:agent -- --vm w1 --adresse 192.168.3.2 2>/dev/null)"
AGENT_VM="$(echo "$ENROLEMENT" | sed -n 's/^vm_id=//p')"
PREFIXE_ENROLE="$(echo "$ENROLEMENT" | sed -n 's/^prefixe=//p')"
AGENT_SECRET="$(echo "$ENROLEMENT" | sed -n 's/^AGENT_SECRET=//p')"
dire "    vm_id            = $AGENT_VM"
dire "    prefixe (enrôlé) = $PREFIXE_ENROLE"
dire "    secret           = <non imprimé — ${#AGENT_SECRET} caractères>"
[ -n "$AGENT_VM" ] && [ -n "$PREFIXE_ENROLE" ] && [ -n "$AGENT_SECRET" ] || { dire '🔴 enrôlement en échec'; echo "$ENROLEMENT"; exit 1; }

dire '=== 3. npm run admin:utilisateur ==='
UID_UTIL="$(cd "$RACINE/plateforme" && printf '%s\n' "$MOTDEPASSE" | env "${ADMIN_ENV[@]}" npm run --silent admin:utilisateur -- --email "$EMAIL" 2>/dev/null | tail -1)"
dire "    utilisateur = $UID_UTIL"
[ -n "$UID_UTIL" ] || { dire '🔴 création du compte en échec'; exit 1; }

dire '=== 4. POST /auth/connexion — un VRAI jeton, par la VRAIE route ==='
C="$(appel x "$SORTIE/auth.json" -X POST "http://192.168.3.1:$PORT/auth/connexion" \
       -H 'content-type: application/json' \
       -d "{\"email\":\"$EMAIL\",\"motdepasse\":\"$MOTDEPASSE\"}")"
ACCES="$(node -e 'console.log(JSON.parse(require("fs").readFileSync(process.argv[1],"utf8")).acces ?? "")' "$SORTIE/auth.json" 2>/dev/null)"
dire "    code = $C, jeton d’accès : ${#ACCES} caractères"
[ -n "$ACCES" ] || { dire '🔴 aucun jeton'; cat "$SORTIE/auth.json"; exit 1; }

dire '=== 5. POST /session AVANT toute attribution — attendu 409 aucune-vm ==='
C="$(appel x "$SORTIE/session-avant-attribution.json" -X POST "http://192.168.3.1:$PORT/session" -H "authorization: Bearer $ACCES")"
dire "    code = $C  corps = $(cat "$SORTIE/session-avant-attribution.json")"

dire '=== 6. npm run admin:attribuer ==='
(cd "$RACINE/plateforme" && env "${ADMIN_ENV[@]}" npm run --silent admin:attribuer -- --email "$EMAIL" --vm w1 2>&1) | sed 's/^/    /'

dire "=== 7. vu_a AVANT que l’agent ne batte : $(vu_a "$AGENT_VM") ==="
dire '=== 8. POST /session, attribuée mais AGENT MUET — attendu 503 agent-injoignable ==='
C="$(appel x "$SORTIE/session-agent-muet.json" -X POST "http://192.168.3.1:$PORT/session" -H "authorization: Bearer $ACCES")"
dire "    code = $C  corps = $(cat "$SORTIE/session-agent-muet.json")"

# --- 9. a window to capture ------------------------------------------------
dire '=== 9. ouverture d’une fenêtre éligible dans la session interactive ==='
cat > /media/vm/dev/p4-fenetre.ps1 <<'PS1'
Get-Process notepad -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Process notepad
Start-Sleep -Seconds 2
(Get-Process notepad | Select-Object -First 1).MainWindowTitle
PS1
node "$RACINE/scripts/winrm.js" \
  "schtasks /delete /tn p4-fenetre /f 2>\$null; \
   schtasks /create /tn p4-fenetre /f /it /ru '${WINDOWS_ADMIN_USERNAME:-Administrateur}' /rp '$WINDOWS_ADMIN_PASSWORD' \
     /sc once /st 00:00 \
     /tr 'powershell -WindowStyle Normal -NoProfile -ExecutionPolicy Bypass -File C:\\dev\\p4-fenetre.ps1'; \
   schtasks /run /tn p4-fenetre" 2>&1 | tail -2 | sed 's/^/    /'
sleep 6

# --- 10. l'agent --------------------------------------------------------------
dire '=== 10. lancement de l’agent, AVEC AGENT_VM et AGENT_SECRET ==='
SIGNALING_URL="ws://192.168.3.1:$PORT" \
SUPERVISEUR=1 \
RUST_LOG=info \
AGENT_VM="$AGENT_VM" \
AGENT_SECRET="$AGENT_SECRET" \
    "$RACINE/scripts/run-agent.sh" 2>&1 | sed 's/^/    /'
dire '    attente du premier battement…'
sleep 20
dire "=== 11. vu_a APRÈS le battement : $(vu_a "$AGENT_VM") ==="

dire '=== 12. POST /session, agent VIVANT — attendu 200 avec le préfixe ==='
C="$(appel x "$SORTIE/session-agent-vivant.json" -X POST "http://192.168.3.1:$PORT/session" -H "authorization: Bearer $ACCES")"
dire "    code = $C  corps = $(cat "$SORTIE/session-agent-vivant.json")"
PREFIXE_ROUTE="$(node -e 'console.log(JSON.parse(require("fs").readFileSync(process.argv[1],"utf8")).prefixe ?? "")' "$SORTIE/session-agent-vivant.json" 2>/dev/null)"

dire '=== 13. GET /vm ==='
C="$(appel x "$SORTIE/vm.json" "http://192.168.3.1:$PORT/vm" -H "authorization: Bearer $ACCES")"
dire "    code = $C  corps = $(cat "$SORTIE/vm.json")"

# --- 14. the shell page, on THE PREFIX RETURNED BY THE ROUTE ---------------------
dire "=== 14. page-shell scriptée sur le préfixe RENDU PAR LA ROUTE, ${DUREE} s ==="
dire "    préfixe employé : $PREFIXE_ROUTE"
"$RACINE/plateforme/node_modules/.bin/tsx" "$INSTRU_P3/shell-scripte.ts" \
    "ws://192.168.3.1:$PORT/" "$PREFIXE_ROUTE" "$SECRET_JETON" "$DUREE" \
    > "$SORTIE/shell.log" 2>&1
dire "    shell : $(grep -c '' "$SORTIE/shell.log") lignes"

dire '=== 15. GET /vm pendant/après la session — sessions_ouvertes ==='
C="$(appel x "$SORTIE/vm-apres.json" "http://192.168.3.1:$PORT/vm" -H "authorization: Bearer $ACCES")"
dire "    code = $C  corps = $(cat "$SORTIE/vm-apres.json")"

# --- 16. the stop, and the evidence -----------------------------------------------
dire '=== 16. arrêt de l’agent et copie des pièces ==='
node "$RACINE/scripts/winrm.js" 'Stop-Process -Name agent -Force -ErrorAction SilentlyContinue; Start-Sleep -Seconds 2; (Get-Process agent -ErrorAction SilentlyContinue).Count' 2>/dev/null | sed 's/^/    agents restants : /'
sleep 2
cp /media/vm/dev/agent.log "$SORTIE/agent.log"
sed 's/\x1b\[[0-9;]*m//g' "$SORTIE/agent.log" > "$SORTIE/agent-plat.log"
dire "    agent.log : $(grep -c '' "$SORTIE/agent-plat.log") lignes"

dire '=== 17. la table session, telle que la plateforme l’a écrite ==='
node -e '
const { DatabaseSync } = require("node:sqlite");
const db = new DatabaseSync(process.argv[1]);
for (const l of db.prepare("SELECT nom_session, utilisateur_id, vm_id, ouverte_a, fermee_a FROM session ORDER BY ouverte_a").all()) {
    console.log(JSON.stringify(l));
}
' "$BASE_FICHIER" 2>&1 | sed 's/^/    /'

dire '=== 18. le RELEVÉ, assertion par assertion ==='
node "$RACINE/docs/superpowers/plans/journaux-plateforme-p4/instrument/verdict-vm.mjs" \
    "$SORTIE" "$PREFIXE_ENROLE" 2>&1
CODE=$?

dire '=== 19. état final de la VM ==='
virsh list --all 2>&1 | sed 's/^/    /'
node "$RACINE/scripts/winrm.js" '(Get-Process agent -ErrorAction SilentlyContinue).Count' 2>/dev/null | sed 's/^/    agents restants : /'
echo
dire "terminé — code du verdict : $CODE (0 = tout tenu)"
exit $CODE
