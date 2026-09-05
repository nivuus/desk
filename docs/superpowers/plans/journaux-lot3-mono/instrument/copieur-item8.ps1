# Lot 3, item 8 (3.2) -- LE GESTE, cote invite, en SESSION 1.
#
# Il ecrit un marqueur unique dans le presse-papier de la VM, puis le relit.
# IL NE CONCLUT RIEN : c'est le compte de messages pousses au navigateur qui
# repond a l'item.
#
# LA SESSION 1 EST OBLIGATOIRE : le presse-papier est par STATION DE FENETRES,
# et `Get-Clipboard` par WinRM (session 0) rend -1 -- mesure par la sonde P0 du
# sous-bloc P1. La sonde IMPRIME sa session plutot que de la supposer.
#
# ATTENTION -- ASCII PUR, invariant :
#     LC_ALL=C grep -c '[^ -~]' copieur-item8.ps1   doit rendre 0
param(
    [Parameter(Mandatory = $true)][string]$Marqueur,
    [string]$Journal = 'C:\nivuus\item8-copieur.log'
)
$ErrorActionPreference = 'Continue'
function Dire($m) {
    $l = "$([DateTime]::UtcNow.ToString('o')) $m"
    Add-Content -Path $Journal -Value $l -Encoding utf8
    Write-Output $l
}
Remove-Item $Journal -Force -ErrorAction SilentlyContinue
Dire ("session = " + (Get-Process -Id $PID).SessionId)
Dire ("marqueur = " + $Marqueur)

# TROIS ecritures espacees : le sondeur du capteur echantillonne le compteur de
# sequence, et une seule ecriture pourrait tomber entre deux tours. Trois
# ecritures rendent le zero du bras mono-fenetre bien plus difficile a
# attribuer a un rate d'echantillonnage.
foreach ($i in 1..3) {
    $texte = "$Marqueur-$i"
    try {
        Set-Clipboard -Value $texte
        Start-Sleep -Seconds 3
        $lu = ''
        try { $lu = Get-Clipboard -Raw } catch { $lu = '<illisible>' }
        Dire ("ecriture $i : pose='$texte' relu='" + $lu.Trim() + "' identique=" + ($lu.Trim() -eq $texte))
    } catch {
        Dire ("ecriture $i : ECHEC -- " + $_.Exception.Message)
    }
}
Dire 'FIN'
