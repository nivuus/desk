# TEMOIN POSITIF, rejoue sur l'etat courant de la VM : 660 Hz joues sur le
# rendu du cable en session 1, ecoutes sur CABLE Output en session 1.
# AUCUNE sortie avant la fin (nodejs-winrm tue le processus distant a la
# premiere ligne recue).
$ErrorActionPreference = 'Continue'
Remove-Item C:\dev\e2-temoin-ecoute.log -ErrorAction SilentlyContinue
schtasks /delete /tn e2-jouer /f *> $null
schtasks /create /tn e2-jouer /f /it /ru $env:USERNAME /sc once /st 00:00 /tr 'powershell -WindowStyle Hidden -NoProfile -ExecutionPolicy Bypass -File C:\dev\micro-e2-jouer.ps1' *> $null
schtasks /delete /tn e2-ecouter /f *> $null
schtasks /create /tn e2-ecouter /f /it /ru $env:USERNAME /sc once /st 00:00 /tr 'powershell -WindowStyle Hidden -NoProfile -ExecutionPolicy Bypass -File C:\dev\micro-ecoute-e2.ps1 -Prefixe ''CABLE Output'' -Secondes 8 -Journal C:\dev\e2-temoin-ecoute.log' *> $null
schtasks /run /tn e2-jouer *> $null
Start-Sleep -Seconds 4
schtasks /run /tn e2-ecouter *> $null
Start-Sleep -Seconds 26
'TEMOIN TERMINE'
