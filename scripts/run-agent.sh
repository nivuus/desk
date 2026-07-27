#!/usr/bin/env bash
# Lance l'agent dans la session interactive (session 1) de la VM Windows.
#
# WinRM s'exécute en session 0 : un agent lancé directement par WinRM ne peut
# ni capturer une fenêtre (Windows.Graphics.Capture) ni injecter des entrées
# (SendInput), ces API ne franchissant pas la frontière de session. La tâche
# planifiée avec /IT s'exécute dans la session de l'utilisateur connecté.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TASK_NAME="guacamole-agent"
USER_NAME="${WINDOWS_ADMIN_USERNAME:-Administrateur}"
: "${WINDOWS_ADMIN_PASSWORD:?WINDOWS_ADMIN_PASSWORD non défini}"

# Les variables d'environnement passent par un script d'amorçage : schtasks ne
# permet pas de les transmettre directement.
cat > /media/vm/dev/run-agent.ps1 <<PS1
\$env:SIGNALING_URL = '${SIGNALING_URL:-ws://192.168.3.1:8080}'
\$env:SESSION_ID    = '${SESSION_ID:-demo}'
\$env:LOCAL_IP      = '${LOCAL_IP:-192.168.3.2}'
\$env:RUST_LOG      = '${RUST_LOG:-info}'
\$env:WINDOW_TITLE  = '${WINDOW_TITLE:-firefox}'
${TEST_FILE:+\$env:TEST_FILE = '$TEST_FILE'}
${CAPTURE_TEST:+\$env:CAPTURE_TEST = '$CAPTURE_TEST'}
${ENCODE_TEST:+\$env:ENCODE_TEST = '$ENCODE_TEST'}
& 'C:\dev\target\debug\agent.exe' *>&1 | Tee-Object -FilePath 'C:\dev\agent.log'
PS1

node "$ROOT/scripts/winrm.js" \
    "schtasks /delete /tn $TASK_NAME /f 2>\$null; \
     schtasks /create /tn $TASK_NAME /f /it /ru '$USER_NAME' /rp '$WINDOWS_ADMIN_PASSWORD' \
       /sc once /st 00:00 \
       /tr 'powershell -NoProfile -ExecutionPolicy Bypass -File C:\\dev\\run-agent.ps1'; \
     schtasks /run /tn $TASK_NAME"

echo "agent lancé en session interactive ; journal : /media/vm/dev/agent.log"
