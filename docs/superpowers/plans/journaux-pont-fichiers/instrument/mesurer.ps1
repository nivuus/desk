# Les criteres 1, 2, 3 et le Step 7, releves SUR LA VM pendant que le lecteur
# est monte.
#
# 🔴 TOUT PASSE PAR `Note`, ET RIEN PAR LE FLUX DE SUCCES.
# Deux versions precedentes de ce script ont menti, et de la meme facon :
#   - la premiere ecrivait sur la sortie WinRM, qui MUTILE les accents (le nom
#     `éphémère été.txt` en revenait coupe en trois lignes) et qui s'est
#     interrompue en silence apres le listage ;
#   - la seconde surchargeait `Write-Output` par une fonction ecrivant dans un
#     fichier -- mais un `Get-ChildItem | ForEach-Object { "D|$rel" }` emet ses
#     chaines DANS LE PIPELINE, pas par `Write-Output`. Le listage partait donc
#     toujours sur le flux WinRM, et le fichier montrait un bloc `NOMS` VIDE.
#     ⚠️ On en a conclu une enumeration ProjFS vide -- c'est-a-dire une panne
#     du PRODUIT -- alors que c'etait l'instrument qui ne regardait pas au bon
#     endroit.
# Une seule voie de sortie, nommee, et aucun emetteur implicite.
$ErrorActionPreference = 'Continue'
$racine = Join-Path $env:USERPROFILE 'Mes Fichiers'
$sortie = 'C:\dev\mesure.txt'

$flux = New-Object System.IO.StreamWriter($sortie, $false, (New-Object System.Text.UTF8Encoding($false)))
function Note([string]$m) { $flux.WriteLine($m); $flux.Flush() }

Note "racine=$racine"
Note ("racine_presente=" + (Test-Path $racine))
if (-not (Test-Path $racine)) { Note 'ABANDON : racine absente'; $flux.Close(); exit 1 }

