# État de départ de la mesure D6 tâche 1 : ZÉRO fenêtre éligible ouverte quand
# le superviseur démarre. LA fenêtre (une seule) est ouverte après lui.
#
# Même principe que `preparer-d5.ps1` : la source doit BOUGER (Desktop
# Duplication n'émet une trame qu'au changement du bureau), donc une fenêtre
# Chrome `--app` sur une page animée, avec son propre `--user-data-dir`.
Get-Process notepad, mspaint, wordpad, chrome -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep -Seconds 3
Get-ChildItem 'C:\dev' -Directory -Filter 'chrome-d*-*' -ErrorAction SilentlyContinue |
  Remove-Item -Recurse -Force -ErrorAction SilentlyContinue
Start-Sleep -Seconds 1
if (-not (Test-Path 'C:\dev\anim-d4.html')) { 'MANQUE anim-d4.html' | Set-Content 'C:\dev\preparer-d6.txt' }
else { 'pret' | Set-Content 'C:\dev\preparer-d6.txt' }
