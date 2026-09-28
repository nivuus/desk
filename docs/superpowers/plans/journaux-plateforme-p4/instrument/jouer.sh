#!/usr/bin/env bash
# Plays one criterion of acceptance run P4 and writes its log.
#
#     instrument/jouer.sh <1|2|3|4> <sqlite|postgres> <output-file>
#
# ⚠️ THE SERVICE'S ERROR OUTPUT IS KEPT, in a separate section: the
# line `operation refused: instantane is not supported…` is the
# "logged" half criterion ① requires, and the probe only judges its GOING
# THROUGH its tee. The two views must agree, and it can be checked here.
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
