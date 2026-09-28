#!/usr/bin/env bash
# Red-run harness of block E3.
#
# Doctrine applied, and each clause cost a red run that stayed green in this
# repository:
#
#  - we compare with a NAMED COPY, never with HEAD. A check based on
#    `git diff` against HEAD does not distinguish "the mutation changed nothing" from
#    "the file was already modified";
#  - `git checkout --` restores HEAD, NOT the state from before. On a shared tree
#    it would destroy uncommitted work. Restoration is done from the
#    named copy, and its fingerprint is checked;
#  - RED 0: a mutation that changes NOTHING must be REFUSED. It is the
#    check of the harness itself; without it, no other red run counts.
#
# Usage: rouge.sh <label> <file> <mutation-script> <check-command>
#   the mutation script receives the file's path as $1.

set -uo pipefail

etiquette="${1:?étiquette}"
file="${2:?fichier}"
mutation="${3:?script de mutation}"
verif="${4:?commande de vérification}"

racine="$(cd "$(dirname "$0")/../../../../.." && pwd)"
cd "$racine" || exit 2

# 🔴 The path is made ABSOLUTE, and the check runs in a SUBSHELL.
# A defect found BY RUNNING, at red run R2b: its check command
# started with `cd client`, the `cd` outlived the command, and the
# restoration by relative path failed — "No such file or
# directory". The named copy was intact and the file was restored by hand,
# fingerprint checked; the harness, for its part, had a failure mode that left
# a mutation in the tree while announcing a verdict.
file="$(readlink -f "$file")"

copie="$(mktemp "/tmp/rouge-${etiquette}-XXXXXX")"
cp "$file" "$copie"
before="$(sha256sum < "$copie" | cut -d' ' -f1)"

echo "=== ROUGE ${etiquette} ==="
echo "fichier      : ${file}"
echo "copie nommée : ${copie}"
echo "sha256 avant : ${before}"

restaurer() {
    cp "$copie" "$file"
    apres="$(sha256sum < "$file" | cut -d' ' -f1)"
    echo "sha256 après restauration : ${apres}"
    if [ "$before" = "$apres" ]; then
        echo "restauration : IDENTIQUE"
    else
        echo "restauration : 🔴 DIVERGENTE — intervenir à la main"
    fi
    rm -f "$copie"
}
trap restaurer EXIT

bash "$mutation" "$file"

# RED 0: did the mutation really change the file?
lignes="$(diff <(cat "$copie") <(cat "$file") | grep -c '^[<>]' || true)"
echo "lignes changées par la mutation : ${lignes}"
if [ "$lignes" -eq 0 ]; then
    echo "🔴 HARNAIS : mutation VIDE — la rouge est REFUSÉE, elle ne prouve rien."
    exit 3
fi

echo "--- vérification : ${verif}"
set +e
( eval "$verif" ) 2>&1
code=$?
set -e
echo "--- code de sortie de la vérification : ${code}"
if [ "$code" -eq 0 ]; then
    echo "VERDICT : 🔴 RESTÉE VERTE — à DIAGNOSTIQUER, jamais à classer."
else
    echo "VERDICT : ROUGE (code ${code})"
fi
