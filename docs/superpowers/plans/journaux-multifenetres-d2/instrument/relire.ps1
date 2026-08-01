[Console]::OutputEncoding = New-Object System.Text.UTF8Encoding($false)
$f = New-Object System.IO.StreamWriter("C:\dev\relecture.txt", $false, (New-Object System.Text.UTF8Encoding($false)))
Add-Type @"
using System;using System.Text;using System.Runtime.InteropServices;
public class R {
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern IntPtr FindWindowEx(IntPtr p, IntPtr c, string cls, string win);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int SendMessage(IntPtr h, uint m, int w, StringBuilder l);
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
}
"@
$fg = New-Object System.Text.StringBuilder 512
[void][R]::GetWindowText([R]::GetForegroundWindow(), $fg, 512)
$f.WriteLine("fenetre au premier plan : " + $fg.ToString())
Get-Process notepad -ErrorAction SilentlyContinue | ForEach-Object {
  $edit = [R]::FindWindowEx($_.MainWindowHandle, [IntPtr]::Zero, "Edit", $null)
  $sb = New-Object System.Text.StringBuilder 2048
  if ($edit -ne [IntPtr]::Zero) { [void][R]::SendMessage($edit, 0x000D, 2048, $sb) }
  $f.WriteLine(("{0}`t{1}`tcontenu=[{2}]" -f $_.Id, $_.MainWindowTitle, $sb.ToString().Replace("`r`n"," / ")))
}
$f.Close()
