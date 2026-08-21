param([string]$sortie = 'C:\dev\mesure-f3.json')
# 🔴 LE RELEVE S'ECRIT A UN CHEMIN NEUF A CHAQUE EXECUTION, ET C'EST PAYE.
#
# Une tentative FIGEE laisse son `powershell` vivant, qui TIENT le fichier de
# relevé. La mesure suivante ecrivait alors dans le vide — « le fichier est en
# cours d'utilisation par un autre processus » — et **le releve etait perdu
# pour une raison etrangere a ce qu'on mesure**. Tuer le processus figé avant
# de lancer est necessaire mais pas suffisant : le tueur lui-meme peut echouer
# (collision de guillemets avec l'enveloppe `nodejs-winrm`, piege D3, paye ici
# meme). Un chemin neuf, lui, ne peut pas etre tenu par personne.
# La mesure cote VM de la recette F3 : elle RENOMME et SUPPRIME dans la racine
# du pont, et rend un JSON que le pilote relit.
#
# 🔴 ELLE COURT PENDANT QUE LE LECTEUR EST MONTE.
#
# ⚠️ CHAQUE GESTE EST SUIVI D'UNE RELECTURE `Test-Path` : c'est l'observation
# (b) de la sonde S1, et sans elle on mesurerait l'absence d'un GESTE plutot
# que l'absence d'une NOTIFICATION. Une sonde de ce depot a deja rendu un faux
# verdict eliminatoire sur une machine saine pour exactement cette raison.
$ErrorActionPreference = 'Continue'
$racine = Join-Path $env:USERPROFILE 'Mes Fichiers'
$r = [ordered]@{ racine = $racine; horodatage = (Get-Date -Format o) }

function Etat([string]$rel) {
    $p = Join-Path $racine $rel
    if (Test-Path $p) { return 'PRESENT' } else { return 'ABSENT' }
}
function Contenu([string]$rel) {
    $p = Join-Path $racine $rel
    if (Test-Path $p) { try { return (Get-Content -Raw -Path $p -ErrorAction Stop) } catch { return '<ILLISIBLE>' } }
    return '<ABSENT>'
}

# ── L'etat AVANT tout geste. Un `0` se qualifie AVANT de se rapporter. ───────
$r.avant = @(Get-ChildItem -Path $racine -Recurse -ErrorAction SilentlyContinue |
    ForEach-Object { $_.FullName.Substring($racine.Length + 1) })
$r.avant_compte = $r.avant.Count

# ── CRITERE ① a : renommer un FICHIER ───────────────────────────────────────
# 🔴 `projete.txt` n'a jamais ete LU : il est a l'etat de SUBSTITUT. Le renommer
# oblige donc ProjFS a nous consulter, ce qu'un fichier deja hydrate ne ferait
# pas — c'est la moitie VM du defaut de casse, sous une autre forme.
$r.c1a_avant = Etat 'projete.txt'
try {
    Rename-Item -Path (Join-Path $racine 'projete.txt') -NewName 'renomme.txt' -ErrorAction Stop
    $r.c1a_issue = 'ok'
} catch { $r.c1a_issue = 'ECHEC:' + $_.Exception.Message }
$r.c1a_source_apres = Etat 'projete.txt'
$r.c1a_cible_apres  = Etat 'renomme.txt'

# ── CRITERE ① b : renommer un REPERTOIRE CONTENANT UN SOUS-REPERTOIRE ───────
# 🔴 C'EST LE DEFAUT DE L'ANCIEN PONT, `web/index.js:631` : une zone morte
# temporelle y fait echouer TOUJOURS ce cas precis.
New-Item -ItemType Directory -Path (Join-Path $racine 'sous-dossier\profond') -Force | Out-Null
Set-Content -Path (Join-Path $racine 'sous-dossier\profond\feuille.txt') -Value 'FEUILLE-F3' -NoNewline
Start-Sleep -Milliseconds 800
$r.c1b_avant = Etat 'sous-dossier\profond\feuille.txt'
try {
    Rename-Item -Path (Join-Path $racine 'sous-dossier') -NewName 'dossier-renomme' -ErrorAction Stop
    $r.c1b_issue = 'ok'
} catch { $r.c1b_issue = 'ECHEC:' + $_.Exception.Message }
$r.c1b_source_apres = Etat 'sous-dossier'
$r.c1b_cible_apres  = Etat 'dossier-renomme\profond\feuille.txt'

# ── CRITERE ② a : supprimer un FICHIER ──────────────────────────────────────
$r.c2a_avant = Etat 'Casse.txt'
try {
    Remove-Item -Path (Join-Path $racine 'Casse.txt') -Force -ErrorAction Stop
    $r.c2a_issue = 'ok'
} catch { $r.c2a_issue = 'ECHEC:' + $_.Exception.Message }
$r.c2a_apres = Etat 'Casse.txt'

