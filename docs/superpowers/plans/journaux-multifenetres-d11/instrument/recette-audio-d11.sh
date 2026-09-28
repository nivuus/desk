#!/usr/bin/env bash
# Sub-block D11 — the VM SEQUENCE of acceptance runs ①, ② and ③, filed rather than
# retyped by hand: without it, the "VM" half of each measurement would live
# only in an agent's history, that is, nowhere (D10's leg 3,
# six findings lost with a gitignored report).
#
# It does NOT launch the driving browser: that is `pilote-audio-d11.mjs`, which
# runs on the HOST, never on the VM (on the VM its window would itself be
# captured, in cascade).
#
# Prerequisites, to check BEFORE (the plan's common preamble):
#   virsh list --all && virsh start Windows
#   until timeout 3 bash -c 'echo > /dev/tcp/192.168.3.2/5985'; do sleep 5; done
#   until ls /media/vm/dev >/dev/null 2>&1; do sleep 5; done
#   set -a && source .env && set +a
#
# ⚠️ `Get-Process agent` is checked again AFTER each attempt, including
# a failed one: a supervisor left alive prevents the new StreamWriter
# from opening `agent.log`, and the copy reread is the STALE one of the previous
# attempt. Met three times out of three in D8. Hence `tuer_agent` below.
#
# Usage :
#   recette-audio-d11.sh preparer
#   recette-audio-d11.sh ouvrir <n> <hz> <profil-chrome>
#   recette-audio-d11.sh fenetres
#   recette-audio-d11.sh tuer-agent
#   recette-audio-d11.sh copier <etiquette>
set -euo pipefail
RACINE="$(cd "$(dirname "$0")/../../../../.." && pwd)"
D11="$RACINE/docs/superpowers/plans/journaux-multifenetres-d11"
VMIT="$RACINE/docs/superpowers/plans/journaux-multifenetres-d10/instrument/vm-it.sh"
winrm() { node "$RACINE/scripts/winrm.js" "$1"; }

case "${1:?commande attendue}" in
preparer)
    # The registry IS NOT touched: the pollution left by the output mode
    # probes is irrelevant to single-window mode, and sub-block
    # D10 made the supervisor tolerant of an oversized output.
    bash "$VMIT" prep-d11 "Get-Process agent, chrome, notepad, mspaint -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep -Seconds 3"
    sleep 6
    winrm 'Get-Process agent,chrome -ErrorAction SilentlyContinue | Select-Object Id,ProcessName | Format-List | Out-String; Write-Output FIN'
    ;;
ouvrir)
    # `--user-data-dir`: ONE PER WINDOW for acceptance run ①; SHARED for ②
    # and ③, which require a SINGLE `chrome.exe` hence a single PID group.
    # It is the caller that decides, by passing the same profile or not.
    n="${2:?rang}"; hz="${3:?frequence}"; profil="${4:?dossier de profil chrome}"
    cp "$RACINE/docs/superpowers/plans/journaux-multifenetres-d7/instrument/ton.html" "/media/vm/dev/ton-$hz.html"
    bash "$VMIT" "ouvrir-ton-$hz" "\$a = @(
  '--app=file:///C:/dev/ton-$hz.html?hz=$hz&gain=0.25',
  '--user-data-dir=$profil',
  '--no-first-run','--no-default-browser-check','--disable-session-crashed-bubble',
  '--window-size=1280,720','--window-position=$((30 + n * 40)),$((30 + n * 40))',
  '--disable-features=CalculateNativeWinOcclusion',
  '--autoplay-policy=no-user-gesture-required',
  '--disable-background-timer-throttling','--disable-backgrounding-occluded-windows',
  '--disable-renderer-backgrounding')
Start-Process 'C:\Program Files\Google\Chrome\Application\chrome.exe' -ArgumentList \$a
Start-Sleep -Seconds 3"
    sleep 12
    ;;
fenetres)
    # READ the titles from the INTERACTIVE SESSION: `MainWindowTitle` read
    # from WinRM (session 0) returns empty for all session 1 windows.
    cp "$RACINE/docs/superpowers/plans/journaux-multifenetres-d10/instrument/listefen.ps1" /media/vm/dev/listefen.ps1
    bash "$VMIT" listefen 'powershell -NoProfile -ExecutionPolicy Bypass -File C:\dev\listefen.ps1'
    sleep 8
    cat /media/vm/dev/fenetres.txt
    ;;
tuer-agent)
    winrm 'Get-Process agent -ErrorAction SilentlyContinue | Stop-Process -Force; Start-Sleep -Seconds 3; Get-Process agent -ErrorAction SilentlyContinue | Select-Object Id | Format-List | Out-String; Write-Output AGENT-FIN'
    ;;
copier)
    # ⚠️ AFTER the real end of the run, hence after `tuer-agent`: the
    # children die when the browser closes, hence AFTER the driver.
    e="${2:?etiquette}"
    cp /media/vm/dev/agent.log "$D11/agent-$e.log"
    sed 's/\x1b\[[0-9;]*m//g' "$D11/agent-$e.log" > "$D11/agent-$e-plat.log"
    ;;
*) echo "commande inconnue : $1" >&2; exit 2 ;;
esac
