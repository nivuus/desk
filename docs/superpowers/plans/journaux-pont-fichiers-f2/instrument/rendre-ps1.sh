#!/usr/bin/env bash
# Rend LOCALEMENT le `.ps1` que `scripts/run-agent.sh` écrit sur la VM.
#
# 🔴 CE N'EST PAS UNE RÉÉCRITURE DU HEREDOC : il est EXTRAIT du script réel, de
# la ligne `cat > /media/vm/dev/run-agent.ps1 <<PS1` jusqu'à son `PS1` de
# fermeture, et évalué tel quel. Une copie recopiée à la main éprouverait la
# copie, pas le script — c'est exactement le patron du `TYPES_AGENT` écrit à la
# main que ce dépôt paie depuis P1.
#
# ⚠️ CE QUE CELA N'ÉTABLIT PAS : que le fichier ARRIVE sur la VM, ni que la
# tâche planifiée le lise. Cela n'établit que la SUBSTITUTION du shell — c'est
# à dire précisément le maillon oublié en D1, D2 et D7, où « la valeur ne
# pouvait simplement pas atteindre le processus ».
set -euo pipefail
racine="$(cd "$(dirname "$0")/../../../../.." && pwd)"
script="$racine/scripts/run-agent.sh"
sortie="${1:?usage: rendre-ps1.sh <fichier de sortie>}"

debut=$(grep -n '^cat > /media/vm/dev/run-agent.ps1 <<PS1$' "$script" | cut -d: -f1)
fin=$(awk -v d="$debut" 'NR>d && $0=="PS1"{print NR; exit}' "$script")
[ -n "$debut" ] && [ -n "$fin" ] || { echo "heredoc introuvable"; exit 2; }

{
  echo 'set -u'
  # Les variables que le heredoc lit sans défaut, posées à vide : le script
  # réel les tient de l'environnement de l'opérateur.
  echo 'AGENT_EXE="C:\\dev\\target\\release\\agent.exe"'
  sed -n "${debut},${fin}p" "$script" | sed "1s|/media/vm/dev/run-agent.ps1|$sortie|"
} > /tmp/f2-heredoc.sh
bash /tmp/f2-heredoc.sh
