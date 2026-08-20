# Joue 660 Hz sur le rendu du cable pendant 26 s, et journalise AUSSI le
# crete-metre de abis-metre.ps1 : c'est le TEMOIN POSITIF de la sonde.
[Console]::OutputEncoding = New-Object System.Text.UTF8Encoding($false)
$flux = New-Object System.IO.StreamWriter('C:\dev\micro-e2-jouer.log', $false, (New-Object System.Text.UTF8Encoding($false)))
try {
  & powershell -NoProfile -ExecutionPolicy Bypass -File C:\dev\abis-jouer.ps1 -Prefixe 'VB-Audio' -Hz 660 -Millisecondes 26000 *>&1 |
    ForEach-Object { $flux.WriteLine([string]$_); $flux.Flush() }
} finally { $flux.Close() }
