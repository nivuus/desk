# TEMOIN NEGATIF de l'instrument : le juge, sans que rien ne joue.
# Sans lui, « le cable ne porte pas » et « le juge ne sait pas mesurer » se
# liraient pareil.
Remove-Item C:\dev\micro-e2-ecoute-silence.log -Force -ErrorAction SilentlyContinue
& powershell -NoProfile -ExecutionPolicy Bypass -File C:\dev\micro-ecoute-e2.ps1 -Prefixe 'CABLE Output' -Secondes 8 -Journal C:\dev\micro-e2-ecoute-silence.log *> $null
Write-Output 'SILENCE TERMINE'
