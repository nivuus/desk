#!/usr/bin/env bash
# Joue un critère de la recette P4 et écrit son journal.
#
#     instrument/jouer.sh <1|2|3|4> <sqlite|postgres> <fichier-de-sortie>
#
# ⚠️ LA SORTIE D'ERREUR DU SERVICE EST CONSERVÉE, dans une section à part : la
# ligne `opération refusée : instantane n'est pas supportée…` est la moitié
# « journalisée » que le critère ① exige, et la sonde n'en juge que le PASSAGE
# par son tee. Les deux vues doivent concorder, et c'est vérifiable ici.
set -uo pipefail

CRITERE="$1"
MOTEUR="$2"
SORTIE="$3"
RACINE="$(cd "$(dirname "$0")/../../../../.." && pwd)"
COMMIT="$(git -C "$RACINE" rev-parse --short HEAD)"

ERR="$(mktemp)"
"$RACINE/plateforme/node_modules/.bin/tsx" \
    "$RACINE/docs/superpowers/plans/journaux-plateforme-p4/instrument/sonde.ts" \
    "$CRITERE" "$MOTEUR" "$COMMIT" >"$SORTIE" 2>"$ERR"
CODE=$?

{
    echo
    echo '# --- journal du service (sortie d’erreur), conservé tel quel ---'
    grep -v 'ExperimentalWarning\|trace-warnings\|node:internal' "$ERR" || true
    echo
    echo "# code de sortie de la sonde : $CODE (0 = tout tenu)"
} >>"$SORTIE"
rm -f "$ERR"
exit $CODE
