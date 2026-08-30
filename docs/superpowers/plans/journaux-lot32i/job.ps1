# Lot 32J : le superviseur est-il DEJA dans un job object ?
# C est la precondition de la voie (b) : si oui, l assignation a un second job
# depend des jobs imbriques (Windows 8+). ASCII pur, imprime sa session.
$src = @"
using System; using System.Runtime.InteropServices;
public class J {
  [DllImport("kernel32.dll", SetLastError=true)] public static extern IntPtr OpenProcess(uint a, bool inh, uint pid);
  [DllImport("kernel32.dll", SetLastError=true)] public static extern bool IsProcessInJob(IntPtr p, IntPtr job, out bool res);
  [DllImport("kernel32.dll")] public static extern bool CloseHandle(IntPtr h);
  [DllImport("kernel32.dll")] public static extern IntPtr GetCurrentProcess();
}
"@
Add-Type -TypeDefinition $src -ErrorAction Stop
"SESSION=" + [System.Diagnostics.Process]::GetCurrentProcess().SessionId
"VERSION=" + [System.Environment]::OSVersion.Version.ToString()
$p = @{}; Get-CimInstance Win32_Process | ForEach-Object { $p[[int]$_.ProcessId] = $_ }
foreach ($proc in @(Get-Process agent -ErrorAction SilentlyContinue)) {
  $h = [J]::OpenProcess(0x1000, $false, [uint32]$proc.Id)   # PROCESS_QUERY_LIMITED_INFORMATION
  if ($h -eq [IntPtr]::Zero) { "AGENT pid=" + $proc.Id + " OUVERTURE_REFUSEE"; continue }
  $dedans = $false
  $ok = [J]::IsProcessInJob($h, [IntPtr]::Zero, [ref]$dedans)
  [void][J]::CloseHandle($h)
  $par = $p[[int]$proc.Id].ParentProcessId
  $nomPar = if ($p.ContainsKey([int]$par)) { $p[[int]$par].Name } else { "?" }
  "AGENT pid={0} parent={1}({2}) appel_ok={3} DANS_UN_JOB={4}" -f $proc.Id, $par, $nomPar, $ok, $dedans
}
foreach ($n in @("notepad","mspaint","win32calc")) {
  foreach ($proc in @(Get-Process -Name $n -ErrorAction SilentlyContinue)) {
    $h = [J]::OpenProcess(0x1000, $false, [uint32]$proc.Id)
    if ($h -eq [IntPtr]::Zero) { continue }
    $dedans = $false; $ok = [J]::IsProcessInJob($h, [IntPtr]::Zero, [ref]$dedans)
    [void][J]::CloseHandle($h)
    "APPLI {0} pid={1} appel_ok={2} DANS_UN_JOB={3}" -f $n, $proc.Id, $ok, $dedans
  }
}
