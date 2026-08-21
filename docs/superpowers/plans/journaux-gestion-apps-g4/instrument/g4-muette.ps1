# G4 -- CRITERE (3) MONTAGE B : le seul montage qui PEUT etre rouge.
#
# `APPS_FAUTE=muette:<n>` AVALE les n prochaines completions -- ni comptees, ni
# journalisees, ni declenchantes. C'est EXACTEMENT la panne que la
# reconciliation periodique achete reellement, et qu'aucun evenement ne peut
# signaler : une surveillance qui CESSE DE DELIVRER SANS ERREUR.
#
# VERT  (periodique armee) : la reconciliation suivante rattrape -> cles=157.
# ROUGE (`seule`)          : rien ne rattrape -> cles reste a 156.
#
# AUCUNE RAFALE ICI, et c'est essentiel : une rafale consommerait le budget
# d'injection en quelques millisecondes, et le temoin ne serait plus avale.
# PURE ASCII.
$ErrorActionPreference = 'Stop'
$journal = 'C:\dev\agent.log'
$sortie  = 'C:\dev\g4-m.log'
$lnk     = Join-Path ([Environment]::GetFolderPath('Desktop')) 'G4 Temoin.lnk'
$obs = 90
if (Test-Path -LiteralPath 'C:\dev\g4-p.txt') {
  foreach ($l in Get-Content -LiteralPath 'C:\dev\g4-p.txt') { if ($l -match '^observation=(\d+)$') { $obs = [int]$matches[1] } }
}
$flux = New-Object System.IO.StreamWriter($sortie, $false, (New-Object System.Text.UTF8Encoding($false)))
function Dire($m) { $flux.WriteLine([string]$m); $flux.Flush() }
function Lignes-Plates {
  if (-not (Test-Path -LiteralPath $journal)) { return @() }
  $b = New-Object System.IO.FileStream($journal, 'Open', 'Read', 'ReadWrite')
  $r = New-Object System.IO.StreamReader($b, (New-Object System.Text.UTF8Encoding($false)))
  $t = $r.ReadToEnd(); $r.Close(); $b.Close()
  return ($t -replace "\x1b\[[0-9;]*m", '') -split "`r?`n"
}
try {
  Dire "=== G4 CRITERE 3 MONTAGE B (muette) -- observation=${obs}s"
  Dire "date    : $(Get-Date -Format o)"
  foreach ($m in (Lignes-Plates | Where-Object { $_ -match 'mode de surveillance retenu|faute de surveillance ARMEE' })) { Dire "mode    : $m" }
  if (Test-Path -LiteralPath $lnk) { Remove-Item -LiteralPath $lnk -Force; Start-Sleep -Seconds 3 }

  $av = @(Lignes-Plates | Where-Object { $_ -match 'catalogue reconcilie' })
  Dire "reconciliations_avant : $($av.Count)"
  if ($av.Count -gt 0 -and $av[-1] -match 'cles=(\d+)') { Dire "cles_avant : $($matches[1])" }

  $nAvant = @(Lignes-Plates | Where-Object { $_ -match 'catalogue reconcilie' }).Count
  $ws = New-Object -ComObject WScript.Shell
  $r = $ws.CreateShortcut($lnk); $r.TargetPath = 'C:\Windows\System32\notepad.exe'
  $r.Arguments = '--g4-temoin'; $r.Save()
  Dire "temoin_cree : $((Get-Date).ToUniversalTime().ToString('o'))"

  Start-Sleep -Seconds $obs

  $ap = @(Lignes-Plates | Where-Object { $_ -match 'catalogue reconcilie' })
  Dire "reconciliations_apres : $($ap.Count)  (soit $($ap.Count - $nAvant) pendant l'observation)"
  if ($ap.Count -gt 0) {
    Dire "DERNIERE_LIGNE : $($ap[-1])"
    if ($ap[-1] -match 'cles=(\d+)') { Dire "CLES_FINALES : $($matches[1])" }
    if ($ap[-1] -match 'declencheur="?(\w+)"?') { Dire "DECLENCHEUR : $($matches[1])" }
    if ($ap[-1] -match 'notifications=(\d+)') { Dire "NOTIFICATIONS : $($matches[1])" }
  }
  $vue = @($ap | Where-Object { $_ -match 'cles=157' }).Count
  Dire "LIGNES_A_157 : $vue"
  Remove-Item -LiteralPath $lnk -Force -ErrorAction SilentlyContinue
  Dire "=== FIN"
}
catch { Dire "ERREUR : $($_.Exception.Message)"; Dire "=== FIN" }
finally { $flux.Close() }
