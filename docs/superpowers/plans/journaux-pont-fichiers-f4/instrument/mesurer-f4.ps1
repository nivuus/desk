param(
    [Parameter(Mandatory=$true)][string]$sortie,
    # Un PLAN de gestes, separes par des virgules : `verbe:argument`.
    [Parameter(Mandatory=$true)][string]$plan,
    # Le repos apres CHAQUE geste, en secondes.
    #
    # 🔴 C'EST CE QUI REND LE DIFFERENTIEL DU RECENSEMENT LISIBLE. Les compteurs
    # de `pont::latence` sont CUMULATIFS et emis a `PERIODE_RECENSEMENT` (10 s) :
    # une mesure se lit par DIFFERENCE entre deux lignes, jamais sur une seule.
    # Vingt-cinq secondes garantissent DEUX recensements apres chaque geste,
    # comme le §1.5 n°6 du plan l'exige.
    [int]$repos = 25
)
$ErrorActionPreference = 'Continue'

# 🔴 LE CHRONOMETRE EST UN `Stopwatch`, JAMAIS `Measure-Command`.
# Celui-ci enveloppe un bloc de script et son PROPRE cout entre dans le chiffre.
$racine = Join-Path $env:USERPROFILE 'Mes Fichiers'
$releves = New-Object System.Collections.ArrayList
$horoUtc = { (Get-Date).ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ss.ffffffZ') }

# 🔴 UN POINT DE REPRISE APRES CHAQUE GESTE, ET NON A LA FIN.
# « Un instrument qui n'ecrit qu'a la fin fait dependre toute la mesure du
# geste le plus fragile » — F3 a perdu deux criteres deja mesures ainsi.
function Enregistrer {
    param($releve, [bool]$fini = $false)
    if ($null -ne $releve) { [void]$releves.Add($releve) }
    $doc = [ordered]@{ racine = $racine; plan = $plan; repos_s = $repos; fini = $fini; releves = $releves }
    Set-Content -Path $sortie -Value ($doc | ConvertTo-Json -Depth 8 -Compress) -Encoding UTF8
}

foreach ($geste in ($plan -split ',')) {
    $geste = $geste.Trim()
    if ($geste -eq '') { continue }
    $bout = $geste -split ':', 2
    $verbe = $bout[0]
    $arg = if ($bout.Count -gt 1) { $bout[1] } else { '' }

    $r = [ordered]@{ geste = $geste; verbe = $verbe; arg = $arg; debut_iso = (& $horoUtc) }
    $sw = [System.Diagnostics.Stopwatch]::StartNew()
    try {
        switch ($verbe) {
            # ── L'ENUMERATION NUE ────────────────────────────────────────
            # ⚠️ LE COMPTE EST RELEVE A CHAQUE RANG, ET C'EST LE CONTROLE QUI
            # ATTRAPE LA TROISIEME ISSUE : « tronque en SILENCE » se voit au
            # COMPTE, jamais a l'absence d'erreur.
            'listage' {
                $chemin = Join-Path $racine ("listage\" + $arg)
                # ⚠️ `Get-ChildItem | ForEach-Object` EMET DANS LE PIPELINE : F1
                # en a conclu une enumeration vide, c'est-a-dire une panne du
                # produit, alors que l'instrument ne regardait pas au bon
                # endroit. On collecte donc dans un tableau, explicitement.
                $entrees = @(Get-ChildItem -LiteralPath $chemin -Force -ErrorAction Stop)
                $sw.Stop()
                $r.ms = $sw.Elapsed.TotalMilliseconds
                $r.entrees = $entrees.Count
                $r.demande = [int]$arg
                $r.complet = ($entrees.Count -eq [int]$arg)
                $r.ok = $true
            }
            # ── LE MOTIF DE L'EXPLORATEUR : lister PUIS interroger chacun ──
            # 🔵 C'est ce COUPLE qui repond a la question du cadrage : un
            # listage rapide suivi de N interrogations lentes degrade
            # l'experience autant qu'un listage lent.
            'attributs' {
                $chemin = Join-Path $racine ("listage\" + $arg)
                $entrees = @(Get-ChildItem -LiteralPath $chemin -Force -ErrorAction Stop)
                $swAttr = [System.Diagnostics.Stopwatch]::StartNew()
                $n = 0
                foreach ($e in $entrees) { $null = $e.Attributes; $n += 1 }
                $swAttr.Stop(); $sw.Stop()
                $r.ms = $sw.Elapsed.TotalMilliseconds
                $r.ms_attributs_seuls = $swAttr.Elapsed.TotalMilliseconds
                $r.entrees = $n
                $r.demande = [int]$arg
                $r.ok = $true
            }
            # ── UN SEUL `GetPlaceholderInfo` ──────────────────────────────
            'item' {
                $chemin = Join-Path $racine $arg
                $it = Get-Item -LiteralPath $chemin -Force -ErrorAction Stop
                $sw.Stop()
                $r.ms = $sw.Elapsed.TotalMilliseconds
                $r.taille = $it.Length
                $r.ok = $true
            }
            # ── LA LECTURE COMPLETE ──────────────────────────────────────
            # ⚠️ `FileStream` et non `Get-Content` : celui-ci decode en texte,
            # ce qui ajouterait un cout de conversion au chiffre mesure.
            'lecture' {
                $chemin = Join-Path $racine $arg
                $fs = [System.IO.File]::OpenRead($chemin)
                try {
                    $tampon = New-Object byte[] 1048576
                    $total = 0
                    while (($lu = $fs.Read($tampon, 0, $tampon.Length)) -gt 0) { $total += $lu }
                } finally { $fs.Dispose() }
                $sw.Stop()
                $r.ms = $sw.Elapsed.TotalMilliseconds
                $r.octets = $total
                $r.debit_kio_s = if ($sw.Elapsed.TotalSeconds -gt 0) { [math]::Round($total / 1024 / $sw.Elapsed.TotalSeconds, 1) } else { 0 }
                $r.ok = $true
            }
            # ── LE RENOMMAGE ─────────────────────────────────────────────
            'renommer' {
                $bouts = $arg -split '>'
                $src = Join-Path $racine $bouts[0]
                Rename-Item -LiteralPath $src -NewName $bouts[1] -ErrorAction Stop
                $sw.Stop()
                $r.ms = $sw.Elapsed.TotalMilliseconds
                $r.source = $bouts[0]; $r.cible = $bouts[1]
                $r.cible_presente = Test-Path (Join-Path (Split-Path $src) $bouts[1])
                $r.source_presente = Test-Path $src
                $r.ok = $true
            }
            # ── L'EXPLORATEUR ────────────────────────────────────────────
            # 🔴 EN M1 IL N'Y A AUCUN SUPERVISEUR, donc cette fenetre n'est
            # capturee par personne. C'est la raison pour laquelle cette mesure
            # est en M1 : sous SUPERVISEUR=1 elle deviendrait une fenetre
            # eligible, donc une session, donc une sortie virtuelle — le piege
            # que P1 presse-papier a paye avec une console PowerShell.
            'explorer' {
                $chemin = Join-Path $racine ("listage\" + $arg)
                Start-Process explorer.exe -ArgumentList "`"$chemin`""
                Start-Sleep -Seconds 30
                $sw.Stop()
                Get-Process explorer -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
                $r.ms = $sw.Elapsed.TotalMilliseconds
                $r.note = 'duree IMPOSEE (30 s de maintien), ce n est PAS une latence'
                $r.ok = $true
            }
            # ── OUVRIR UNE VRAIE FENETRE D'APPLICATION ───────────────────
            # 🔴 SANS ELLE, « M2 » N'EST PAS M2. Un superviseur sur une VM sans
            # aucune fenetre eligible ne lance AUCUN enfant, donc n'ouvre AUCUNE
            # PeerConnection video, donc n'exerce AUCUNE contention : le delta
            # M2 − M1 serait alors nul PAR CONSTRUCTION, et se lirait comme
            # « le pont ne concurrence pas la video » (R5) alors qu'on n'aurait
            # rien mesure. Mesure a l'appui : une premiere tentative de M2 a
            # rendu `enfant lance` = 0.
            'application' {
                Start-Process $arg
                Start-Sleep -Seconds 20
                $sw.Stop()
                $r.ms = $sw.Elapsed.TotalMilliseconds
                $r.note = 'duree IMPOSEE (20 s), ce n est PAS une latence'
                $r.fenetres = @(Get-Process -Name ([System.IO.Path]::GetFileNameWithoutExtension($arg)) -ErrorAction SilentlyContinue).Count
                $r.ok = $true
            }
            'sommeil' { Start-Sleep -Seconds ([int]$arg); $sw.Stop(); $r.ms = $sw.Elapsed.TotalMilliseconds; $r.ok = $true }
            default { $sw.Stop(); $r.ok = $false; $r.erreur = "verbe inconnu : $verbe" }
        }
    } catch {
        $sw.Stop()
        $r.ms = $sw.Elapsed.TotalMilliseconds
        $r.ok = $false
        # ⚠️ LE TYPE DE L'EXCEPTION AUTANT QUE SON TEXTE : « echoue proprement »
        # et « tronque en silence » sont deux issues distinctes, et seule la
        # premiere porte un type.
        $r.erreur = $_.Exception.Message
        $r.erreur_type = $_.Exception.GetType().FullName
        $r.hresult = try { $_.Exception.HResult } catch { $null }
    }
    $r.fin_iso = (& $horoUtc)
    Enregistrer $r
    Start-Sleep -Seconds $repos
    # ⚠️ Le repos est DANS la fenetre du geste suivant, jamais dans celle-ci :
    # `fin_iso` est pose AVANT lui, et c'est ce qui permet a l'analyse de
    # borner le differentiel du recensement sur `[fin_iso, fin_iso + repos]`.
}

# 🔴 LE MARQUEUR DE FIN EST POSE APRES LE DERNIER REPOS, ET C'EST UN DEFAUT
# D'INSTRUMENT PAYE A LA PREMIERE EXECUTION.
#
# L'hote attendait que le releve porte AUTANT de gestes que le plan en demande.
# Or `Enregistrer` ecrit AVANT le repos : la condition etait donc satisfaite
# des la fin du dernier geste, l'hote reprenait la main, TUAIT CHROME, et le
# canal du pont tombait AVANT que les deux recensements du repos ne soient
# emis. **Le differentiel de latence aurait ete lu sur une seule ligne, ou sur
# aucune** — c'est-a-dire la mesure que ce sous-bloc existe pour prendre.
#
# ⚠️ Le symptome n'avait rien d'une panne : la mesure rendait « 1 gestes en 5 s »
# et un code de sortie zero.
Enregistrer $null $true
