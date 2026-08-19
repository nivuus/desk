param([string]$Prefixe = '', [int]$Hz = 440)
[Console]::OutputEncoding = New-Object System.Text.UTF8Encoding($false)
$flux = New-Object System.IO.StreamWriter('C:\dev\abis-tonalite.log', $false, (New-Object System.Text.UTF8Encoding($false)))
try {
  & powershell -NoProfile -ExecutionPolicy Bypass -File C:\dev\abis-jouer.ps1 -Prefixe $Prefixe -Hz $Hz -Millisecondes 26000 *>&1 |
    ForEach-Object { $flux.WriteLine([string]$_); $flux.Flush() }
} finally { $flux.Close() }