# ── CRITERE ② b : supprimer un REPERTOIRE NON VIDE ──────────────────────────
# 🔴 QUESTION ③ DE LA SONDE S1 : ProjFS emet-il une notification PAR ENFANT ?
# Windows, lui, efface les enfants un a un — `rd /s` compris. Si la reponse est
# non, la suppression laissera les enfants sur le poste local, et le critere
# tombe pour sa seconde moitie : DEGRADE, ne bloque pas.
New-Item -ItemType Directory -Path (Join-Path $racine 'a-effacer\dedans') -Force | Out-Null
Set-Content -Path (Join-Path $racine 'a-effacer\un.txt') -Value 'UN' -NoNewline
Set-Content -Path (Join-Path $racine 'a-effacer\dedans\deux.txt') -Value 'DEUX' -NoNewline
Start-Sleep -Milliseconds 800
try {
    Remove-Item -Path (Join-Path $racine 'a-effacer') -Recurse -Force -ErrorAction Stop
    $r.c2b_issue = 'ok'
} catch { $r.c2b_issue = 'ECHEC:' + $_.Exception.Message }
$r.c2b_apres = Etat 'a-effacer'

# ── CRITERE ④ : les gestes REELS qui produisent des codes du §5 ─────────────
# ⚠️ Une INJECTION prouve que la table n'est pas decorative ; elle ne prouve PAS
# que la cause est atteignable en exploitation. Les deux colonnes sont
# distinguees, ici comme au plan.
$c4 = [ordered]@{}
# `Introuvable` — un chemin absent.
try { Get-Item -Path (Join-Path $racine 'jamais-existe.txt') -ErrorAction Stop | Out-Null; $c4.introuvable = 'ok?' }
catch { $c4.introuvable = 'refus:' + $_.Exception.GetType().Name }
# `CheminIntrouvable` — un chemin SOUS un repertoire absent.
try { Get-Item -Path (Join-Path $racine 'pas-la\dedans.txt') -ErrorAction Stop | Out-Null; $c4.chemin_introuvable = 'ok?' }
catch { $c4.chemin_introuvable = 'refus:' + $_.Exception.GetType().Name }
# `DejaPresent` — renommer sur un nom qui existe deja.
Set-Content -Path (Join-Path $racine 'deja-a.txt') -Value 'A' -NoNewline
Set-Content -Path (Join-Path $racine 'deja-b.txt') -Value 'B' -NoNewline
Start-Sleep -Milliseconds 800
try { Move-Item -Path (Join-Path $racine 'deja-a.txt') -Destination (Join-Path $racine 'deja-b.txt') -Force -ErrorAction Stop; $c4.deja_present = 'ok (Windows a ecrase AVANT nous)' }
catch { $c4.deja_present = 'refus:' + $_.Exception.Message.Substring(0, [Math]::Min(120, $_.Exception.Message.Length)) }
# `NonSupporte` — un lien dur dans la racine.
try { & cmd /c mklink /H "$racine\lien-dur.txt" "$racine\renomme.txt" 2>&1 | Out-Null; $c4.lien_dur = 'sortie=' + $LASTEXITCODE }
catch { $c4.lien_dur = 'exception:' + $_.Exception.Message }
# `AccesRefuse`, `DisquePlein`, `DelaiDepasse` — par INJECTION, cote navigateur.
foreach ($f in @('.faute-acces-refuse', '.faute-disque-plein', '.faute-non-supporte')) {
    try { Get-ChildItem -Path (Join-Path $racine $f) -ErrorAction Stop | Out-Null; $c4[$f] = 'ok?' }
    catch { $c4[$f] = 'refus:' + $_.Exception.GetType().Name }
}
$r.critere4 = $c4

# ── L'etat APRES ────────────────────────────────────────────────────────────
Start-Sleep -Seconds 3
$r.apres = @(Get-ChildItem -Path $racine -Recurse -ErrorAction SilentlyContinue |
    ForEach-Object { $_.FullName.Substring($racine.Length + 1) })
$r.apres_compte = $r.apres.Count

# 🔴 POINT DE REPRISE N°1 — LE JSON EST ECRIT ICI, PAS SEULEMENT A LA FIN.
#
# Paye sur place : la premiere execution de cette mesure s'est FIGEE sur un
# geste de la suite (un `Remove-Item` d'un repertoire que ProjFS declare non
# vide, ou une lecture que le navigateur ne sert JAMAIS par injection). Le
# fichier est reste a ZERO octet, et **les criteres ① et ②, qui etaient
# pourtant deja mesures, ont ete perdus avec le reste.**
#
# ⚠️ Un instrument qui n'ecrit qu'a la fin fait dependre TOUTE la mesure du
# geste le plus fragile. Les deux points de reprise coutent deux lignes.
$r | ConvertTo-Json -Depth 6 -Compress | Set-Content -Path $sortie -Encoding utf8

