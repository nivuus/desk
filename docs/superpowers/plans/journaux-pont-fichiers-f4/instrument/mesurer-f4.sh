#!/usr/bin/env bash
# Copie la mesure sur la VM, la lance PAR TACHE PLANIFIEE /it, et rend son JSON.
#
#     mesurer-f4.sh "<plan de gestes>" [repos_s]
#
# 🔴 SESSION 1 ET NON WinRM (session 0). La racine ProjFS est montee par un
# processus de la session 1, et l'Explorateur du geste `explorer` n'a aucun
# bureau en session 0. Un seul chemin pour tous les gestes, c'est un chemin de
# moins a expliquer.
set -uo pipefail
PLAN="$1"
REPOS="${2:-25}"
RACINE=/home/mallanic/Projects/Guacamole
I="$RACINE/docs/superpowers/plans/journaux-pont-fichiers-f4/instrument"
set -a; source "$RACINE/.env"; set +a

# 🔴 TUER LES MESURES FIGEES D'ABORD. Une tentative figee laisse son
# `powershell` vivant, et ce processus TIENT son fichier de releve par la
# poignee de son `Out-File` : la mesure suivante ecrit alors dans le vide, et le
# releve est perdu POUR UNE RAISON QUI N'A RIEN A VOIR AVEC CE QU'ON MESURE.
# Le verrou a survecu a TROIS tentatives en F3.
#
# ⚠️ LE TUEUR PASSE PAR UN FICHIER, JAMAIS PAR UNE COMMANDE EN LIGNE :
# `nodejs-winrm` enveloppe tout dans `powershell -Command "& { ... }"`, et un
# script inline a guillemets doubles entre en collision avec cette enveloppe —
# le symptome etant un script qui NE TOURNE JAMAIS.
cat > /media/vm/dev/tuer-mesures-f4.ps1 <<'PSKILL'
schtasks /end /tn mesure-f4 2>$null | Out-Null
Get-CimInstance Win32_Process | Where-Object { $_.CommandLine -like '*mesure-f4*' } |
    ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }
PSKILL
node "$RACINE/scripts/winrm.js" 'powershell -NoProfile -ExecutionPolicy Bypass -File C:\dev\tuer-mesures-f4.ps1' >/dev/null 2>&1

cp "$I/mesurer-f4.ps1" /media/vm/dev/mesurer-f4.ps1
# 🔴 UN CHEMIN DE RELEVE NEUF PAR EXECUTION : « une tentative figee tient son
# fichier de releve, et la suivante ecrit dans le vide ».
HORO=$(date +%H%M%S%N | cut -c1-9)
SORTIE="C:\\dev\\mesure-f4-$HORO.json"
LOCAL="/media/vm/dev/mesure-f4-$HORO.json"
rm -f "$LOCAL" /media/vm/dev/mesure-f4.trace.txt

# 🔴 LA TRACE ET LE RELEVE DANS DEUX FICHIERS DISTINCTS : `Out-File` tronque a
# l'ouverture et TIENT la poignee, donc un `Set-Content` intercalaire — les
# points de reprise — serait perdu ou refuse, PRECISEMENT quand il sert.
cat > /media/vm/dev/lancer-mesure-f4.ps1 <<PS1
& 'C:\dev\mesurer-f4.ps1' -sortie '$SORTIE' -plan '$PLAN' -repos $REPOS *>&1 |
    Out-File -FilePath 'C:\dev\mesure-f4.trace.txt' -Encoding utf8
PS1

node "$RACINE/scripts/winrm.js" \
  "schtasks /delete /tn mesure-f4 /f 2>\$null; \
   schtasks /create /tn mesure-f4 /f /it /ru '$WINDOWS_ADMIN_USERNAME' /rp '$WINDOWS_ADMIN_PASSWORD' \
     /sc once /st 00:00 \
     /tr 'powershell -WindowStyle Hidden -NoProfile -ExecutionPolicy Bypass -File C:\dev\lancer-mesure-f4.ps1'; \
   schtasks /run /tn mesure-f4" >/dev/null 2>&1

# Attendre le FAIT : le releve porte AUTANT de gestes que le plan en demande.
#
# 🔴 IL N'Y A AUCUNE BORNE DE JOB. `Start-Job` + `Wait-Job -Timeout` rend un
# INTERBLOCAGE sous tache planifiee (`BlockedJobsDeadlockWithWaitJob`, trace
# versee par F3). La SEULE borne qui tienne est celle du PRODUIT — les budgets
# de `table.rs` —, et c'est precisement ce que F4 mesure. L'attente ici est donc
# BORNEE cote hote, et un releve PARTIEL est rendu EN LE DISANT : « un releve
# partiel annonce vaut mieux qu'un releve vide ».
# 🔴 LE FAIT ATTENDU EST LE MARQUEUR `"fini":true`, ET NON LE COMPTE DE GESTES.
# `Enregistrer` ecrit AVANT le repos : attendre le compte rendait la main des la
# fin du dernier geste, l'hote tuait Chrome, et LE CANAL DU PONT TOMBAIT AVANT
# LES DEUX RECENSEMENTS DU REPOS — c'est-a-dire avant la mesure elle-meme.
ATTENDUS=$(echo "$PLAN" | tr ',' '\n' | grep -c .)
BUDGET=$(( ATTENDUS * (REPOS + 120) / 2 ))
COMPLET=non
for i in $(seq 1 "$BUDGET"); do
    if [ -s "$LOCAL" ] && grep -qa '"fini":true' "$LOCAL"; then COMPLET=oui; break; fi
    sleep 2
done
cp /media/vm/dev/mesure-f4.trace.txt "${TRACE:-/dev/null}" 2>/dev/null || true
if [ "$COMPLET" = non ]; then
    echo "MESURE PARTIELLE : $(grep -ao '"geste"' "$LOCAL" 2>/dev/null | wc -l)/$ATTENDUS gestes en $((BUDGET*2)) s." >&2
fi
cat "$LOCAL" 2>/dev/null || echo '{"erreur":"mesure-f4.json absent"}'
