$out = 'C:\dev\g3-breakaway.txt'
function L($m) { $m | Out-File -FilePath $out -Append -Encoding utf8 }
"=== LE JOB AUTORISE-T-IL LA SORTIE (CREATE_BREAKAWAY_FROM_JOB) ? ===" | Out-File -FilePath $out -Encoding utf8

Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public class B {
  [StructLayout(LayoutKind.Sequential, CharSet=CharSet.Unicode)]
  public struct STARTUPINFO {
    public int cb; public string lpReserved, lpDesktop, lpTitle;
    public int dwX, dwY, dwXSize, dwYSize, dwXCountChars, dwYCountChars, dwFillAttribute, dwFlags;
    public short wShowWindow, cbReserved2; public IntPtr lpReserved2, hStdInput, hStdOutput, hStdError;
  }
  [StructLayout(LayoutKind.Sequential)]
  public struct PROCESS_INFORMATION { public IntPtr hProcess, hThread; public int dwProcessId, dwThreadId; }
  [DllImport("kernel32.dll", SetLastError=true, CharSet=CharSet.Unicode)]
  public static extern bool CreateProcess(string app, string cmd, IntPtr pa, IntPtr ta, bool inh,
      uint flags, IntPtr env, string dir, ref STARTUPINFO si, out PROCESS_INFORMATION pi);
  [DllImport("kernel32.dll", SetLastError=true)]
  public static extern bool IsProcessInJob(IntPtr h, IntPtr job, out bool r);
  [DllImport("kernel32.dll")] public static extern bool TerminateProcess(IntPtr h, uint c);
  [DllImport("kernel32.dll")] public static extern IntPtr OpenProcess(uint a, bool i, int pid);
  public static string Essai(uint flags, string nom) {
    STARTUPINFO si = new STARTUPINFO(); si.cb = Marshal.SizeOf(typeof(STARTUPINFO));
    PROCESS_INFORMATION pi;
    string cmd = "C:\\Windows\\System32\\notepad.exe";
    bool ok = CreateProcess(null, cmd, IntPtr.Zero, IntPtr.Zero, false, flags, IntPtr.Zero, null, ref si, out pi);
    if (!ok) return nom + " : ECHEC, GetLastError=" + Marshal.GetLastWin32Error();
    bool dedans = false; IsProcessInJob(pi.hProcess, IntPtr.Zero, out dedans);
    string r = nom + " : REUSSI, pid=" + pi.dwProcessId + ", enfant DANS UN JOB = " + dedans;
    TerminateProcess(pi.hProcess, 0);
    return r;
  }
}
'@

$moi = $false
[void][B]::IsProcessInJob([System.Diagnostics.Process]::GetCurrentProcess().Handle, [IntPtr]::Zero, [ref]$moi)
L ("le lanceur (cette tache planifiee) est dans un job : " + $moi)
L ""
L ([B]::Essai(0x08000000, "SANS breakaway (CREATE_NO_WINDOW seul)"))
L ([B]::Essai(0x08000000 -bor 0x01000000, "AVEC CREATE_BREAKAWAY_FROM_JOB"))
L ""
L "0x01000000 = CREATE_BREAKAWAY_FROM_JOB ; ECHEC 5 = ACCES REFUSE = le job INTERDIT la sortie"
