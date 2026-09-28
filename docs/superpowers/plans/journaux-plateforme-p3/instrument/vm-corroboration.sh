#!/usr/bin/env bash
# TÂCHE 23 — la corroboration sur VM réelle, HORS CRITÈRE.
#
#     instrument/vm-corroboration.sh <repertoire-de-sortie> [duree-secondes]
#
# 🔴 TOUT SE FAIT DANS UN SEUL APPEL, ET C'EST UNE CONTRAINTE, PAS UN STYLE.
# Une commande mise en arrière-plan par le harnais NE SURVIT PAS à la fin du
# tour de l'agent qui l'a lancée — deux recettes du sous-bloc D10 y ont perdu
# une exécution chacune, et le symptôme est un journal TRONQUÉ copié depuis une
# VM où l'agent, lui, continue de tourner. Le service de plateforme, la
# page-shell scriptée et l'agent doivent donc vivre et mourir dans cet appel-ci.
#
# ⚠️ AUCUN SECRET N'EST ÉCRIT DANS LE JOURNAL VERSÉ. Le secret d'enrôlement
# tiré par `npm run admin:agent` est capturé dans une variable et JAMAIS
# imprimé : le journal ne porte que sa longueur. Le secret de signature des
# jetons est fixe et sans valeur hors de cette exécution jetable.
set -uo pipefail

SORTIE="${1:?usage : vm-corroboration.sh <repertoire-de-sortie> [duree]}"
DUREE="${2:-60}"
RACINE="$(cd "$(dirname "$0")/../../../../.." && pwd)"
INSTRU="$RACINE/docs/superpowers/plans/journaux-plateforme-p3/instrument"
cd "$RACINE"
mkdir -p "$SORTIE"

set -a; source "$RACINE/.env"; set +a

SECRET_JETON='***RETIRE-DE-L-HISTORIQUE***'
# ⚠️ 8081, ET PAS 8080. Un service de plateforme d'une recette ANTÉRIEURE (le
# bloc E1) écoutait déjà sur 0.0.0.0:8080 depuis près de cinq heures au moment
# de cette exécution, avec sa propre base et son propre secret de signature.
# Le tuer aurait été un geste sur le travail d'un autre ; s'y brancher aurait
# fait mesurer un service dont l'environnement n'est pas le nôtre — c'est le
# piège que le chantier TURN a payé (« vérifier l'environnement du processus
# QUI ÉCOUTE RÉELLEMENT »). On prend un port libre, et `SIGNALING_URL` suit.
PORT=8081
BASE_FICHIER="$SORTIE/plateforme-vm.sqlite"
rm -f "$BASE_FICHIER"

dire() { echo "[$(date +%H:%M:%S)] $*"; }

# --- 0. l'état de la VM, AVANT toute chose ------------------------------------
dire '=== 0. état de la VM ==='
virsh list --all 2>&1 | sed 's/^/    /'
if ! virsh list --state-running --name 2>/dev/null | grep -q '^Windows$'; then
    dire 'la VM est éteinte — démarrage'
    virsh start Windows 2>&1 | sed 's/^/    /'
fi
# ⚠️ ON ATTEND LE MONTAGE, PAS LE PORT. `/media/vm` se monte APRÈS que WinRM
# répond, et son entrée CIFS survit à une VM éteinte : `mountpoint -q` rendrait
# vrai sur une connexion morte. Il faut éprouver un ACCÈS réel.
dire 'attente de WinRM puis du montage…'
for _ in $(seq 1 60); do timeout 3 bash -c 'echo > /dev/tcp/192.168.3.2/5985' 2>/dev/null && break; sleep 5; done
for _ in $(seq 1 60); do ls /media/vm/dev >/dev/null 2>&1 && break; sleep 5; done
dire "agent.exe sur la VM : $(stat -c '%s octets, %y' /media/vm/dev/target/release/agent.exe 2>&1)"
BRUT="$(node "$RACINE/scripts/winrm.js" '(Get-Process agent -ErrorAction SilentlyContinue).Count' 2>/dev/null)"
# ⚠️ On n'accepte QUE le cas où la commande a rendu un entier court. Un `tr -dc`
# sur un message d'erreur réseau rendrait « 192168325985… », c'est-à-dire un
# nombre qui n'est pas nul et qui déclencherait un `Stop-Process` inutile sur
# une VM injoignable — MESURÉ à la première exécution de ce script.
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

# --- 2. l'enrôlement ----------------------------------------------------------
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

# --- 3. une fenêtre à capturer ------------------------------------------------
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

# --- 4. l'agent, AVEC son identité --------------------------------------------
dire '=== 4. lancement de l’agent, AVEC AGENT_VM et AGENT_SECRET ==='
SIGNALING_URL="ws://192.168.3.1:$PORT" \
SUPERVISEUR=1 \
RUST_LOG=info \
AGENT_VM="$AGENT_VM" \
AGENT_SECRET="$AGENT_SECRET" \
    "$RACINE/scripts/run-agent.sh" 2>&1 | sed 's/^/    /'

# --- 5. la page-shell scriptée -------------------------------------------------
dire "=== 5. page-shell scriptée, $DUREE s ==="
"$RACINE/plateforme/node_modules/.bin/tsx" "$INSTRU/shell-scripte.ts" \
    "ws://192.168.3.1:$PORT/" "$PREFIXE" "$SECRET_JETON" "$DUREE" \
    > "$SORTIE/shell.log" 2>&1
dire "    shell : $(grep -c '' "$SORTIE/shell.log") lignes"

# --- 6. l'arrêt, et les pièces --------------------------------------------------
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

# --- 8. la ROUGE de la tâche 20 : SANS les variables ---------------------------
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
