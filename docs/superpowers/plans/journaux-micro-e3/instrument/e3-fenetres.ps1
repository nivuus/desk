# Ouvre DEUX fenetres Bloc-notes en session 1 (tache planifiee /it), et rend
# leurs PID et leurs titres.
#
# 🔴 TACHE PLANIFIEE /it, ET NON WinRM : un processus lance depuis WinRM vit en
# SESSION 0, ou il n'a ni bureau visible ni point de terminaison audio. Une
# fenetre y serait invisible du superviseur.
#
# ⚠️ UN BLOC-NOTES FAIT AVANCER LE COMPTEUR DE SESSIONS DE DEUX (D2) : une
# seconde fenetre eligible et fugace est detectee par lancement. On COMPTE LES
# FENETRES DANS LE JOURNAL D'AGENT, jamais les lancements.
$ErrorActionPreference = 'Continue'
Get-Process notepad -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep -Seconds 1
foreach ($i in 1..2) {
    $n = "e3-fenetre-$i"
    schtasks /delete /tn $n /f 2>$null | Out-Null
    schtasks /create /tn $n /f /it /ru $env:USERNAME /sc once /st 00:00 /tr 'notepad.exe' 2>$null | Out-Null
    schtasks /run /tn $n 2>$null | Out-Null
    Start-Sleep -Seconds 3
}
Start-Sleep -Seconds 3
$p = Get-Process notepad -ErrorAction SilentlyContinue
if ($p) { ($p | ForEach-Object { 'NOTEPAD pid=' + $_.Id + ' titre=' + $_.MainWindowTitle }) -join "`n" } else { 'NOTEPAD aucun' }
