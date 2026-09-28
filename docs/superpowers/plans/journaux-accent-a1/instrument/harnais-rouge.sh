#!/usr/bin/env bash
# Red-run harness of sub-block A1 — §6.4 of the plan.
#
# The eight steps, in order, and the report carries the eight lines:
#   1. NAMED copy + sha256 of both
#   2. the mutation, by line number or by a pattern anchored on the SIGNATURE
#   3. 🔴 `diff <file> <copy>` NOT EMPTY — and NOT `git diff --numstat`, which
#      P3 measured VACUOUS (it compares with HEAD, hence stays non-empty whatever
#      the mutation, and even if there is none)
#   4. the check command, and the report says WHICH assertion turned red
#   5. restoration FROM THE COPY — never `git checkout --`, which restores to
#      HEAD and erased uncommitted work twice in this repository
#   6. sha256 afterwards, equal to the first
#   7. `git status --porcelain <file>` empty
#
# Usage: harnais-rouge.sh <name> <file> <python-mutation-script> <cmd...>
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
"$@" 2>&1 | tail -80
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
