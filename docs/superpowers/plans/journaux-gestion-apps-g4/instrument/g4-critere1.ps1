# G4 -- CRITERE (1) : un raccourci cree apparait-il en moins de cinq secondes ?
#
# L'HORODATAGE DE CREATION SE PREND SUR LA VM, par le script qui cree le
# fichier -- JAMAIS sur l'hote : les deux horloges divergent, et agent.log est
# en heure VM.
#
# PURE ASCII.
$ErrorActionPreference = 'Stop'
$journal = 'C:\dev\agent.log'
$sortie  = 'C:\dev\g4-c1.log'
$lnk     = Join-Path ([Environment]::GetFolderPath('Desktop')) 'G4 Temoin.lnk'

$flux = New-Object System.IO.StreamWriter($sortie, $false, (New-Object System.Text.UTF8Encoding($false)))
function Dire($m) { $flux.WriteLine([string]$m); $flux.Flush() }

# agent.log porte les sequences ANSI de `tracing` : les retirer AVANT tout
# filtre, sans quoi 'declencheur=notification' ne matche jamais litteralement.
function Lignes-Plates {
  if (-not (Test-Path -LiteralPath $journal)) { return @() }
  $b = New-Object System.IO.FileStream($journal, 'Open', 'Read', 'ReadWrite')
  $r = New-Object System.IO.StreamReader($b, (New-Object System.Text.UTF8Encoding($false)))
  $t = $r.ReadToEnd(); $r.Close(); $b.Close()
  return ($t -replace "\x1b\[[0-9;]*m", '') -split "`r?`n"
}
function Reconciliations { @(Lignes-Plates | Where-Object { $_ -match 'catalogue reconcilie' }) }

try {
  Dire "=== G4 CRITERE 1"
  Dire "date_debut : $(Get-Date -Format o)"
  Dire "session    : $((Get-Process -Id $PID).SessionId)"

  # LE MODE RETENU -- c'est la preuve que la variable a ATTEINT le processus.
  $mode = @(Lignes-Plates | Where-Object { $_ -match 'mode de surveillance retenu' })
  Dire "mode_lignes : $($mode.Count)"
  foreach ($m in $mode) { Dire "mode       : $m" }

  if (Test-Path -LiteralPath $lnk) { Remove-Item -LiteralPath $lnk -Force }

  # ATTENDRE LE FAIT : au moins une reconciliation doit avoir eu lieu, sans quoi
  # on mesurerait le demarrage de l'agent.
  $t = 0
  while ((Reconciliations).Count -lt 1 -and $t -lt 120) { Start-Sleep -Seconds 1; $t++ }
  $base = (Reconciliations)
  Dire "reconciliations_avant : $($base.Count)"
  if ($base.Count -lt 1) { Dire "ERREUR : aucune reconciliation, l'agent ne tourne pas"; Dire "=== FIN"; exit }
  Dire "derniere_avant : $($base[-1])"

  # LE ROUGE N'EST ROUGE QUE SI LE DELAI DEPASSE 5 s : un raccourci cree une
  # seconde avant une reconciliation periodique passerait. On cree donc JUSTE
  # APRES une ligne `catalogue reconcilie`, ce qui donne au bras desarme la
  # periode ENTIERE a couvrir.
  $avantCount = $base.Count
  $t = 0
  while ((Reconciliations).Count -eq $avantCount -and $t -lt 600) { Start-Sleep -Milliseconds 100; $t++ }
  Dire "attente_synchro_ms : $($t * 100)"

  # ---- T0, SUR LA VM
  $sw = [Diagnostics.Stopwatch]::StartNew()
  $t0 = Get-Date
  $ws = New-Object -ComObject WScript.Shell
  $r = $ws.CreateShortcut($lnk)
  $r.TargetPath = 'C:\Windows\System32\notepad.exe'
  # ARGUMENT DISTINCT : sans lui la cle du temoin serait celle d'un raccourci
  # existant et le catalogue n'en garderait qu'un -- G2 a perdu un temoin sur
  # deux pour cette raison exacte.
  $r.Arguments = '--g4-temoin'
  $r.Save()
  Dire "t0_creation : $($t0.ToString('o'))"
  Dire "lnk         : $lnk"

  $apresCount = (Reconciliations).Count
  # ---- attendre la reconciliation qui VOIT le temoin
  $trouvee = $null
  $t = 0
  while ($null -eq $trouvee -and $t -lt 1200) {
    $r2 = Reconciliations
    if ($r2.Count -gt $apresCount) {
      foreach ($l in $r2[$apresCount..($r2.Count - 1)]) {
        if ($l -match 'apparues=([1-9]\d*)') { $trouvee = $l; break }
      }
      $apresCount = $r2.Count
    }
    if ($null -eq $trouvee) { Start-Sleep -Milliseconds 100; $t++ }
  }
  $sw.Stop()
  if ($null -eq $trouvee) {
    Dire "VERDICT : AUCUNE reconciliation avec apparues>=1 en $([math]::Round($sw.Elapsed.TotalSeconds,1)) s"
  } else {
    Dire "ligne_trouvee : $trouvee"
    # Le delai se prend sur l'HORODATAGE DE LA LIGNE, pas sur mon sondage : mon
    # sondage a une granularite de 100 ms qui n'est pas celle du produit.
    if ($trouvee -match '^(\S+?)Z?\s') {
      $t1 = [datetime]::Parse($matches[1], [Globalization.CultureInfo]::InvariantCulture, [Globalization.DateTimeStyles]::AdjustToUniversal)
      $delta = ($t1 - $t0.ToUniversalTime()).TotalMilliseconds
      Dire "t1_ligne    : $($t1.ToString('o'))"
      Dire "DELAI_MS    : $([math]::Round($delta,0))"
    }
    Dire "delai_sondage_ms : $([math]::Round($sw.Elapsed.TotalMilliseconds,0))"
    if ($trouvee -match 'declencheur="?(\w+)"?') { Dire "DECLENCHEUR : $($matches[1])" }
    if ($trouvee -match 'notifications=(\d+)') { Dire "notifications : $($matches[1])" }
    if ($trouvee -match 'debordements=(\d+)') { Dire "debordements  : $($matches[1])" }
    if ($trouvee -match 'cles=(\d+)') { Dire "cles          : $($matches[1])" }
  }

  # ---- nettoyage et recomptage
  Remove-Item -LiteralPath $lnk -Force -ErrorAction SilentlyContinue
  Start-Sleep -Seconds 2
  $racines = @(
    [Environment]::GetFolderPath('Desktop'),
    (Join-Path $env:PUBLIC 'Desktop'),
    (Join-Path $env:APPDATA 'Microsoft\Windows\Start Menu'),
    (Join-Path $env:ProgramData 'Microsoft\Windows\Start Menu')
  )
  $tot = 0
  foreach ($d in $racines) { if (Test-Path -LiteralPath $d) { $tot += @(Get-ChildItem -LiteralPath $d -Recurse -Filter *.lnk -Force -ErrorAction SilentlyContinue).Count } }
  Dire "corpus_apres : $tot  (220 exige)"
  Dire "=== FIN"
}
catch { Dire "ERREUR : $($_.Exception.Message)"; Dire "TRACE : $($_.ScriptStackTrace)"; Dire "=== FIN" }
finally { $flux.Close() }
