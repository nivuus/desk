#!/usr/bin/env bash
# Joue une sonde de la recette P3 et écrit son journal.
#
#     instrument/jouer.sh <sonde> <sqlite|postgres> <fichier-de-sortie>
#
# 🔴 TURN_URL ET TURN_SECRET SONT POSÉS ICI, ET CE N'EST PAS DE LA
# CONFIGURATION DE CONFORT. Sans eux, le relais n'envoie AUCUNE `ice-config` à
# personne, et l'assertion « l'intrus n'en a pas reçu » serait VRAIE sur un
# service dont la garde aurait été entièrement retirée — un contrôle incapable
# d'échouer, le mode de défaillance que ce dépôt a payé quatre fois au
# sous-bloc D10. Les valeurs sont celles de la recette P2, reprises telles
# quelles pour que les deux journaux `e2-*` se lisent ligne à ligne.
#
# ⚠️ La sortie d'erreur du service est CONSERVÉE, dans une section à part : la
# ligne `poignée de main refusée : … elle ne porte pas son préfixe` est la
# moitié « journalisée » que la spec exige du refus, et elle ne vit que là.
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
