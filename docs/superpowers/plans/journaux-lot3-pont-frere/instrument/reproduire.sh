#!/usr/bin/env bash
# Lot 3, item 3 (3.8) — UNE exécution : reproduire la disparition du frère.
#
#     reproduire.sh <etiquette> [--sans-cache]
#
# 🔴 LE DÉFAUT VISÉ, tel que F5 l'a mesuré le 21 août 2026 : après un
# RENOMMAGE DANS LA VM, le répertoire frère `sous-dossier` DISPARAÎT du listage
# de la VM **alors qu'il existe toujours dans OPFS**. F5 a aussi tranché son
# attribution : le défaut est PRÉEXISTANT, et le cache le PROLONGE.
#
# ⚠️ CE SCRIPT NE JUGE RIEN : il relève les quatre maillons, un par un, et
# c'est le verdict qui conclut. Disculper un maillon ne désigne pas le suivant.
set -uo pipefail
unset -f chpwd 2>/dev/null || true

ETIQUETTE="${1:?usage : reproduire.sh <etiquette> [--sans-cache]}"
SANS_CACHE="${2:-}"
RACINE="$(git rev-parse --show-toplevel)" || {
    echo "🔴 hors du dépôt git : impossible de dériver RACINE" >&2; exit 1; }
J="${RACINE}/docs/superpowers/plans/journaux-lot3-pont-frere"
I="${J}/instrument"
# 🔴 LE PILOTE EST CELUI DE L'ITEM 1, RÉUTILISÉ PAR PARAMÈTRE — jamais copié.
PILOTE="${RACINE}/docs/superpowers/plans/journaux-lot3-pont-sauvegarde/instrument/pilote-item1.mjs"

set -a; . "${RACINE}/.env"; set +a
. "${RACINE}/docs/superpowers/plans/journaux-lot3/instrument/harnais-appliance.sh"

VM_RACINE='C:\Users\Administrator\Mes Fichiers'
etape() { echo; echo "=== $(date -Is) $* ==="; }

# Le listage de la VM, rendu comme une LISTE DE NOMS triée : c'est la forme qui
# rend « le frère a disparu » lisible d'un coup d'œil, et comparable entre bras.
lister_vm() {
    W 'Get-ChildItem "C:\Users\Administrator\Mes Fichiers" -Force -ErrorAction SilentlyContinue |
       Sort-Object Name | ForEach-Object { ($(if ($_.PSIsContainer) {"D"} else {"F"})) + " " + $_.Name }' 120
}

etape "PRÉ-VOL"
vm_prete || exit 1

# 🔴 LA RACINE ProjFS DE LA VM GARDE CE QUE L'EXÉCUTION PRÉCÉDENTE Y A HYDRATÉ,
# ET LA PURGE D'OPFS NE L'ATTEINT PAS. Mesuré le 5 septembre 2026 : à la
# deuxième exécution, `renomme.txt` était encore là et le renommage a ÉCHOUÉ
# (« issue du renommage : False ») — le bras était perdu pour une raison
# étrangère au défaut cherché. On vide donc la racine de la VM AVANT que le
# pont ne monte : rien n'est alors poussé au poste local, le pont étant absent.
etape "PURGE de la racine ProjFS de la VM (l'etat hydrate d'une execution precedente)"
# 🔴 LA PURGE EST UNE PORTE, PAS UN COMPTE RENDU. La premiere redaction
# imprimait « restant apres purge : 3 » ET CONTINUAIT : le jeu de l'execution
# precedente restait en place, le renommage echouait (« issue du renommage :
# False »), et meme le temoin negatif ne tirait plus. DEUX bras ont ete perdus
# ainsi le 5 septembre 2026. Un releve qu'on imprime sans en rien faire n'est
# pas un controle.
# ⚠️ La suppression peut etre refusee tant que ProjFS virtualise encore la
# racine : on reessaie, puis on ABANDONNE en le disant.
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
    # Désarme le cache d'énumération (TTL_ENUMERATION = 30 s) : chaque listage
    # repaie son aller-retour, c'est-à-dire EXACTEMENT le produit d'avant F5.
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
# ⚠️ LE SECOND LISTAGE EST LE POINT DE L'A/B : cache armé il reste absent
# jusqu'au TTL ; PONT_CACHE=0 il doit revenir. Un seul listage ne les sépare pas.
sleep 3
lister_vm

echo
echo "### LISTAGE DE LA VM — TROISIÈME, après plus de TTL_ENUMERATION (30 s)"
sleep 33
lister_vm

echo
echo "### 🔴 TÉMOIN NÉGATIF — CE LISTAGE PEUT-IL SEULEMENT MONTRER UNE DISPARITION ?"
# Sans ce bras, « sous-dossier est toujours là » serait rendu par un listage
# INCAPABLE de perdre quoi que ce soit — un contrôle qui ne peut pas échouer,
# le patron que ce dépôt punit. On supprime donc POUR DE BON un témoin, et le
# listage suivant DOIT le perdre. S'il ne le perd pas, tout le relevé
# ci-dessus est sans valeur et le verdict doit le dire.
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
