$out = 'C:\dev\g3-porte-elev.txt'
function L($m) { $m | Out-File -FilePath $out -Append -Encoding utf8 }
"=== (b) UNE ELEVATION A-T-ELLE EU LIEU ? ===" | Out-File -FilePath $out -Encoding utf8
L ("depart : " + (Get-Date -Format 'HH:mm:ss.fff'))
L ("consent.exe AVANT : " + (Get-Process consent -ErrorAction SilentlyContinue | Measure-Object).Count)

$proc = $null
try {
    $proc = Start-Process -FilePath 'C:\Windows\System32\notepad.exe' -Verb RunAs -PassThru -ErrorAction Stop
    L ("Start-Process -Verb RunAs : ACCEPTE, pid=" + $proc.Id)
} catch {
    L ("Start-Process -Verb RunAs : REFUSE, message=" + $_.Exception.Message)
}

$vu = 0
for ($i = 0; $i -lt 12; $i++) {
    $n = (Get-Process consent -ErrorAction SilentlyContinue | Measure-Object).Count
    if ($n -gt 0) { $vu++ }
    L ("t+{0,2}s  consent={1}  notepad={2}" -f $i, $n, (Get-Process notepad -ErrorAction SilentlyContinue | Measure-Object).Count)
    Start-Sleep -Seconds 1
}
L ("consent.exe VU pendant la fenetre : " + $vu + " echantillons sur 12")

if ($proc -ne $null) {
    try {
        $h = Get-Process -Id $proc.Id -ErrorAction Stop
        L ("pid " + $proc.Id + " vivant, nom=" + $h.ProcessName)
    } catch { L ("pid " + $proc.Id + " deja mort") }
}
Get-Process notepad -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
L ("fin : " + (Get-Date -Format 'HH:mm:ss.fff'))
