# D : rendu session 0 et capture session 0. Le moins cher, le moins concluant.
Remove-Item C:\dev\micro-e2-ecoute-D.log,C:\dev\micro-e2-jouer-s0.log -Force -ErrorAction SilentlyContinue
Start-Process powershell -WindowStyle Hidden -ArgumentList '-NoProfile','-ExecutionPolicy','Bypass','-File','C:\dev\micro-e2-jouer-s0.ps1' *> $null
Start-Sleep -Seconds 4
& powershell -NoProfile -ExecutionPolicy Bypass -File C:\dev\micro-ecoute-e2.ps1 -Prefixe 'CABLE Output' -Secondes 8 -Journal C:\dev\micro-e2-ecoute-D.log *> $null
Start-Sleep -Seconds 20
Copy-Item C:\dev\micro-e2-jouer-s0.log C:\dev\micro-e2-jouer-D.log -Force
Write-Output 'COMBINAISON D TERMINEE'
