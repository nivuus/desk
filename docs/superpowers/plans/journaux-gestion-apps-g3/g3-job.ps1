$out = 'C:\dev\g3-job.txt'
function L($m) { $m | Out-File -FilePath $out -Append -Encoding utf8 }
"=== TACHE 2 : SONDE DU JOB OBJECT ===" | Out-File -FilePath $out -Encoding utf8
L ("releve : " + (Get-Date -Format 'HH:mm:ss'))

Add-Type -Namespace J -Name K -MemberDefinition @'
[DllImport("kernel32.dll", SetLastError=true)]
public static extern bool IsProcessInJob(IntPtr hProcess, IntPtr hJob, out bool result);
'@

$procs = Get-Process agent -ErrorAction SilentlyContinue
L ("processus agent vivants : " + ($procs | Measure-Object).Count)
L ""
L ("{0,-8} {1,-8} {2}" -f 'PID', 'PARENT', 'DANS UN JOB ?')
foreach ($p in $procs) {
    $parent = (Get-CimInstance Win32_Process -Filter ("ProcessId=" + $p.Id)).ParentProcessId
    $dedans = $false
    $ok = $false
    try { $ok = [J.K]::IsProcessInJob($p.Handle, [IntPtr]::Zero, [ref]$dedans) } catch { }
    if ($ok) {
        L ("{0,-8} {1,-8} {2}" -f $p.Id, $parent, $dedans)
    } else {
        L ("{0,-8} {1,-8} appel echoue, GetLastError={2}" -f $p.Id, $parent, [System.Runtime.InteropServices.Marshal]::GetLastWin32Error())
    }
}
L ""
L "-- temoin : le PowerShell de cette sonde lui-meme --"
$moi = [System.Diagnostics.Process]::GetCurrentProcess()
$d = $false
[void][J.K]::IsProcessInJob($moi.Handle, [IntPtr]::Zero, [ref]$d)
L ("  pid " + $moi.Id + " (powershell, tache planifiee) dans un job : " + $d)
L ""
L "-- temoin : un processus enfant CREE PAR cette sonde --"
$enf = Start-Process -FilePath 'C:\Windows\System32\notepad.exe' -PassThru
Start-Sleep -Milliseconds 800
$d2 = $false
[void][J.K]::IsProcessInJob($enf.Handle, [IntPtr]::Zero, [ref]$d2)
L ("  pid " + $enf.Id + " (notepad, enfant) dans un job : " + $d2)
Stop-Process -Id $enf.Id -Force -ErrorAction SilentlyContinue
