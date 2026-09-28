#!/usr/bin/env bash
# Plays ONE red run: applies a named mutation, launches the check, RESTORES.
#
#     instrument/rouge.sh <file> <mutation-python> <check-command>
#
# 🔴 RESTORATION GOES NEITHER THROUGH `git checkout` NOR THROUGH `git stash`. The
# original content is copied into a temporary file BEFORE the mutation and
# written back AFTER, then `git diff --quiet` on that SINGLE file proves the tree
# is back to identical. Four agents of this repository got trapped the
# same day by a `git checkout` that took uncommitted work with it; this
# script cannot make that mistake, it only knows one file.
#
# ⚠️ THE CHECK'S EXIT CODE IS RECORDED, NOT PROPAGATED. A SUCCESSFUL red run
# is a check that FAILS: propagating its code would pass the red run off as
# an incident. This script exits with 0 if the restoration is clean, and with 1
# otherwise — it is the only thing that should alarm it.
set -uo pipefail

FILE="$1"
MUTATION="$2"
CONTROLE="$3"
RACINE="$(cd "$(dirname "$0")/../../../../.." && pwd)"
cd "$RACINE"

ORIGINAL="$(mktemp)"
cp "$FILE" "$ORIGINAL"
# 🔴 RESTORATION IS A `trap`, NOT A LINE AT THE END OF THE SCRIPT. A
# check command containing `exit` (they do contain some, to propagate
# the check's code through `eval`) would end the script BEFORE the
# restoration and leave the mutation in the tree — MEASURED at the first
# wording of this script, on `identite/garde.ts`.
restaurer() { cp "$ORIGINAL" "$FILE"; }
trap restaurer EXIT

python3 -c "$MUTATION" || { cp "$ORIGINAL" "$FILE"; echo 'MUTATION EN ÉCHEC' >&2; exit 1; }

# ⚠️ `diff -u` ON THE COPY, AND NOT `git diff`. One of the probes mutated here is
# the instrument itself, which is not yet tracked by git when the
# red run is played: `git diff` would show NOTHING of it, and `git diff --quiet`
# would return 0 on a file left mutated — a restoration check unable
# to fail. MEASURED at the first wording of this script, on red run ④.
echo "--- la mutation, en diff ---"
diff -u "$ORIGINAL" "$FILE" | sed 's/^/    /'
echo
echo "--- le contrôle, sur l'arbre MUTÉ ---"
# SUBSHELL: an `exit` in the check command must not
# take this script down with it (see the `trap` above, which catches the case where it
# would anyway).
( eval "$CONTROLE" ) 2>&1
CODE=$?
echo
echo "# code de sortie du contrôle sur l'arbre muté : $CODE"
echo "# (un code NON NUL est ce qu'on cherche : le contrôle dénonce la mutation)"

restaurer
trap - EXIT
echo
# The proof of restoration is a FINGERPRINT, not a `git diff`: it holds
# for a tracked file as well as for one that is not.
BEFORE="$(sha256sum < "$ORIGINAL" | cut -d' ' -f1)"
APRES="$(sha256sum < "$FILE" | cut -d' ' -f1)"
rm -f "$ORIGINAL"
echo "# sha256 avant la mutation : $BEFORE"
echo "# sha256 après restauration: $APRES"
if [ "$BEFORE" = "$APRES" ]; then
    echo "# source RESTAURÉE À L'IDENTIQUE (empreintes égales)"
    exit 0
fi
echo "🔴 RESTAURATION EN ÉCHEC sur $FILE" >&2
exit 1
