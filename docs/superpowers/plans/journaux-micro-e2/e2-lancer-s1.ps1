# Lance un script en session 1 par tache planifiee /it, puis attend.
param([string]$Script, [string]$Tache = 'e2-s1', [int]$AttenteSecondes = 6)
$ErrorActionPreference='Continue'
schtasks /delete /tn $Tache /f 2>$null | Out-Null
schtasks /create /tn $Tache /f /it /ru $env:USERNAME /sc once /st 00:00 /tr "powershell -WindowStyle Hidden -NoProfile -ExecutionPolicy Bypass -File $Script" 2>$null | Out-Null
schtasks /run /tn $Tache 2>$null | Out-Null
Start-Sleep -Seconds $AttenteSecondes
'LANCE ' + $Script
