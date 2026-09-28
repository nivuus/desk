#!/usr/bin/env bash
# Plays ONE red run of acceptance run P4: mutate, play the criterion, restore, PROVE
# the restoration.
#
#     instrument/rouge.sh <label> <mutated-file> <criterion> <engine> <output>
#
# The mutation itself is applied by the calling script, BEFORE the call:
# this script only frames it. It writes into the log the diff, the
# `sha256` BEFORE mutation (read from the repository), the one AFTER restoration, and their
# comparison.
#
# 🔴 `git checkout` DOES NOT RESTORE AN UNTRACKED FILE. Two mutations
# survived it in this repository on August 20th, 2026, including a `setTimeout(2000)` left in
# a PRODUCTION ROUTE. It is the reason for the fingerprint
# comparison below: it does not trust `git checkout`, it
# checks it. All files mutated by this acceptance run are tracked — it is
# checked by `git ls-files --error-unmatch` before any mutation.
set -uo pipefail

ETIQUETTE="$1"; FICHIER="$2"; CRITERE="$3"; MOTEUR="$4"; SORTIE="$5"
RACINE="$(cd "$(dirname "$0")/../../../../.." && pwd)"
# ⚠️ NO APOSTROPHE IN THIS MESSAGE: bash parses the word of a ${var:?word} with
# the quoting rules, and an apostrophe there opened a single quote that
# made the whole script syntactically invalid. Caught at the first run.
AVANT="${AVANT_SHA:?AVANT_SHA doit etre pose par le script appelant, AVANT la mutation}"

{
    echo "# ROUGE ${ETIQUETTE} — critère ${CRITERE}, moteur ${MOTEUR}"
    echo "# Jouée le $(date -Is), commit $(git -C "$RACINE" rev-parse --short HEAD)"
    echo "#"
    echo "--- le fichier muté est-il SUIVI par git ? (sans quoi checkout ne le restaurerait pas) ---"
    git -C "$RACINE" ls-files --error-unmatch "$FICHIER" >/dev/null 2>&1 \
        && echo "SUIVI : oui" || echo "SUIVI : 🔴 NON — la restauration serait ILLUSOIRE"
    echo
    echo "--- la mutation, en diff ---"
    git -C "$RACINE" --no-pager diff -- "$FICHIER" | sed 's/^/    /'
    echo
    echo "--- le contrôle, sur l'arbre MUTÉ ---"
} > "$SORTIE"

"$(dirname "$0")/jouer.sh" "$CRITERE" "$MOTEUR" /tmp/rouge-corps.log
CODE=$?
cat /tmp/rouge-corps.log >> "$SORTIE"

git -C "$RACINE" checkout -- "$FICHIER"
APRES="$(sha256sum "$RACINE/$FICHIER" | cut -d' ' -f1)"

{
    echo
    echo "--- restauration ---"
    echo "sha256 AVANT mutation    : $AVANT"
    echo "sha256 APRÈS restauration: $APRES"
    if [ "$AVANT" = "$APRES" ]; then
        echo "les deux empreintes CONCORDENT : le fichier est rendu à l'octet près."
    else
        echo "🔴 LES DEUX EMPREINTES DIVERGENT : la mutation a SURVÉCU. NE PAS COMMITER."
    fi
    echo
    echo "# code de sortie de la sonde SOUS MUTATION : $CODE"
    echo "# (1 = au moins une assertion NON TENUE = la rouge a bien rougi ;"
    echo "#  0 = 🔴 LA MUTATION EST RESTÉE VERTE, l'assertion visée n'éprouve rien)"
} >> "$SORTIE"

[ "$AVANT" = "$APRES" ] || exit 2
[ "$CODE" -ne 0 ] || exit 3
exit 0
