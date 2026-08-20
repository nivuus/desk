#!/usr/bin/env bash
# Joue le rouge (i) du Step 6 et attend son verdict.
set -uo pipefail
RACINE=/home/mallanic/Projects/Guacamole
set -a; source "$RACINE/.env"; set +a
rm -f /media/vm/dev/rouge-i.txt

# Le PID du pont, RELEVE sur la derniere ligne `pont fichiers lancé pid=` du
# journal. ⚠️ La chaine est bien `pont fichiers lancé`, PAS `pont lancé` que le
# plan de F1 prescrit au Step 4 : ce dernier ne matche AUCUNE trace et rendrait
# 0, ce qui se lirait comme un pont absent.
PID_PONT=$(sed 's/\x1b\[[0-9;]*m//g' /media/vm/dev/agent.log \
    | grep -a 'pont fichiers lancé pid=' | tail -1 \
    | sed 's/.*pid=\([0-9]*\).*/\1/')
if [ -z "$PID_PONT" ]; then echo '# AUCUN pid de pont dans le journal'; exit 1; fi
echo "# pid du pont releve dans agent.log : $PID_PONT"
printf '%s' "$PID_PONT" > /media/vm/dev/pid-pont.txt
# ⚠️ APPEL DIRECT, EN ARRIERE-PLAN -- PAS de `Start-Process`.
# `Start-Process` lance depuis un PowerShell invoque par WinRM s'est revele
# NON FIABLE : il rend bien un PID, et le script cible ne tourne pas (aucun
# fichier de sortie cree). Mesure : le meme script invoque DIRECTEMENT par
# `-File` ecrit son fichier immediatement. On lance donc directement, en
# arriere-plan, et on attend le FAIT -- l'appel WinRM peut tres bien rendre la
# main avant la fin du script, cela n'a aucune importance ici.
node "$RACINE/scripts/winrm.js" \
  'powershell -NoProfile -ExecutionPolicy Bypass -File C:\dev\rouge-i.ps1' >/dev/null 2>&1 &
for _ in $(seq 1 60); do
    grep -aq 'FIN ROUGE I' /media/vm/dev/rouge-i.txt 2>/dev/null && break
    sleep 3
done
cat /media/vm/dev/rouge-i.txt 2>/dev/null || echo '# aucun fichier'
