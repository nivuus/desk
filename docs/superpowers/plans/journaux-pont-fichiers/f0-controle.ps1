$f = Get-WindowsOptionalFeature -Online -FeatureName Client-ProjFS
'FeatureName : ' + $f.FeatureName
'State       : ' + $f.State
'ProjectedFSLib.dll : ' + (Test-Path ($env:SystemRoot + '\system32\ProjectedFSLib.dll'))
'PrjFlt.sys         : ' + (Test-Path ($env:SystemRoot + '\system32\drivers\PrjFlt.sys'))
$s = Get-Service PrjFlt -ErrorAction SilentlyContinue
if ($s) { 'service PrjFlt : ' + $s.Status + ' / ' + $s.StartType } else { 'service PrjFlt : ABSENT' }
'--- fltmc filters ---'
fltmc filters
