# Meme joueur, journal distinct : lance en SESSION 0 (fils d'un shell WinRM).
[Console]::OutputEncoding = New-Object System.Text.UTF8Encoding($false)
$flux = New-Object System.IO.StreamWriter('C:\dev\micro-e2-jouer-s0.log', $false, (New-Object System.Text.UTF8Encoding($false)))
try {
  & powershell -NoProfile -ExecutionPolicy Bypass -File C:\dev\abis-jouer.ps1 -Prefixe 'VB-Audio' -Hz 660 -Millisecondes 26000 *>&1 |
    ForEach-Object { $flux.WriteLine([string]$_); $flux.Flush() }
} finally { $flux.Close() }
