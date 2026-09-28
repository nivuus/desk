#!/usr/bin/env bash
# Harnais de rouge du bloc E3.
#
# Doctrine appliquée, et chaque clause a coûté une rouge restée verte dans ce
# dépôt :
#
#  - on compare à une COPIE NOMMÉE, jamais à HEAD. Un contrôle fondé sur
#    `git diff` contre HEAD ne distingue pas « la mutation n'a rien changé » de
#    « le fichier était déjà modifié » ;
#  - `git checkout --` restaure HEAD, PAS l'état d'avant. Sur un arbre partagé
#    il détruirait le travail non commité. La restauration se fait depuis la
#    copie nommée, et son empreinte est vérifiée ;
#  - ROUGE 0 : une mutation qui ne change RIEN doit être REFUSÉE. C'est le
#    contrôle du harnais lui-même ; sans lui, aucune autre rouge ne compte.
#
# Usage : rouge.sh <étiquette> <fichier> <script-de-mutation> <commande-de-verif>
#   le script de mutation reçoit le chemin du fichier en $1.

set -uo pipefail

etiquette="${1:?étiquette}"
fichier="${2:?fichier}"
mutation="${3:?script de mutation}"
verif="${4:?commande de vérification}"

racine="$(cd "$(dirname "$0")/../../../../.." && pwd)"
cd "$racine" || exit 2

# 🔴 Le chemin est rendu ABSOLU, et la vérification tourne dans un SOUS-SHELL.
# Défaut trouvé PAR L'EXÉCUTION, à la rouge R2b : sa commande de vérification
# commençait par `cd client`, le `cd` a survécu à la commande, et la
# restauration par chemin relatif a échoué — « Aucun fichier ou dossier de ce
# nom ». La copie nommée était intacte et le fichier a été restauré à la main,
# empreinte vérifiée ; le harnais, lui, avait un mode de panne qui laissait
# une mutation dans l'arbre en annonçant un verdict.
fichier="$(readlink -f "$fichier")"

copie="$(mktemp "/tmp/rouge-${etiquette}-XXXXXX")"
cp "$fichier" "$copie"
avant="$(sha256sum < "$copie" | cut -d' ' -f1)"

echo "=== ROUGE ${etiquette} ==="
echo "fichier      : ${fichier}"
echo "copie nommée : ${copie}"
echo "sha256 avant : ${avant}"

restaurer() {
    cp "$copie" "$fichier"
    apres="$(sha256sum < "$fichier" | cut -d' ' -f1)"
    echo "sha256 après restauration : ${apres}"
    if [ "$avant" = "$apres" ]; then
        echo "restauration : IDENTIQUE"
    else
        echo "restauration : 🔴 DIVERGENTE — intervenir à la main"
    fi
    rm -f "$copie"
}
trap restaurer EXIT

bash "$mutation" "$fichier"

# ROUGE 0 : la mutation a-t-elle réellement changé le fichier ?
lignes="$(diff <(cat "$copie") <(cat "$fichier") | grep -c '^[<>]' || true)"
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
