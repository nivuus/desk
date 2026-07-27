#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
node "$ROOT/scripts/winrm.js" \
    "schtasks /end /tn guacamole-agent 2>\$null; \
     Stop-Process -Name agent -Force -ErrorAction SilentlyContinue; 'arrêté'"
