#!/usr/bin/env bash
# Synchronise puis compile l'agent sur la VM Windows.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
"$ROOT/scripts/sync-agent.sh"

# Le quota WinRM par shell (MaxMemoryPerShellMB) vaut 1024 Mo par défaut sur
# Windows Server. C'est trop juste pour le codegen LLVM parallèle de rustc
# lors de la compilation des crates proc-macro (proc-macro2, parking_lot_core
# notamment) : le build échoue alors avec une erreur qui ne mentionne jamais
# la mémoire — `STATUS_STACK_BUFFER_OVERRUN (0xc0000409)`, qui ressemble à
# une corruption de pile mais vient en réalité du Job Object WinRM trop
# contraint. On relève le quota une fois pour toutes, de façon idempotente :
# on lit la valeur courante et on ne la modifie que si elle est insuffisante,
# jamais à la baisse.
MIN_SHELL_MB=4096
CURRENT_SHELL_MB="$(node "$ROOT/scripts/winrm.js" \
    '(Get-Item WSMan:\localhost\Shell\MaxMemoryPerShellMB).Value' 2>/dev/null | tr -dc '0-9')"
if ! [[ "$CURRENT_SHELL_MB" =~ ^[0-9]+$ ]] || [ "$CURRENT_SHELL_MB" -lt "$MIN_SHELL_MB" ]; then
    echo "quota WinRM MaxMemoryPerShellMB insuffisant (${CURRENT_SHELL_MB:-inconnu} Mo) : relèvement à ${MIN_SHELL_MB} Mo" >&2
    node "$ROOT/scripts/winrm.js" \
        "Set-Item -Path WSMan:\\localhost\\Shell\\MaxMemoryPerShellMB -Value $MIN_SHELL_MB" >/dev/null
else
    echo "quota WinRM MaxMemoryPerShellMB déjà suffisant (${CURRENT_SHELL_MB} Mo)" >&2
fi

# `release` par défaut : les mesures du jalon 1 (débit ~30 i/s, latence
# médiane 276 ms) ont toutes été prises sur un binaire `debug`, sans que ce
# soit un choix — c'était simplement la valeur par défaut de ce script, et
# `run-agent.sh` lançait le chemin `target\debug` en dur. Passer `debug` en
# premier argument reste possible pour déboguer.
PROFILE="${1:-release}"
FLAG=""
[ "$PROFILE" = "release" ] && FLAG="--release"

node "$ROOT/scripts/winrm.js" \
    "\$env:Path += ';C:\\Users\\Administrateur\\.cargo\\bin'; Set-Location C:\\dev; cargo build $FLAG 2>&1 | Out-String"
