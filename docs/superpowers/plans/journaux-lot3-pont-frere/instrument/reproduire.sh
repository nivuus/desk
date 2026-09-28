#!/usr/bin/env bash
# Batch 3, item 3 (3.8) — ONE run: reproduce the sibling's disappearance.
#
#     reproduire.sh <etiquette> [--sans-cache]
#
# 🔴 THE TARGETED DEFECT, as F5 measured it on August 21st, 2026: after a
# RENAME IN THE VM, the sibling directory `sous-dossier` DISAPPEARS from the VM's
# listing **while it still exists in OPFS**. F5 also settled its
# attribution: the defect is PRE-EXISTING, and the cache PROLONGS it.
#
# ⚠️ THIS SCRIPT JUDGES NOTHING: it records the four links, one by one, and
# it is the verdict that concludes. Clearing one link does not designate the next.
set -uo pipefail
unset -f chpwd 2>/dev/null || true

ETIQUETTE="${1:?usage : reproduire.sh <etiquette> [--sans-cache]}"
SANS_CACHE="${2:-}"
RACINE="$(git rev-parse --show-toplevel)" || {
    echo "🔴 hors du dépôt git : impossible de dériver RACINE" >&2; exit 1; }
J="${RACINE}/docs/superpowers/plans/journaux-lot3-pont-frere"
I="${J}/instrument"
# 🔴 THE DRIVER IS ITEM 1'S, REUSED BY PARAMETER — never copied.
PILOTE="${RACINE}/docs/superpowers/plans/journaux-lot3-pont-sauvegarde/instrument/pilote-item1.mjs"

set -a; . "${RACINE}/.env"; set +a
. "${RACINE}/docs/superpowers/plans/journaux-lot3/instrument/harnais-appliance.sh"

VM_RACINE='C:\Users\Administrator\Mes Fichiers'
etape() { echo; echo "=== $(date -Is) $* ==="; }

# The VM's listing, returned as a sorted LIST OF NAMES: it is the shape that
# makes "the sibling disappeared" readable at a glance, and comparable between arms.
lister_vm() {
    W 'Get-ChildItem "C:\Users\Administrator\Mes Fichiers" -Force -ErrorAction SilentlyContinue |
       Sort-Object Name | ForEach-Object { ($(if ($_.PSIsContainer) {"D"} else {"F"})) + " " + $_.Name }' 120
}

etape "PRÉ-VOL"
vm_prete || exit 1

# 🔴 THE VM'S ProjFS ROOT KEEPS WHAT THE PREVIOUS RUN HYDRATED THERE,
# AND PURGING OPFS DOES NOT REACH IT. Measured on September 5th, 2026: at the
# second run, `renomme.txt` was still there and the rename FAILED
# ("rename outcome: False") — the arm was lost for a reason
# foreign to the defect sought. We therefore empty the VM's root BEFORE the
# bridge mounts: nothing is then pushed to the local machine, the bridge being absent.
etape "PURGE de la racine ProjFS de la VM (l'etat hydrate d'une execution precedente)"
# 🔴 THE PURGE IS A GATE, NOT A REPORT. The first wording
# printed "remaining after purge: 3" AND CARRIED ON: the previous run's
# data set stayed in place, the rename failed ("rename outcome:
# False"), and even the negative control no longer fired. TWO arms were lost
# that way on September 5th, 2026. A reading one prints without doing anything with it is
# not a check.
# ⚠️ Deletion can be refused as long as ProjFS still virtualises the
# root: we retry, then we GIVE UP saying so.
PURGE_RESTANT=9
for tentative in 1 2 3 4 5; do
    PURGE_RESTANT=$(W 'Get-ChildItem "C:\Users\Administrator\Mes Fichiers" -Force -ErrorAction SilentlyContinue | ForEach-Object { Remove-Item $_.FullName -Recurse -Force -ErrorAction SilentlyContinue }; Start-Sleep -Milliseconds 500; @(Get-ChildItem "C:\Users\Administrator\Mes Fichiers" -Force -ErrorAction SilentlyContinue).Count' 120 | tr -dc '0-9')
    echo "tentative ${tentative} : restant = ${PURGE_RESTANT:-?}"
    [ "${PURGE_RESTANT:-9}" = "0" ] && break
    sleep 6
done
if [ "${PURGE_RESTANT:-9}" != "0" ]; then
    echo "🔴 LA RACINE ProjFS N A PAS PU ETRE VIDEE (${PURGE_RESTANT} entree(s))." >&2
    echo "   Le jeu de l execution precedente subsisterait, le renommage echouerait," >&2
    echo "   et le bras serait perdu pour une raison ETRANGERE au defaut cherche." >&2
    exit 1
