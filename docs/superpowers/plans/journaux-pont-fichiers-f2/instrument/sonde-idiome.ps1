# SONDE PRÉALABLE DE F2 — quel idiome d'enregistrement emploie CETTE VM ?
#
# ✅ ELLE A ÉTÉ EXÉCUTÉE, DEUX FOIS, le 21 aout 2026 — en session 0 (WinRM) et
# en session 1 (tache planifiee /it). Ses deux journaux sont verses a cote
# d'elle (`f2-tache14-sonde-idiome-session{0,1}.log`), et son verdict dans
# `f2-tache14-verdict.md`.
#
# ⚠️ Une redaction anterieure de cet en-tete disait « ELLE N'A PAS ETE
# EXECUTEE » : c'etait exact a sa date, la VM etant tenue par un chantier
# concurrent.
#
# 🔵 VERDICT : R-F2-1 EST LEVE. Les CINQ outils eprouves ecrivent EN PLACE --
# `WriteAllText`, `Add-Content`, `cmd >`, `Set-Content`, et `notepad.exe` en
# session interactive. ⚠️ PORTEE EXACTE : mesure sur un repertoire NTFS
# ORDINAIRE, pas dans une racine ProjFS, et cela ne dit RIEN de LibreOffice ni
# de Word -- dont l'idiome « ecrire un temporaire, renommer, supprimer » est
# precisement l'objet de F3.
#
# POURQUOI ELLE EXISTE, ET POURQUOI AVANT LA RECETTE
#
# F2 REFUSE `PRE_RENAME` et `PRE_DELETE` : `Renommer` et `Supprimer` sont des
# livrables de F3. Une application qui emploie l'idiome « écrire un temporaire,
# renommer, supprimer l'ancien » échouera donc BRUYAMMENT au renommage — ce qui
# est le seul comportement honnête, mais qui rend son enregistrement
# inobservable par F2.
#
# 🔴 SI AUCUN OUTIL DE CETTE VM N'ÉCRIT EN PLACE, LE CRITÈRE ① DE F2 N'EST PAS
# RECETTABLE, et c'est le risque R-F2-1 : le seul qui rendrait F2 non livrable.
# Il faut le savoir AVANT d'écrire un critère, pour ne pas imputer au produit un
# échec de protocole.
#
# ⚠️ SON CRITÈRE EST UNE PRÉSENCE, JAMAIS UNE ABSENCE. Elle conclut « temp+rename »
# sur l'APPARITION d'un fichier temporaire pendant l'enregistrement ; un
# répertoire observé sans qu'aucun enregistrement n'ait eu lieu rend
# « NON MESURE », jamais « en place ». C'est le remède au faux verdict
# éliminatoire de la sonde P0 du presse-papier, qui avait lu trois zéros sur une
# VM saine et conclu à une panne.
#
# ⚠️ ELLE N'ÉCRIT QUE DANS `C:\dev\sonde-idiome`, jamais dans la racine du pont
# — qui n'existe pas encore à ce stade.
#
# ⚠️ AUCUN GUILLEMET EN LIGNE DE COMMANDE : `nodejs-winrm` enveloppe toujours la
# commande dans `powershell -Command "& { … }"`, et un guillemet y entre en
# collision SANS message clair. Ce script s'invoque par `-File`.

$ErrorActionPreference = 'Continue'
$base = 'C:\dev\sonde-idiome'
if (Test-Path $base) { Remove-Item $base -Recurse -Force }
New-Item -ItemType Directory -Path $base | Out-Null