# ── Critere 1 : l'arborescence, AUX DEUX NIVEAUX ────────────────────────────
# Compare par ENSEMBLE DE NOMS cote hote, jamais par cardinal (piege maison :
# un compteur ne suffit pas quand un tiers agit sur le systeme).
Note '=== NOMS DEBUT ==='
try {
    $entrees = @(Get-ChildItem -LiteralPath $racine -Recurse -Force -ErrorAction Stop)
    foreach ($e in $entrees) {
        $rel = $e.FullName.Substring($racine.Length).TrimStart('\')
        if ($e.PSIsContainer) { Note "D|$rel" } else { Note ("F|$rel|" + $e.Length) }
    }
    Note ("noms_total=" + $entrees.Count)
} catch {
    Note ('NOMS_ERREUR|' + ($_.Exception.Message -replace "`r|`n", ' '))
}
Note '=== NOMS FIN ==='

# ── SONDE DE LECTURE : petit fichier d'abord, puis une TRANCHE du gros ──────
# 🔴 L'ORDRE EST LE POINT. Une premiere version copiait les 12 Mio EN PREMIER ;
# la copie s'est BLOQUEE, et tout le reste du script -- casse, creation locale,
# lectures -- n'a jamais tourne. On ne savait alors pas si la lecture etait
# cassee ou seulement lente, ni si les petits fichiers marchaient.
# Ici : le plus petit d'abord, puis une tranche bornee, puis la copie entiere.
Note '=== SONDE LECTURE DEBUT ==='
$t = Get-Date
try {
    $c = [System.IO.File]::ReadAllText((Join-Path $racine 'Casse.txt'))
    Note ("sonde|petit|OK|" + [int]((Get-Date) - $t).TotalMilliseconds + " ms|" + ($c -replace "`r|`n", ' ').Trim())
} catch {
    Note ("sonde|petit|ECHEC|" + [int]((Get-Date) - $t).TotalMilliseconds + " ms|" + ($_.Exception.Message -replace "`r|`n", ' '))
}
Note '=== SONDE PETITE FIN ==='

# ── Step 7 : le LEGS DE CASSE ───────────────────────────────────────────────
# Windows est insensible a la casse, la File System Access API ne l'est pas.
# On OBSERVE, on ne corrige pas : une correspondance insensible exigerait
# d'enumerer le repertoire a chaque resolution, et c'est une decision de
# conception, pas une correction de recette.
Note '=== CASSE DEBUT ==='
foreach ($n in 'Casse.txt', 'casse.txt', 'CASSE.TXT', 'GROS.BIN') {
    $p = Join-Path $racine $n
    try {
        $c = [System.IO.File]::ReadAllText($p)
        Note ("casse|$n|OK|" + ($c -replace "`r|`n", ' ').Trim())
    } catch {
        Note ("casse|$n|ECHEC|" + ($_.Exception.Message -replace "`r|`n", ' '))
    }
}
Note '=== CASSE FIN ==='

# ── Step 7 : la CREATION LOCALE, attendue REFUSEE (ERROR_WRITE_PROTECT) ─────
Note '=== CREATION DEBUT ==='
$neuf = Join-Path $racine 'creation-interdite.txt'
Remove-Item -LiteralPath $neuf -Force -ErrorAction SilentlyContinue
try {
    [System.IO.File]::WriteAllText($neuf, 'ceci ne devrait pas exister')
    Note 'creation|CREEE (DIVERGENCE : attendu ERROR_WRITE_PROTECT)'
    Note ("creation_presente=" + (Test-Path $neuf))
} catch {
    Note ('creation|REFUSEE|' + ($_.Exception.Message -replace "`r|`n", ' '))
    try { Note ('creation_hresult=' + ('0x{0:X8}' -f $_.Exception.InnerException.HResult)) } catch {}
}
Note '=== CREATION FIN ==='

# ── Lecture du fichier imbrique et du nom accentue ──────────────────────────
Note '=== LECTURES DEBUT ==='
foreach ($n in 'sous-dossier\imbrique.txt', "$([char]0xE9)ph$([char]0xE9)m$([char]0xE8)re $([char]0xE9)t$([char]0xE9).txt") {
    $p = Join-Path $racine $n
    try {
        $c = [System.IO.File]::ReadAllText($p)
        Note ("lecture|$n|OK|" + ($c -replace "`r|`n", ' ').Trim())
    } catch {
        Note ("lecture|$n|ECHEC|" + ($_.Exception.Message -replace "`r|`n", ' '))
    }
}
Note '=== LECTURES FIN ==='

# ── LES GROSSES LECTURES, EN DERNIER ET C'EST DELIBERE ──────────────────────
# 🔴 Une lecture MULTI-TRAMES peut CALER indefiniment (constat de cette
# recette : hydratation figee a 42 octets pendant plus de neuf minutes, sans
# `commande expiree`, sur une racine par ailleurs saine ou l'enumeration et les
# petites lectures ont reussi). Placees en premier, elles empechaient le Step 7
# -- casse, creation locale, lectures imbriquees -- de tourner du tout : la
# recette ne rendait alors AUCUN de ces relevés, faute d'etre arrivee jusqu'a
# eux. Ici, tout ce qui est borne passe d'abord.
Note '=== GROSSES LECTURES DEBUT ==='
# Une TRANCHE de 1 Mio du gros fichier, lue par FileStream : borne le temps et
# dit si le probleme tient a la TAILLE ou a la lecture tout court.
$t = Get-Date
try {
    $fs = [System.IO.File]::Open((Join-Path $racine 'gros.bin'), 'Open', 'Read', 'ReadWrite')
    $tampon = New-Object byte[] 1048576
    $lu = $fs.Read($tampon, 0, $tampon.Length)
    $fs.Close()
    Note ("sonde|tranche_1Mio|OK|" + [int]((Get-Date) - $t).TotalMilliseconds + " ms|octets_lus=$lu")
} catch {
    Note ("sonde|tranche_1Mio|ECHEC|" + [int]((Get-Date) - $t).TotalMilliseconds + " ms|" + ($_.Exception.Message -replace "`r|`n", ' '))
}
Note '=== SONDE LECTURE FIN ==='

# ── Critere 2 : le condensat du fichier de plus de 10 Mio ───────────────────
# 🔴 LE SEUL CRITERE QUI NE PUISSE PAS ETRE SATISFAIT PAR ACCIDENT (spec §8).
# « Le fichier s'ouvre » serait verifie par un fichier TRONQUE, par un fichier
# dont les PLAGES sont dans le desordre, et par un fichier dont la DERNIERE
# trame manque -- les trois sont des defauts que `pont/decoupe.rs` peut
# reellement produire.
$src = Join-Path $racine 'gros.bin'
$dst = 'C:\gros-copie.bin'
Remove-Item -LiteralPath $dst -Force -ErrorAction SilentlyContinue
$t0 = Get-Date
try {
    Copy-Item -LiteralPath $src -Destination $dst -Force -ErrorAction Stop
    Note "copie_ok=True"
    Note ("copie_ms=" + [int]((Get-Date) - $t0).TotalMilliseconds)
    Note ("copie_taille=" + (Get-Item -LiteralPath $dst).Length)
    Note ("copie_sha256=" + (Get-FileHash -LiteralPath $dst -Algorithm SHA256).Hash.ToLower())
} catch {
    Note "copie_ok=False"
    Note ('copie_erreur=' + ($_.Exception.Message -replace "`r|`n", ' '))
}

Note '=== GROSSES LECTURES FIN ==='
Note ("disque_libre_octets=" + (Get-PSDrive C).Free)
Note 'FIN DE MESURE'
$flux.Close()
