# Enveloppe : execute la sonde en SESSION 1 par tache planifiee /it, et rend
# son journal. 🔴 Un rendu ou une enumeration audio lances depuis WinRM vivent
# en SESSION 0, ou les points de terminaison de la session interactive ne sont
# pas ceux-la.
$ErrorActionPreference = 'Continue'
Remove-Item C:\dev\e3-defaut-capture.log -ErrorAction SilentlyContinue
schtasks /delete /tn e3-defaut /f 2>$null | Out-Null
schtasks /create /tn e3-defaut /f /it /ru $env:USERNAME /sc once /st 00:00 /tr 'powershell -NoProfile -ExecutionPolicy Bypass -File C:\dev\e3-defaut-capture.ps1' 2>$null | Out-Null
schtasks /run /tn e3-defaut 2>$null | Out-Null
Start-Sleep -Seconds 12
if (Test-Path C:\dev\e3-defaut-capture.log) { Get-Content C:\dev\e3-defaut-capture.log -Raw } else { 'AUCUN JOURNAL' }
