#!/usr/bin/env bash
# Synchronise les sources Rust vers C:\dev (monté sur /media/vm) pour
# compilation sur Windows.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DEST="/media/vm/dev"

if ! mountpoint -q /media/vm; then
    echo "erreur : /media/vm n'est pas monté" >&2
    exit 1
fi

# On ne copie que ce que git suit sous les chemins utiles à la compilation
# Rust : une liste d'inclusion ciblant ce qui doit traverser vers la VM,
# plutôt qu'une énumération de ce qui ne doit pas l'être. Ce choix est
# délibérément robuste aux futurs ajouts dans l'arbre — `node_modules/`,
# `target/`, `dist/`, `.git/` — puisqu'aucun n'est jamais suivi par git
# (le premier par .gitignore, les autres par nature) et donc jamais listé
# par `git ls-files`.
#
# Historique : la version précédente rsync-ait `proto/` en entier, y compris
# `proto/node_modules/` (outillage JS des tests de vecteurs partagés). Le
# montage CIFS vers la VM ne sait pas poser d'horodatage sur les liens
# symboliques de `node_modules/.bin/` (`Operation not supported`, errno 95),
# ce qui faisait sortir rsync en code 23 et interrompait ce script avant
# même d'atteindre l'étape de compilation.
mkdir -p "$DEST"

# On repart d'un arbre propre côté VM pour les répertoires synchronisés :
# ça élimine tout résidu d'une exécution précédente (comme l'ancien
# `proto/node_modules/`) sans dépendre des subtilités de `rsync --delete`
# combiné à `--files-from`.
rm -rf "$DEST/agent" "$DEST/proto"

cd "$ROOT"

# Garde-fou : un fichier créé sous agent/ ou proto/ puis jamais `git add`é
# n'apparaît pas dans `git ls-files` et serait donc silencieusement absent de
# la VM — on compilerait un état du code différent de celui qu'on a sous les
# yeux, sans le moindre avertissement. On avertit plutôt que de bloquer : un
# brouillon non indexé peut être testé volontairement, mais pas sans le savoir.
untracked="$(git ls-files --others --exclude-standard -- agent proto)"
if [ -n "$untracked" ]; then
    echo "attention : fichiers non suivis par git sous agent/ ou proto/ — ils ne seront PAS synchronisés vers la VM :" >&2
    echo "$untracked" | sed 's/^/  /' >&2
    echo "  (lancez « git add » si ces fichiers doivent faire partie de la compilation Windows)" >&2
fi

git ls-files -z -- Cargo.toml Cargo.lock rust-toolchain.toml agent proto |
    rsync -a --from0 --files-from=- "$ROOT/" "$DEST/"

echo "sources synchronisées vers $DEST"
