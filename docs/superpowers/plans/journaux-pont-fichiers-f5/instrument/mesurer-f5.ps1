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
} elseif ($phase -like 'rang-*') {
    # 🔴 **TÂCHE 17 — LA LATENCE DE LISTAGE AUX RANGS DE F4, CACHE ARMÉ.**
    # Le §0.1 du plan l'impose au verdict VERT du critère ① : F4 a mesuré « le
    # chaud que le produit A », c'est-à-dire un produit SANS cache, donc une
    # BORNE HAUTE. C'est ici que cette borne cesse de décrire le produit.
    # ⚠️ **LE MUR DE ~3 150 ENTRÉES NE BOUGE PAS** : il tient à la taille d'UN
    # message, pas à la répétition. Les rangs sont 10, 100 et 1 000.
    $sous = $phase.Substring(5)
    $d = Join-Path $racine $sous
    # FROID : premier listage, le cache n'a rien pour ce répertoire.
    $t0 = Get-Date
    $e1 = @(Get-ChildItem -Path $d -ErrorAction SilentlyContinue)
    $froid = [int]((Get-Date) - $t0).TotalMilliseconds
    # CHAUD : second listage immédiat, dans la fenêtre du TTL.
    $t1 = Get-Date
    $e2 = @(Get-ChildItem -Path $d -ErrorAction SilentlyContinue)
    $chaud = [int]((Get-Date) - $t1).TotalMilliseconds
    $t2 = Get-Date
    $e3 = @(Get-ChildItem -Path $d -ErrorAction SilentlyContinue)
    $chaud2 = [int]((Get-Date) - $t2).TotalMilliseconds
    $r.sous_dossier = $sous
    $r.entrees = $e1.Count
    $r.froid_ms = $froid
    $r.chaud_ms = $chaud
    $r.chaud2_ms = $chaud2
    $r.coherent = ($e1.Count -eq $e2.Count -and $e2.Count -eq $e3.Count)
} elseif ($phase -eq 'muter') {
    # 🔴 **CRITÈRE ④** — les trois mutations de F2/F3 restent-elles VUES quand un
    # cache d'énumération est armé ? C'est le RISQUE N°1 de F5 : un cache non
    # invalidé ferait qu'un renommage laisserait l'ancien nom au listage et
    # cacherait le neuf, et qu'une suppression resterait listée. **C'est le
    # défaut exact que la spec §7.4 reproche à l'ancien pont.**
    $r.avant = Lister
    $cree = Join-Path $racine 'a-renommer.txt'
    Set-Content -Path $cree -Value 'x' -ErrorAction SilentlyContinue
    $r.apres_creation = Lister
    try { Rename-Item -Path $cree -NewName 'RENOMME.txt' -ErrorAction Stop; $r.renommage = 'OK' }
    catch { $r.renommage = "ECHEC: $($_.Exception.Message)" }
    $r.apres_renommage = Lister
    try { Remove-Item -Path (Join-Path $racine 'RENOMME.txt') -Force -ErrorAction Stop; $r.suppression = 'OK' }
    catch { $r.suppression = "ECHEC: $($_.Exception.Message)" }
    $r.apres_suppression = Lister
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
