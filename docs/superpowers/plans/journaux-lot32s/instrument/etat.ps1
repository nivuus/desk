# Lot 32S : etat exact des hwnd que le produit tente de replacer.
$src = @"
using System; using System.Text; using System.Runtime.InteropServices;
public class W {
  [DllImport("user32.dll")] public static extern bool IsWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern bool IsIconic(IntPtr h);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowTextW(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetClassNameW(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll")] public static extern IntPtr GetWindowLongPtrW(IntPtr h, int i);
  [DllImport("dwmapi.dll")] public static extern int DwmGetWindowAttribute(IntPtr h, int a, out int v, int s);
  [StructLayout(LayoutKind.Sequential)] public struct R { public int l,t,r,b; }
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out R r);
}
"@
Add-Type -TypeDefinition $src -ErrorAction Stop
"SESSION=" + [System.Diagnostics.Process]::GetCurrentProcess().SessionId
foreach ($v in @(0x190360, 0x2a03c8)) {
  $h = [IntPtr]$v
  $sb = New-Object Text.StringBuilder 256; [void][W]::GetWindowTextW($h,$sb,256)
  $cb = New-Object Text.StringBuilder 256; [void][W]::GetClassNameW($h,$cb,256)
  $r = New-Object W+R; [void][W]::GetWindowRect($h,[ref]$r)
  $cl = 0; [void][W]::DwmGetWindowAttribute($h,14,[ref]$cl,4)
  $ex = [int64][W]::GetWindowLongPtrW($h,-20)
  $st = [int64][W]::GetWindowLongPtrW($h,-16)
  "HWND=0x{0:X} IsWindow={1} VISIBLE={2} ICONIC={3} CLOAKED={4} WS_VISIBLE={5} rect={6}x{7}+{8}+{9} classe='{10}' titre='{11}'" -f `
    $v, [W]::IsWindow($h), [W]::IsWindowVisible($h), [W]::IsIconic($h), $cl, (($st -band 0x10000000) -ne 0), `
    ($r.r-$r.l), ($r.b-$r.t), $r.l, $r.t, $cb.ToString(), $sb.ToString()
}
