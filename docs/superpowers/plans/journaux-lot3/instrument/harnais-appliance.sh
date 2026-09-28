#!/usr/bin/env bash
# The batch 3 campaign harness, RE-LOCATED ONTO THE APPLIANCE. TO BE SOURCED.
#
#     source docs/superpowers/plans/journaux-lot3/instrument/harnais-appliance.sh
#
# 🔴 WHY THIS FILE EXISTS NEXT TO `harnais.sh`, AND DOES NOT REPLACE IT.
#
# `harnais.sh` was written on August 28th, 2026. On August 29th, 2026, the
# `package-nivuus` workstream switched the target VM: it is no longer the
# development machine, it is an APPLIANCE provisioned by the neighbouring package
# `packages/installer`. That switch removed, DELIBERATELY, the three
# supports `harnais.sh` rests on:
#
#   1. `/media/vm` — the CIFS mount //192.168.3.2/c. READ on September 5th,
#      2026: `mount | grep media/vm` returns NOTHING, and `/media/vm` is an
#      empty directory. `vm_prete` waits there for `ls /media/vm/dev` for 300 s
#      then returns 1. It can no longer return 0.
#   2. `C:\dev` — READ on September 5th, 2026: `Get-ChildItem C:\` does not
#      list it. The binary now lives in `C:\nivuus\agent\agent.exe`
#      and its log in `C:\nivuus\agent.log`.
#   3. `scripts/winrm.js` — Basic transport. READ on September 5th, 2026:
#      « Failed to process the request, status Code: » (the guest only offers
#      Negotiate since `Enable-PSRemoting`). The path that answers is
#      `console/guest/winrm_exec.py`, over NTLM.
#
# CLAUDE.md § "Windows VM lifecycle" states these three facts and
# concludes from them that batch 3 is SUSPENDED. This file lifts that suspension by
# re-locating the harness, and NOTHING ELSE: it does not decide the fate of
# `scripts/winrm.js`, of `/media/vm` nor of `scripts/build-agent.sh` — that fate
# belongs to the repository owner, and CLAUDE.md says so. `harnais.sh`
# therefore stays in place, intact: it is a piece dated August 28th, 2026.
#
# 🔴 THIS HARNESS JUDGES NOTHING, like the one it re-locates. It returns states and
# counts; it is the item that decides whether the state is the one it expected.
unset -f chpwd 2>/dev/null || true

RACINE_HARNAIS="$(git rev-parse --show-toplevel)"
# The neighbouring package, derived — never hard-coded: the repository has already paid for 48 scripts
# that carried `/home/mallanic/Projects/Guacamole` after a move.
CONSOLE_HARNAIS="$(cd "${RACINE_HARNAIS}/../installer" && pwd)"
WINRM_HARNAIS="${CONSOLE_HARNAIS}/console/guest/winrm_exec.py"
JOURNAL_VM='C:\nivuus\agent.log'
TACHE_VM='guacamole-agent'

# A PowerShell command on the guest. The filter removes the CLIXML
# PowerShell pours onto stderr at the first module load — it is not
# an error, and letting it through would pollute every reading.
W() {
    timeout "${2:-150}" python3 "${WINRM_HARNAIS}" ps "$1" 2>&1 \
        | grep -v 'CLIXML' | grep -v '^<Objs'
}

vm_prete() {
    virsh list --all | grep -q "Windows.*en cours" || virsh start Windows
    # 1. The WinRM port answers.
    local i
    for i in $(seq 1 60); do
        timeout 3 bash -c 'echo > /dev/tcp/192.168.3.2/5985' 2>/dev/null && break
        sleep 5
    done
    timeout 3 bash -c 'echo > /dev/tcp/192.168.3.2/5985' 2>/dev/null || {
        echo "🔴 le port 5985 ne répond pas"; return 1; }
    # 2. THEN a REAL ACCESS to the guest — it is the point of the original harness,
    #    transposed: an open port is not a WinRM session, exactly
    #    as a CIFS entry was not a live mount. We therefore test
    #    a COMPLETE ROUND TRIP that reads the agent's log, that is,
    #    the resource all items depend on.
    local size
    for i in $(seq 1 60); do
        size=$(W "(Get-Item ${JOURNAL_VM} -ErrorAction SilentlyContinue).Length" 30 | tr -dc '0-9')
        [ -n "${size}" ] && break
        sleep 5
    done
    [ -n "${size}" ] || { echo "🔴 WinRM ouvre mais ne rend pas ${JOURNAL_VM}"; return 1; }
    echo "vm prête (journal=${size} octets) : $(bash "${RACINE_HARNAIS}/docs/superpowers/plans/journaux-lot3/instrument/etat-vm.sh")"
}

agent_absent() {
    # 🔴 A surviving agent holds agent.log FOR WRITING, and one then rereads
    # the log of the PREVIOUS attempt believing one reads one's own. To
    # call before EACH attempt, including one that has just failed.
    #
    # ⚠️ The return code of `winrm_exec.py` is the ONLY way to distinguish
    # "the request succeeded and returns 0" from "the request failed and has nothing to
    # say": on a transport failure, the error message carries digits
    # (an address, a port) that a `tr -dc '0-9'` would pick up.
    local sortie code restants
    sortie=$(timeout 120 python3 "${WINRM_HARNAIS}" ps \
        '(@(Get-Process agent -ErrorAction SilentlyContinue)).Count' 2>/dev/null)
    code=$?
    if [ "${code}" -ne 0 ]; then
        echo "🔴 winrm_exec.py a échoué (code ${code}) : impossible de confirmer l'absence d'agent"
        return 1
    fi
    restants=$(printf '%s' "${sortie}" | tr -dc '0-9')
    [ "${restants:-0}" = "0" ] || { echo "🔴 ${restants} agent(s) survivant(s)"; return 1; }
    echo "aucun agent survivant"
}

