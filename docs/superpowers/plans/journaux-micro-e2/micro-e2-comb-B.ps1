# B : rendu session 1 (tache /it), capture session 0 (ce processus WinRM).
Remove-Item C:\dev\micro-e2-ecoute-B.log -Force -ErrorAction SilentlyContinue
schtasks /run /tn micro-e2-jouer *> $null
Start-Sleep -Seconds 4
& powershell -NoProfile -ExecutionPolicy Bypass -File C:\dev\micro-ecoute-e2.ps1 -Prefixe 'CABLE Output' -Secondes 8 -Journal C:\dev\micro-e2-ecoute-B.log *> $null
Start-Sleep -Seconds 20
Copy-Item C:\dev\micro-e2-jouer.log C:\dev\micro-e2-jouer-B.log -Force
Write-Output 'COMBINAISON B TERMINEE'
