# Émetteur UDP, côté VM. Datagrammes de 1200 octets — la taille utile d'un
# paquet RTP sur ce chemin — émis aussi vite que PowerShell le permet.
#
# ⚠️ Cette mesure peut être bornée par l'ÉMETTEUR (coût par datagramme dans
# PowerShell) et non par le pont. C'est pourquoi le script rapporte AUSSI son
# propre débit d'émission : si le reçu égale l'émis, le chiffre est un
# plancher du pont, pas sa capacité.
$ErrorActionPreference = 'Stop'
$Hote  = '192.168.3.1'
$Port  = 9998
$Duree = 10

$tampon = New-Object byte[] 1200
(New-Object Random).NextBytes($tampon)

$sock = New-Object System.Net.Sockets.UdpClient
$sock.Client.SendBufferSize = 4MB
$sock.Connect($Hote, $Port)

$chrono = [System.Diagnostics.Stopwatch]::StartNew()
$paquets = [int64]0
while ($chrono.Elapsed.TotalSeconds -lt $Duree) {
    for ($i = 0; $i -lt 200; $i++) { [void]$sock.Send($tampon, $tampon.Length) }
    $paquets += 200
}
$chrono.Stop()
$sock.Close()

$s = $chrono.Elapsed.TotalSeconds
$octets = $paquets * 1200
$mbps = [math]::Round(($octets * 8) / $s / 1e6, 2)
"emis_paquets=$paquets emis_octets=$octets secondes=$([math]::Round($s,3)) mbps_emis_calcule=$mbps" |
    Set-Content 'C:\dev\debit-udp.txt' -Encoding UTF8
