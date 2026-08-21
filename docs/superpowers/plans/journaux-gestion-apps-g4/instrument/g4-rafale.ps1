# G4 -- PORTE S1 : une rafale peut-elle faire deborder un tampon de 64 Kio ?
#
# Cette sonde se joue AVANT toute ligne de produit, et SANS une ligne de
# produit : elle emploie le FileSystemWatcher de .NET, qui est
# ReadDirectoryChangesW sous le capot et dont InternalBufferSize est passe tel
# quel a l'appel. Un debordement y leve InternalBufferOverflowException sur
# l'evenement Error.
#
# RESERVE QUI SURVIT A TOUS LES VERDICTS : le FileSystemWatcher a SA PROPRE
# file geree et SON PROPRE fil de distribution entre ReadDirectoryChangesW et
# l'appelant. Notre surveillance rearme dans une boucle serree, sans
# marshalling. Elle peut deborder plus facilement, ou moins. S1 BORNE LA
# QUESTION, ELLE NE LA REPOND PAS POUR NOTRE CODE.
#
# PURE ASCII : un .ps1 sans BOM contenant un seul caractere non-ASCII ne
# s'analyse pas, et l'erreur designe une AUTRE ligne que la vraie cause
# (premier piege de G3).
$ErrorActionPreference = 'Stop'

# N ARRIVE PAR UN FICHIER, ET NON PAR L'ENVIRONNEMENT.
#
# LE SECOND JET DE CETTE SONDE LE LISAIT DANS $env:G4_RAFALE_N, ET LA VALEUR
# N'ATTEIGNAIT JAMAIS LE PROCESSUS : une tache planifiee demarre dans un
# environnement NEUF, et `schtasks /run` ne transporte rien de l'appelant.
# Trois executions demandees a 1 000, 5 000 et 20 000 ont TOUTES rendu N=1000,
# et seul l'en-tete du journal l'a dit. C'est EXACTEMENT le piege que ce depot
# a paye cinq fois sur `scripts/run-agent.sh` (SUPERVISEUR en D1,
# MULTIFENETRE_REPRISE en D2, AUDIO en D7), rejoue par mon propre instrument.
# Le journal de ce second jet est conserve (s1-v2-N-non-transmis.log).
$fichierN = 'C:\dev\g4-rafale-n.txt'
$N = if (Test-Path -LiteralPath $fichierN) { [int](Get-Content -LiteralPath $fichierN -Raw).Trim() } else { 1000 }

$prep  = 'C:\dev\g4-preparation'
$guet  = Join-Path $env:APPDATA 'Microsoft\Windows\Start Menu'
$cible = Join-Path $guet 'Programs\g4-rafale'

$flux = New-Object System.IO.StreamWriter('C:\dev\g4-s1.log', $false, (New-Object System.Text.UTF8Encoding($false)))
function Dire($m) { $flux.WriteLine([string]$m); $flux.Flush() }

function Compte-Lnk {
  $racines = @(
    [Environment]::GetFolderPath('Desktop'),
    (Join-Path $env:PUBLIC 'Desktop'),
    (Join-Path $env:APPDATA 'Microsoft\Windows\Start Menu'),
    (Join-Path $env:ProgramData 'Microsoft\Windows\Start Menu')
  )
  $t = 0
  foreach ($d in $racines) {
    if (Test-Path -LiteralPath $d) {
      $t += @(Get-ChildItem -LiteralPath $d -Recurse -Filter *.lnk -Force -ErrorAction SilentlyContinue).Count
    }
  }
  return $t
}

