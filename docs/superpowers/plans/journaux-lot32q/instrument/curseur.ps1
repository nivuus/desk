# Lot 32Q : ou le curseur ATTERRIT reellement, hors du flux video.
#
# C est le bras qui tranche entre "la souris ne marche pas" et "elle ne
# clique pas au bon endroit" -- et, s il est joue AVANT le remede, c est la
# seule fois ou l etat d avant existera.
#
# Echantillonne GetCursorPos a 100 ms et imprime chaque position DISTINCTE
# avec son horodatage : le pilote vise trois points a des instants connus, et
# la correlation se fait apres coup, sur les horodatages.
#
# Imprime sa session : un releve WinRM est celui de la session 0. ASCII pur.
$src = @"
using System; using System.Runtime.InteropServices;
public class C {
  [StructLayout(LayoutKind.Sequential)] public struct P { public int x, y; }
  [DllImport("user32.dll")] public static extern bool GetCursorPos(out P p);
  [DllImport("user32.dll")] public static extern int GetSystemMetrics(int i);
}
"@
Add-Type -TypeDefinition $src -ErrorAction Stop
$s = [System.Diagnostics.Process]::GetCurrentProcess().SessionId
"SESSION=$s"
# 76..79 = SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN
"BUREAU_VIRTUEL x={0} y={1} l={2} h={3}" -f [C]::GetSystemMetrics(76), [C]::GetSystemMetrics(77), [C]::GetSystemMetrics(78), [C]::GetSystemMetrics(79)
$chrono = [Diagnostics.Stopwatch]::StartNew()
$dernier = ""
while ($chrono.Elapsed.TotalSeconds -lt 40) {
  $p = New-Object C+P
  [void][C]::GetCursorPos([ref]$p)
  $cle = "$($p.x),$($p.y)"
  if ($cle -ne $dernier) {
    "CURSEUR t_ms={0} x={1} y={2}" -f [int]$chrono.Elapsed.TotalMilliseconds, $p.x, $p.y
    $dernier = $cle
  }
  Start-Sleep -Milliseconds 100
}
"FIN"
