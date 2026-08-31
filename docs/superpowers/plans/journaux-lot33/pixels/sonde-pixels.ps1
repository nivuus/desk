$ErrorActionPreference = 'Continue'
$out = 'D:\nivuus-lot33\pixels.txt'
$L = New-Object System.Collections.ArrayList
function W($t) { [void]$L.Add([string]$t) }
W ("== session = " + (Get-Process -Id $PID).SessionId)
W ("== heure = " + (Get-Date -Format 'o'))

Add-Type -AssemblyName System.Drawing
Add-Type @"
using System;
using System.Runtime.InteropServices;
using System.Text;
public class X {
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc cb, IntPtr p);
  public delegate bool EnumProc(IntPtr h, IntPtr p);
  [DllImport("user32.dll")] public static extern bool IsIconic(IntPtr h);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetClassName(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll")] public static extern int GetWindowThreadProcessId(IntPtr h, out int pid);
  [DllImport("dwmapi.dll")] public static extern int DwmGetWindowAttribute(IntPtr h, int attr, out RECT r, int size);
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int left, top, right, bottom; }
}
"@

# Trouve la fenetre Notepad servie : visible, non minimisee, la plus grande.
$cible = $null; $aire = 0
$cb = [X+EnumProc]{
  param($h, $p)
  $cl = New-Object Text.StringBuilder 256
  [void][X]::GetClassName($h, $cl, 256)
  if ($cl.ToString() -eq 'Notepad' -and -not [X]::IsIconic($h) -and [X]::IsWindowVisible($h)) {
    $d = New-Object X+RECT
    if ([X]::DwmGetWindowAttribute($h, 9, [ref]$d, 16) -eq 0) {
      $a = ($d.right-$d.left) * ($d.bottom-$d.top)
      if ($a -gt $aire) { $script:aire = $a; $script:cible = @{ h=$h; r=$d } }
    }
  }
  return $true
}
[void][X]::EnumWindows($cb, [IntPtr]::Zero)

if ($null -eq $cible) { W "AUCUNE fenetre Notepad servie trouvee"; $L -join "`r`n" | Out-File $out -Encoding utf8; exit }

$r = $cible.r
$x = $r.left; $y = $r.top; $w = $r.right - $r.left; $h = $r.bottom - $r.top
W ("== recadrage lu (cadre DWM) = " + $w + "x" + $h + "+" + $x + "+" + $y)

$bmp = New-Object System.Drawing.Bitmap($w, $h)
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.CopyFromScreen($x, $y, 0, 0, (New-Object System.Drawing.Size($w, $h)))
$g.Dispose()

function Echantillon($nom, $points) {
  $cols = @{}
  foreach ($p in $points) {
    $c = $bmp.GetPixel($p[0], $p[1])
    $k = "{0:X2}{1:X2}{2:X2}" -f $c.R, $c.G, $c.B
    if (-not $cols.ContainsKey($k)) { $cols[$k] = 0 }
    $cols[$k] = $cols[$k] + 1
  }
  $tri = $cols.GetEnumerator() | Sort-Object Value -Descending | Select-Object -First 3
  $txt = ($tri | ForEach-Object { "#" + $_.Key + " x" + $_.Value }) -join "  "
  W ("  " + $nom.PadRight(28) + " : " + $txt)
}

# 11 points repartis le long de chaque bord, en evitant les coins arrondis
function Le($n, $a, $b) { $r=@(); for ($i=1; $i -lt 12; $i++) { $r += ,@(($a + [int](($b-$a)*$i/12)), $n) }; return $r }
function Co($n, $a, $b) { $r=@(); for ($i=1; $i -lt 12; $i++) { $r += ,@($n, ($a + [int](($b-$a)*$i/12))) }; return $r }

W "== RANGEES (horizontales) =="
Echantillon "rangee 0 (bord HAUT)"      (Le 0        0 $w)
Echantillon "rangee 1 (voisine)"        (Le 1        0 $w)
Echantillon "rangee 2"                  (Le 2        0 $w)
Echantillon ("rangee " + ($h-3))        (Le ($h-3)   0 $w)
Echantillon ("rangee " + ($h-2))        (Le ($h-2)   0 $w)
Echantillon ("rangee " + ($h-1) + " (bord BAS)") (Le ($h-1) 0 $w)
W "== COLONNES (verticales) =="
Echantillon "colonne 0 (bord GAUCHE)"   (Co 0        0 $h)
Echantillon "colonne 1 (voisine)"       (Co 1        0 $h)
Echantillon "colonne 2"                 (Co 2        0 $h)
Echantillon ("colonne " + ($w-3))       (Co ($w-3)   0 $h)
Echantillon ("colonne " + ($w-2))       (Co ($w-2)   0 $h)
Echantillon ("colonne " + ($w-1) + " (bord DROIT)") (Co ($w-1) 0 $h)

$bmp.Save('D:\nivuus-lot33\capture.png', [System.Drawing.Imaging.ImageFormat]::Png)
$bmp.Dispose()
W "== image complete ecrite dans D:\nivuus-lot33\capture.png =="
$L -join "`r`n" | Out-File -FilePath $out -Encoding utf8
