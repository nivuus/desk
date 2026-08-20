# Tue tout agent vivant et arrete la tache, AVANT toute nouvelle tentative.
# 🔴 Un agent survivant tient C:\dev\agent.log : le nouveau StreamWriter ne peut
# pas l'ouvrir, et l'on relit alors le journal de la tentative PRECEDENTE en
# croyant lire le sien. Paye trois fois sur trois en D8, et une fois de plus ici.
$ErrorActionPreference='Continue'
schtasks /end /tn guacamole-agent *> $null
$p = Get-Process agent -ErrorAction SilentlyContinue
if ($p) { $p | Stop-Process -Force -ErrorAction SilentlyContinue }
Start-Sleep -Seconds 3
$r = Get-Process agent -ErrorAction SilentlyContinue
if ($r) { 'RESTE ' + @($r).Count } else { 'AUCUN AGENT' }
