$out = 'C:\dev\g3-surv.txt'
function L($m) { $m | Out-File -FilePath $out -Append -Encoding utf8 }
"=== CRITERE 7 : UN ENFANT SURVIT-IL A LA MORT DE SON LANCEUR ? ===" | Out-File -FilePath $out -Encoding utf8
Get-Process notepad -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
Start-Sleep -Milliseconds 500

# Voie 1 : enfant direct (ce que fait execution.rs aujourd'hui).
$direct = Start-Process -FilePath 'C:\Windows\System32\notepad.exe' -PassThru
L ("voie 1  enfant DIRECT             : pid=" + $direct.Id)

# Voie 2 : par le planificateur de taches, donc hors du job du lanceur.
schtasks /delete /tn g3enfant /f 2>$null | Out-Null
schtasks /create /tn g3enfant /f /it /ru $env:USERNAME /sc once /st 00:00 /tr 'C:\Windows\System32\notepad.exe' | Out-Null
schtasks /run /tn g3enfant | Out-Null
Start-Sleep -Seconds 3
$tous = Get-Process notepad -ErrorAction SilentlyContinue
$parTache = $tous | Where-Object { $_.Id -ne $direct.Id }
L ("voie 2  par le PLANIFICATEUR      : pid=" + (($parTache | ForEach-Object { $_.Id }) -join ','))
L ("pids notes : direct=" + $direct.Id + " planificateur=" + (($parTache | ForEach-Object { $_.Id }) -join ','))
L ("depart de l'attente : " + (Get-Date -Format 'HH:mm:ss'))
Start-Sleep -Seconds 25
L ("(ce lanceur n'a pas ete tue : ligne de controle) " + (Get-Date -Format 'HH:mm:ss'))
