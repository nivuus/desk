# Joue une tonalite pure sur un peripherique de rendu DESIGNE PAR SON NOM.
#
# waveOut, et non System.Media.SoundPlayer : SoundPlayer suit le DEFAUT de
# Windows, ce qui ne permettrait pas de dissocier « ou le son joue » de « ce
# que l'agent capte » — la dissociation est exactement l'objet de la recette.
#
# ⚠️ La WAVEHDR vit en memoire NON MANAGEE (AllocHGlobal), pas en `[ref]` :
# un `[ref]` PowerShell marshale une COPIE temporaire, dont l'adresse est
# invalide des le retour de l'appel. Le pilote garde ce pointeur pendant toute
# la lecture. Premiere version : waveOutOpen/Write rendaient 0 et RIEN ne
# jouait — verifie au crete-metre independant, sur les trois endpoints.
param([string]$Prefixe = '', [int]$Hz = 440, [int]$Millisecondes = 26000)
$ErrorActionPreference = 'Stop'
Add-Type -TypeDefinition @'
using System; using System.Runtime.InteropServices;
[StructLayout(LayoutKind.Sequential)] public struct WAVEFORMATEX {
  public short wFormatTag; public short nChannels; public int nSamplesPerSec;
  public int nAvgBytesPerSec; public short nBlockAlign; public short wBitsPerSample; public short cbSize; }
[StructLayout(LayoutKind.Sequential)] public struct WAVEHDR {
  public IntPtr lpData; public uint dwBufferLength; public uint dwBytesRecorded;
  public IntPtr dwUser; public uint dwFlags; public uint dwLoops; public IntPtr lpNext; public IntPtr reserved; }
[StructLayout(LayoutKind.Sequential, CharSet=CharSet.Unicode)] public struct WAVEOUTCAPS {
  public short wMid; public short wPid; public int vDriverVersion;
  [MarshalAs(UnmanagedType.ByValTStr, SizeConst=32)] public string szPname;
  public int dwFormats; public short wChannels; public short wReserved1; public int dwSupport; }
public static class Wave {
  [DllImport("winmm.dll")] public static extern int waveOutGetNumDevs();
  [DllImport("winmm.dll", CharSet=CharSet.Unicode)] public static extern int waveOutGetDevCapsW(IntPtr id, ref WAVEOUTCAPS c, int n);
  [DllImport("winmm.dll")] public static extern int waveOutOpen(out IntPtr h, IntPtr id, ref WAVEFORMATEX f, IntPtr cb, IntPtr inst, int flags);
  [DllImport("winmm.dll")] public static extern int waveOutPrepareHeader(IntPtr h, IntPtr hdr, int n);
  [DllImport("winmm.dll")] public static extern int waveOutWrite(IntPtr h, IntPtr hdr, int n);
  [DllImport("winmm.dll")] public static extern int waveOutUnprepareHeader(IntPtr h, IntPtr hdr, int n);
  [DllImport("winmm.dll")] public static extern int waveOutReset(IntPtr h);
  [DllImport("winmm.dll")] public static extern int waveOutClose(IntPtr h);
}
'@
$n = [Wave]::waveOutGetNumDevs(); $cible = -1; $nomCible = ''
for ($i = 0; $i -lt $n; $i++) {
  $caps = New-Object WAVEOUTCAPS
  [void][Wave]::waveOutGetDevCapsW([IntPtr]$i, [ref]$caps, [Runtime.InteropServices.Marshal]::SizeOf($caps))
  Write-Output ("waveOut[{0}] = {1}" -f $i, $caps.szPname)
  if ($Prefixe -ne '' -and $cible -lt 0 -and $caps.szPname -like "*$Prefixe*") { $cible = $i; $nomCible = $caps.szPname }
}
if ($Prefixe -eq '') { Write-Output 'ENUMERATION SEULE'; exit 0 }
if ($cible -lt 0) { Write-Output "AUCUN waveOut ne contient '$Prefixe'"; exit 1 }
Write-Output ("CIBLE waveOut[{0}] = {1}" -f $cible, $nomCible)

$taux = 48000
$fmt = New-Object WAVEFORMATEX
$fmt.wFormatTag = 1; $fmt.nChannels = [int16]2; $fmt.nSamplesPerSec = $taux
$fmt.wBitsPerSample = [int16]16; $fmt.nBlockAlign = [int16]4
$fmt.nAvgBytesPerSec = $taux * 4; $fmt.cbSize = [int16]0

# Un seul bloc BOUCLE (dwLoops) : plus court a allouer, et joue aussi
# longtemps qu'on veut sans reecrire.
$trames = $taux    # 1 s, boucle ensuite
$octets = New-Object byte[] ($trames * 4)
for ($t = 0; $t -lt $trames; $t++) {
  $e = [int16]([Math]::Round([Math]::Sin(2.0*[Math]::PI*$Hz*$t/$taux) * 0.45 * 32767))
  $b = [BitConverter]::GetBytes($e); $o = $t*4
  $octets[$o]=$b[0]; $octets[$o+1]=$b[1]; $octets[$o+2]=$b[0]; $octets[$o+3]=$b[1]
}
$tampon = [Runtime.InteropServices.Marshal]::AllocHGlobal($octets.Length)
[Runtime.InteropServices.Marshal]::Copy($octets, 0, $tampon, $octets.Length)

$h = [IntPtr]::Zero
$r = [Wave]::waveOutOpen([ref]$h, [IntPtr]$cible, [ref]$fmt, [IntPtr]::Zero, [IntPtr]::Zero, 0)
Write-Output "waveOutOpen = $r"
if ($r -ne 0) { exit 1 }

$hdr = New-Object WAVEHDR
$hdr.lpData = $tampon; $hdr.dwBufferLength = [uint32]$octets.Length
$hdr.dwFlags = 0x00000004 -bor 0x00000008   # WHDR_BEGINLOOP | WHDR_ENDLOOP
$hdr.dwLoops = [uint32][Math]::Ceiling($Millisecondes / 1000.0)
$taille = [Runtime.InteropServices.Marshal]::SizeOf($hdr)
# ADRESSE STABLE : c'est tout l'objet de ce bloc.
$pHdr = [Runtime.InteropServices.Marshal]::AllocHGlobal($taille)
[Runtime.InteropServices.Marshal]::StructureToPtr($hdr, $pHdr, $false)
$rp = [Wave]::waveOutPrepareHeader($h, $pHdr, $taille)
$rw = [Wave]::waveOutWrite($h, $pHdr, $taille)
Write-Output "waveOutPrepareHeader = $rp ; waveOutWrite = $rw"
Write-Output ("LECTURE {0} Hz pendant {1} ms sur '{2}'" -f $Hz, $Millisecondes, $nomCible)
& powershell -NoProfile -ExecutionPolicy Bypass -File C:\dev\abis-metre.ps1
Start-Sleep -Milliseconds ($Millisecondes)
[void][Wave]::waveOutReset($h)
[void][Wave]::waveOutUnprepareHeader($h, $pHdr, $taille)
[void][Wave]::waveOutClose($h)
[Runtime.InteropServices.Marshal]::FreeHGlobal($pHdr)
[Runtime.InteropServices.Marshal]::FreeHGlobal($tampon)
Write-Output 'LECTURE TERMINEE'
