# Lot 32K : peut-on assigner a NOTRE job un processus DEJA dans le job du
# Planificateur ? C est la precondition de la voie (b), et elle n est pas
# supposable : les jobs imbriques existent depuis Windows 8, mais les limites
# du job englobant peuvent refuser. ASCII pur, imprime sa session.
$src = @"
using System; using System.Runtime.InteropServices;
public class J {
  [DllImport("kernel32.dll", SetLastError=true)] public static extern IntPtr CreateJobObjectW(IntPtr a, IntPtr n);
  [DllImport("kernel32.dll", SetLastError=true)] public static extern IntPtr OpenProcess(uint a, bool inh, uint pid);
  [DllImport("kernel32.dll", SetLastError=true)] public static extern bool AssignProcessToJobObject(IntPtr job, IntPtr proc);
  [DllImport("kernel32.dll", SetLastError=true)] public static extern bool IsProcessInJob(IntPtr p, IntPtr job, out bool res);
  [DllImport("kernel32.dll")] public static extern bool CloseHandle(IntPtr h);
}
"@
Add-Type -TypeDefinition $src -ErrorAction Stop
"SESSION=" + [System.Diagnostics.Process]::GetCurrentProcess().SessionId
$job = [J]::CreateJobObjectW([IntPtr]::Zero, [IntPtr]::Zero)
"JOB_CREE=" + ($job -ne [IntPtr]::Zero)
# PROCESS_SET_QUOTA(0x100) | PROCESS_TERMINATE(0x1) | PROCESS_QUERY_LIMITED(0x1000)
$droits = 0x100 -bor 0x1 -bor 0x1000
foreach ($n in @("notepad","mspaint","win32calc")) {
  foreach ($proc in @(Get-Process -Name $n -ErrorAction SilentlyContinue)) {
    $h = [J]::OpenProcess([uint32]$droits, $false, [uint32]$proc.Id)
    if ($h -eq [IntPtr]::Zero) { "CIBLE {0} pid={1} OUVERTURE_REFUSEE err={2}" -f $n,$proc.Id,[Runtime.InteropServices.Marshal]::GetLastWin32Error(); continue }
    $avant = $false; [void][J]::IsProcessInJob($h, $job, [ref]$avant)
    $ok = [J]::AssignProcessToJobObject($job, $h)
    $err = if ($ok) { 0 } else { [Runtime.InteropServices.Marshal]::GetLastWin32Error() }
    $apres = $false; [void][J]::IsProcessInJob($h, $job, [ref]$apres)
    "CIBLE {0} pid={1} dans_NOTRE_job_avant={2} ASSIGNATION={3} err={4} dans_NOTRE_job_apres={5}" -f $n,$proc.Id,$avant,$ok,$err,$apres
    [void][J]::CloseHandle($h)
  }
}
# On garde le job ouvert 20 s pour prouver que rien ne meurt, puis on ferme.
Start-Sleep -Seconds 20
foreach ($n in @("notepad","mspaint","win32calc")) {
  "SURVIE {0} = {1}" -f $n, (@(Get-Process -Name $n -ErrorAction SilentlyContinue)).Count
}
[void][J]::CloseHandle($job)
Start-Sleep -Seconds 5
"=== APRES FERMETURE DU JOB (sans KILL_ON_JOB_CLOSE, rien ne doit mourir) ==="
foreach ($n in @("notepad","mspaint","win32calc")) {
  "SURVIE_APRES_FERMETURE {0} = {1}" -f $n, (@(Get-Process -Name $n -ErrorAction SilentlyContinue)).Count
}
