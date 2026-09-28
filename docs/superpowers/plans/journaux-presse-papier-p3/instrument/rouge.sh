#!/usr/bin/env bash
# The red-run harness of §6.3 of plan P3 — MANDATORY, and it REFUSES a red run
# that mutates nothing.
#
# The seven lines of the report, in this order:
#   1. sha256sum BEFORE
#   2. the mutation, by LINE NUMBER or by a pattern anchored on the SYNTAX —
#      never by the substring alone. The platform's P4 saw a red run
#      stay GREEN because the string to mutate appeared first in the
#      COMMENT justifying it, and this repository comments its invariants.
#   3. 🔴 THE MUTATION CHANGED SOMETHING — comparison WITH THE NAMED COPY,
#      never `git diff`. An empty output IS A FAILURE OF THE RED RUN, never a
#      success of the product.
#
#      ⚠️ **And `git diff --numstat` CANNOT fill that role here, which was
#      noted on this very harness**: it compares with HEAD, so it stays
#      NON-EMPTY as long as an uncommitted fix lives in the file — whatever
#      the mutation, and even if there is none. The check meant to
#      refuse a red run that mutates nothing could then NOT fail. It is
#      the pattern "a check never seen red is not a
#      check" applied to the check itself.
#   4. the check command, and the report says WHICH ASSERTION turned red —
#      not only the exit code.
#   5. RESTORATION, FROM A NAMED COPY — never `git checkout --`
#   6. sha256sum AFTER, equal to the first
#   7. `git status --porcelain` EMPTY
#
# 🔴 **WHY NOT `git checkout --`, AND IT IS NOT A THEORETICAL
# PRECAUTION: THIS HARNESS PAID FOR IT.** Its first wording restored through
# `git checkout -- <file>`, which returns the file to HEAD — and not to its state
# before the mutation. Red run A therefore ERASED the uncommitted fix that
# the following red runs were to test; red runs B and C then
# stopped on "anchor not found", that is, on the ONLY visible symptom
# of lost work. The sha256 check did shout "DIVERGENT" — that is what
# made it visible —, but it shouted AFTER the loss.
#
# The named copy is therefore taken BEFORE the mutation and put back AFTER, and the
# sha256 checks it. `git status --porcelain` is still recorded for what it really
# says: that the file did not stay modified RELATIVE TO HEAD — which
# is FALSE while an uncommitted fix lives there, and the report says so
# rather than keeping quiet about it.
#
# Usage: rouge.sh <label> <file> <python-mutation-script> <cargo-filter>
set -uo pipefail
cd "$(git rev-parse --show-toplevel)"

ETIQ="$1"; FILE="$2"; MUTATION="$3"; FILTRE="$4"

echo "=== ROUGE « $ETIQ » ==="
echo "date : $(date -Is)   commit : $(git rev-parse --short HEAD)"
echo "fichier mute : $FILE"
echo
BEFORE=$(sha256sum "$FILE" | cut -d' ' -f1)
COPIE=$(mktemp)
cp -- "$FILE" "$COPIE"
echo "1. sha256 AVANT : $BEFORE   (copie nommee prise : $COPIE)"

echo "2. mutation :"
python3 -c "$MUTATION" || { echo "🔴 LA MUTATION A ECHOUE (ancre introuvable) — ce n'est PAS une rouge."; exit 2; }

MUTE=$(diff -u -- "$COPIE" "$FILE")
NUMSTAT=$(printf '%s' "$MUTE" | grep -cE '^[-+][^-+]' || true)
echo "3. lignes changees PAR LA MUTATION (contre la copie nommee) : ${NUMSTAT}"
echo "   (pour memoire, git diff --numstat contre HEAD : $(git diff --numstat -- "$FILE" | tr '\t' ' '))"
if [ -z "$MUTE" ]; then
    echo "🔴 SORTIE VIDE — ECHEC DE LA ROUGE, jamais un succes du produit."
    cp -- "$COPIE" "$FILE"; rm -f -- "$COPIE"; exit 3
fi
echo "   diff de la mutation :"
printf '%s\n' "$MUTE" | grep -E '^[-+][^-+]' | sed 's/^/     /'

# The check is `cargo test -p agent <filter>` by default; ROUGE_CMD
# replaces it for the CLIENT's red runs, which are played under vitest. The report must
# say WHICH ASSERTION turned red in both cases — it is rule 4 of §6.3, and
# it does not depend on the runner.
if [ -n "${ROUGE_CMD:-}" ]; then
    echo "4. controle : $ROUGE_CMD"
    eval "$ROUGE_CMD" 2>&1 \
        | grep -E "FAIL|AssertionError|→ |Expected|Received|Number of calls|Tests +[0-9]" \
        | head -25 | sed 's/^/     /'
else
    echo "4. controle : cargo test -p agent $FILTRE"
    ( cd agent && cargo test -p agent "$FILTRE" 2>&1 ) \
        | grep -E "^---- .* stdout|panicked at|assertion|^ *left:|^ *right:|^test result" \
        | sed 's/^/     /'
fi

cp -- "$COPIE" "$FILE"; rm -f -- "$COPIE"
echo "5. restauration DEPUIS LA COPIE NOMMEE (jamais git checkout --)"
APRES=$(sha256sum "$FILE" | cut -d' ' -f1)
echo "6. sha256 APRES : $APRES"
[ "$BEFORE" = "$APRES" ] && echo "   ✅ IDENTIQUE" || { echo "   🔴 DIVERGENT"; exit 4; }
PORCELAIN=$(git status --porcelain -- "$FILE")
echo "7. git status --porcelain : ${PORCELAIN:-<VIDE>}"
echo "   (un « M » ici ne dit PAS que la mutation a survecu : il dit que le"
echo "    fichier porte le correctif NON COMMITE que la rouge eprouve. Ce qui"
echo "    etablit la restauration est la ligne 6, et elle seule.)"
echo
