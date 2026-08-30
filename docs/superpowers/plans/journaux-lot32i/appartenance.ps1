# Lot 32I : pour chaque fenetre qui passe le critere ACTUEL, dit si son
# processus DESCEND de desk. C est la mesure qui decide si la regle
# d appartenance est praticable.
# Elle IMPRIME SA SESSION : un releve WinRM est celui de la session 0, et
# EnumWindows n y voit rien. ASCII pur.
$src = @"
using System; using System.Text; using System.Runtime.InteropServices;
public class W {
  public delegate bool Cb(IntPtr h, IntPtr p);
  [DllImport("user32.dll")] public static extern bool EnumWindows(Cb cb, IntPtr p);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowTextW(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetClassNameW(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll")] public static extern IntPtr GetWindow(IntPtr h, uint c);
  [DllImport("user32.dll")] public static extern IntPtr GetWindowLongPtrW(IntPtr h, int i);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("dwmapi.dll")] public static extern int DwmGetWindowAttribute(IntPtr h, int a, out int v, int s);
}
"@
Add-Type -TypeDefinition $src -ErrorAction Stop
"SESSION=" + [System.Diagnostics.Process]::GetCurrentProcess().SessionId
$p = @{}; Get-CimInstance Win32_Process | ForEach-Object { $p[[int]$_.ProcessId] = $_ }
$agents = @(Get-Process agent -ErrorAction SilentlyContinue | ForEach-Object { $_.Id })
"AGENTS=" + ($agents -join ",")
$res = New-Object Collections.ArrayList
$cb = [W+Cb]{ param($h,$x)
  $sb = New-Object Text.StringBuilder 512
  [void][W]::GetWindowTextW($h,$sb,512); $t = $sb.ToString()
  $vis = [W]::IsWindowVisible($h); $own = [W]::GetWindow($h,4)
  $ex = [int64][W]::GetWindowLongPtrW($h,-20)
  $cl = 0; [void][W]::DwmGetWindowAttribute($h,14,[ref]$cl,4)
  $ok = $vis -and ($own -eq [IntPtr]::Zero) -and ($cl -eq 0) -and ($t -ne "") -and (((($ex -band 0x80) -eq 0)) -or (($ex -band 0x40000) -ne 0))
  if ($ok) {
    $q = 0; [void][W]::GetWindowThreadProcessId($h,[ref]$q)
    $cur = [int]$q; $d = $false; $chaine = @()
    for ($i=0; $i -lt 12 -and $p.ContainsKey($cur); $i++) {
      $chaine += ($p[$cur].Name + "(" + $cur + ")")
      if ($agents -contains $cur) { $d = $true; break }
      $cur = [int]$p[$cur].ParentProcessId; if ($cur -eq 0) { break } }
    $c2 = New-Object Text.StringBuilder 256
    [void][W]::GetClassNameW($h,$c2,256)
    [void]$res.Add(("A|adoptee_si_appartenance={0}|classe={1}|titre={2}|chaine={3}" -f $d, $c2.ToString(), $t, ($chaine -join "<-")))
  }
  return $true }
[void][W]::EnumWindows($cb,[IntPtr]::Zero)
"PASSENT_LE_CRITERE_ACTUEL=" + $res.Count
$res | ForEach-Object { $_ }
