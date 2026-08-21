# G4 -- la rafale REJOUEE SUR LE PRODUIT, et le comptage des reconciliations
# dans une FENETRE BORNEE PAR DEUX HORODATAGES.
#
# Jamais sur un total de fichier : c'est la consequence que apps/boucle.rs
# ecrit lui-meme de l'ensemble `ecartes`, et que le sous-bloc D2 a payee sur ses
# " 44 avant / 44 apres ".
#
# Parametres par FICHIER (une tache planifiee demarre dans un environnement
# NEUF) : C:\dev\g4-p.txt, lignes cle=valeur.
#   n=<fichiers>   temoin=<0|1>   observation=<secondes>
#
# PURE ASCII.
$ErrorActionPreference = 'Stop'
$journal = 'C:\dev\agent.log'
$sortie  = 'C:\dev\g4-p.log'
$prep    = 'C:\dev\g4-preparation'
$guet    = Join-Path $env:APPDATA 'Microsoft\Windows\Start Menu'
$cible   = Join-Path $guet 'Programs\g4-rafale'
$lnk     = Join-Path ([Environment]::GetFolderPath('Desktop')) 'G4 Temoin.lnk'

$p = @{ n = 20000; temoin = 0; observation = 40 }
if (Test-Path -LiteralPath 'C:\dev\g4-p.txt') {
  foreach ($l in Get-Content -LiteralPath 'C:\dev\g4-p.txt') {
    if ($l -match '^(\w+)=(.+)$') { $p[$matches[1]] = [int]$matches[2] }
  }
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
function Compte-Lnk {
  $racines = @(
    [Environment]::GetFolderPath('Desktop'), (Join-Path $env:PUBLIC 'Desktop'),
    (Join-Path $env:APPDATA 'Microsoft\Windows\Start Menu'),
    (Join-Path $env:ProgramData 'Microsoft\Windows\Start Menu'))
  $t = 0
  foreach ($d in $racines) { if (Test-Path -LiteralPath $d) { $t += @(Get-ChildItem -LiteralPath $d -Recurse -Filter *.lnk -Force -ErrorAction SilentlyContinue).Count } }
  return $t
}

try {
  Dire "=== G4 PRODUIT -- n=$($p.n) temoin=$($p.temoin) observation=$($p.observation)s"
  Dire "date    : $(Get-Date -Format o)"
  Dire "session : $((Get-Process -Id $PID).SessionId)"
  foreach ($m in (Lignes-Plates | Where-Object { $_ -match 'mode de surveillance retenu|faute de surveillance ARMEE|APPS_SURVEILLANCE : valeur inconnue' })) { Dire "mode    : $m" }
  Dire "corpus_avant : $(Compte-Lnk)"
  if (Test-Path -LiteralPath $lnk) { Remove-Item -LiteralPath $lnk -Force }

  # preparation HORS des racines surveillees
  if (Test-Path -LiteralPath $prep) { Remove-Item -LiteralPath $prep -Recurse -Force }
  New-Item -ItemType Directory -Path $prep -Force | Out-Null
  for ($i = 0; $i -lt $p.n; $i++) { [IO.File]::WriteAllText((Join-Path $prep ("g4-rafale-{0:D5}.tmp" -f $i)), 'x') }
  Dire "preparation_ok : $($p.n) fichiers"

  # ---- LA FENETRE S'OUVRE ICI, et son horodatage est ECRIT
  $t0 = (Get-Date).ToUniversalTime()
  Dire "FENETRE_DEBUT : $($t0.ToString('o'))"

  New-Item -ItemType Directory -Path $cible -Force | Out-Null
  $sw = [Diagnostics.Stopwatch]::StartNew()
  $null = & robocopy $prep $cible /MT:32 /NFL /NDL /NJH /NJS /R:0 /W:0 2>&1
  $sw.Stop()
  Dire "rafale_ms : $($sw.ElapsedMilliseconds)"

  if ($p.temoin -eq 1) {
    # le temoin est cree PENDANT/juste apres la rafale, avec son ARGUMENT
    # DISTINCT sans quoi sa cle serait celle d'un raccourci existant.
    $ws = New-Object -ComObject WScript.Shell
    $r = $ws.CreateShortcut($lnk); $r.TargetPath = 'C:\Windows\System32\notepad.exe'
    $r.Arguments = '--g4-temoin'; $r.Save()
    Dire "temoin_cree : $((Get-Date).ToUniversalTime().ToString('o'))"
  }

  Start-Sleep -Seconds $p.observation
  $t1 = (Get-Date).ToUniversalTime()
  Dire "FENETRE_FIN   : $($t1.ToString('o'))"

  # ---- comptage DANS LA FENETRE, bornee par les deux horodatages
  $lignes = Lignes-Plates
  $dansFenetre = @()
  foreach ($l in $lignes) {
    if ($l -match '^(\S+)Z\s') {
      $h = [datetime]::Parse($matches[1] + 'Z', [Globalization.CultureInfo]::InvariantCulture, [Globalization.DateTimeStyles]::AdjustToUniversal)
      if ($h -ge $t0 -and $h -le $t1) { $dansFenetre += $l }
    }
  }
  $rec = @($dansFenetre | Where-Object { $_ -match 'catalogue reconcilie' })
  Dire "RECONCILIATIONS_DANS_FENETRE : $($rec.Count)"
  foreach ($l in $rec) {
    $d = if ($l -match 'declencheur="?(\w+)"?') { $matches[1] } else { '?' }
    $c = if ($l -match 'cles=(\d+)') { $matches[1] } else { '?' }
    $nn = if ($l -match 'notifications=(\d+)') { $matches[1] } else { '?' }
    $db = if ($l -match 'debordements=(\d+)') { $matches[1] } else { '?' }
    Dire "  rec: declencheur=$d cles=$c notifications=$nn debordements=$db"
  }
  Dire "PERDUES_DANS_FENETRE : $(@($dansFenetre | Where-Object { $_ -match 'notifications perdues' }).Count)"
  Dire "PERDUES_TOTAL_FICHIER : $(@($lignes | Where-Object { $_ -match 'notifications perdues' }).Count)"
  Dire "RACINE_PERDUE : $(@($lignes | Where-Object { $_ -match 'racine de surveillance PERDUE' }).Count)"
  Dire "RACINE_RETABLIE : $(@($lignes | Where-Object { $_ -match 'racine de surveillance R' }).Count)"
  $dern = @($lignes | Where-Object { $_ -match 'catalogue reconcilie' })
  if ($dern.Count -gt 0) { Dire "DERNIERE_LIGNE : $($dern[-1])" }

  # ---- nettoyage et recomptage
  Remove-Item -LiteralPath $cible -Recurse -Force -ErrorAction SilentlyContinue
  Remove-Item -LiteralPath $prep  -Recurse -Force -ErrorAction SilentlyContinue
  if ($p.temoin -eq 1) { Remove-Item -LiteralPath $lnk -Force -ErrorAction SilentlyContinue }
  Start-Sleep -Seconds 3
  Dire "corpus_apres : $(Compte-Lnk)  (220 exige)"
  Dire "=== FIN"
}
catch { Dire "ERREUR : $($_.Exception.Message)"; Dire "TRACE : $($_.ScriptStackTrace)"; Dire "=== FIN" }
finally { $flux.Close() }
