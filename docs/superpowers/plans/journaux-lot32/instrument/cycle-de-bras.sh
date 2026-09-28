#!/usr/bin/env bash
# One arm cycle of batch 32: stops the agent, sets (or removes) the bench
# variable SORTIE_DESIGNEE, marks the log, restarts.
#   usage: cycle-de-bras.sh <arme|desarme> <label>
#
# 🔴 TWO TRAPS PAID FOR BY THIS VERY SCRIPT, ON AUGUST 30TH, 2026. The first version
# set the line through a `-replace` across three layers of quotes: it
# failed silently, and the file stayed without the variable. The second
# set it through a simple `+=`, hence AT THE END of run-agent.ps1 — that is,
# AFTER the line launching the agent, hence NEVER EXECUTED.
#
# BOTH TIMES, THE FILE CONTAINED (or not) THE LINE AND A CODE TRACE
# WOULD HAVE CONCLUDED WRONGLY. What said so is the TRACE IN THE LOG. Hence the
# final check below, which is the only check that counts: the
# trap of D1 (SUPERVISEUR), D2 (MULTIFENETRE_REPRISE) and D7 (AUDIO).
#
# ⚠️ The variable is set in C:\nivuus\agent\run-agent.ps1 — the script of the
# `console` package that REALLY launches the agent — and not in the
# repository's scripts/run-agent.sh, which targets the vanished development VM.
set -euo pipefail
unset -f chpwd 2>/dev/null || true
cd /home/mallanic/Projects/Nivuus/packages/installer
W() { timeout 150 python3 console/guest/winrm_exec.py ps "$1" 2>&1 | grep -v CLIXML | grep -v '^<Objs'; }
MODE="$1"; ETIQ="$2"

# ⚠️ A surviving agent holds agent.log: we would reread the log of the
# previous attempt believing we read our own.
W '
Stop-ScheduledTask -TaskName guacamole-agent -ErrorAction SilentlyContinue
Start-Sleep -Seconds 2
Get-Process agent -ErrorAction SilentlyContinue | ForEach-Object { Stop-Process -Id $_.Id -Force }
Start-Sleep -Seconds 5
"agents avant relance (doit etre 0) : " + (@(Get-Process agent -ErrorAction SilentlyContinue).Count)'

if [ "$MODE" = "desarme" ]; then
  # Insertion AFTER the env:SUPERVISEUR anchor, hence BEFORE the launch.
  W '
$p = "C:\nivuus\agent\run-agent.ps1"
$l = @(Get-Content $p -Encoding UTF8 | Where-Object { $_ -notmatch "SORTIE_DESIGNEE" })
$idx = ($l | Select-String "env:SUPERVISEUR" | Select-Object -First 1).LineNumber
if (-not $idx) { throw "ancre env:SUPERVISEUR introuvable" }
$neuf = @()
for ($i=0; $i -lt $l.Count; $i++) {
  $neuf += $l[$i]
  if ($i -eq ($idx-1)) { $neuf += ("{0}env:SORTIE_DESIGNEE = ''0''" -f [char]36) }
}
Set-Content -Path $p -Value $neuf -Encoding UTF8
"ligne posee apres l ancre SUPERVISEUR (ligne $idx)"'
else
  W '
$p = "C:\nivuus\agent\run-agent.ps1"
Set-Content -Path $p -Value @(Get-Content $p -Encoding UTF8 | Where-Object { $_ -notmatch "SORTIE_DESIGNEE" }) -Encoding UTF8
"ligne retiree"'
fi

W "
Add-Content -Path C:\\nivuus\\agent.log -Value '===== LOT32 BRAS $ETIQ =====' -Encoding UTF8
Start-ScheduledTask -TaskName guacamole-agent
Start-Sleep -Seconds 18
'agents vivants : ' + (@(Get-Process agent -ErrorAction SilentlyContinue).Count)
\$t = Get-Content C:\\nivuus\\agent.log -Encoding UTF8
\$i = (\$t | Select-String '===== LOT32 BRAS $ETIQ =====' | Select-Object -Last 1).LineNumber
\$bras = \$t[(\$i)..(\$t.Count-1)]
'TRACE DE DESARMEMENT (le seul controle qui vaille) : ' + @(\$bras | Select-String 'designation de sortie DESARMEE').Count"
