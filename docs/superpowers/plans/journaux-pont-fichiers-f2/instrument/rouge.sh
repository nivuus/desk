#!/usr/bin/env bash
# Harnais de mutation. Il REFUSE de compter une rouge dont le diff est VIDE.
#
# 🔴 POURQUOI CE GARDE EXISTE : le sous-bloc S3 a relevé que QUATRE rouges sur
# seize de son prédécesseur ne prouvaient rien — deux rougissaient sur une
# clause préexistante, deux n'avaient rien muté du tout (`exit=1` lu comme un
# succès de la rouge). Et le sous-bloc P4 a relevé qu'une mutation par
# substitution de chaîne frappe LE COMMENTAIRE avant le code, dans un dépôt qui
# commente ses invariants : plus un invariant est documenté, plus sa rouge est
# fragile. D'où la mutation par NUMÉRO DE LIGNE (`sed -i '<n>s/…/…/'`), jamais
# par la seule sous-chaîne, et d'où ce garde.
#
# 🔴 ET POURQUOI LA SAUVEGARDE EST UNE COPIE, JAMAIS `git checkout --`.
# Payé sur place, le 20 août 2026 : une première rédaction de ce harnais
# restaurait par `git checkout -- <fichier>`, ce qui a effacé le travail NON
# COMMITÉ de la tâche en cours. Deux défauts en un :
#   ① `git checkout` restaure HEAD, pas l'état d'avant la mutation ;
#   ② `git diff` contre HEAD est NON VIDE dès que le fichier porte du travail
#      en cours — le garde du diff vide était donc VACUEUX, exactement le
#      patron qu'il existe pour interdire.
# La référence est une copie prise à l'instant, et rien d'autre.
#
# Usage : rouge.sh <étiquette> <fichier> <commande sed> -- <commande de contrôle…>
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

# 🔴 LE GARDE. Un diff vide veut dire que la mutation N'A RIEN MUTÉ : le
# contrôle qui suit ne dirait alors rien du produit.
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

# 🔴 `cp` SANS `-p`, ET C'EST LE CONTRAIRE DE CE QU'ON ÉCRIT SPONTANÉMENT.
# Payé sur place, le 20 août 2026, sur `proto/src/fichiers.rs` : `cp -p`
# préserve la date de modification, si bien que le fichier RESTAURÉ paraît
# INCHANGÉ à cargo — qui garde alors l'artefact compilé de la version MUTÉE.
# Le symptôme est un test correct qui échoue pour une raison INVISIBLE DANS LA
# SOURCE : ici, `charge.len() > TAILLE_TRAME_MAX` refusait une charge de 65536
# contre un maximum de 65536, ce qu'aucune lecture du fichier ne peut
# expliquer. Le pire cas est l'inverse : une suite VERTE exécutant encore le
# code muté.
cp "$sauvegarde" "$fichier"; rm -f "$sauvegarde"
apres="$(sha256sum "$fichier" | cut -d' ' -f1)"
echo "--- sha256 APRÈS restauration : $apres"
if [ "$avant" = "$apres" ]; then echo "--- restauration VÉRIFIÉE"; else echo "!!! RESTAURATION FAUSSE"; exit 4; fi
echo
