#!/usr/bin/env bash
# THE HARNESS OF G5'S RED RUNS — eight steps, and it REFUSES a null mutation.
#
# 🔴 IT CANNOT USE `git diff`. That check compares with **HEAD**, and
#    therefore stays non-empty as long as uncommitted work lives in the file —
#    EVEN WHEN THE MUTATION IS NULL. Measured by P3, paid again by A1. The harness
#    compares with a NAMED COPY, taken right before the mutation.
#
# 🔴 IT NEVER RESTORES THROUGH `git checkout --`, which restores to HEAD and
#    ERASED UNCOMMITTED WORK TWICE in this repository. It restores FROM
#    THE COPY.
#
# 🔴 IT REQUIRES THE ANCHOR TO BE PRESENT EXACTLY ONCE. A1 measured an
#    anchor present FIVE times; S3 saw FOUR red runs out of sixteen stay green
#    or turn red for the wrong reason; P4 saw a mutation hit the
#    COMMENT justifying the line instead of the line.
#
# Usage: rouge.sh <name> <file> <anchor> <replacement> <command…>
set -uo pipefail
unset -f chpwd 2>/dev/null || true

NOM="${1:?nom}"; FICHIER="${2:?fichier}"; ANCRE="${3?ancre}"; REMPLACEMENT="${4?remplacement}"
shift 4

RACINE="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../../.." && pwd)"
cd "$RACINE"
COPIE="$(mktemp "/tmp/rouge-g5-XXXXXX")"

echo "════════════════════════════════════════════════════════════════"
echo "ROUGE $NOM"
echo "  fichier : $FICHIER"
echo "  ancre   : $ANCRE"
echo "  vers    : $REMPLACEMENT"

# ① the named copy, and its fingerprint
cp "$FICHIER" "$COPIE"
AVANT="$(sha256sum "$FICHIER" | cut -d' ' -f1)"
echo "  sha256 avant : $AVANT"

# ② the anchor exists EXACTLY once — a count, never a rereading
N="$(python3 - "$FICHIER" "$ANCRE" <<'PY'
import sys
print(open(sys.argv[1]).read().count(sys.argv[2]))
PY
)"
echo "  occurrences de l'ancre : $N"
if [ "$N" != "1" ]; then
  echo "  ⛔ REFUSEE : l'ancre doit apparaitre EXACTEMENT une fois (vue $N fois)"
  rm -f "$COPIE"; exit 3
fi

# ③ la mutation
python3 - "$FICHIER" "$ANCRE" "$REMPLACEMENT" <<'PY'
import sys
p,a,r = sys.argv[1], sys.argv[2], sys.argv[3]
s = open(p).read()
open(p,'w').write(s.replace(a, r, 1))
PY

# ④ PROOF that the diff AGAINST THE COPY is not empty — otherwise we REFUSE
LIGNES="$(diff "$COPIE" "$FICHIER" | wc -l)"
echo "  lignes de diff contre la copie : $LIGNES"
if [ "$LIGNES" = "0" ]; then
  echo "  ⛔ REFUSEE : DIFF VIDE — la mutation n'a rien mute, ce n'est pas une rouge"
  cp "$COPIE" "$FICHIER"; rm -f "$COPIE"; exit 4
fi

# ⑤ the check
echo "  ── sortie du controle ──"
set +e
"$@"
CODE=$?
set -e
echo "  ── code de sortie du controle : $CODE ──"

# ⑥ restoration, FROM THE COPY
cp "$COPIE" "$FICHIER"

# ⑦ the fingerprint is EQUAL — THAT is the proof of restoration
APRES="$(sha256sum "$FICHIER" | cut -d' ' -f1)"
echo "  sha256 apres : $APRES"
if [ "$AVANT" != "$APRES" ]; then
  echo "  ⛔ RESTAURATION FAUSSE : les empreintes different"
  rm -f "$COPIE"; exit 5
fi
echo "  ✅ restauration verifiee par l'empreinte"
# ⑧ ⚠️ `git status --porcelain` on a NEW file returns `??` and not empty:
#    the proof of restoration that counts is step ⑦.
rm -f "$COPIE"
echo "VERDICT ROUGE $NOM : controle sorti en $CODE"
exit 0