function Mesurer([string]$nom, [scriptblock]$enregistrer) {
    $dossier = Join-Path $base $nom
    New-Item -ItemType Directory -Path $dossier | Out-Null
    $cible = Join-Path $dossier 'document.txt'
    Set-Content -Path $cible -Value 'contenu initial' -Encoding UTF8

    # L'observateur note TOUT ce qui bouge dans le dossier pendant
    # l'enregistrement. C'est lui qui distingue les deux idiomes :
    #   en place    -> Changed sur document.txt, et RIEN d'autre ;
    #   temp+rename -> Created d'un nom tiers, puis Renamed, puis Deleted.
    $observateur = New-Object System.IO.FileSystemWatcher
    $observateur.Path = $dossier
    $observateur.IncludeSubdirectories = $false
    $observateur.NotifyFilter = [System.IO.NotifyFilters]::FileName -bor [System.IO.NotifyFilters]::LastWrite
    $evenements = New-Object System.Collections.ArrayList
    $sync = [System.Collections.ArrayList]::Synchronized($evenements)
    foreach ($genre in 'Created', 'Deleted', 'Renamed', 'Changed') {
        Register-ObjectEvent -InputObject $observateur -EventName $genre -MessageData $sync -Action {
            [void] $Event.MessageData.Add(('{0} {1}' -f $Event.SourceEventArgs.ChangeType, $Event.SourceEventArgs.Name))
        } | Out-Null
    }
    $observateur.EnableRaisingEvents = $true

    $issue = 'OK'
    try { & $enregistrer $cible } catch { $issue = ('EXCEPTION ' + $_.Exception.Message) }
    Start-Sleep -Milliseconds 1500
    $observateur.EnableRaisingEvents = $false
    Get-EventSubscriber | Where-Object { $_.SourceObject -eq $observateur } | Unregister-Event
    $observateur.Dispose()

    $vus = @($sync)
    $noms = @(Get-ChildItem -Path $dossier | Select-Object -ExpandProperty Name)
    $contenu = if (Test-Path $cible) { Get-Content -Path $cible -Raw } else { '<ABSENT>' }

    # 🔴 LE VERDICT, ET SES TROIS ÉTATS. « NON MESURE » n'est pas un échec du
    # produit : c'est une mesure qui n'a pas eu lieu, et les confondre est ce
    # que cette sonde existe pour empêcher.
    $verdict =
        if ($vus.Count -eq 0) { 'NON MESURE (aucun evenement : l enregistrement n a pas eu lieu)' }
        elseif ($vus -match '^Renamed ') { 'TEMP+RENAME (un renommage est observe)' }
        elseif (($vus | Where-Object { $_ -match '^Created ' -and $_ -notmatch 'document\.txt$' }).Count -gt 0) {
            'TEMP+RENAME probable (un fichier tiers est apparu)'
        }
        else { 'EN PLACE' }

    Write-Output ('=== ' + $nom)
    Write-Output ('    issue          : ' + $issue)
    Write-Output ('    evenements     : ' + ($vus -join ' | '))
    Write-Output ('    fichiers restes: ' + ($noms -join ', '))
    Write-Output ('    contenu final  : ' + ($contenu -replace '\r?\n', '\n'))
    Write-Output ('    VERDICT        : ' + $verdict)
    Write-Output ''
}

Write-Output ('sonde-idiome, ' + (Get-Date -Format o))
Write-Output ('session Windows : ' + (Get-Process -Id $PID).SessionId + ' (0 = WinRM, 1 = interactive)')
Write-Output ''

Mesurer 'io-file-writealltext' { param($c) [IO.File]::WriteAllText($c, 'contenu reecrit par WriteAllText') }
Mesurer 'add-content'          { param($c) Add-Content -Path $c -Value 'une ligne de plus' }
Mesurer 'cmd-redirection'      { param($c) cmd /c ('echo reecrit par cmd> "' + $c + '"') }
Mesurer 'set-content'          { param($c) Set-Content -Path $c -Value 'reecrit par Set-Content' }

# 🔴 LE BLOC-NOTES EXIGE UNE SESSION INTERACTIVE, et il le DIT plutôt que de
# rendre un faux « NON MESURE ». `SendKeys` n'atteint aucun bureau depuis la
# session 0 de WinRM : lancer ce script par `powershell -File` via WinRM ne
# mesurera JAMAIS cet outil. Il faut une tâche planifiée `/it`, comme
# `scripts/run-agent.sh`.
if ((Get-Process -Id $PID).SessionId -eq 0) {
    Write-Output '=== notepad'
    Write-Output '    VERDICT        : NON MESURABLE DEPUIS LA SESSION 0 (WinRM).'
    Write-Output '    Relancer ce script par une tache planifiee /it pour le mesurer.'
} else {
    Mesurer 'notepad' {
        param($c)
        $p = Start-Process notepad.exe -ArgumentList $c -PassThru
        Start-Sleep -Seconds 2
        $shell = New-Object -ComObject WScript.Shell
        [void] $shell.AppActivate($p.Id)
        Start-Sleep -Milliseconds 500
        $shell.SendKeys('reecrit par le Bloc-notes')
        Start-Sleep -Milliseconds 500
        $shell.SendKeys('^s')
        Start-Sleep -Seconds 2
        Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue
    }
}

Write-Output ''
Write-Output 'LECTURE DU RESULTAT :'
Write-Output '  au moins un EN PLACE  -> il devient l instrument du critere (1) de F2 ;'
Write-Output '  aucun EN PLACE        -> F2 n est pas recevable sans F3 (risque R-F2-1),'
Write-Output '                           et la recette le DECLARE au lieu de mesurer autre chose.'
