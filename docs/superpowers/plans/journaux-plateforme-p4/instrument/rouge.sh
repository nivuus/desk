#!/usr/bin/env bash
# Joue UNE rouge de la recette P4 : muter, jouer le critère, restaurer, PROUVER
# la restauration.
#
#     instrument/rouge.sh <étiquette> <fichier-muté> <critère> <moteur> <sortie>
#
# La mutation elle-même est appliquée par le script appelant, AVANT l'appel :
# ce script ne fait que l'encadrer. Il écrit dans le journal le diff, le
# `sha256` AVANT mutation (lu du dépôt), celui APRÈS restauration, et leur
# comparaison.
#
# 🔴 `git checkout` NE RESTAURE PAS UN FICHIER NON SUIVI. Deux mutations y ont
# survécu dans ce dépôt le 20 août 2026, dont un `setTimeout(2000)` laissé dans
# une ROUTE DE PRODUCTION. C'est la raison d'être de la comparaison
# d'empreintes ci-dessous : elle ne fait pas confiance à `git checkout`, elle
# le vérifie. Tous les fichiers mutés par cette recette sont suivis — c'est
# contrôlé par `git ls-files --error-unmatch` avant toute mutation.
set -uo pipefail

ETIQUETTE="$1"; FICHIER="$2"; CRITERE="$3"; MOTEUR="$4"; SORTIE="$5"
RACINE="$(cd "$(dirname "$0")/../../../../.." && pwd)"
# ⚠️ PAS D'APOSTROPHE DANS CE MESSAGE : bash parse le mot d'un ${var:?mot} avec
# les règles de citation, et un « l'appelant » y ouvrait une simple quote qui
# rendait le script entier insyntaxique. Attrapé à la première exécution.
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