# ── CRITERE ④, SUITE : les gestes qui exigent un miroir AYANT DERIVE ────────
# ⚠️ Ils ne produisent leur code QUE si le pilote a pose la divergence
# (`DIVERGER=1`). Sans elle, ils passent sans rien exercer — et le dire evite
# qu'on lise un silence comme un succes.
$c5 = [ordered]@{}
# `RepertoireNonVide` — supprimer un repertoire que le poste local n'a pas vide.
New-Item -ItemType Directory -Path (Join-Path $racine 'vide-cote-vm') -Force | Out-Null
Start-Sleep -Milliseconds 500
# ⚠️ BORNE, comme tout geste dont l'issue depend du navigateur.
# 🔴 GESTE RETIRE, ET C'EST UN RESULTAT — PAS UN RENONCEMENT.
#
# `Remove-Item vide-cote-vm -Force` (non recursif) sur un repertoire que le
# poste local a rempli **NE REND JAMAIS LA MAIN** : mesure, deux executions,
# le releve reste bloque a ce geste. La cause se lit dans le releve lui-meme —
# `apres` porte `vide-cote-vm\inconnu-de-la-vm.txt` : **ProjFS montre a la VM
# le contenu que seul le poste local connait.** Windows voit donc un
# repertoire NON VIDE et n'atteint jamais notre garde.
#
# ⚠️ Ce que cela etablit : `repertoire-non-vide` et `deja-present` sont des
# codes de DIAGNOSTIC d'une derive du miroir, et **la derive que ce montage
# sait fabriquer est resolue par Windows AVANT de nous parvenir**. Ils restent
# donc hors d'atteinte par un geste reel ici. Les laisser manquants au tableau
# du critere ④ est honnete ; les faire tomber par un geste qui fige la mesure
# ne le serait pas.
$c5.repertoire_non_vide = 'HORS ATTEINTE : le geste fige la mesure (voir le commentaire)'
# `DejaPresent` — renommer vers un nom qui n'existe QUE cote local.
Set-Content -Path (Join-Path $racine 'a-deplacer.txt') -Value 'X' -NoNewline
Start-Sleep -Milliseconds 500
try { Rename-Item -Path (Join-Path $racine 'a-deplacer.txt') -NewName 'occupe-cote-local.txt' -ErrorAction Stop; $c5.deja_present = 'ok' }
catch { $m = $_.Exception.Message; $c5.deja_present = 'refus:' + $m.Substring(0, [Math]::Min(110, $m.Length)) }
# `Inattendue` via `casse-ambigue` — lire un nom dont deux homonymes existent.
try { Get-Content -Path (Join-Path $racine 'Ambigu.txt') -ErrorAction Stop | Out-Null; $c5.casse_ambigue = 'ok?' }
catch { $c5.casse_ambigue = 'refus:' + $_.Exception.GetType().Name }
# `DelaiDepasse` — le navigateur ne repond JAMAIS (`.faute-silence`).
#
# 🔴 BORNE PAR UN JOB, ET C'EST OBLIGATOIRE. Le pont expire de lui-meme au
# budget `DELAI_LISTER`, mais rien ne garantit que l'appel PowerShell rende la
# main : un geste non borne a deja fige toute cette mesure une fois.
# ⚠️ `Start-Job` + `Wait-Job -Timeout` a ete essaye ici et RENDU UN
# INTERBLOCAGE sous tache planifiee (`BlockedJobsDeadlockWithWaitJob`, trace
# versee). La borne qui tient est celle du PONT lui-meme (`DELAI_LISTER`), qui
# a bien rendu `delai-depasse=1` : l'appel revient de son propre chef.
# 🔵 MARQUE POSEE JUSTE AVANT LA COMMANDE QUI NE REVIENT JAMAIS.
#
# C'est la fenetre — et la seule — pendant laquelle une commande du pont est
# **REELLEMENT EN VOL** : le navigateur ne repondra pas, et le pont tiendra
# l'entree jusqu'a `DELAI_LISTER`. Le pilote guette cette marque pour poser sa
# coupure DESSUS, au lieu de parier sur une duree. Une coupure calee sur une
# horloge est tombee trois fois dans le vide : `abandonnee=0` avec un canal
# pourtant ferme, faute d'avoir quoi que ce soit a abandonner.
Set-Content -Path 'C:\dev\marque-silence.txt' -Value $r.horodatage -Encoding utf8
try { Get-ChildItem -Path (Join-Path $racine '.faute-silence') -ErrorAction Stop | Out-Null; $c5.silence = 'rendu sans erreur' }
catch { $c5.silence = 'refus:' + $_.Exception.GetType().Name }
$r.critere5 = $c5
$r.apres2 = @(Get-ChildItem -Path $racine -Recurse -ErrorAction SilentlyContinue |
    ForEach-Object { $_.FullName.Substring($racine.Length + 1) })
# 🔴 POINT DE REPRISE N°2 — le releve complet.
$r | ConvertTo-Json -Depth 6 -Compress | Set-Content -Path $sortie -Encoding utf8
