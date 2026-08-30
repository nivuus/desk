#!/usr/bin/env bash
# Un cycle de bras du lot 32 : arrete l'agent, pose (ou retire) la variable de
# banc SORTIE_DESIGNEE, marque le journal, relance.
#   usage : cycle-de-bras.sh <arme|desarme> <etiquette>
#
# 🔴 DEUX PIEGES PAYES PAR CE SCRIPT MEME, LE 30 AOUT 2026. La premiere version
# posait la ligne par un `-replace` a travers trois couches de guillemets : il
# echouait en silence, et le fichier restait sans la variable. La seconde la
# posait par un simple `+=`, donc A LA FIN du run-agent.ps1 — c'est-a-dire
# APRES la ligne qui lance l'agent, donc JAMAIS EXECUTEE.
#
# LES DEUX FOIS, LE FICHIER CONTENAIT (ou non) LA LIGNE ET LE TRACE DE CODE
# AURAIT CONCLU A TORT. Ce qui l'a dit est la TRACE DANS LE JOURNAL. D'ou la
# verification finale ci-dessous, qui est le seul controle qui vaille : le
# piege de D1 (SUPERVISEUR), D2 (MULTIFENETRE_REPRISE) et D7 (AUDIO).
#
# ⚠️ La variable est posee dans C:\nivuus\agent\run-agent.ps1 — le script du
# package `console` qui lance REELLEMENT l'agent — et non dans le
# scripts/run-agent.sh du depot, qui vise la VM de developpement disparue.
set -euo pipefail
unset -f chpwd 2>/dev/null || true
cd /home/mallanic/Projects/Nivuus/packages/installer
W() { timeout 150 python3 console/guest/winrm_exec.py ps "$1" 2>&1 | grep -v CLIXML | grep -v '^<Objs'; }
MODE="$1"; ETIQ="$2"

# ⚠️ Un agent survivant tient agent.log : on relirait le journal de la
# tentative precedente en croyant lire le sien.
W '
Stop-ScheduledTask -TaskName guacamole-agent -ErrorAction SilentlyContinue
Start-Sleep -Seconds 2
Get-Process agent -ErrorAction SilentlyContinue | ForEach-Object { Stop-Process -Id $_.Id -Force }
Start-Sleep -Seconds 5
"agents avant relance (doit etre 0) : " + (@(Get-Process agent -ErrorAction SilentlyContinue).Count)'

if [ "$MODE" = "desarme" ]; then
  # Insertion APRES l'ancre env:SUPERVISEUR, donc AVANT le lancement.
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
