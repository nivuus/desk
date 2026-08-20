$ErrorActionPreference='Continue'
"DATE=" + (Get-Date).ToString('o')
"UPTIME=" + ((Get-Date) - (Get-CimInstance Win32_OperatingSystem).LastBootUpTime).ToString()
$p = Get-Process agent -ErrorAction SilentlyContinue
if ($p) { $p | ForEach-Object { "AGENT pid=" + $_.Id + " debut=" + $_.StartTime.ToString('o') } } else { "AGENT aucun" }
$c = Get-Process chrome -ErrorAction SilentlyContinue
if ($c) { "CHROME n=" + @($c).Count } else { "CHROME aucun" }
$n = Get-Process notepad -ErrorAction SilentlyContinue
if ($n) { "NOTEPAD n=" + @($n).Count } else { "NOTEPAD aucun" }
"AUDIOSRV=" + (Get-Service Audiosrv).Status
Get-PnpDevice -Class AudioEndpoint -ErrorAction SilentlyContinue | Where-Object { $_.Status -eq 'OK' } | ForEach-Object { "ENDPOINT_OK " + $_.FriendlyName }
