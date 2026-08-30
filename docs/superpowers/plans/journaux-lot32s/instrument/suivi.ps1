# Lot 32S : la fenetre bouge-t-elle entre deux SetWindowPos du produit ?
# 50 ms d echantillonnage : un aller-retour invisible a 1 Hz se verrait.
#
# ACCUMULE DANS UNE LISTE, ET IMPRIME A LA FIN. Les chaines emises DANS un
# callback appele par du code natif (EnumWindows) ne rejoignent PAS le pipeline
# PowerShell : une premiere version imprimait directement et rendait un releve
# VIDE alors que le phenomene courait. Le vide n etait pas une mesure.
$src = @"
using System; using System.Text; using System.Runtime.InteropServices;
public class W {
  public delegate bool Cb(IntPtr h, IntPtr p);
  [DllImport("user32.dll")] public static extern bool EnumWindows(Cb cb, IntPtr p);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowTextW(IntPtr h, StringBuilder s, int n);
  [StructLayout(LayoutKind.Sequential)] public struct R { public int l,t,r,b; }
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out R r);
}
"@
Add-Type -TypeDefinition $src -ErrorAction Stop
"SESSION=" + [System.Diagnostics.Process]::GetCurrentProcess().SessionId
$script:etat = @{}
$script:lignes = New-Object Collections.ArrayList
$chrono = [Diagnostics.Stopwatch]::StartNew()
$cb = [W+Cb]{ param($h,$x)
  if (-not [W]::IsWindowVisible($h)) { return $true }
  $sb = New-Object Text.StringBuilder 256
  [void][W]::GetWindowTextW($h,$sb,256)
  if ($sb.Length -eq 0) { return $true }
  $r = New-Object W+R
  [void][W]::GetWindowRect($h,[ref]$r)
  $k = $h.ToString()
  $v = "{0}x{1}+{2}+{3}" -f ($r.r-$r.l), ($r.b-$r.t), $r.l, $r.t
  if (-not $script:etat.ContainsKey($k) -or $script:etat[$k] -ne $v) {
    [void]$script:lignes.Add(("T={0} hwnd=0x{1:X} rect={2} titre='{3}'" -f [int]$chrono.Elapsed.TotalMilliseconds, $h.ToInt64(), $v, $sb.ToString()))
    $script:etat[$k] = $v
  }
  return $true }
while ($chrono.Elapsed.TotalSeconds -lt 25) {
  [void][W]::EnumWindows($cb,[IntPtr]::Zero)
  Start-Sleep -Milliseconds 50
}
"ECHANTILLONS_AVEC_CHANGEMENT=" + $script:lignes.Count
$script:lignes | ForEach-Object { $_ }
"FIN"
