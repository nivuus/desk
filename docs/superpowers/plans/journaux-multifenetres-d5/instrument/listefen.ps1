[Console]::OutputEncoding = New-Object System.Text.UTF8Encoding($false)
$f = New-Object System.IO.StreamWriter("C:\dev\fenetres.txt", $false, (New-Object System.Text.UTF8Encoding($false)))
Add-Type @"
using System;using System.Runtime.InteropServices;
public class W { [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
 [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L,T,R,B; } }
"@
Get-Process | Where-Object { $_.MainWindowHandle -ne 0 } | ForEach-Object {
  $r = New-Object W+RECT
  [void][W]::GetWindowRect($_.MainWindowHandle, [ref]$r)
  $f.WriteLine(("{0}`t{1}`t0x{2}`t{3}x{4}+{5}+{6}`t{7}" -f $_.Id, $_.ProcessName, $_.MainWindowHandle.ToString("x"), ($r.R-$r.L), ($r.B-$r.T), $r.L, $r.T, $_.MainWindowTitle))
}
$f.Close()
