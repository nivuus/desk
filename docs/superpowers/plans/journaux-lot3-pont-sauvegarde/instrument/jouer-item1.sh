#!/usr/bin/env bash
# Batch 3, item 1 (3.7) — ONE complete run.
#
#     jouer-item1.sh <etiquette> [--rouge]
#
# It mounts the bridge with a REAL local root (OPFS), makes
# Notepad save INTO the ProjFS root from SESSION 1, and rereads the local machine.
#
# 🔴 TWO RUNS NEVER OVERLAP. F1 lost one that way: two
# overlapped by 2 min 23 s, the second killed the first one's agent IN THE MIDDLE OF
# MEASURING, and the log filed under the first one's name was the
# second's.
set -uo pipefail
unset -f chpwd 2>/dev/null || true

ETIQUETTE="${1:?usage : jouer-item1.sh <etiquette> [--rouge]}"
ROUGE="${2:-}"
RACINE="$(git rev-parse --show-toplevel)" || {
    echo "🔴 hors du dépôt git : impossible de dériver RACINE" >&2; exit 1; }
I="${RACINE}/docs/superpowers/plans/journaux-lot3-pont-sauvegarde/instrument"
J="${RACINE}/docs/superpowers/plans/journaux-lot3-pont-sauvegarde"
CONSOLE="$(cd "${RACINE}/../installer" && pwd)"

set -a; . "${RACINE}/.env"; set +a
. "${RACINE}/docs/superpowers/plans/journaux-lot3/instrument/harnais-appliance.sh"

RACINE_PROJFS='C:\Users\Administrator\Mes Fichiers'
FILE='item1-sauvegarde.txt'
PORT_HTTP=8099

etape() { echo; echo "=== $(date -Is) $* ==="; }

# 🔴 FREE THE PORT BY READ PID, never by pattern. An orphan of a
# previous run ANSWERS IN PLACE of ours, from the wrong
# directory, and the symptom is a 404 that reads as a failed upload — paid
# for twice by this repository (batches 32Q and 32T).
servir() {
    local pid
    for pid in $(ss -ltnp 2>/dev/null | grep ":${PORT_HTTP}" \
                 | grep -o 'pid=[0-9]*' | cut -d= -f2 | sort -u); do
        echo "port ${PORT_HTTP} tenu par le PID ${pid}, je le libère"; kill -9 "${pid}" || true
    done
    sleep 1
    # `--directory` removes the `cd`: without it, `$!` designates the SUBSHELL, the
    # kill kills it, and python survives.
    python3 -m http.server "${PORT_HTTP}" --bind 192.168.3.1 --directory "$1" >/dev/null 2>&1 &
    echo $! > /var/tmp/lot3-item1-http.pid
    sleep 2
    curl -sf -o /dev/null "http://192.168.3.1:${PORT_HTTP}/" || {
        echo "🔴 rien ne sert $1"; exit 1; }
}
arreter_le_service() {
    kill "$(cat /var/tmp/lot3-item1-http.pid)" 2>/dev/null || true
    for _ in 1 2 3 4 5; do
        curl -sf -o /dev/null --max-time 1 "http://192.168.3.1:${PORT_HTTP}/" || return 0
        sleep 1
    done
    echo "🔴 le serveur HTTP survit au kill"; exit 1
}

etape "PRÉ-VOL : la VM, et le dépôt de l'observateur"
vm_prete || exit 1
servir "${I}"
W '
$ProgressPreference = "SilentlyContinue"
Invoke-WebRequest -Uri "http://192.168.3.1:'"${PORT_HTTP}"'/observer-editeurs.ps1" -OutFile "C:\nivuus\observer-editeurs.ps1" -UseBasicParsing
"observateur sha256 : " + (Get-FileHash C:\nivuus\observer-editeurs.ps1 -Algorithm SHA256).Hash' 120
arreter_le_service
# 🔴 THE CHECK THAT COUNTS IS COMPARING THE TWO SHAs: it is what
# caught the orphan server defect both times, where no return
# code could.
echo "observateur local sha256 : $(sha256sum "${I}/observer-editeurs.ps1" | tr 'a-f' 'A-F' | cut -c1-64)"

