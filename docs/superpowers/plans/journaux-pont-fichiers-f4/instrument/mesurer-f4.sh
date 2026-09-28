#!/usr/bin/env bash
# Copies the measurement to the VM, launches it THROUGH AN /it SCHEDULED TASK, and returns its JSON.
#
#     mesurer-f4.sh "<plan de gestes>" [repos_s]
#
# 🔴 SESSION 1 AND NOT WinRM (session 0). The ProjFS root is mounted by a
# session 1 process, and the Explorer of the `explorer` gesture has no
# desktop in session 0. One single path for all gestures is one path
# less to explain.
set -uo pipefail
PLAN="$1"
REPOS="${2:-25}"
RACINE="$(git rev-parse --show-toplevel)" || { echo "🔴 hors du depot git : impossible de deriver RACINE (git rev-parse a echoue)" >&2; exit 1; }
I="$RACINE/docs/superpowers/plans/journaux-pont-fichiers-f4/instrument"
set -a; source "$RACINE/.env"; set +a

# 🔴 KILL THE FROZEN MEASUREMENTS FIRST. A frozen attempt leaves its
# `powershell` alive, and that process HOLDS its reading file through the
# handle of its `Out-File`: the next measurement then writes into the void, and the
# reading is lost FOR A REASON THAT HAS NOTHING TO DO WITH WHAT IS MEASURED.
# The lock survived THREE attempts in F3.
#
# ⚠️ THE KILLER GOES THROUGH A FILE, NEVER THROUGH AN INLINE COMMAND:
# `nodejs-winrm` wraps everything in `powershell -Command "& { ... }"`, and an
# inline script with double quotes collides with that wrapper —
# the symptom being a script that NEVER RUNS.
cat > /media/vm/dev/tuer-mesures-f4.ps1 <<'PSKILL'
schtasks /end /tn mesure-f4 2>$null | Out-Null
Get-CimInstance Win32_Process | Where-Object { $_.CommandLine -like '*mesure-f4*' } |
    ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }
PSKILL
node "$RACINE/scripts/winrm.js" 'powershell -NoProfile -ExecutionPolicy Bypass -File C:\dev\tuer-mesures-f4.ps1' >/dev/null 2>&1

cp "$I/mesurer-f4.ps1" /media/vm/dev/mesurer-f4.ps1
# 🔴 A FRESH READING PATH PER RUN: "a frozen attempt holds its
# reading file, and the next one writes into the void".
HORO=$(date +%H%M%S%N | cut -c1-9)
SORTIE="C:\\dev\\mesure-f4-$HORO.json"
LOCAL="/media/vm/dev/mesure-f4-$HORO.json"
rm -f "$LOCAL" /media/vm/dev/mesure-f4.trace.txt

# 🔴 THE TRACE AND THE READING IN TWO DISTINCT FILES: `Out-File` truncates at
# opening and HOLDS the handle, so an interposed `Set-Content` — the
# checkpoints — would be lost or refused, PRECISELY when it matters.
cat > /media/vm/dev/lancer-mesure-f4.ps1 <<PS1
& 'C:\dev\mesurer-f4.ps1' -sortie '$SORTIE' -plan '$PLAN' -repos $REPOS *>&1 |
    Out-File -FilePath 'C:\dev\mesure-f4.trace.txt' -Encoding utf8
PS1

node "$RACINE/scripts/winrm.js" \
  "schtasks /delete /tn mesure-f4 /f 2>\$null; \
   schtasks /create /tn mesure-f4 /f /it /ru '$WINDOWS_ADMIN_USERNAME' /rp '$WINDOWS_ADMIN_PASSWORD' \
     /sc once /st 00:00 \
     /tr 'powershell -WindowStyle Hidden -NoProfile -ExecutionPolicy Bypass -File C:\dev\lancer-mesure-f4.ps1'; \
   schtasks /run /tn mesure-f4" >/dev/null 2>&1

# Wait for the FACT: the reading carries AS MANY gestures as the plan requests.
#
# 🔴 THERE IS NO JOB BOUND. `Start-Job` + `Wait-Job -Timeout` produces a
# DEADLOCK under a scheduled task (`BlockedJobsDeadlockWithWaitJob`, trace
# filed by F3). The ONLY bound that holds is the PRODUCT's — the budgets
# of `table.rs` —, and that is precisely what F4 measures. The wait here is therefore
# BOUNDED on the host side, and a PARTIAL reading is returned SAYING SO: "an announced
# partial reading is worth more than an empty reading".
# 🔴 THE AWAITED FACT IS THE `"fini":true` MARKER, AND NOT THE GESTURE COUNT.
# `Enregistrer` writes BEFORE the rest: waiting for the count handed back control as soon as
# the last gesture ended, the host killed Chrome, and THE BRIDGE CHANNEL DROPPED BEFORE
# THE TWO CENSUSES OF THE REST — that is, before the measurement itself.
ATTENDUS=$(echo "$PLAN" | tr ',' '\n' | grep -c .)
BUDGET=$(( ATTENDUS * (REPOS + 120) / 2 ))
COMPLET=non
for i in $(seq 1 "$BUDGET"); do
    if [ -s "$LOCAL" ] && grep -qa '"fini":true' "$LOCAL"; then COMPLET=oui; break; fi
    sleep 2
done
cp /media/vm/dev/mesure-f4.trace.txt "${TRACE:-/dev/null}" 2>/dev/null || true
if [ "$COMPLET" = non ]; then
    echo "MESURE PARTIELLE : $(grep -ao '"geste"' "$LOCAL" 2>/dev/null | wc -l)/$ATTENDUS gestes en $((BUDGET*2)) s." >&2
fi
cat "$LOCAL" 2>/dev/null || echo '{"erreur":"mesure-f4.json absent"}'
