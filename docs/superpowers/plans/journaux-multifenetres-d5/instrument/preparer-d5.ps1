# Etat de depart de la recette D5 : ZERO fenetre eligible ouverte quand le
# superviseur demarre. Les fenetres sont ouvertes une par une APRES lui.
#
# Meme principe que `preparer-d4b.ps1` : la source doit BOUGER (Desktop
# Duplication n'emet une trame qu'au changement du bureau), donc une fenetre
# Chrome `--app` sur une page animee, avec un `--user-data-dir` par fenetre.
# Les profils de la recette precedente sont effaces pour qu'aucune bulle de
# restauration n'apparaisse.
Get-Process notepad, mspaint, wordpad, chrome -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep -Seconds 3
Get-ChildItem 'C:\dev' -Directory -Filter 'chrome-d5-*' -ErrorAction SilentlyContinue |
  Remove-Item -Recurse -Force -ErrorAction SilentlyContinue
Get-ChildItem 'C:\dev' -Directory -Filter 'chrome-d4-*' -ErrorAction SilentlyContinue |
  Remove-Item -Recurse -Force -ErrorAction SilentlyContinue
Start-Sleep -Seconds 1
if (-not (Test-Path 'C:\dev\anim-d4.html')) { 'MANQUE anim-d4.html' | Set-Content 'C:\dev\preparer-d5.txt' }
else { 'pret' | Set-Content 'C:\dev\preparer-d5.txt' }
