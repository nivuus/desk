param([string]$sortie = 'C:\dev\mesure-f5.json', [string]$phase = 'lister')
# La mesure côté VM de la recette F5. Elle LISTE, et selon la phase elle CRÉE
# un fichier DANS la VM.
#
# 🔴 LE RELEVÉ S'ÉCRIT À UN CHEMIN NEUF À CHAQUE EXÉCUTION — piège de F3, payé
# là-bas : une tentative figée laisse son `powershell` vivant, qui TIENT le
# fichier de relevé, et la mesure suivante écrit dans le vide.
#
# 🔴 ELLE COURT DANS LA SESSION INTERACTIVE, jamais depuis WinRM : la racine
# ProjFS est montée par un processus de la session 1.
$ErrorActionPreference = 'Continue'
$racine = Join-Path $env:USERPROFILE 'Mes Fichiers'
$r = [ordered]@{ racine = $racine; phase = $phase; horodatage = (Get-Date -Format o) }

function Lister() {
    # ⚠️ **PAS de `-Recurse`** : le critère ① porte sur UN répertoire, et un
    # parcours de fond empoisonnerait le cache avec des listages que personne
    # n'a demandés — c'est ce que l'en-tête de `pont::cache` interdit.
    $t0 = Get-Date
    $e = @(Get-ChildItem -Path $racine -ErrorAction SilentlyContinue | ForEach-Object { $_.Name })
    $ms = [int]((Get-Date) - $t0).TotalMilliseconds
    return @{ noms = $e; compte = $e.Count; ms = $ms }
}

if ($phase -eq 'creer-dans-la-vm') {
    # 🔴 **PORTE P3** — un fichier créé DANS la VM, sous la racine.
    # La question : le filtre ProjFS le fusionne-t-il lui-même avec ce que le
    # fournisseur énumère, ou nous rappelle-t-il ?
    $avant = Lister
    $cible = Join-Path $racine 'ne-vient-pas-du-navigateur.txt'
    try {
        Set-Content -Path $cible -Value 'cree cote VM' -ErrorAction Stop
        $r.creation = 'OK'
    } catch {
        $r.creation = "ECHEC: $($_.Exception.Message)"
    }
    # RELISTER IMMÉDIATEMENT : c'est l'instant qui répond à P3.
    $apres = Lister
    $r.avant = $avant
    $r.apres = $apres
    $r.present_apres = ($apres.noms -contains 'ne-vient-pas-du-navigateur.txt')
} elseif ($phase -eq 'occupation') {
    # 🔴 **PORTE P1** — les trois candidates d'occupation disque.
    # (b) GetDiskFreeSpaceExW, par le chemin PowerShell équivalent.
    $vol = (Get-Item $racine).PSDrive
    $r.disque_libre_octets = $vol.Free
    $r.disque_utilise_octets = $vol.Used
    # (c) `metadata` sur des chemins DÉJÀ hydratés, et sur des chemins JAMAIS
    #     touchés. Le second est LE TÉMOIN : sans lui, un zéro de traversées
    #     serait indiscernable d'un compteur qui ne compte pas.
    $t0 = Get-Date
    $tailles = 0
    foreach ($f in (Get-ChildItem -Path $racine -File -ErrorAction SilentlyContinue)) {
        $tailles += $f.Length
    }
    $r.somme_longueurs_octets = $tailles
    $r.duree_metadata_ms = [int]((Get-Date) - $t0).TotalMilliseconds
} else {
    $r.liste = Lister
}

$r | ConvertTo-Json -Depth 6 | Set-Content -Path $sortie -Encoding utf8
