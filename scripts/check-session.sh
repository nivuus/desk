#!/usr/bin/env bash
# Vérifie que l'agent, lancé via une tâche planifiée interactive (/it), tourne
# bien en session 1 (session graphique de l'utilisateur connecté) et non en
# session 0 (session des services). C'est le point le plus important de la
# chaîne de compilation : un agent en session 0 ne peut ni capturer une
# fenêtre (Windows.Graphics.Capture) ni injecter des entrées (SendInput).
#
# L'agent actuel se contente de logger un message puis quitte (quelques
# millisecondes d'exécution) : un `Get-Process` distant lancé après un
# `sleep` arrive presque toujours trop tard pour l'observer, ce qui rendrait
# la vérification non reproductible. Ce script capture donc le SessionId de
# façon synchrone côté Windows, au moment même du lancement, via le même
# mécanisme que run-agent.sh (schtasks /it) — plutôt que de deviner depuis
# l'extérieur si l'agent a eu le temps de démarrer.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TASK_NAME="guacamole-agent-check-session"
USER_NAME="${WINDOWS_ADMIN_USERNAME:-Administrateur}"
: "${WINDOWS_ADMIN_PASSWORD:?WINDOWS_ADMIN_PASSWORD non défini}"

if ! mountpoint -q /media/vm; then
    echo "erreur : /media/vm n'est pas monté" >&2
    exit 1
fi

RESULT_FILE="/media/vm/dev/check-session-result.txt"
rm -f "$RESULT_FILE"

# Script d'amorçage : démarre l'agent et écrit son SessionId dans un fichier
# avant de lui laisser le temps de se terminer.
cat > /media/vm/dev/check-session.ps1 <<PS1
\$p = Start-Process -FilePath 'C:\dev\target\debug\agent.exe' -PassThru -RedirectStandardOutput 'C:\dev\agent.log'
"\$(\$p.SessionId)" | Out-File -FilePath 'C:\dev\check-session-result.txt' -Encoding ascii -NoNewline
Start-Sleep -Milliseconds 800
PS1

node "$ROOT/scripts/winrm.js" \
    "schtasks /delete /tn $TASK_NAME /f 2>\$null; \
     schtasks /create /tn $TASK_NAME /f /it /ru '$USER_NAME' /rp '$WINDOWS_ADMIN_PASSWORD' \
       /sc once /st 00:00 \
       /tr 'powershell -NoProfile -ExecutionPolicy Bypass -File C:\\dev\\check-session.ps1'; \
     schtasks /run /tn $TASK_NAME" >/dev/null

# schtasks /run rend la main avant que la tâche ne se soit réellement
# exécutée : on attend l'apparition du fichier de résultat plutôt qu'un délai
# fixe arbitraire.
FOUND=0
for _ in $(seq 1 20); do
    if [ -f "$RESULT_FILE" ]; then
        FOUND=1
        break
    fi
    sleep 0.5
done

node "$ROOT/scripts/winrm.js" "schtasks /delete /tn $TASK_NAME /f" >/dev/null 2>&1 || true

if [ "$FOUND" -ne 1 ]; then
    echo "erreur : aucun résultat de vérification de session après 10s (le fichier $RESULT_FILE n'a jamais été créé)" >&2
    exit 1
fi

SESSION_ID="$(tr -d '[:space:]' < "$RESULT_FILE")"
rm -f /media/vm/dev/check-session.ps1 "$RESULT_FILE"

if [ "$SESSION_ID" = "1" ]; then
    echo "OK : l'agent démarre en session 1 (session interactive)"
    exit 0
fi

echo "erreur : l'agent démarre en session '${SESSION_ID:-inconnue}', pas 1." >&2
echo "vérifier la présence du drapeau /it dans la tâche planifiée et qu'un utilisateur est bien connecté sur la console (node scripts/winrm.js quser)." >&2
exit 1
