#!/usr/bin/env bash
# Checks that the agent, launched through an interactive scheduled task (/it), really runs
# in session 1 (graphical session of the logged-in user) and not in
# session 0 (services session). It is the most important point of the
# build chain: an agent in session 0 can neither capture a
# window (Windows.Graphics.Capture) nor inject input (SendInput).
#
# The current agent merely logs a message then quits (a few
# milliseconds of execution): a remote `Get-Process` launched after a
# `sleep` almost always arrives too late to observe it, which would make
# the check non-reproducible. This script thus captures the SessionId
# synchronously on the Windows side, at the very moment of launch, through the same
# mechanism as run-agent.sh (schtasks /it) — rather than guessing from
# outside whether the agent had time to start.

# ── DEAD PATH, 29 August 2026 — see scripts/voie-morte.sh ───────────────────
. "$(dirname "$0")/voie-morte.sh"
voie_morte "checked that the agent runs in session 1, by reading /media/vm" \
"     The appliance attests its session itself: C:\nivuus\state\agent-session.txt,
     written by run-agent.ps1 right before launching the agent, hence only if the
     launch really happened."
# ─── Below, the original body, kept as a historical record. ──────

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TASK_NAME="guacamole-agent-check-session"
USER_NAME="${WINDOWS_ADMIN_USERNAME:-Administrateur}"
: "${WINDOWS_ADMIN_PASSWORD:?WINDOWS_ADMIN_PASSWORD not set}"

if ! mountpoint -q /media/vm; then
    echo "error: /media/vm is not mounted" >&2
    exit 1
fi

RESULT_FILE="/media/vm/dev/check-session-result.txt"
rm -f "$RESULT_FILE"

# Bootstrap script: starts the agent and writes its SessionId to a file
# before giving it time to finish.
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

# schtasks /run returns before the task has really
# run: we wait for the result file to appear rather than an arbitrary fixed
# delay.
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
    echo "error: no session check result after 10s (the file $RESULT_FILE was never created)" >&2
    exit 1
fi

SESSION_ID="$(tr -d '[:space:]' < "$RESULT_FILE")"
rm -f /media/vm/dev/check-session.ps1 "$RESULT_FILE"

if [ "$SESSION_ID" = "1" ]; then
    echo "OK: the agent starts in session 1 (interactive session)"
    exit 0
fi

echo "error: the agent starts in session '${SESSION_ID:-unknown}', not 1." >&2
echo "check that the scheduled task carries the /it flag and that a user is logged in on the console (node scripts/winrm.js quser)." >&2
exit 1
