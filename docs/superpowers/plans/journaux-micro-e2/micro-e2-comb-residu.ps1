# REPRODUCTION d'un artefact observe a la premiere execution du temoin de
# silence : une CRETE non nulle mesuree alors que plus rien ne jouait, avec
# une AMPLITUDE nulle a toutes les frequences. On joue, on laisse le flux se
# fermer entierement, puis on juge.
Remove-Item C:\dev\micro-e2-ecoute-residu.log -Force -ErrorAction SilentlyContinue
schtasks /run /tn micro-e2-jouer *> $null
Start-Sleep -Seconds 45
& powershell -NoProfile -ExecutionPolicy Bypass -File C:\dev\micro-ecoute-e2.ps1 -Prefixe 'CABLE Output' -Secondes 8 -Journal C:\dev\micro-e2-ecoute-residu.log *> $null
Write-Output 'RESIDU TERMINE'
