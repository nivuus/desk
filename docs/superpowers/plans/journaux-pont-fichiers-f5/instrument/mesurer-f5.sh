#!/usr/bin/env bash
# Copie la mesure sur la VM, la lance PAR TÂCHE PLANIFIÉE /it, et rend son JSON.
# Modelé sur `mesurer-f3.sh`, dont il reprend les DEUX pièges payés là-bas :
#   - tuer les mesures figées d'abord, PAR UN FICHIER et jamais en ligne
#     (`nodejs-winrm` enveloppe dans `powershell -Command "& { … }"`, et un
#     script inline à guillemets doubles NE TOURNE JAMAIS, en silence) ;
#   - un chemin de sortie NEUF à chaque exécution, qu'aucun processus figé ne
#     peut tenir.
set -uo pipefail
PHASE="${1:-lister}"
RACINE=/home/mallanic/Projects/Guacamole
I="$RACINE/docs/superpowers/plans/journaux-pont-fichiers-f5/instrument"
set -a; source "$RACINE/.env"; set +a

cat > /media/vm/dev/tuer-mesures-f5.ps1 <<'PSKILL'
schtasks /end /tn mesure-f5 2>$null | Out-Null
Get-CimInstance Win32_Process | Where-Object { $_.CommandLine -like '*mesure-f5*' } |
    ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }
PSKILL
node "$RACINE/scripts/winrm.js" 'powershell -NoProfile -ExecutionPolicy Bypass -File C:\dev\tuer-mesures-f5.ps1' >/dev/null 2>&1

cp "$I/mesurer-f5.ps1" /media/vm/dev/mesurer-f5.ps1
HORO=$(date +%H%M%S%N)
SORTIE="C:\\dev\\mesure-f5-$HORO.json"
LOCAL="/media/vm/dev/mesure-f5-$HORO.json"
rm -f "$LOCAL"
cat > /media/vm/dev/lancer-mesure-f5.ps1 <<PS1
& 'C:\dev\mesurer-f5.ps1' -sortie '$SORTIE' -phase '$PHASE' *>&1 | Out-File -FilePath 'C:\dev\mesure-f5.trace.txt' -Encoding utf8
PS1

node "$RACINE/scripts/winrm.js" \
  "schtasks /delete /tn mesure-f5 /f 2>\$null; \
   schtasks /create /tn mesure-f5 /f /it /ru '$WINDOWS_ADMIN_USERNAME' /rp '$WINDOWS_ADMIN_PASSWORD' \
     /sc once /st 00:00 \
     /tr 'powershell -WindowStyle Hidden -NoProfile -ExecutionPolicy Bypass -File C:\dev\lancer-mesure-f5.ps1'; \
   schtasks /run /tn mesure-f5" >/dev/null 2>&1

# Attendre le FAIT, jamais une durée. Borne : DELAI_LISTER vaut 20 s côté pont,
# et un listage au-delà du mur gèle exactement cette durée — la borne doit donc
# être PLUSIEURS FOIS plus longue, jamais calée dessus.
for i in $(seq 1 60); do
    [ -s "$LOCAL" ] && { cat "$LOCAL"; exit 0; }
    sleep 2
done
echo '{"erreur":"mesure-f5 absente au terme de 120 s"}'
