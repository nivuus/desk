# Recensement du lot 32F : QUELLES fenetres le critere actuel laisse passer.
#
# Il calcule le MEME predicat que agent/src/superviseur/fenetres.rs
# (visible, pas de proprietaire, pas TOOLWINDOW sauf APPWINDOW, pas occultee
# DWM, titre non vide) et releve EN PLUS ce que le produit n ignore :
# la CLASSE, la TAILLE, le PROCESSUS. Sans ce releve, toute regle
# d exclusion serait devinee.
#
# Il echantillonne pour mesurer la DUREE DE VIE : une fenetre technique
# transitoire se distingue d une application par sa persistance.
#
# Il IMPRIME SA SESSION : un releve WinRM est celui de la session 0.
# ASCII pur.
$src = @"
using System;
using System.Text;
using System.Runtime.InteropServices;
public class W {
  public delegate bool Cb(IntPtr h, IntPtr p);
  [DllImport("user32.dll")] public static extern bool EnumWindows(Cb cb, IntPtr p);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowTextW(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetClassNameW(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll")] public static extern IntPtr GetWindow(IntPtr h, uint c);
  [DllImport("user32.dll")] public static extern IntPtr GetWindowLongPtrW(IntPtr h, int i);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [StructLayout(LayoutKind.Sequential)] public struct R { public int l, t, r, b; }
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out R r);
  [DllImport("dwmapi.dll")] public static extern int DwmGetWindowAttribute(IntPtr h, int a, out int v, int s);
}
"@
Add-Type -TypeDefinition $src -ErrorAction Stop

$vues = @{}
$t0 = Get-Date
$echantillons = 0
while (((Get-Date) - $t0).TotalSeconds -lt 55) {
  $echantillons++
  $cb = [W+Cb]{
    param($h, $p)
    $sb = New-Object Text.StringBuilder 512
    [void][W]::GetWindowTextW($h, $sb, 512); $titre = $sb.ToString()
    $cb2 = New-Object Text.StringBuilder 256
    [void][W]::GetClassNameW($h, $cb2, 256); $classe = $cb2.ToString()
    $vis = [W]::IsWindowVisible($h)
    $own = [W]::GetWindow($h, 4)
    $ex  = [int64][W]::GetWindowLongPtrW($h, -20)
    $tool = ($ex -band 0x80) -ne 0
    $app  = ($ex -band 0x40000) -ne 0
    $cl = 0; [void][W]::DwmGetWindowAttribute($h, 14, [ref]$cl, 4)
    $r = New-Object W+R; [void][W]::GetWindowRect($h, [ref]$r)
    $pid = 0; [void][W]::GetWindowThreadProcessId($h, [ref]$pid)
    # LE MEME PREDICAT QUE LE PRODUIT
    $merite = $vis -and ($own -eq [IntPtr]::Zero) -and ($cl -eq 0) -and ($titre -ne "") -and ((-not $tool) -or $app)
    if ($merite) {
      $k = "$classe|$titre|$pid"
      if (-not $vues.ContainsKey($k)) {
        $vues[$k] = [pscustomobject]@{ n = 0; classe = $classe; titre = $titre; pid = $pid;
          l = ($r.r - $r.l); h = ($r.b - $r.t); premier = $echantillons; dernier = $echantillons }
      }
      $vues[$k].n++
      $vues[$k].dernier = $echantillons
    }
    return $true
  }
  [void][W]::EnumWindows($cb, [IntPtr]::Zero)
  Start-Sleep -Milliseconds 150
}
"SESSION=" + [System.Diagnostics.Process]::GetCurrentProcess().SessionId
"ECHANTILLONS=$echantillons"
"DISTINCTES=" + $vues.Count
foreach ($k in $vues.Keys) {
  $v = $vues[$k]
  $p = try { (Get-Process -Id $v.pid -ErrorAction Stop).ProcessName } catch { "?" }
  "F|vues={0}/{1}|proc={2}|classe={3}|{4}x{5}|titre={6}" -f $v.n, $echantillons, $p, $v.classe, $v.l, $v.h, $v.titre
}
