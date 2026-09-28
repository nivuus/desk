#!/usr/bin/env bash
# Renders LOCALLY the `.ps1` that `scripts/run-agent.sh` writes on the VM.
#
# 🔴 IT IS NOT A REWRITE OF THE HEREDOC: it is EXTRACTED from the real script, from
# the line `cat > /media/vm/dev/run-agent.ps1 <<PS1` up to its closing `PS1`,
# and evaluated as is. A hand-copied copy would test the
# copy, not the script — it is exactly the pattern of the hand-written `TYPES_AGENT`
# this repository has been paying for since P1.
#
# ⚠️ WHAT THIS DOES NOT ESTABLISH: that the file ARRIVES on the VM, nor that the
# scheduled task reads it. It only establishes the shell SUBSTITUTION — that
# is, precisely the link forgotten in D1, D2 and D7, where "the value simply
# could not reach the process".
set -euo pipefail
racine="$(cd "$(dirname "$0")/../../../../.." && pwd)"
script="$racine/scripts/run-agent.sh"
sortie="${1:?usage: rendre-ps1.sh <fichier de sortie>}"

debut=$(grep -n '^cat > /media/vm/dev/run-agent.ps1 <<PS1$' "$script" | cut -d: -f1)
fin=$(awk -v d="$debut" 'NR>d && $0=="PS1"{print NR; exit}' "$script")
[ -n "$debut" ] && [ -n "$fin" ] || { echo "heredoc introuvable"; exit 2; }

{
  echo 'set -u'
  # The variables the heredoc reads without a default, set empty: the real
  # script gets them from the operator's environment.
  echo 'AGENT_EXE="C:\\dev\\target\\release\\agent.exe"'
  sed -n "${debut},${fin}p" "$script" | sed "1s|/media/vm/dev/run-agent.ps1|$sortie|"
} > /tmp/f2-heredoc.sh
bash /tmp/f2-heredoc.sh
