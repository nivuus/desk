#!/usr/bin/env bash
# Joue UNE rouge : applique une mutation nommée, lance le contrôle, RESTAURE.
#
#     instrument/rouge.sh <fichier> <python-de-mutation> <commande-de-controle>
#
# 🔴 LA RESTAURATION NE PASSE PAS PAR `git checkout` NI PAR `git stash`. Le
# contenu d'origine est copié dans un fichier temporaire AVANT la mutation et
# réécrit APRÈS, puis `git diff --quiet` sur ce SEUL fichier prouve que l'arbre
# est revenu à l'identique. Quatre agents de ce dépôt se sont fait piéger le
# même jour par un `git checkout` qui a emporté du travail non commité ; ce
# script ne peut pas commettre cette erreur, il ne connaît qu'un fichier.
#
# ⚠️ LE CODE DE SORTIE DU CONTRÔLE EST RELEVÉ, PAS PROPAGÉ. Une rouge RÉUSSIE
# est un contrôle qui ÉCHOUE : propager son code ferait passer la rouge pour
# un incident. Ce script sort en 0 si la restauration est propre, et en 1
# sinon — c'est la seule chose qui doive l'alarmer.
set -uo pipefail

FICHIER="$1"
MUTATION="$2"
CONTROLE="$3"
RACINE="$(cd "$(dirname "$0")/../../../../.." && pwd)"
cd "$RACINE"

ORIGINAL="$(mktemp)"
cp "$FICHIER" "$ORIGINAL"
# 🔴 LA RESTAURATION EST UN `trap`, PAS UNE LIGNE DE FIN DE SCRIPT. Une
# commande de contrôle contenant `exit` (elles en contiennent, pour propager
# le code du contrôle à travers `eval`) terminerait le script AVANT la
# restauration et laisserait la mutation dans l'arbre — MESURÉ à la première
# rédaction de ce script, sur `identite/garde.ts`.
restaurer() { cp "$ORIGINAL" "$FICHIER"; }
trap restaurer EXIT

python3 -c "$MUTATION" || { cp "$ORIGINAL" "$FICHIER"; echo 'MUTATION EN ÉCHEC' >&2; exit 1; }

# ⚠️ `diff -u` SUR LA COPIE, ET NON `git diff`. Une des sondes mutées ici est
# l'instrument lui-même, qui n'est pas encore suivi par git au moment où la
# rouge se joue : `git diff` n'en montrerait RIEN, et `git diff --quiet`
# rendrait 0 sur un fichier resté muté — un contrôle de restauration incapable
# d'échouer. MESURÉ à la première rédaction de ce script, sur la rouge ④.
echo "--- la mutation, en diff ---"
diff -u "$ORIGINAL" "$FICHIER" | sed 's/^/    /'
echo
echo "--- le contrôle, sur l'arbre MUTÉ ---"
# SOUS-COQUILLE : un `exit` dans la commande de contrôle ne doit pas
# emporter ce script (voir le `trap` ci-dessus, qui rattrape le cas où il le
# ferait quand même).
( eval "$CONTROLE" ) 2>&1
CODE=$?
echo
echo "# code de sortie du contrôle sur l'arbre muté : $CODE"
echo "# (un code NON NUL est ce qu'on cherche : le contrôle dénonce la mutation)"

restaurer
trap - EXIT
echo
# La preuve de restauration est une EMPREINTE, pas un `git diff` : elle vaut
# pour un fichier suivi comme pour un fichier qui ne l'est pas.
AVANT="$(sha256sum < "$ORIGINAL" | cut -d' ' -f1)"
APRES="$(sha256sum < "$FICHIER" | cut -d' ' -f1)"
rm -f "$ORIGINAL"
echo "# sha256 avant la mutation : $AVANT"
echo "# sha256 après restauration: $APRES"
if [ "$AVANT" = "$APRES" ]; then
    echo "# source RESTAURÉE À L'IDENTIQUE (empreintes égales)"
    exit 0
fi
echo "🔴 RESTAURATION EN ÉCHEC sur $FICHIER" >&2
exit 1