try {
  Dire "=== G4 PORTE S1 -- N=$N"
  Dire "date          : $(Get-Date -Format o)"
  Dire "session       : $((Get-Process -Id $PID).SessionId)"
  Dire "guet          : $guet"
  Dire "cible         : $cible"

  # ---- (a) le corpus AVANT. Sans ce chiffre, aucun controle de sortie ne peut
  # echouer.
  $avant = Compte-Lnk
  Dire "a_corpus_avant : $avant"

  # ---- preparation HORS des racines surveillees. Fabriquer sur place
  # mesurerait la vitesse de PowerShell, pas celle du systeme de fichiers.
  if (Test-Path -LiteralPath $prep) { Remove-Item -LiteralPath $prep -Recurse -Force }
  New-Item -ItemType Directory -Path $prep -Force | Out-Null
  $tPrep = [Diagnostics.Stopwatch]::StartNew()
  for ($i = 0; $i -lt $N; $i++) {
    [IO.File]::WriteAllText((Join-Path $prep ("g4-rafale-{0:D5}.tmp" -f $i)), 'x')
  }
  $tPrep.Stop()
  Dire "preparation_ms : $($tPrep.ElapsedMilliseconds)  fichiers=$N"

  # ---- le guetteur
  #
  # LE PREMIER JET DE CETTE SONDE COMPTAIT AVEC UN Register-ObjectEvent -Action,
  # ET IL NE MESURAIT PAS CE QU'IL ANNONCAIT : sur 1 000 fichiers poses a
  # 3 289/s, il n'a rendu que DIX evenements, a 3,6 par seconde -- c'est-a-dire
  # qu'il mesurait la CADENCE DE LA POMPE D'EVENEMENTS DE POWERSHELL, pas le
  # tampon. Le journal de ce premier jet est conserve
  # (s1-v1-instrument-defectueux.log) : une sonde qui rend un chiffre qu'on
  # n'attendait pas est une piece, pas un brouillon.
  #
  # Le compteur vit desormais dans une classe C# qui s'abonne DIRECTEMENT au
  # FileSystemWatcher : plus aucun runspace PowerShell sur le chemin chaud,
  # donc un consommateur aussi rapide que le notre.
  Add-Type -TypeDefinition @"
using System;
using System.IO;
using System.Threading;
public class G4Compteur {
    public long Crees, Erreurs, Debordements;
    public long PremierTicks, DernierTicks;
    private FileSystemWatcher w;
    public void Armer(string chemin, int tampon) {
        w = new FileSystemWatcher(chemin);
        w.IncludeSubdirectories = true;
        w.InternalBufferSize = tampon;
        w.NotifyFilter = NotifyFilters.FileName | NotifyFilters.DirectoryName | NotifyFilters.LastWrite;
        w.Created += delegate(object s, FileSystemEventArgs e) {
            long n = Interlocked.Increment(ref Crees);
            long t = DateTime.UtcNow.Ticks;
            if (n == 1) PremierTicks = t;
            DernierTicks = t;
        };
        w.Changed += delegate(object s, FileSystemEventArgs e) { };
        w.Error += delegate(object s, ErrorEventArgs e) {
            Interlocked.Increment(ref Erreurs);
            if (e.GetException() is InternalBufferOverflowException) {
                Interlocked.Increment(ref Debordements);
            }
        };
        w.EnableRaisingEvents = true;
    }
    public void Desarmer() { if (w != null) { w.EnableRaisingEvents = false; w.Dispose(); } }
}
"@
  $c = New-Object G4Compteur
  $c.Armer($guet, 65536)
  Dire "guetteur_arme  : tampon=65536 sousarbre=True consommateur=C#"

  # ---- (b) la rafale, en PARALLELE. Une boucle qui cree cinq mille fichiers
  # un par un ne debordera JAMAIS : a ~166 creations par seconde, le lecteur
  # draine chaque evenement en microsecondes.
  New-Item -ItemType Directory -Path $cible -Force | Out-Null
  $tRaf = [Diagnostics.Stopwatch]::StartNew()
  $null = & robocopy $prep $cible /MT:32 /NFL /NDL /NJH /NJS /R:0 /W:0 2>&1
  $codeRobocopy = $LASTEXITCODE
  $tRaf.Stop()
  Dire "d_rafale_ms    : $($tRaf.ElapsedMilliseconds)  robocopy_code=$codeRobocopy"
  $poses = @(Get-ChildItem -LiteralPath $cible -Filter *.tmp -Force -ErrorAction SilentlyContinue).Count
  Dire "d_fichiers_poses : $poses"
  if ($tRaf.ElapsedMilliseconds -gt 0) {
    Dire "d_debit_fichiers_par_s : $([math]::Round($poses * 1000.0 / $tRaf.ElapsedMilliseconds, 1))"
  }

  # ATTENDRE LE FAIT, JAMAIS UNE DUREE : on draine jusqu'a ce que le compteur
  # cesse de bouger. Un `Start-Sleep` fixe rendrait un compte tronque, et un
  # compte tronque ressemble EXACTEMENT a un debordement -- deux causes, une
  # seule observation.
  $stable = 0; $precedent = -1; $tours = 0
  while ($stable -lt 10 -and $tours -lt 600) {
    Start-Sleep -Milliseconds 500
    $tours++
    if ($c.Crees -eq $precedent) { $stable++ } else { $stable = 0; $precedent = $c.Crees }
  }
  Dire "drainage_ms    : $($tours * 500)  (stable pendant $($stable * 500) ms)"
  $c.Desarmer()

  # ---- (c) LE VERDICT
  Dire "c_crees        : $($c.Crees)"
  Dire "c_erreurs      : $($c.Erreurs)"
  Dire "c_debordements : $($c.Debordements)"
  if ($c.PremierTicks -gt 0 -and $c.DernierTicks -gt 0) {
    $ms = ($c.DernierTicks - $c.PremierTicks) / 10000.0
    Dire "c_fenetre_evenements_ms : $([math]::Round($ms,1))"
    if ($ms -gt 0) { Dire "c_debit_evenements_par_s : $([math]::Round($c.Crees * 1000.0 / $ms, 1))" }
  }
  # UN COMPTE TRONQUE N'EST PAS UN DEBORDEMENT, et le dire ici evite de les
  # confondre a la lecture.
  Dire "c_manquants    : $($poses - $c.Crees)"
  if ($c.Debordements -gt 0) { Dire "VERDICT : DEBORDE" } else { Dire "VERDICT : NE DEBORDE PAS" }

  # ---- (e) NETTOYAGE, puis RECOMPTAGE. C'est la seule preuve que l'instrument
  # ne detruit pas ce qu'il mesure.
  Remove-Item -LiteralPath $cible -Recurse -Force -ErrorAction SilentlyContinue
  Remove-Item -LiteralPath $prep  -Recurse -Force -ErrorAction SilentlyContinue
  Start-Sleep -Seconds 2
  $apres = Compte-Lnk
  Dire "e_corpus_apres : $apres"
  if ($apres -eq $avant) {
    Dire "e_nettoyage : OK ($avant inchange)"
  } else {
    Dire "e_nettoyage : ECHEC ($avant -> $apres)"
  }
  # ---- (f) l'etat si le nettoyage a echoue, ecrit D'AVANCE parce qu'un
  # nettoyage a mi-course est le cas qu'on ne veut pas decouvrir.
  if (Test-Path -LiteralPath $cible) {
    $restants = @(Get-ChildItem -LiteralPath $cible -Force -ErrorAction SilentlyContinue).Count
    Dire "f_repertoire_rafale_SURVIT : $cible  fichiers=$restants"
  } else {
    Dire "f_repertoire_rafale : retire"
  }
  if (Test-Path -LiteralPath $prep) { Dire "f_preparation_SURVIT : $prep" } else { Dire "f_preparation : retiree" }
  Dire "=== FIN"
}
catch {
  Dire "ERREUR : $($_.Exception.Message)"
  Dire "TRACE  : $($_.ScriptStackTrace)"
}
finally {
  $flux.Close()
}