# The /it scheduled task: SESSION 1 IS MANDATORY. SendKeys reaches
# no desktop from session 0, and the observer PRINTS its session rather
# than assuming it.
# 🔴 TWO WRITING DEFECTS OF THIS `.cmd`, PAID FOR ONE AFTER THE OTHER ON
# SEPTEMBER 5TH, 2026, AND THE SECOND READ LIKE THE FIRST:
#   ① `\"` escapes NOTHING in PowerShell — the escape is the backtick.
#      The file received `-Racine \"C:\Users\…` literally, cmd.exe
#      cut the argument at the first space. Symptom: `item1-sortie.txt`
#      ABSENT, the observer's log EMPTY, `LastTaskResult` never read.
#   ② The `-f` operator has a LOWER precedence than the comma:
#      `@("a", ("b") -f $x)` reads `@( ("a","b") -f $x )`, so it formats
#      THE WHOLE ARRAY and returns ONE SINGLE string. The `.cmd` came out on ONE
#      line — `@echo off powershell.exe …` — and cmd.exe ran `echo`
#      with all the rest as arguments. Symptom: `LastTaskResult = 0`, an
#      output that looks like the command, and NOTHING executed.
# 🔵 The remedy is to use NEITHER `-f` NOR escaping: a variable
# `$q = [char]34`, interpolated through `${q}`. And the check that counts is to
# REREAD the deposited file COUNTING ITS LINES — the `.cmd` must have
# TWO. Without that count, ② is invisible.
W '$q = [char]34
$ligne = "powershell.exe -NoProfile -ExecutionPolicy Bypass -File C:\nivuus\observer-editeurs.ps1 " +
         "-Racine ${q}C:\Users\Administrator\Mes Fichiers${q} -Fichier item1-sauvegarde.txt " +
         "> C:\nivuus\item1-sortie.txt 2>&1"
Set-Content -Path C:\nivuus\item1.cmd -Encoding ASCII -Value @("@echo off", $ligne)
schtasks /create /tn lot3-item1 /tr C:\nivuus\item1.cmd /sc once /st 00:00 /it /ru Administrator /rl HIGHEST /f | Out-Null
"--- le .cmd REELLEMENT depose : il doit faire DEUX lignes ---"
"lignes = " + @(Get-Content C:\nivuus\item1.cmd).Count
Get-Content C:\nivuus\item1.cmd | ForEach-Object { "  | " + $_ }' 120

if [ "${ROUGE}" = "--rouge" ]; then
    etape "BRAS ROUGE : PONT_ECRITURE=0"
    # 🔴 WHY `PONT_ECRITURE=0` AND NOT `PONT_MUTATION=0`, WHICH THE PLAN
    # PRESCRIBED — AND WHY IT IS NOT THE TRAP IT NAMES.
    #
    # The plan writes: "Do not use PONT_ECRITURE=0 in its place: it sets
    # inscriptible=false, which makes PRE_ refuse for ANOTHER reason —
    # one of F3's red runs was DISQUALIFIED there." That warning targets F3's
    # criterion, which is a RENAME: disarming writing there would turn red for a
    # reason foreign to renaming.
    #
    # ⚠️ BUT THIS ITEM'S CRITERION IS NOT A RENAME. It is
    # "does the save arrive WHOLE on the local machine?", and the measurement of
    # run 1 established that Notepad writes IN PLACE in the ProjFS
    # root — no temporary, no rename, in all THREE inventories.
    # `PONT_MUTATION=0` disarms renaming and deletion: on a path
    # that takes NONE of them, it would change nothing, and the red run would be GREEN
    # for a reason that has nothing to do with what it claims to test.
    #
    # 🔵 The disarming that bites on THIS mechanism is `PONT_ECRITURE=0`: the
    # bridge keeps detecting, logging and counting the owed
    # writes, AND PUSHES NONE OF THEM. It is exactly the reverse of the judging figure.
    agent_arreter
    variable_de_banc poser PONT_ECRITURE 0
    W 'Get-Content C:\nivuus\agent\run-agent.ps1 -Encoding UTF8 | Select-String "env:PONT_ECRITURE|agent.exe" | ForEach-Object { "ligne $($_.LineNumber) : $($_.Line.Trim())" }' 90
    agent_relancer 30
