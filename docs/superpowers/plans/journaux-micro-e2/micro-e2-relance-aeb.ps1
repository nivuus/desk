# Le controle est la RELECTURE, jamais le code de retour : le registre porte
# desormais 48000 et GetMixFormat rend toujours 44100. AudioEndpointBuilder
# est le service qui CONSTRUIT les points de terminaison et leur format ;
# Audiosrv en depend. On le relance, et l'on remesure.
$l = New-Object System.Collections.Generic.List[string]
$l.Add('RELANCE AudioEndpointBuilder — ' + (Get-Date).ToString('o'))
try { Restart-Service AudioEndpointBuilder -Force -ErrorAction Stop; $l.Add('AudioEndpointBuilder : relance') }
catch { $l.Add('AudioEndpointBuilder REFUSE : ' + $_.Exception.Message) }
Start-Sleep -Seconds 3
foreach ($s in @('AudioEndpointBuilder','Audiosrv')) {
  try { if ((Get-Service $s).Status -ne 'Running') { Start-Service $s -ErrorAction Stop } } catch { $l.Add($s + ' demarrage refuse : ' + $_.Exception.Message) }
  $l.Add(('  ' + $s + ' = ' + (Get-Service $s).Status))
}
Start-Sleep -Seconds 5
$l | Out-File -FilePath C:\dev\micro-e2-relance-aeb.log -Encoding utf8 -Width 500
