# La mesure cote VM de la recette F2 : elle ECRIT dans la racine du pont, et
# rend un JSON que le pilote relit.
#
# 🔴 ELLE COURT PENDANT QUE LE LECTEUR EST MONTE. Une mesure prise apres la
# fermeture de la page lirait soit un cache, soit un echec, sans qu'on puisse
# les distinguer.
#
# ⚠️ L'INSTRUMENT D'ECRITURE EST CELUI QUE LA SONDE DE LA TACHE 14 A RETENU :
# `[IO.File]::WriteAllBytes` / `WriteAllText`, mesures EN PLACE aux deux
# sessions. Le Bloc-notes l'est aussi, et il est employe pour le critere ② —
# c'est l'outil qu'un humain emploierait.
$ErrorActionPreference = 'Continue'
$racine = Join-Path $env:USERPROFILE 'Mes Fichiers'
$r = [ordered]@{ racine = $racine; horodatage = (Get-Date -Format o) }

function Sha([string]$chemin) {
    if (-not (Test-Path $chemin)) { return '<ABSENT>' }
    (Get-FileHash -Path $chemin -Algorithm SHA256).Hash.ToLower()
}

# ── Ce que la racine montre AVANT toute ecriture ────────────────────────────
# 🔴 Le relever d'abord : si la racine est vide ici, tout ce qui suit mesure une
# course, pas le produit. Un `0` se qualifie AVANT de se rapporter.
$r.avant = @(Get-ChildItem -Path $racine -Recurse -ErrorAction SilentlyContinue |
    ForEach-Object { $_.FullName.Substring($racine.Length + 1) })
$r.avant_compte = $r.avant.Count

# ── CRITERE ① : un fichier NEUF d'au moins 1 Mio, donc PLUSIEURS morceaux ────
# TAILLE_TRAME_MAX vaut 64 Kio : 1,5 Mio fait 24 morceaux, dont le dernier est
# partiel. Un fichier d'un seul morceau ne pourrait pas montrer un decoupage
# faux.
$neuf = Join-Path $racine 'ecrit-par-la-vm.bin'
$octets = New-Object byte[] 1572869   # 1,5 Mio + 5 : le dernier morceau est PARTIEL
$rng = New-Object System.Random 20260821
$rng.NextBytes($octets)
[IO.File]::WriteAllBytes($neuf, $octets)
$r.critere1_chemin = 'ecrit-par-la-vm.bin'
$r.critere1_taille = $octets.Length
$r.critere1_sha_vm = Sha $neuf

# ── CRITERE ② : un fichier DEJA PROJETE, modifie EN PLACE ───────────────────
# 🔴 C'EST LE CHEMIN `PRE_CONVERT_TO_FULL`, ET IL N'A JAMAIS ETE EXERCE — ni par
# F1 ni par F2 jusqu'ici (legs 8 de F1). Le fichier est LISTE mais JAMAIS LU
# avant d'etre ecrit : c'est ce qui le laisse a l'etat de SUBSTITUT, donc ce qui
# oblige ProjFS a demander l'hydratation complete.
$projete = Join-Path $racine 'projete.txt'
$r.critere2_present_avant = (Test-Path $projete)
$r.critere2_taille_avant = if (Test-Path $projete) { (Get-Item $projete).Length } else { -1 }
$texte = 'REECRIT PAR LA VM le ' + (Get-Date -Format o)
try {
    [IO.File]::WriteAllText($projete, $texte)
    $r.critere2_ecriture = 'OK'
} catch {
    $r.critere2_ecriture = 'REFUSEE ' + $_.Exception.Message
}
$r.critere2_sha_vm = Sha $projete
$r.critere2_taille_apres = if (Test-Path $projete) { (Get-Item $projete).Length } else { -1 }

# ── Un fichier VIDE : le cas nominal d'un « nouveau document » ───────────────
$vide = Join-Path $racine 'vide.txt'
[IO.File]::WriteAllBytes($vide, (New-Object byte[] 0))
$r.vide_taille = (Get-Item $vide).Length

# ── Un REPERTOIRE neuf, puis un fichier dedans ──────────────────────────────
$dossier = Join-Path $racine 'dossier-neuf'
New-Item -ItemType Directory -Path $dossier -Force | Out-Null
[IO.File]::WriteAllText((Join-Path $dossier 'dedans.txt'), 'un fichier dans un dossier neuf')
$r.sous_fichier_sha_vm = Sha (Join-Path $dossier 'dedans.txt')

# ── LA GARDE DE CASSE : un homonyme qui ne differe que par la casse ─────────
# 🔴 Cote VM l'ecriture REUSSIT (NTFS est insensible a la casse) ; c'est le
# NAVIGATEUR qui doit refuser, et le compteur qui doit le NOMMER.
$casse = Join-Path $racine 'CASSE.TXT'
try {
    [IO.File]::WriteAllText($casse, 'ecrit sous une AUTRE casse')
    $r.casse_ecriture = 'OK cote VM'
} catch {
    $r.casse_ecriture = 'REFUSEE ' + $_.Exception.Message
}

# ── Ce que la racine montre APRES ───────────────────────────────────────────
Start-Sleep -Seconds 3
$r.apres = @(Get-ChildItem -Path $racine -Recurse -ErrorAction SilentlyContinue |
    ForEach-Object { $_.FullName.Substring($racine.Length + 1) })
$r.apres_compte = $r.apres.Count

$r | ConvertTo-Json -Depth 6 -Compress