fi

REPERE=$(journal_reperer)
etape "REPÈRE du journal de l'agent : ${REPERE}"

etape "LE PILOTE : monte le pont, puis TIENT la session pendant que l'invité écrit"
nohup node "${I}/pilote-item1.mjs" --etiquette="${ETIQUETTE}" --maintien=120 \
      --sortie="${J}/sequence-${ETIQUETTE}.json" > "${J}/pilote-${ETIQUETTE}.log" 2>&1 &
PILOTE=$!
echo "pilote pid=${PILOTE}"

# Wait for the bridge to be REALLY mounted, never a duration: it is a FACT
# we wait for, and the agent publishes it.
for _ in $(seq 1 40); do
    sleep 3
    if grep -aq "pont monté côté navigateur" "${J}/pilote-${ETIQUETTE}.log" 2>/dev/null; then
        echo "pont monté (vu dans le journal du pilote)"; break
    fi
done
grep -a "racine locale préparée\|pont monté\|clic sur" "${J}/pilote-${ETIQUETTE}.log" || true

etape "L'OBSERVATEUR, en SESSION 1 : le Bloc-notes ouvre, écrit, enregistre, ferme"
W 'Remove-Item C:\nivuus\item1-sortie.txt -Force -ErrorAction SilentlyContinue
   schtasks /run /tn lot3-item1 | Out-Null
   "observateur lance"' 90
sleep 35
# 🔴 THE TASK'S CODE BEFORE ITS OUTPUT: an empty output has TWO causes — the
# task did not run, or it ran without writing anything — and only
# `LastTaskResult` separates them.
W '$t = Get-ScheduledTaskInfo -TaskName lot3-item1
   "LastTaskResult = " + $t.LastTaskResult
   "LastRunTime    = " + $t.LastRunTime
   "--- sortie ---"
   Get-Content C:\nivuus\item1-sortie.txt -ErrorAction SilentlyContinue' 120 \
  | tee "${J}/observateur-${ETIQUETTE}.txt"

etape "ATTENTE de la fin du pilote (il relit le poste local APRÈS)"
wait "${PILOTE}" 2>/dev/null
tail -6 "${J}/pilote-${ETIQUETTE}.log"

etape "CE QUE LE PONT A NOTIFIÉ — les notifications ProjFS, dans l'ordre"
journal_depuis "${REPERE}" | sed 's/\x1b\[[0-9;]*m//g' \
  | grep -aiE "pont|projfs|PRE_RENAME|PRE_DELETE|notification|ecriture|poussee|mutation" \
  | tee "${J}/notifications-${ETIQUETTE}.log" | tail -30

if [ "${ROUGE}" = "--rouge" ]; then
    etape "RETOUR À L'ÉTAT LIVRÉ"
    agent_arreter
    variable_de_banc retirer PONT_ECRITURE
    agent_relancer 30
fi

etape "MÉNAGE"
# ⚠️ THE TASK GOES, THE EVIDENCE STAYS. The first wording erased
# `observer-editeurs.ps1`, `item1.cmd` and the output — so that when
# diagnosing a failure, THERE WAS NOTHING LEFT TO READ. A clean-up that destroys
# what explains the failure is a clean-up that costs a run.
W 'schtasks /delete /tn lot3-item1 /f 2>$null | Out-Null
   "taches lot3 restantes : " + (@(Get-ScheduledTask | Where-Object { $_.TaskName -match "lot3" })).Count
   "pieces conservees : " + ((Test-Path C:\nivuus\observer-editeurs.ps1) -and (Test-Path C:\nivuus\item1.cmd))' 90
bash "${RACINE}/docs/superpowers/plans/journaux-lot3/instrument/etat-vm.sh"
