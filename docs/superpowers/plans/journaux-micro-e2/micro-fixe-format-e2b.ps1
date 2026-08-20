# SECONDE TENTATIVE, et ce n'est pas un contournement de l'ACL : c'est
# l'emploi exact du droit que l'ACL ACCORDE.
#
# Le premier essai (micro-fixe-format-e2.ps1) echoue en SecurityException
# alors que Get-Acl montre << BUILTIN\Administrateurs Allow SetValue,
# ReadKey >>. La cause n'est pas la politique mais l'outil : le fournisseur
# Registry de PowerShell ouvre la cle en KEY_WRITE, qui vaut SetValue ET
# CreateSubKey — et CreateSubKey n'est PAS accorde aux administrateurs sur
# cette cle. On ouvre donc en demandant SetValue seul.
#
# Aucun changement de proprietaire, aucune modification d'ACL, aucun privilege
# reclame. Si ce droit-la est refuse aussi, c'est un RESULTAT et on s'arrete.
$ErrorActionPreference = 'Continue'
$l = New-Object System.Collections.Generic.List[string]
$FMT = '{f19f064d-082c-4e27-bc73-6882a1bb8e4c},0'
$sous = 'SOFTWARE\Microsoft\Windows\CurrentVersion\MMDevices\Audio'
$gRendu = '{deec1914-6490-47ee-9475-091b9a2ea537}'
$gCapt  = '{5fae72b2-f885-4038-9827-0d91f862a7d5}'
function Hexa($o) { if ($null -eq $o) { '(null)' } else { ($o | ForEach-Object { $_.ToString('X2') }) -join ' ' } }

$l.Add('SECONDE TENTATIVE — ' + (Get-Date).ToString('o'))
$kr = [Microsoft.Win32.Registry]::LocalMachine.OpenSubKey("$sous\Render\$gRendu\Properties", $false)
$rendu = $kr.GetValue($FMT); $kr.Close()
$l.Add('RENDU   : ' + (Hexa $rendu))

try {
  $kc = [Microsoft.Win32.Registry]::LocalMachine.OpenSubKey(
          "$sous\Capture\$gCapt\Properties",
          [Microsoft.Win32.RegistryKeyPermissionCheck]::ReadWriteSubTree,
          [System.Security.AccessControl.RegistryRights]::SetValue -bor
          [System.Security.AccessControl.RegistryRights]::QueryValues)
  $avant = $kc.GetValue($FMT)
  $l.Add('CAPTURE AVANT (SAUVEGARDE) : ' + (Hexa $avant))
  $neuf = New-Object byte[] $avant.Length
  [Array]::Copy($avant, 0, $neuf, 0, 8)
  [Array]::Copy($rendu, 8, $neuf, 8, $rendu.Length - 8)
  $kc.SetValue($FMT, $neuf, [Microsoft.Win32.RegistryValueKind]::Binary)
  $l.Add('ECRITURE : acceptee')
  $kc.Close()
} catch { $l.Add('ECRITURE REFUSEE : ' + $_.Exception.GetType().FullName + ' — ' + $_.Exception.Message) }

$kv = [Microsoft.Win32.Registry]::LocalMachine.OpenSubKey("$sous\Capture\$gCapt\Properties", $false)
$relu = $kv.GetValue($FMT); $kv.Close()
$l.Add('CAPTURE RELU  : ' + (Hexa $relu))
$l.Add('  Hz relus : ' + $(if ($relu -is [byte[]]) { [BitConverter]::ToInt32($relu, 12) } else { '(illisible)' }))
try { Restart-Service Audiosrv -Force -ErrorAction Stop; $l.Add('REDEMARRAGE Audiosrv : accepte') }
catch { $l.Add('REDEMARRAGE Audiosrv REFUSE : ' + $_.Exception.Message) }
$l.Add('Etat d''Audiosrv : ' + (Get-Service Audiosrv).Status)
$l.Add('FIN — ' + (Get-Date).ToString('o'))
$l | Out-File -FilePath C:\dev\micro-fixe-format-e2b.log -Encoding utf8 -Width 500
