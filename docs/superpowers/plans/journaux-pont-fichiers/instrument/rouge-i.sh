#!/usr/bin/env bash
# Plays red run (i) of Step 6 and waits for its verdict.
set -uo pipefail
RACINE="$(git rev-parse --show-toplevel)" || { echo "🔴 hors du depot git : impossible de deriver RACINE (git rev-parse a echoue)" >&2; exit 1; }
set -a; source "$RACINE/.env"; set +a
rm -f /media/vm/dev/rouge-i.txt

# The bridge's PID, READ from the last `file bridge launched pid=` line of the
# log. ⚠️ The string is indeed `file bridge launched`, NOT `bridge launched`, which
# F1's plan prescribes at Step 4: the latter matches NO trace and would return
# 0, which would read as an absent bridge.
PID_PONT=$(sed 's/\x1b\[[0-9;]*m//g' /media/vm/dev/agent.log \
    | grep -a 'pont fichiers lancé pid=' | tail -1 \
    | sed 's/.*pid=\([0-9]*\).*/\1/')
if [ -z "$PID_PONT" ]; then echo '# AUCUN pid de pont dans le journal'; exit 1; fi
echo "# pid du pont releve dans agent.log : $PID_PONT"
printf '%s' "$PID_PONT" > /media/vm/dev/pid-pont.txt
# ⚠️ DIRECT CALL, IN THE BACKGROUND -- NO `Start-Process`.
# `Start-Process` launched from a PowerShell invoked by WinRM proved
# UNRELIABLE: it does return a PID, and the target script does not run (no
# output file created). Measured: the same script invoked DIRECTLY through
# `-File` writes its file immediately. We therefore launch directly, in the
# background, and wait for the FACT -- the WinRM call may well hand back
# control before the end of the script, it does not matter here.
node "$RACINE/scripts/winrm.js" \
  'powershell -NoProfile -ExecutionPolicy Bypass -File C:\dev\rouge-i.ps1' >/dev/null 2>&1 &
for _ in $(seq 1 60); do
    grep -aq 'FIN ROUGE I' /media/vm/dev/rouge-i.txt 2>/dev/null && break
    sleep 3
done
cat /media/vm/dev/rouge-i.txt 2>/dev/null || echo '# aucun fichier'
