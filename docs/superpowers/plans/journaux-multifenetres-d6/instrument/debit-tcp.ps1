# Émetteur de débit brut, côté VM. Écrit sur un socket TCP vers l'hôte aussi
# vite que le chemin l'accepte, pendant $DUREE secondes.
#
# Écrit sur le PARTAGE et invoqué par -File : `nodejs-winrm` enveloppe toute
# commande dans `powershell -Command "& { ... }"`, et un script inline portant
# des guillemets doubles n'y tourne jamais — sans erreur claire (piège
# documenté dans CLAUDE.md).
$ErrorActionPreference = 'Stop'
$Hote  = '192.168.3.1'
$Port  = 9999
$Duree = 10

$tampon = New-Object byte[] (1MB)
(New-Object Random).NextBytes($tampon)

$client = New-Object System.Net.Sockets.TcpClient
$client.NoDelay = $true
$client.SendBufferSize = 4MB
$client.Connect($Hote, $Port)
$flux = $client.GetStream()

$chrono = [System.Diagnostics.Stopwatch]::StartNew()
$octets = [int64]0
while ($chrono.Elapsed.TotalSeconds -lt $Duree) {
    $flux.Write($tampon, 0, $tampon.Length)
    $octets += $tampon.Length
}
$flux.Flush()
$chrono.Stop()
$client.Close()

$s = $chrono.Elapsed.TotalSeconds
$mbps = [math]::Round(($octets * 8) / $s / 1e6, 2)
"emis_octets=$octets secondes=$([math]::Round($s,3)) mbps_emis_calcule=$mbps" |
    Set-Content 'C:\dev\debit-tcp.txt' -Encoding UTF8