agent_arreter() {
    # 🔴 By READ PID, never by pattern — and the scheduled task first:
    # it would restart the process we have just killed.
    W '
Stop-ScheduledTask -TaskName '"${TACHE_VM}"' -ErrorAction SilentlyContinue
Start-Sleep -Seconds 2
Get-Process agent -ErrorAction SilentlyContinue | ForEach-Object { Stop-Process -Id $_.Id -Force }
Start-Sleep -Seconds 5
"agents apres arret : " + (@(Get-Process agent -ErrorAction SilentlyContinue)).Count' 120
}

agent_relancer() {
    # $1 = seconds to wait before handing back control (default 25).
    W '
Start-ScheduledTask -TaskName '"${TACHE_VM}"'
Start-Sleep -Seconds '"${1:-25}"'
"agents vivants : " + (@(Get-Process agent -ErrorAction SilentlyContinue)).Count' 200
}

journal_reperer() {
    # Returns the log's NUMBER OF LINES at this instant: the marker that bounds
    # an arm. Without a marker, a `grep` over 50 MiB would return lines from a
    # previous arm, and the reading would be that of ANOTHER measurement — the repository has already
    # filed a second run's log under the first one's name (F1).
    #
    # 🔴 WHY A POSITION MARKER AND NOT A MARKER WRITTEN INTO THE LOG. The
    # first version of this harness set the marker through `Add-Content`,
    # like `journaux-lot32/instrument/cycle-de-bras.sh`. MEASURED on September 5th,
    # 2026: « The process cannot access the file 'C:\nivuus\agent.log' because
    # it is being used by another process » — the `StreamWriter` of
    # `run-agent.ps1` holds the file FOR WRITING as long as the agent runs.
    # `cycle-de-bras.sh` did not notice because it STOPS the agent
    # before marking; an arm measuring the LIVE agent cannot.
    # ⚠️ A marker failing silently would let one read the previous arm
    # believing one reads one's own: it is the exact defect the marker was meant
    # to prevent.
    W "@(Get-Content ${JOURNAL_VM} -Encoding UTF8).Count" 200 | tr -dc '0-9'
}

journal_depuis() {
    # Returns the log lines AFTER marker $1.
    W '
$t = Get-Content '"${JOURNAL_VM}"' -Encoding UTF8
if ($t.Count -le '"$1"') { "AUCUNE LIGNE APRES LE REPERE '"$1"' (total " + $t.Count + ")"; exit 0 }
$t['"$1"'..($t.Count-1)]' 250
}

variable_de_banc() {
    # Sets (or removes) a bench variable in the run-agent.ps1 GENERATED ON
    # THE VM — never in `scripts/run-agent.sh`, which targets the vanished
    # development VM.
    #   usage: variable_de_banc poser NAME VALUE | variable_de_banc retirer NAME
    #
    # 🔴 THE INSERTION IS ANCHORED ON `env:SUPERVISEUR`, AND THAT IS THE POINT.
    # A line added at the end of the file falls AFTER the call to agent.exe,
    # hence is NEVER executed — the file would contain it, a code trace
    # would conclude wrongly, and only the TRACE IN THE LOG would say so
    # (a trap paid for twice on August 30th, 2026, batch 32).
    local geste="$1" nom="$2" value="${3:-}" script
    # ⚠️ THE POWERSHELL LITERAL IS BUILT HERE, NOT NESTED INSIDE SHELL
    # QUOTES. The first wording stacked four levels of
    # quoting and produced `env:MICRO_PERIPHERIQUE = "NVIDIA""` — refused at
    # parse time (« Unexpected token »), hence NO variable set. It was
    # not the trace that said so, it was REREADING THE GENERATED run-agent.ps1,
    # which carried no line: the check this repository imposes, and which
    # served right here.
    if [ "${geste}" = "poser" ]; then
        script=$(cat <<PS
\$p = "C:\\nivuus\\agent\\run-agent.ps1"
\$l = @(Get-Content \$p -Encoding UTF8 | Where-Object { \$_ -notmatch "env:${nom}" })
\$idx = (\$l | Select-String "env:SUPERVISEUR" | Select-Object -First 1).LineNumber
if (-not \$idx) { throw "ancre env:SUPERVISEUR introuvable" }
\$neuf = @()
for (\$i=0; \$i -lt \$l.Count; \$i++) {
  \$neuf += \$l[\$i]
  if (\$i -eq (\$idx-1)) { \$neuf += ("{0}env:${nom} = '${value}'" -f [char]36) }
}
Set-Content -Path \$p -Value \$neuf -Encoding UTF8
"${nom} posee apres l ancre SUPERVISEUR (ligne \$idx)"
PS
)
    else
        script=$(cat <<PS
\$p = "C:\\nivuus\\agent\\run-agent.ps1"
Set-Content -Path \$p -Value @(Get-Content \$p -Encoding UTF8 | Where-Object { \$_ -notmatch "env:${nom}" }) -Encoding UTF8
"${nom} retiree"
PS
)
    fi
    W "${script}" 90
}
