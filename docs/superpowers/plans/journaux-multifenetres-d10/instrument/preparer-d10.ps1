# Etat de depart de la recette D10 (tache 10) : ZERO fenetre eligible ouverte
# quand le superviseur demarre. Les fenetres sont ouvertes une par une APRES
# lui -- le cas produit (un utilisateur ouvre une application), pas
# l'enumeration initiale.
#
# ATTENTION -- CE SCRIPT NE TOUCHE JAMAIS AU REGISTRE D'AFFICHAGE. Le registre
# est laisse SALE A DESSEIN : c'est l'etat que cette recette mesure (une
# sortie virtuelle nait a la derniere taille laissee par une mesure
# anterieure, pas a la taille demandee). Aucun appel a
# ChangeDisplaySettingsExW, aucun MULTIFENETRE_MODE_SORTIE ici.
#
# Repris de preparer-d4b.ps1 / preparer-d5.ps1 : la source doit BOUGER
# (Desktop Duplication n'emet une trame qu'au changement du bureau), donc une
# fenetre Chrome `--app` sur une page animee, avec un `--user-data-dir` par
# fenetre. On tue Chrome (et les applications residuelles des sondes
# precedentes) et on efface UNIQUEMENT les profils Chrome de CETTE recette
# (chrome-d10-*), pour qu'aucune bulle de restauration n'apparaisse -- on ne
# touche a aucun profil des sondes anterieures, ni a leurs fichiers.
Get-Process notepad, mspaint, wordpad, chrome -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep -Seconds 3
Get-ChildItem 'C:\dev' -Directory -Filter 'chrome-d10-*' -ErrorAction SilentlyContinue |
  Remove-Item -Recurse -Force -ErrorAction SilentlyContinue
Start-Sleep -Seconds 1
if (-not (Test-Path 'C:\dev\anim-d4.html')) { 'MANQUE anim-d4.html' | Set-Content 'C:\dev\preparer-d10.txt' }
else { 'pret (registre NON touche)' | Set-Content 'C:\dev\preparer-d10.txt' }
