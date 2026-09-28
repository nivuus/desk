#!/usr/bin/env bash
# Plays one probe of acceptance run P3 and writes its log.
#
#     instrument/jouer.sh <probe> <sqlite|postgres> <output-file>
#
# 🔴 TURN_URL AND TURN_SECRET ARE SET HERE, AND IT IS NOT COMFORT
# CONFIGURATION. Without them, the relay sends NO `ice-config` to
# anyone, and the assertion "the intruder did not receive one" would be TRUE on a
# service whose guard had been entirely removed — a check unable
# to fail, the failure mode this repository paid for four times in
# sub-block D10. The values are those of acceptance run P2, taken as
# is so that the two `e2-*` logs can be read line by line.
#
# ⚠️ The service's error output is KEPT, in a separate section: the
# line `handshake refused: … it does not carry its prefix` is the
# "logged" half the spec requires of the refusal, and it only lives there.
set -uo pipefail

SONDE="$1"
MOTEUR="$2"
SORTIE="$3"
RACINE="$(cd "$(dirname "$0")/../../../../.." && pwd)"
COMMIT="$(git -C "$RACINE" rev-parse --short HEAD)"

ERR="$(mktemp)"
TURN_URL='turn:127.0.0.1:3478' \
TURN_SECRET='recette-p3-turn-secret' \
    "$RACINE/plateforme/node_modules/.bin/tsx" \
    "$RACINE/docs/superpowers/plans/journaux-plateforme-p3/instrument/sonde.ts" \
    "$SONDE" "$MOTEUR" "$COMMIT" >"$SORTIE" 2>"$ERR"
CODE=$?

{
    echo
    echo '# --- journal du service (sortie d’erreur), conservé tel quel ---'
    grep -v 'ExperimentalWarning\|trace-warnings' "$ERR" || true
    echo
    echo "# code de sortie de la sonde : $CODE (0 = tout tenu)"
} >>"$SORTIE"
rm -f "$ERR"
exit $CODE
