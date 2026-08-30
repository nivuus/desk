# Lot 32G : combien de temps une fenetre TECHNIQUE reste-t-elle "passante" ?
#
# Le crochet du produit evalue le predicat a l instant de EVENT_OBJECT_SHOW.
# L occultation DWM, le proprietaire et WS_EX_TOOLWINDOW sont poses APRES.
# Cette sonde mesure ce DELTA : pour chaque HWND, l instant ou il commence a
# passer le predicat, et l instant ou il cesse de le passer.
#
# Elle echantillonne a ~15 ms : c est ce qui lui permet de voir une fenetre
# dans son etat passant, ce qu un releve a 150 ms ne pouvait pas.
# Elle IMPRIME SA SESSION. ASCII pur.
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
  [DllImport("dwmapi.dll")] public static extern int DwmGetWindowAttribute(IntPtr h, int a, out int v, int s);
}
"@
Add-Type -TypeDefinition $src -ErrorAction Stop

$etat = @{}          # hwnd -> [debut_passant, fin_passant, classe, titre]
$chrono = [Diagnostics.Stopwatch]::StartNew()
$n = 0
while ($chrono.Elapsed.TotalSeconds -lt 55) {
  $n++
  $vus = @{}
  $cb = [W+Cb]{
    param($h, $p)
    $sb = New-Object Text.StringBuilder 512
    [void][W]::GetWindowTextW($h, $sb, 512); $titre = $sb.ToString()
    $vis = [W]::IsWindowVisible($h)
    $own = [W]::GetWindow($h, 4)
    $ex  = [int64][W]::GetWindowLongPtrW($h, -20)
    $tool = ($ex -band 0x80) -ne 0
    $app  = ($ex -band 0x40000) -ne 0
    $cl = 0; [void][W]::DwmGetWindowAttribute($h, 14, [ref]$cl, 4)
    $passe = $vis -and ($own -eq [IntPtr]::Zero) -and ($cl -eq 0) -and ($titre -ne "") -and ((-not $tool) -or $app)
    $k = $h.ToString()
    $vus[$k] = $true
    if ($passe) {
      if (-not $etat.ContainsKey($k)) {
        $cb2 = New-Object Text.StringBuilder 256
        [void][W]::GetClassNameW($h, $cb2, 256)
        $etat[$k] = [pscustomobject]@{ debut = $chrono.Elapsed.TotalMilliseconds; fin = -1.0
                                       classe = $cb2.ToString(); titre = $titre }
      }
    } elseif ($etat.ContainsKey($k) -and $etat[$k].fin -lt 0) {
      $etat[$k].fin = $chrono.Elapsed.TotalMilliseconds
    }
    return $true
  }
  [void][W]::EnumWindows($cb, [IntPtr]::Zero)
  # Une fenetre DETRUITE cesse aussi de passer : on la borne ici.
  foreach ($k in @($etat.Keys)) { if ($etat[$k].fin -lt 0 -and -not $vus.ContainsKey($k)) { $etat[$k].fin = $chrono.Elapsed.TotalMilliseconds } }
  Start-Sleep -Milliseconds 15
}
"SESSION=" + [System.Diagnostics.Process]::GetCurrentProcess().SessionId
"ECHANTILLONS=$n"
"SUIVIES=" + $etat.Count
foreach ($k in $etat.Keys) {
  $v = $etat[$k]
  $d = if ($v.fin -lt 0) { -1 } else { [int]($v.fin - $v.debut) }
  "P|passant_ms={0}|classe={1}|titre={2}" -f $d, $v.classe, $v.titre
}
