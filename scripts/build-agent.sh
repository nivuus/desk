#!/usr/bin/env bash
# Synchronise puis compile l'agent sur la VM Windows.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
"$ROOT/scripts/sync-agent.sh"

PROFILE="${1:-debug}"
FLAG=""
[ "$PROFILE" = "release" ] && FLAG="--release"

node "$ROOT/scripts/winrm.js" \
    "\$env:Path += ';C:\\Users\\Administrateur\\.cargo\\bin'; Set-Location C:\\dev; cargo build $FLAG 2>&1 | Out-String"
