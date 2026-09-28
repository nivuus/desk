#!/usr/bin/env bash
# Mutation harness. It REFUSES to count a red run whose diff is EMPTY.
#
# 🔴 WHY THIS GUARD EXISTS: sub-block S3 found that FOUR red runs out of
# sixteen of its predecessor proved nothing — two turned red on a
# pre-existing clause, two had mutated nothing at all (`exit=1` read as a
# success of the red run). And sub-block P4 found that a mutation by
# string substitution hits THE COMMENT before the code, in a repository that
# comments its invariants: the more an invariant is documented, the more fragile
# its red run. Hence mutation by LINE NUMBER (`sed -i '<n>s/…/…/'`), never
# by the substring alone, and hence this guard.
#
# 🔴 AND WHY THE BACKUP IS A COPY, NEVER `git checkout --`.
# Paid for on the spot, on August 20th, 2026: a first wording of this harness
# restored through `git checkout -- <file>`, which erased the UNCOMMITTED
# work of the ongoing task. Two defects in one:
#   ① `git checkout` restores HEAD, not the state before the mutation;
#   ② `git diff` against HEAD is NON-EMPTY as soon as the file carries work
#      in progress — the empty-diff guard was therefore VACUOUS, exactly the
#      pattern it exists to forbid.
# The reference is a copy taken at that instant, and nothing else.
#
# Usage: rouge.sh <label> <file> <sed command> -- <check command…>
set -uo pipefail
etiquette="$1"; fichier="$2"; sedcmd="$3"; shift 3
[ "${1:-}" = "--" ] && shift

racine="$(cd "$(dirname "$0")/../../../../.." && pwd)"
cd "$racine" || exit 2

echo "=== ROUGE : $etiquette"
echo "--- fichier : $fichier"
sauvegarde="$(mktemp)"
cp "$fichier" "$sauvegarde"
avant="$(sha256sum "$fichier" | cut -d' ' -f1)"
echo "--- sha256 AVANT : $avant"

sed -i "$sedcmd" "$fichier"

# 🔴 THE GUARD. An empty diff means the mutation MUTATED NOTHING: the
# check that follows would then say nothing about the product.
if diff -q "$sauvegarde" "$fichier" >/dev/null; then
    echo "!!! LA MUTATION N'A RIEN MUTÉ : cette rouge NE COMPTE PAS."
    cp "$sauvegarde" "$fichier"; rm -f "$sauvegarde"
    exit 3
fi
echo "--- diff de la MUTATION SEULE (contre la copie prise à l'instant) :"
diff -u "$sauvegarde" "$fichier" | sed -n '3,60p'

echo "--- contrôle :"
"$@" 2>&1 | tail -40
echo "--- (le code de sortie du contrôle est celui de la commande ci-dessus)"

# 🔴 `cp` WITHOUT `-p`, AND IT IS THE OPPOSITE OF WHAT ONE WRITES SPONTANEOUSLY.
# Paid for on the spot, on August 20th, 2026, on `proto/src/fichiers.rs`: `cp -p`
# preserves the modification date, so that the RESTORED file looks
# UNCHANGED to cargo — which then keeps the compiled artefact of the MUTATED version.
# The symptom is a correct test failing for a reason INVISIBLE IN THE
# SOURCE: here, `charge.len() > TAILLE_TRAME_MAX` refused a payload of 65536
# against a maximum of 65536, which no reading of the file can
# explain. The worst case is the reverse: a GREEN suite still running the
# mutated code.
cp "$sauvegarde" "$fichier"; rm -f "$sauvegarde"
apres="$(sha256sum "$fichier" | cut -d' ' -f1)"
echo "--- sha256 APRÈS restauration : $apres"
if [ "$avant" = "$apres" ]; then echo "--- restauration VÉRIFIÉE"; else echo "!!! RESTAURATION FAUSSE"; exit 4; fi
echo
