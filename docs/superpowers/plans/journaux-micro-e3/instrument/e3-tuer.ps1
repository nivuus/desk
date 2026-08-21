# Tue tout agent survivant ET les fenetres de recette. Sans condition.
#
# 🔴 Un agent survivant tient C:\dev\agent.log : le nouveau StreamWriter ne peut
# pas l'ouvrir, et l'on relit alors le journal de la tentative PRECEDENTE en
# croyant lire le sien. Paye trois fois en D8, une fois de plus en E2.
$ErrorActionPreference = 'Continue'
Get-Process agent -ErrorAction SilentlyContinue | Stop-Process -Force
Get-Process notepad -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep -Seconds 2
'AGENTS_RESTANTS=' + (Get-Process agent -ErrorAction SilentlyContinue | Measure-Object).Count
