# C : rendu session 0 (fils detache de ce processus), capture session 1.
Remove-Item C:\dev\micro-e2-ecoute-C.log,C:\dev\micro-e2-jouer-s0.log -Force -ErrorAction SilentlyContinue
Start-Process powershell -WindowStyle Hidden -ArgumentList '-NoProfile','-ExecutionPolicy','Bypass','-File','C:\dev\micro-e2-jouer-s0.ps1' *> $null
Start-Sleep -Seconds 4
schtasks /run /tn micro-e2-ecouter-C *> $null
Start-Sleep -Seconds 26
Copy-Item C:\dev\micro-e2-jouer-s0.log C:\dev\micro-e2-jouer-C.log -Force
Write-Output 'COMBINAISON C TERMINEE'
