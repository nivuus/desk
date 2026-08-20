# Remet la VM dans un etat propre entre deux executions.
#
# ⚠️ `Get-Process agent` se revérifie APRES CHAQUE tentative, y compris
# echouee : un superviseur vivant empeche le nouveau StreamWriter d'ouvrir
# `agent.log`, et la copie relue est celle, PERIMEE, de la tentative
# precedente -- on mesure le run d'avant en croyant lire le sien (D8, trois
# fois sur trois).
foreach ($n in 'agent','chrome','notepad','firefox') {
    Get-Process $n -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
}
Start-Sleep -Seconds 3
Write-Output '--- processus restants ---'
foreach ($n in 'agent','chrome','notepad','firefox') {
    $p = Get-Process $n -ErrorAction SilentlyContinue
    if ($p) { Write-Output ("$n : " + ($p | Measure-Object).Count) } else { Write-Output "$n : 0" }
}
# 🔴 LA RACINE EST RETIREE ENTRE DEUX EXECUTIONS, ET C'EST UNE NECESSITE DE
# MESURE, pas du confort.
#
# Une racine de virtualisation SURVIT a un arret brutal du superviseur : le
# `Drop` de `Virtualisation` (qui appelle `PrjStopVirtualizing`) ne court que
# sur une sortie NORMALE, jamais sur le `TerminateProcess` que le job object
# inflige (`agent/src/pont.rs`, en-tete de `executer`). Le repertoire, ses
# marqueurs et ses substituts restent donc sur le disque.
#
# MESURE : reutiliser une racine laissee par l'execution precedente a rendu une
# enumeration VIDE (`racine hydratee ... entrees=0`) alors que le montage
# annoncait un succes, et a laisse la creation locale reussir. Deux executions
# de suite n'auraient PAS mesure la meme chose -- et la seconde aurait eu l'air
# d'une panne du produit.
$r = Join-Path $env:USERPROFILE 'Mes Fichiers'
if (Test-Path $r) {
    Remove-Item -LiteralPath $r -Recurse -Force -ErrorAction SilentlyContinue
}
Write-Output '--- racine du pont ---'
Write-Output ("racine presente apres nettoyage : " + (Test-Path $r))
Remove-Item 'C:\gros-copie.bin' -Force -ErrorAction SilentlyContinue
Remove-Item 'C:\dev\mesure.txt' -Force -ErrorAction SilentlyContinue
