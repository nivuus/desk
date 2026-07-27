#!/usr/bin/env bash
# Copie les sources Rust vers C:\dev (monté sur /media/vm) pour compilation sur Windows.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DEST="/media/vm/dev"

if ! mountpoint -q /media/vm; then
    echo "erreur : /media/vm n'est pas monté" >&2
    exit 1
fi

mkdir -p "$DEST"
rsync -a --delete \
    --exclude 'target/' \
    "$ROOT/Cargo.toml" "$ROOT/rust-toolchain.toml" \
    "$ROOT/proto" "$ROOT/agent" \
    "$DEST/"

echo "sources synchronisées vers $DEST"
