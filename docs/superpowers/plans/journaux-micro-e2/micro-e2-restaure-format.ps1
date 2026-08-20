# RESTAURATION. L'ecriture du remede a rendu CABLE Output INITIALISABLE :
# IAudioClient::Initialize rend 0x88890008 (AUDCLNT_E_UNSUPPORTED_FORMAT)
# alors que GetMixFormat rend toujours 44100. On remet les octets sauvegardes
# par micro-fixe-format-e2.ps1, mot pour mot.
$l = New-Object System.Collections.Generic.List[string]
$FMT  = '{f19f064d-082c-4e27-bc73-6882a1bb8e4c},0'
$sous = 'SOFTWARE\Microsoft\Windows\CurrentVersion\MMDevices\Audio'
$gCapt = '{5fae72b2-f885-4038-9827-0d91f862a7d5}'
$hex = '41 00 35 00 01 00 00 00 FE FF 02 00 44 AC 00 00 98 09 04 00 06 00 18 00 16 00 18 00 03 00 00 00 01 00 00 00 00 00 10 00 80 00 00 AA 00 38 9B 71'
$octets = [byte[]]($hex -split ' ' | ForEach-Object { [Convert]::ToByte($_, 16) })
function Hexa($o) { if ($null -eq $o) { '(null)' } else { ($o | ForEach-Object { $_.ToString('X2') }) -join ' ' } }
$l.Add('RESTAURATION — ' + (Get-Date).ToString('o'))
$l.Add('  A REMETTRE : ' + (Hexa $octets) + '  (' + $octets.Length + ' octets)')
try {
  $kc = [Microsoft.Win32.Registry]::LocalMachine.OpenSubKey("$sous\Capture\$gCapt\Properties",
          [Microsoft.Win32.RegistryKeyPermissionCheck]::ReadWriteSubTree,
          [System.Security.AccessControl.RegistryRights]::SetValue -bor
          [System.Security.AccessControl.RegistryRights]::QueryValues)
  $l.Add('  AVANT RESTAURATION : ' + (Hexa $kc.GetValue($FMT)))
  $kc.SetValue($FMT, $octets, [Microsoft.Win32.RegistryValueKind]::Binary)
  $kc.Close(); $l.Add('  ECRITURE : acceptee')
} catch { $l.Add('  ECHEC : ' + $_.Exception.Message) }
$kv = [Microsoft.Win32.Registry]::LocalMachine.OpenSubKey("$sous\Capture\$gCapt\Properties", $false)
$l.Add('  RELU  : ' + (Hexa $kv.GetValue($FMT))); $kv.Close()
try { Restart-Service AudioEndpointBuilder -Force -ErrorAction Stop; $l.Add('  AudioEndpointBuilder relance') } catch { $l.Add('  ' + $_.Exception.Message) }
Start-Sleep -Seconds 5
foreach ($s in @('AudioEndpointBuilder','Audiosrv')) { $l.Add('  ' + $s + ' = ' + (Get-Service $s).Status) }
$l.Add('FIN — ' + (Get-Date).ToString('o'))
$l | Out-File -FilePath C:\dev\micro-e2-restaure-format.log -Encoding utf8 -Width 500
