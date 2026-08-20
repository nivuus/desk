# Etat de la VM a la sortie du bloc : aucun agent vivant, registre restaure.
$l = New-Object System.Collections.Generic.List[string]
$l.Add('ETAT FINAL — ' + (Get-Date).ToString('o'))
$l.Add('agents vivants : ' + @(Get-Process agent -ErrorAction SilentlyContinue).Count)
$c = 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\MMDevices\Audio\Capture\{5fae72b2-f885-4038-9827-0d91f862a7d5}\Properties'
$b = (Get-ItemProperty -Path $c).'{f19f064d-082c-4e27-bc73-6882a1bb8e4c},0'
$l.Add('CABLE Output, Hz au registre : ' + [BitConverter]::ToInt32($b, 12) + '   (attendu 44100 — restaure)')
$l.Add('blob : ' + (($b | ForEach-Object { $_.ToString('X2') }) -join ' '))
$l | Out-File -FilePath C:\dev\micro-e2-etat-final.log -Encoding utf8 -Width 500
