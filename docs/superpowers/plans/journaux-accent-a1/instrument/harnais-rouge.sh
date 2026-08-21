#!/usr/bin/env bash
# Harnais de rouge du sous-bloc A1 — §6.4 du plan.
#
# Les huit étapes, dans l'ordre, et le relevé porte les huit lignes :
#   1. copie NOMMÉE + sha256 des deux
#   2. la mutation, par numéro de ligne ou par un motif ancré sur la SIGNATURE
#   3. 🔴 `diff <fichier> <copie>` NON VIDE — et PAS `git diff --numstat`, que
#      P3 a mesuré VACUEUX (il compare à HEAD, donc reste non vide quelle que
#      soit la mutation, et même s'il n'y en a aucune)
#   4. la commande de contrôle, et le relevé dit QUELLE assertion a rougi
#   5. restauration DEPUIS LA COPIE — jamais `git checkout --`, qui restaure à
#      HEAD et a effacé du travail non commité deux fois dans ce dépôt
#   6. sha256 après, égal au premier
#   7. `git status --porcelain <fichier>` vide
#
# Usage : harnais-rouge.sh <nom> <fichier> <script-python-de-mutation> <cmd...>
set -uo pipefail
unset -f chpwd 2>/dev/null || true

NOM="$1"; FICHIER="$2"; MUTATION="$3"; shift 3
COPIE="/tmp/a1-rouge-${NOM}.orig"

echo "───────────────────────────────────────────────────────────────"
echo "ROUGE « $NOM »  sur  $FICHIER"
cp "$FICHIER" "$COPIE"
AVANT=$(sha256sum "$FICHIER" | cut -d' ' -f1)
echo "1. copie nommée : $COPIE   sha256(avant) = $AVANT"

python3 -c "$MUTATION" "$FICHIER" || { echo "🔴 la mutation a ÉCHOUÉ (ancre introuvable ?) — rouge NON JOUÉE"; cp "$COPIE" "$FICHIER"; exit 2; }
echo "2. mutation appliquée"

if diff -q "$FICHIER" "$COPIE" >/dev/null; then
    echo "3. 🔴 DIFF VIDE — LE HARNAIS REFUSE CETTE ROUGE : elle ne mute RIEN."
    cp "$COPIE" "$FICHIER"
    echo "   (restauré ; rouge NON COMPTÉE)"
    echo "REFUSÉE"
    exit 3
fi
echo "3. diff NON VIDE ($(diff "$FICHIER" "$COPIE" | grep -c '^[<>]') ligne(s)) ✅"

echo "4. contrôle : $*"
"$@" 2>&1 | tail -30
echo "   (code de sortie du contrôle : ${PIPESTATUS[0]})"

cp "$COPIE" "$FICHIER"
APRES=$(sha256sum "$FICHIER" | cut -d' ' -f1)
echo "5. restauré DEPUIS LA COPIE"
echo "6. sha256(après) = $APRES"
[ "$AVANT" = "$APRES" ] && echo "   ✅ ÉGAL" || { echo "   🔴 DIFFÉRENT — restauration ratée"; exit 4; }
echo -n "7. git status --porcelain : "
S=$(git status --porcelain "$FICHIER")
case "$S" in
    "")   echo "VIDE ✅" ;;
    "??"*) echo "« $S » — le fichier est NEUF, pas encore commité : la preuve de restauration est le sha256 de l ETAPE 6 ✅" ;;
    *)    echo "🔴 « $S » — le fichier est MODIFIÉ par rapport à HEAD" ;;
esac
