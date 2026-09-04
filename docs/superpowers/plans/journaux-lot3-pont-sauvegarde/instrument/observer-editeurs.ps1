# Lot 3, item 1 (3.7) -- l'observateur d'editeurs, cote invite, SESSION 1.
#
# Il ouvre un fichier de la racine ProjFS du pont dans un editeur REEL, y ecrit
# un marqueur unique, enregistre, ferme. IL NE CONCLUT RIEN : ce sont les
# notifications ProjFS de l'agent, et la relecture cote navigateur, qui disent
# ce qui s'est passe.
#
# LA SESSION 1 EST OBLIGATOIRE : SendKeys n'atteint aucun bureau depuis la
# session 0 (WinRM), et la sonde d'idiome du 21 aout 2026 le DIT plutot que de
# rendre un faux << NON MESURE >>.
#
# LA RACINE ARRIVE EN PARAMETRE, jamais par git : l'invite n'a pas le depot.
#
# ATTENTION -- CE FICHIER EST EN ASCII PUR, ET C'EST UN INVARIANT :
#     LC_ALL=C grep -c '[^ -~]' observer-editeurs.ps1   doit rendre 0
# Un .ps1 sans BOM portant un seul caractere non-ASCII ne s'analyse pas, et
# l'erreur designe une AUTRE ligne.
param(
    [Parameter(Mandatory = $true)][string]$Racine,
    [Parameter(Mandatory = $true)][string]$Fichier,
    [string]$Journal = 'C:\nivuus\item1-observateur.log'
)
$ErrorActionPreference = 'Continue'
Add-Type -AssemblyName System.Windows.Forms

function Dire($m) {
    $ligne = "$([DateTime]::UtcNow.ToString('o')) $m"
    Add-Content -Path $Journal -Value $ligne -Encoding utf8
    Write-Output $ligne
}

Remove-Item $Journal -Force -ErrorAction SilentlyContinue
Dire ("session = " + (Get-Process -Id $PID).SessionId)
Dire ("racine  = " + $Racine)
Dire ("fichier = " + $Fichier)

$chemin = Join-Path $Racine $Fichier
Dire ("le fichier est-il visible dans la racine ProjFS ? " + (Test-Path $chemin))
if (Test-Path $chemin) {
    Dire ("contenu AVANT (longueur) : " + (Get-Content $chemin -Raw -ErrorAction SilentlyContinue).Length)
}

# L'INVENTAIRE AVANT, pour que << aucun temporaire >> soit un ecart mesure et
# non une impression. On prend les NOMS, pas un compte : un temporaire cree
# puis renomme laisserait le compte inchange.
$avant = @(Get-ChildItem $Racine -Force -ErrorAction SilentlyContinue | ForEach-Object { $_.Name })
Dire ("inventaire AVANT : " + ($avant -join ' | '))

$marqueur = "MARQUEUR-LOT3-ITEM1-" + [DateTime]::UtcNow.ToString('HHmmss')
Dire ("marqueur = " + $marqueur)

# ---- L'EDITEUR REEL -------------------------------------------------------
# Un seul editeur existe sur cette appliance : le Bloc-notes. Ni WordPad ni
# VS Code n'y sont installes -- releve, pas suppose ; voir le verdict.
Dire 'ouverture du Bloc-notes'
$p = Start-Process notepad.exe -ArgumentList "`"$chemin`"" -PassThru
Start-Sleep -Seconds 4

# Porter la fenetre au premier plan : SendKeys frappe la fenetre ACTIVE, et si
# une autre l'est, on mesurerait une frappe partie ailleurs.
try {
    [Microsoft.VisualBasic.Interaction]::AppActivate($p.Id)
} catch {
    Add-Type -AssemblyName Microsoft.VisualBasic
    try { [Microsoft.VisualBasic.Interaction]::AppActivate($p.Id) } catch { Dire ("AppActivate a echoue : " + $_.Exception.Message) }
}
Start-Sleep -Milliseconds 800

[System.Windows.Forms.SendKeys]::SendWait('^{END}')
Start-Sleep -Milliseconds 300
[System.Windows.Forms.SendKeys]::SendWait($marqueur)
Start-Sleep -Milliseconds 600
Dire 'Ctrl+S'
[System.Windows.Forms.SendKeys]::SendWait('^s')
Start-Sleep -Seconds 3

# L'INVENTAIRE PENDANT que l'editeur tient encore le fichier : un temporaire
# vit parfois le temps de l'enregistrement seulement.
$pendant = @(Get-ChildItem $Racine -Force -ErrorAction SilentlyContinue | ForEach-Object { $_.Name })
Dire ("inventaire PENDANT (editeur ouvert, apres Ctrl+S) : " + ($pendant -join ' | '))

Dire 'fermeture'
try { $p | Stop-Process -Force -ErrorAction SilentlyContinue } catch { }
Start-Sleep -Seconds 3

$apres = @(Get-ChildItem $Racine -Force -ErrorAction SilentlyContinue | ForEach-Object { $_.Name })
Dire ("inventaire APRES : " + ($apres -join ' | '))

# CE QUE L'INVITE VOIT DU FICHIER. Ce n'est PAS le critere -- le critere est ce
# que le poste local recoit -- mais l'ecart entre les deux est ce qui
# nommerait une perte silencieuse.
if (Test-Path $chemin) {
    $texte = Get-Content $chemin -Raw -ErrorAction SilentlyContinue
    Dire ("contenu APRES (longueur) : " + $texte.Length)
    Dire ("le marqueur est-il dans le fichier, cote INVITE ? " + $texte.Contains($marqueur))
} else {
    Dire 'le fichier a DISPARU de la racine ProjFS'
}

Dire ("nouveaux noms (APRES moins AVANT) : " + ((Compare-Object $avant $apres | Where-Object { $_.SideIndicator -eq '=>' } | ForEach-Object { $_.InputObject }) -join ' | '))
Dire 'FIN'
