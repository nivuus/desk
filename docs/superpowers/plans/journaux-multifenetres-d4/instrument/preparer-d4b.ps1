# Etat de depart de la recette D4 (tache 11) : ZERO fenetre eligible ouverte
# quand le superviseur demarre. Les fenetres sont ouvertes une par une APRES
# lui : c'est le cas produit (un utilisateur ouvre une application), pas
# l'enumeration initiale.
#
# Difference avec `preparer-d4.ps1` (tache 9) : la source n'est plus le
# Bloc-notes, IMMOBILE -- Desktop Duplication n'emet une trame qu'au
# changement du bureau -- mais une fenetre Chrome `--app` sur une page
# animee. On tue donc aussi Chrome, et on efface les profils de la recette
# precedente pour qu'aucune bulle de restauration n'apparaisse.
Get-Process notepad, mspaint, wordpad, chrome -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep -Seconds 3
Get-ChildItem 'C:\dev' -Directory -Filter 'chrome-d4-*' -ErrorAction SilentlyContinue |
  Remove-Item -Recurse -Force -ErrorAction SilentlyContinue
Start-Sleep -Seconds 1
if (-not (Test-Path 'C:\dev\anim-d4.html')) { 'MANQUE anim-d4.html' | Set-Content 'C:\dev\preparer-d4b.txt' }
else { 'pret' | Set-Content 'C:\dev\preparer-d4b.txt' }
