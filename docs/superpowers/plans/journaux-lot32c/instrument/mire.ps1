# Mire animee du lot 32C : une source qui CHANGE, a cadence connue, et qui
# AFFICHE sa propre cadence.
#
# POURQUOI ELLE DOIT BOUGER. Desktop Duplication n'emet qu'au CHANGEMENT du
# bureau : une mire immobile rend framesDecoded=0 sur TOUS les bras, et le
# rouge devient vacueux. Ce depot l'a paye.
#
# ET POURQUOI ELLE AFFICHE SA CADENCE. Un pilote qui "sait" que la source
# anime a 10 Hz sans que la source le dise mesure sa propre croyance. Le
# compteur et les i/s mesurees sont peints DANS l'image capturee.
#
# ASCII pur : un .ps1 sans BOM portant un seul caractere non-ASCII ne
# s'analyse pas, et l'erreur designe une AUTRE ligne.
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing

$f = New-Object System.Windows.Forms.Form
$f.Text = "MIRE-LOT32C"
$f.Width = 900
$f.Height = 600
$f.BackColor = [System.Drawing.Color]::Black
$f.TopMost = $true

$n = 0
$t0 = Get-Date
$police = New-Object System.Drawing.Font("Consolas", 28, [System.Drawing.FontStyle]::Bold)

$f.Add_Paint({
    param($s, $e)
    $g = $e.Graphics
    $ecoule = ((Get-Date) - $t0).TotalSeconds
    $ips = if ($ecoule -gt 0) { $n / $ecoule } else { 0 }
    # Une barre qui traverse : le CHANGEMENT que la duplication doit voir.
    $x = ($n * 17) % 820
    $g.FillRectangle([System.Drawing.Brushes]::Lime, $x, 300, 60, 160)
    # Un damier qui alterne : garantit un delta meme si la barre sort du cadre.
    $c = if ($n % 2 -eq 0) { [System.Drawing.Brushes]::White } else { [System.Drawing.Brushes]::Red }
    $g.FillRectangle($c, 20, 20, 120, 120)
    # La cadence, PEINTE dans l image : c est la source qui la declare.
    $g.DrawString(("TRAME " + $n), $police, [System.Drawing.Brushes]::Yellow, 170, 30)
    $g.DrawString(("MESURE " + $ips.ToString("0.00") + " i/s"), $police, [System.Drawing.Brushes]::Cyan, 170, 80)
    $g.DrawString(("CIBLE 10.00 i/s"), $police, [System.Drawing.Brushes]::Gray, 170, 130)
})

$t = New-Object System.Windows.Forms.Timer
$t.Interval = 100
$t.Add_Tick({ $script:n++; $f.Invalidate() })
$t.Start()

[System.Windows.Forms.Application]::Run($f)
