# Sonde CCD du lot 32 : pour chaque chemin d'affichage ACTIF, la cible
# (adaptateur LUID, id, statusFlags) et le nom GDI de sa source.
#
# Elle IMPRIME SA SESSION plutot que de la supposer : un releve WinRM est
# celui de la session 0, ou la configuration d'affichage n'est pas la meme.
#
# ASCII pur : un .ps1 sans BOM portant un seul caractere non-ASCII ne
# s'analyse pas, et l'erreur designe une AUTRE ligne.
$src = @"
using System;
using System.Runtime.InteropServices;
public class Ccd {
  [StructLayout(LayoutKind.Sequential)] public struct LUID { public uint LowPart; public int HighPart; }
  [StructLayout(LayoutKind.Sequential)] public struct SRC { public LUID adapterId; public uint id; public uint modeInfoIdx; public uint statusFlags; }
  [StructLayout(LayoutKind.Sequential)] public struct RAT { public uint Numerator; public uint Denominator; }
  [StructLayout(LayoutKind.Sequential)] public struct TGT {
    public LUID adapterId; public uint id; public uint modeInfoIdx;
    public int outputTechnology; public int rotation; public int scaling;
    public RAT refreshRate; public int scanLineOrdering;
    public int targetAvailable; public uint statusFlags; }
  [StructLayout(LayoutKind.Sequential)] public struct PATH { public SRC sourceInfo; public TGT targetInfo; public uint flags; }
  [StructLayout(LayoutKind.Sequential, Size=64)] public struct MODE { public int infoType; }
  [StructLayout(LayoutKind.Sequential)] public struct HDR { public int type; public uint size; public LUID adapterId; public uint id; }
  [StructLayout(LayoutKind.Sequential, CharSet=CharSet.Unicode)] public struct SRCNAME {
    public HDR header; [MarshalAs(UnmanagedType.ByValTStr, SizeConst=32)] public string viewGdiDeviceName; }
  [DllImport("user32.dll")] public static extern int GetDisplayConfigBufferSizes(uint flags, out uint nPath, out uint nMode);
  [DllImport("user32.dll")] public static extern int QueryDisplayConfig(uint flags, ref uint nPath, [Out] PATH[] paths, ref uint nMode, [Out] MODE[] modes, IntPtr topo);
  [DllImport("user32.dll")] public static extern int DisplayConfigGetDeviceInfo(ref SRCNAME req);
}
"@
Add-Type -TypeDefinition $src -ErrorAction Stop

$QDC_ONLY_ACTIVE_PATHS = 2
$GET_SOURCE_NAME = 1

"SESSION = " + [System.Diagnostics.Process]::GetCurrentProcess().SessionId
"HORODATAGE = " + (Get-Date).ToUniversalTime().ToString("s")

$nPath = 0; $nMode = 0
$r = [Ccd]::GetDisplayConfigBufferSizes($QDC_ONLY_ACTIVE_PATHS, [ref]$nPath, [ref]$nMode)
"BUFFERSIZES = $r  chemins=$nPath modes=$nMode"
if ($r -ne 0) { "ARRET : GetDisplayConfigBufferSizes a echoue"; exit 1 }

$paths = New-Object 'Ccd+PATH[]' $nPath
$modes = New-Object 'Ccd+MODE[]' $nMode
$r = [Ccd]::QueryDisplayConfig($QDC_ONLY_ACTIVE_PATHS, [ref]$nPath, $paths, [ref]$nMode, $modes, [IntPtr]::Zero)
"QUERY = $r  chemins_rendus=$nPath"
if ($r -ne 0) { "ARRET : QueryDisplayConfig a echoue"; exit 1 }

for ($i = 0; $i -lt $nPath; $i++) {
  $p = $paths[$i]
  $n = New-Object 'Ccd+SRCNAME'
  $n.header.type = $GET_SOURCE_NAME
  $n.header.size = [System.Runtime.InteropServices.Marshal]::SizeOf([type]'Ccd+SRCNAME')
  $n.header.adapterId = $p.sourceInfo.adapterId
  $n.header.id = $p.sourceInfo.id
  $rc = [Ccd]::DisplayConfigGetDeviceInfo([ref]$n)
  $nom = if ($rc -eq 0) { $n.viewGdiDeviceName } else { "(GetDeviceInfo=$rc)" }
  $flags = '0x{0:X}' -f $p.targetInfo.statusFlags
  "CHEMIN $i : cible_luid=$($p.targetInfo.adapterId.LowPart)/$($p.targetInfo.adapterId.HighPart) cible_id=$($p.targetInfo.id) statusFlags=$flags | source_id=$($p.sourceInfo.id) nom_gdi=$nom"
}
"--- moniteurs PnP ---"
$pnp = @(Get-PnpDevice -Class Monitor -Status OK -ErrorAction SilentlyContinue)
"moniteurs_pnp_ok = " + $pnp.Count
$pnp | ForEach-Object { "  " + $_.InstanceId }
