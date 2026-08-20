# Nommer le refus : QUI possede la cle, et qui peut y ecrire. C'est une
# LECTURE — on ne contourne rien, on decrit ce qui refuse.
$c = 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\MMDevices\Audio\Capture\{5fae72b2-f885-4038-9827-0d91f862a7d5}\Properties'
$l = New-Object System.Collections.Generic.List[string]
$a = Get-Acl -Path $c
$l.Add('CLE      : ' + $c)
$l.Add('PROPRIETAIRE : ' + $a.Owner)
foreach ($r in $a.Access) {
  $l.Add(('  {0,-45} {1,-6} {2}' -f $r.IdentityReference, $r.AccessControlType, $r.RegistryRights))
}
$l | Out-File -FilePath C:\dev\micro-e2-acl.log -Encoding utf8 -Width 500