fi

if [ "${SANS_CACHE}" = "--sans-cache" ]; then
    etape "BRAS D'ATTRIBUTION : PONT_CACHE=0"
    # Disarms the enumeration cache (TTL_ENUMERATION = 30 s): each listing
    # pays its round trip again, that is, EXACTLY the product from before F5.
    agent_arreter
    variable_de_banc poser PONT_CACHE 0
    W 'Get-Content C:\nivuus\agent\run-agent.ps1 -Encoding UTF8 | Select-String "env:PONT_CACHE|agent.exe" | ForEach-Object { "ligne $($_.LineNumber) : $($_.Line.Trim())" }' 90
    agent_relancer 30
fi

REPERE=$(journal_reperer)
etape "REPÈRE du journal : ${REPERE}"

etape "LE PILOTE monte le pont et TIENT la session"
nohup node "${PILOTE}" --etiquette="${ETIQUETTE}" --maintien=150 \
      --injection=../../journaux-lot3-pont-frere/instrument/injection-item3.js \
      --prepare=__item3Preparer --relire=__item3Relire \
      --sortie="${J}/opfs-${ETIQUETTE}.json" > "${J}/pilote-${ETIQUETTE}.log" 2>&1 &
PILOTE_PID=$!
for _ in $(seq 1 40); do
    sleep 3
    grep -aq "pont monté côté navigateur" "${J}/pilote-${ETIQUETTE}.log" 2>/dev/null && break
done
grep -a "racine locale préparée\|pont monté" "${J}/pilote-${ETIQUETTE}.log" || true
sleep 4

{
echo "### MAILLON 4 (référence) — CE QUE LE POSTE LOCAL CONTIENT, AVANT"
grep -a "relecture AVANT" "${J}/pilote-${ETIQUETTE}.log" || echo "(pas encore relu)"

echo
echo "### LISTAGE DE LA VM — AVANT le renommage"
lister_vm

echo
echo "### LE GESTE : renommer a-renommer.txt DANS LA VM"
W 'Rename-Item -Path "C:\Users\Administrator\Mes Fichiers\a-renommer.txt" -NewName "renomme.txt" -ErrorAction Continue
   "issue du renommage : " + $?' 120

echo
echo "### LISTAGE DE LA VM — APRÈS le renommage (le frère est-il encore là ?)"
sleep 3
lister_vm

echo
echo "### LISTAGE DE LA VM — SECOND listage, sans aucun Rafraichir"
# ⚠️ THE SECOND LISTING IS THE POINT OF THE A/B: cache armed it stays absent
# until the TTL; PONT_CACHE=0 it must come back. A single listing does not separate them.
sleep 3
lister_vm

echo
echo "### LISTAGE DE LA VM — TROISIÈME, après plus de TTL_ENUMERATION (30 s)"
sleep 33
lister_vm

echo
echo "### 🔴 TÉMOIN NÉGATIF — CE LISTAGE PEUT-IL SEULEMENT MONTRER UNE DISPARITION ?"
# Without this arm, "sous-dossier is still there" would be returned by a listing
# UNABLE to lose anything — a check that cannot fail,
# the pattern this repository punishes. We therefore delete a control FOR GOOD, and the
# next listing MUST lose it. If it does not, the whole reading
# above is worthless and the verdict must say so.
W 'Remove-Item "C:\Users\Administrator\Mes Fichiers\temoin-2.txt" -Force -ErrorAction Continue
   "issue de la suppression : " + $?' 120
sleep 4
echo "--- listage APRÈS la suppression de temoin-2.txt (il doit AVOIR DISPARU) ---"
lister_vm
} 2>&1 | tee "${J}/listages-${ETIQUETTE}.log"

etape "MAILLONS 1 à 3 — ce que l'agent a NOTIFIÉ, RETENU et CACHÉ"
journal_depuis "${REPERE}" | sed 's/\x1b\[[0-9;]*m//g' \
  | grep -aiE "RENAME|renomm|mutation|cache|enumeration|Lister|invalid|frere|sous-dossier" \
  | tee "${J}/maillons-${ETIQUETTE}.log" | tail -25

etape "ATTENTE de la fin du pilote — il relit OPFS APRÈS"
wait "${PILOTE_PID}" 2>/dev/null
grep -a "relecture APRÈS" "${J}/pilote-${ETIQUETTE}.log" || true

if [ "${SANS_CACHE}" = "--sans-cache" ]; then
    etape "RETOUR À L'ÉTAT LIVRÉ"
    agent_arreter
    variable_de_banc retirer PONT_CACHE
    agent_relancer 30
fi
bash "${RACINE}/docs/superpowers/plans/journaux-lot3/instrument/etat-vm.sh"
