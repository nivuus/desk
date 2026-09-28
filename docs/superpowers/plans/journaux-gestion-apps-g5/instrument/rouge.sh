#!/usr/bin/env bash
# LE HARNAIS DES ROUGES DE G5 — huit étapes, et il REFUSE une mutation nulle.
#
# 🔴 IL NE PEUT PAS EMPLOYER `git diff`. Ce contrôle compare à **HEAD**, et
#    reste donc non vide tant qu'un travail non commité vit dans le fichier —
#    MÊME QUAND LA MUTATION EST NULLE. Mesuré par P3, repayé par A1. Le harnais
#    compare à une COPIE NOMMÉE, prise juste avant la mutation.
#
# 🔴 IL NE RESTAURE JAMAIS PAR `git checkout --`, qui restaure à HEAD et a
#    EFFACÉ DU TRAVAIL NON COMMITÉ DEUX FOIS dans ce dépôt. Il restaure DEPUIS
#    LA COPIE.
#
# 🔴 IL EXIGE QUE L'ANCRE SOIT PRÉSENTE EXACTEMENT UNE FOIS. A1 a mesuré une
#    ancre présente CINQ fois ; S3 a vu QUATRE rouges sur seize rester vertes
#    ou rougir pour la mauvaise raison ; P4 a vu une mutation frapper le
#    COMMENTAIRE qui justifie la ligne au lieu de la ligne.
#
# Usage : rouge.sh <nom> <fichier> <ancre> <remplacement> <commande…>
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

# ① la copie nommée, et son empreinte
cp "$FICHIER" "$COPIE"
AVANT="$(sha256sum "$FICHIER" | cut -d' ' -f1)"
echo "  sha256 avant : $AVANT"

# ② l'ancre existe EXACTEMENT une fois — un compte, jamais une relecture
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

# ④ PREUVE que le diff CONTRE LA COPIE est non vide — sinon on REFUSE
LIGNES="$(diff "$COPIE" "$FICHIER" | wc -l)"
echo "  lignes de diff contre la copie : $LIGNES"
if [ "$LIGNES" = "0" ]; then
  echo "  ⛔ REFUSEE : DIFF VIDE — la mutation n'a rien mute, ce n'est pas une rouge"
  cp "$COPIE" "$FICHIER"; rm -f "$COPIE"; exit 4
fi

# ⑤ le contrôle
echo "  ── sortie du controle ──"
set +e
"$@"
CODE=$?
set -e
echo "  ── code de sortie du controle : $CODE ──"

# ⑥ la restauration, DEPUIS LA COPIE
cp "$COPIE" "$FICHIER"

# ⑦ l'empreinte est ÉGALE — c'est CELA, la preuve de restauration
APRES="$(sha256sum "$FICHIER" | cut -d' ' -f1)"
echo "  sha256 apres : $APRES"
if [ "$AVANT" != "$APRES" ]; then
  echo "  ⛔ RESTAURATION FAUSSE : les empreintes different"
  rm -f "$COPIE"; exit 5
fi
echo "  ✅ restauration verifiee par l'empreinte"
# ⑧ ⚠️ `git status --porcelain` sur un fichier NEUF rend `??` et non le vide :
#    la preuve de restauration qui vaut est l'etape ⑦.
rm -f "$COPIE"
echo "VERDICT ROUGE $NOM : controle sorti en $CODE"
exit 0
