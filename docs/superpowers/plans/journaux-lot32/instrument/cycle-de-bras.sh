#!/usr/bin/env bash
# Un cycle de bras : arrete l'agent, purge les sorties orphelines, pose (ou
# retire) SORTIE_DESIGNEE, marque le journal, relance.
#   usage : lot32-cycle.sh <arme|desarme> <etiquette>
set -euo pipefail
cd /home/mallanic/Projects/Nivuus/packages/installer
W() { python3 console/guest/winrm_exec.py ps "$1" 2>/dev/null | grep -v CLIXML | grep -v '^<Objs'; }
MODE="$1"; ETIQ="$2"

if [ "$MODE" = "desarme" ]; then LIGNE="\$env:SORTIE_DESIGNEE = '0'"; else LIGNE=""; fi

W "
Stop-ScheduledTask -TaskName guacamole-agent -ErrorAction SilentlyContinue
Get-Process agent -ErrorAction SilentlyContinue | ForEach-Object { Stop-Process -Id \$_.Id -Force }
Start-Sleep -Seconds 3
'agents encore vivants : ' + (@(Get-Process agent -ErrorAction SilentlyContinue).Count)

# Une sortie virtuelle survit a un arret brutal : le Drop ne court pas sur un
# TerminateProcess. Purge AVANT de relancer.
\$env:MULTIFENETRE_VDD_PURGE = '1'
& 'C:\nivuus\agent\agent.exe' 2>&1 | Select-String 'purge|orpheline|retiree' | Select-Object -First 6 | ForEach-Object { 'PURGE: ' + \$_.Line }
Remove-Item Env:\MULTIFENETRE_VDD_PURGE

# La variable de banc, posee dans le script QUI LANCE REELLEMENT.
\$p = 'C:\nivuus\agent\run-agent.ps1'
\$t = Get-Content \$p -Encoding UTF8 | Where-Object { \$_ -notmatch 'SORTIE_DESIGNEE' }
\$ligne = '$LIGNE'
if (\$ligne -ne '') { \$t = \$t -replace \"(\\\`\$env:SUPERVISEUR.*)\", \"\`\$1\`r\`n\$ligne\" }
Set-Content -Path \$p -Value \$t -Encoding UTF8
'SORTIE_DESIGNEE dans le script lance : ' + (@(Get-Content \$p -Encoding UTF8 | Select-String 'SORTIE_DESIGNEE').Count)

# Marqueur : c'est lui qui borne le releve de CE bras dans un journal de 15 Mio.
Add-Content -Path C:\nivuus\agent.log -Value ('===== LOT32 BRAS $ETIQ =====') -Encoding UTF8
Start-ScheduledTask -TaskName guacamole-agent
Start-Sleep -Seconds 12
'agents vivants apres relance : ' + (@(Get-Process agent -ErrorAction SilentlyContinue).Count)
"
