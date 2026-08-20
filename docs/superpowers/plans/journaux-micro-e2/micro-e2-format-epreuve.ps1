# EPREUVE REPRODUCTIBLE du remede de format, restauration COMPRISE.
#
# Applique le blob symetrique, remesure, puis REMET l'ancien et remesure. La
# restauration est dans le MEME script que l'application : la VM ne peut pas
# rester de travers si le pilote perd la main. Precedent : D8/C1, un registre
# audio laisse de travers a bloque le produit entier.
$l = New-Object System.Collections.Generic.List[string]
$FMT  = '{f19f064d-082c-4e27-bc73-6882a1bb8e4c},0'
$sous = 'SOFTWARE\Microsoft\Windows\CurrentVersion\MMDevices\Audio'
$gCapt = '{5fae72b2-f885-4038-9827-0d91f862a7d5}'
$gRendu = '{deec1914-6490-47ee-9475-091b9a2ea537}'
function Hexa($o) { if ($null -eq $o) { '(null)' } else { ($o | ForEach-Object { $_.ToString('X2') }) -join ' ' } }
function Ecrire($octets) {
  try {
    $k = [Microsoft.Win32.Registry]::LocalMachine.OpenSubKey("$sous\Capture\$gCapt\Properties",
           [Microsoft.Win32.RegistryKeyPermissionCheck]::ReadWriteSubTree,
           [System.Security.AccessControl.RegistryRights]::SetValue -bor
           [System.Security.AccessControl.RegistryRights]::QueryValues)
    $k.SetValue($FMT, $octets, [Microsoft.Win32.RegistryValueKind]::Binary); $k.Close(); return 'acceptee'
  } catch { return 'REFUSEE : ' + $_.Exception.Message }
}
function Lire($guid) {
  $k = [Microsoft.Win32.Registry]::LocalMachine.OpenSubKey("$sous\$(if($guid -eq $gRendu){'Render'}else{'Capture'})\$guid\Properties", $false)
  $v = $k.GetValue($FMT); $k.Close()
  # ⚠️ Le CAST est obligatoire : `return` deroule un byte[] en Object[], et
  # SetValue le refuse ensuite en RegistryValueKind::Binary. La premiere
  # version de ce script a laisse la VM DE TRAVERS pour cette seule raison —
  # l'application avait reussi, la restauration non.
  return ,[byte[]]$v
}
function Juger($etiquette) {
  & powershell -NoProfile -ExecutionPolicy Bypass -File C:\dev\micro-ecoute-e2.ps1 `
      -Prefixe 'CABLE Output' -Secondes 3 -Journal "C:\dev\micro-e2-format-epreuve-$etiquette.log" *> $null
}
$l.Add('EPREUVE DU REMEDE DE FORMAT — ' + (Get-Date).ToString('o'))
$rendu = Lire $gRendu
$sauvegarde = Lire $gCapt
$l.Add('RENDU        : ' + (Hexa $rendu))
$l.Add('SAUVEGARDE   : ' + (Hexa $sauvegarde))
$neuf = New-Object byte[] $sauvegarde.Length
[Array]::Copy($sauvegarde, 0, $neuf, 0, 8)
[Array]::Copy($rendu, 8, $neuf, 8, $rendu.Length - 8)
$l.Add('CIBLE        : ' + (Hexa $neuf))
$l.Add('--- APPLICATION ---')
$l.Add('  ecriture : ' + (Ecrire $neuf))
$l.Add('  relu     : ' + (Hexa (Lire $gCapt)))
Restart-Service AudioEndpointBuilder -Force -ErrorAction SilentlyContinue
Start-Sleep -Seconds 6
Juger 'applique'
$l.Add('--- RESTAURATION ---')
$l.Add('  ecriture : ' + (Ecrire $sauvegarde))
$l.Add('  relu     : ' + (Hexa (Lire $gCapt)))
Restart-Service AudioEndpointBuilder -Force -ErrorAction SilentlyContinue
Start-Sleep -Seconds 6
Juger 'restaure'
$l.Add('FIN — ' + (Get-Date).ToString('o'))
$l | Out-File -FilePath C:\dev\micro-e2-format-epreuve.log -Encoding utf8 -Width 500
