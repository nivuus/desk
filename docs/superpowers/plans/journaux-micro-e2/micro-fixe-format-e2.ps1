# REMEDE de format : rendre CABLE Output symetrique de CABLE Input.
#
# Le script que E1 declarait << pret sur le partage >> n'existe pas : le releve
# du 20 aout 2026 ne trouve que micro-format-e1.ps1, qui MESURE. Celui-ci
# ECRIT, et c'est la tache 2 du plan E2.
#
# La transposition est minimale et elle est decrite par le verdict de E1 :
# recopier les octets 8.. du blob PKEY_AudioEngine_DeviceFormat du RENDU dans
# celui de la CAPTURE, en laissant intacts les 8 octets d'en-tete PROPVARIANT
# propres a chaque valeur -- seuls nSamplesPerSec et nAvgBytesPerSec different.
#
# ⚠️ Le blob de depart est journalise AVANT toute modification : c'est la seule
# facon de revenir en arriere, et un registre audio laisse de travers a deja
# bloque le produit entier une fois (D8, C1).
#
# ⚠️ AUCUNE sortie standard avant la fin : nodejs-winrm tue le processus
# distant des la premiere ligne recue.
param([string]$Journal = 'C:\dev\micro-fixe-format-e2.log')
$ErrorActionPreference = 'Continue'
$lignes = New-Object System.Collections.Generic.List[string]
function Dire([string]$l) { $lignes.Add($l) }

$FMT  = '{f19f064d-082c-4e27-bc73-6882a1bb8e4c},0'
$DESC = '{b3f8fa53-0004-438e-9003-51a46e139bfc},6'

function TrouverCable([string]$sens) {
  $racine = 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\MMDevices\Audio\' + $sens
  foreach ($k in (Get-ChildItem $racine -ErrorAction SilentlyContinue)) {
    $chemin = Join-Path $k.PSPath 'Properties'
    $pr = Get-ItemProperty -Path $chemin -ErrorAction SilentlyContinue
    if ($null -eq $pr) { continue }
    if ("$($pr.$DESC)" -notlike '*VB-Audio*') { continue }
    return [pscustomobject]@{ Chemin = $chemin; Guid = $k.PSChildName; Blob = $pr.$FMT }
  }
  return $null
}

function Hexa($o) { if ($null -eq $o) { '(null)' } else { ($o | ForEach-Object { $_.ToString('X2') }) -join ' ' } }

Dire ('REMEDE DE FORMAT — ' + (Get-Date).ToString('o'))
Dire ('Identite du processus : ' + [Security.Principal.WindowsIdentity]::GetCurrent().Name)
$eleve = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole(
  [Security.Principal.WindowsBuiltInRole]::Administrator)
Dire ('Jeton eleve (Administrateur) : ' + $eleve)

$rendu   = TrouverCable 'Render'
$capture = TrouverCable 'Capture'
if ($null -eq $rendu -or $null -eq $capture) {
  Dire 'ECHEC : endpoint VB-Audio introuvable en registre'
  $lignes | Out-File -FilePath $Journal -Encoding utf8 -Width 500; exit 1
}
Dire ''
Dire ('RENDU   ' + $rendu.Guid)
Dire ('  AVANT : ' + (Hexa $rendu.Blob))
Dire ('CAPTURE ' + $capture.Guid)
Dire ('  AVANT (SAUVEGARDE — a recopier telle quelle pour revenir en arriere) :')
Dire ('  ' + (Hexa $capture.Blob))

if ($rendu.Blob -isnot [byte[]] -or $capture.Blob -isnot [byte[]]) {
  Dire 'ECHEC : DeviceFormat n''est pas un byte[]'
  $lignes | Out-File -FilePath $Journal -Encoding utf8 -Width 500; exit 1
}
if ($rendu.Blob.Length -ne $capture.Blob.Length) {
  Dire ('ECHEC : longueurs differentes (' + $rendu.Blob.Length + ' contre ' + $capture.Blob.Length + ')')
  $lignes | Out-File -FilePath $Journal -Encoding utf8 -Width 500; exit 1
}

$neuf = New-Object byte[] $capture.Blob.Length
[Array]::Copy($capture.Blob, 0, $neuf, 0, 8)                                  # en-tete PROPVARIANT : INTACT
[Array]::Copy($rendu.Blob, 8, $neuf, 8, $rendu.Blob.Length - 8)               # le WAVEFORMATEXTENSIBLE du rendu
Dire ''
Dire ('  CIBLE : ' + (Hexa $neuf))
Dire ('  Hz lus dans la cible : ' + [BitConverter]::ToInt32($neuf, 12))

try {
  Set-ItemProperty -Path $capture.Chemin -Name $FMT -Value $neuf -Type Binary -ErrorAction Stop
  Dire 'ECRITURE REGISTRE : acceptee'
} catch { Dire ('ECRITURE REGISTRE REFUSEE : ' + $_.Exception.GetType().FullName + ' — ' + $_.Exception.Message) }

$relu = (Get-ItemProperty -Path $capture.Chemin -ErrorAction SilentlyContinue).$FMT
Dire ('  RELU  : ' + (Hexa $relu))
Dire ('  Hz relus : ' + $(if ($relu -is [byte[]]) { [BitConverter]::ToInt32($relu, 12) } else { '(illisible)' }))

try {
  Restart-Service Audiosrv -Force -ErrorAction Stop
  Dire 'REDEMARRAGE Audiosrv : accepte'
} catch { Dire ('REDEMARRAGE Audiosrv REFUSE : ' + $_.Exception.GetType().FullName + ' — ' + $_.Exception.Message) }
Dire ('Etat d''Audiosrv : ' + (Get-Service Audiosrv).Status)
Dire ('FIN — ' + (Get-Date).ToString('o'))
$lignes | Out-File -FilePath $Journal -Encoding utf8 -Width 500
