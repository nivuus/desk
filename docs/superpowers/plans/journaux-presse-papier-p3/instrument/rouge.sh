#!/usr/bin/env bash
# Le harnais de rouge du §6.3 du plan P3 — OBLIGATOIRE, et il REFUSE une rouge
# qui ne mute rien.
#
# Les sept lignes du relevé, dans cet ordre :
#   1. sha256sum AVANT
#   2. la mutation, par NUMÉRO DE LIGNE ou par un motif ancré sur la SYNTAXE —
#      jamais par la seule sous-chaîne. P4 de la plateforme a vu une rouge
#      rester VERTE parce que la chaîne à muter apparaissait d'abord dans le
#      COMMENTAIRE qui la justifie, et ce dépôt commente ses invariants.
#   3. 🔴 LA MUTATION A CHANGÉ QUELQUE CHOSE — comparaison À LA COPIE NOMMÉE,
#      jamais `git diff`. Une sortie vide EST UN ÉCHEC DE LA ROUGE, jamais un
#      succès du produit.
#
#      ⚠️ **Et `git diff --numstat` NE PEUT PAS remplir ce rôle ici, ce qui a
#      été relevé sur ce harnais même** : il compare à HEAD, donc il reste
#      NON VIDE tant qu'un correctif non commité vit dans le fichier — quelle
#      que soit la mutation, et même s'il n'y en a aucune. Le contrôle censé
#      refuser une rouge qui ne mute rien ne pouvait alors PAS échouer. C'est
#      le patron « un contrôle qu'on n'a jamais vu rouge n'est pas un
#      contrôle » appliqué au contrôle lui-même.
#   4. la commande de contrôle, et le relevé dit QUELLE ASSERTION a rougi —
#      pas seulement le code de sortie.
#   5. la RESTAURATION, DEPUIS UNE COPIE NOMMÉE — jamais `git checkout --`
#   6. sha256sum APRÈS, égal au premier
#   7. `git status --porcelain` VIDE
#
# 🔴 **POURQUOI PAS `git checkout --`, ET CE N'EST PAS UNE PRÉCAUTION
# THÉORIQUE : CE HARNAIS L'A PAYÉ.** Sa première rédaction restaurait par
# `git checkout -- <fichier>`, qui rend le fichier à HEAD — et non à son état
# d'avant la mutation. La rouge A a donc EFFACÉ le correctif non commité que
# les rouges suivantes devaient éprouver ; les rouges B et C se sont alors
# arrêtées sur « ancre introuvable », c'est-à-dire sur le SEUL symptôme visible
# d'un travail perdu. Le contrôle de sha256 a bien crié « DIVERGENT » — c'est ce
# qui l'a fait voir —, mais il criait APRÈS la perte.
#
# La copie nommée est donc prise AVANT la mutation et remise APRÈS, et le
# sha256 la vérifie. `git status --porcelain` reste relevé pour ce qu'il dit
# vraiment : que le fichier n'est pas resté modifié PAR RAPPORT À HEAD — ce qui
# est FAUX pendant qu'un correctif non commité y vit, et le relevé le dit
# plutôt que de le taire.
#
# Usage : rouge.sh <etiquette> <fichier> <script-python-de-mutation> <filtre-cargo>
set -uo pipefail
cd "$(git rev-parse --show-toplevel)"

ETIQ="$1"; FICHIER="$2"; MUTATION="$3"; FILTRE="$4"

echo "=== ROUGE « $ETIQ » ==="
echo "date : $(date -Is)   commit : $(git rev-parse --short HEAD)"
echo "fichier mute : $FICHIER"
echo
AVANT=$(sha256sum "$FICHIER" | cut -d' ' -f1)
COPIE=$(mktemp)
cp -- "$FICHIER" "$COPIE"
echo "1. sha256 AVANT : $AVANT   (copie nommee prise : $COPIE)"

echo "2. mutation :"
python3 -c "$MUTATION" || { echo "🔴 LA MUTATION A ECHOUE (ancre introuvable) — ce n'est PAS une rouge."; exit 2; }

MUTE=$(diff -u -- "$COPIE" "$FICHIER")
NUMSTAT=$(printf '%s' "$MUTE" | grep -cE '^[-+][^-+]' || true)
echo "3. lignes changees PAR LA MUTATION (contre la copie nommee) : ${NUMSTAT}"
echo "   (pour memoire, git diff --numstat contre HEAD : $(git diff --numstat -- "$FICHIER" | tr '\t' ' '))"
if [ -z "$MUTE" ]; then
    echo "🔴 SORTIE VIDE — ECHEC DE LA ROUGE, jamais un succes du produit."
    cp -- "$COPIE" "$FICHIER"; rm -f -- "$COPIE"; exit 3
fi
echo "   diff de la mutation :"
printf '%s\n' "$MUTE" | grep -E '^[-+][^-+]' | sed 's/^/     /'

# Le controle est `cargo test -p agent <filtre>` par defaut ; ROUGE_CMD le
# remplace pour les rouges du CLIENT, qui se jouent sous vitest. Le releve doit
# dire QUELLE ASSERTION a rougi dans les deux cas — c'est la regle 4 du §6.3, et
# elle ne depend pas du lanceur.
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

cp -- "$COPIE" "$FICHIER"; rm -f -- "$COPIE"
echo "5. restauration DEPUIS LA COPIE NOMMEE (jamais git checkout --)"
APRES=$(sha256sum "$FICHIER" | cut -d' ' -f1)
echo "6. sha256 APRES : $APRES"
[ "$AVANT" = "$APRES" ] && echo "   ✅ IDENTIQUE" || { echo "   🔴 DIVERGENT"; exit 4; }
PORCELAIN=$(git status --porcelain -- "$FICHIER")
echo "7. git status --porcelain : ${PORCELAIN:-<VIDE>}"
echo "   (un « M » ici ne dit PAS que la mutation a survecu : il dit que le"
echo "    fichier porte le correctif NON COMMITE que la rouge eprouve. Ce qui"
echo "    etablit la restauration est la ligne 6, et elle seule.)"
echo
