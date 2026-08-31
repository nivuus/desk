#!/usr/bin/env bash
# Lot 32T (E1) : LA fenetre unique — rouge sur le binaire d'aujourd'hui,
# deploiement, verte.
#
# 🔴 L'ATTENDU EST POSE AVANT LA MESURE, ET IL NE DERIVE D'AUCUN POINT MESURE.
# Il vient de deux lignes du journal du process VIVANT, relevees avant
# d'ouvrir le creneau :
#     duplication de sortie etablie  desktop_width=1860 desktop_height=1080
#     session NVENC native initialisee   largeur=1428 hauteur=1080
# Le juge est la PENTE, qui est independante de l'origine :
#     ecart(A->C) = (0.98 - 0.02) x largeur_de_reference
#     binaire d aujourd hui (sortie entiere) : 0.96 x 1860 = 1785.6 px
#     binaire E1            (image)          : 0.96 x 1428 = 1370.9 px
# En y les deux valent 1080, donc 0.96 x 1080 = 1036.8 px DANS LES DEUX BRAS :
# c'est le temoin negatif, a l'interieur du meme releve.
#
# ⚠️ C'est ce que le lot 32R n'avait PAS : il derivait son rectangle des trois
# points mesures, puis comparait les points a ce rectangle. Voir le piege
# << son attendu derive de la mesure elle-meme >> dans CLAUDE.md.
#
# ⚠️ LA FENETRE EST CONTINUE des la mesure rouge : le role `client` est
# EXCLUSIF par session, donc le pilote prend sa place des qu'il se connecte.
set -euo pipefail
unset -f chpwd 2>/dev/null || true
DESK=/home/mallanic/Projects/Nivuus/packages/desk
INST=$DESK/docs/superpowers/plans/journaux-lot32t/instrument
OUT=$DESK/docs/superpowers/plans/journaux-lot32t
CONS=/home/mallanic/Projects/Nivuus/packages/installer
W() { timeout 250 python3 "$CONS/console/guest/winrm_exec.py" ps "$1" 2>&1 | grep -v CLIXML | grep -v '^<Objs'; }
etape() { echo; echo "=== $(date -Is) $* ==="; }

# 🔴 LE PIEGE DU 404, PAYE DEUX FOIS (lot 32Q puis lot 32T).
# `(cd D && python3 -m http.server ... & echo $!)` met le `cd && python3`
# ENTIER en arriere-plan : `$!` est le PID du SOUS-SHELL, le kill le tue, et
# python survit. Le second serveur ne peut plus se lier au port, le PREMIER
# reste en place, et il rend 404 pour un fichier qu il n a pas -- ce qui se lit
# comme un depot rate alors que c est le SERVEUR qui est le mauvais.
# `--directory` supprime le `cd`, et `$!` designe alors python lui-meme.
# ⚠️ Le controle qui vaut reste la COMPARAISON DES DEUX SHA, imprimee plus bas :
# c est elle qui a attrape ce defaut les deux fois.
servir() {
  # Libere le port par PID RELEVE, jamais par motif : `pkill -f` depuis un
  # shell dont la ligne de commande porte le motif tue le shell (exit 144).
  # Un orphelin d une execution precedente est exactement ce qui a produit le
  # 404 : il repondait, mais depuis le mauvais repertoire.
  for pid in $(ss -ltnp 2>/dev/null | grep ':8099' | grep -o 'pid=[0-9]*' | cut -d= -f2 | sort -u); do
    echo "port 8099 tenu par le PID $pid, je le libere"; kill -9 "$pid" || true
  done
  sleep 1
  python3 -m http.server 8099 --bind 192.168.3.1 --directory "$1" >/dev/null 2>&1 &
  echo $! > /var/tmp/lot32t-http.pid
  sleep 2
  curl -sf -o /dev/null "http://192.168.3.1:8099/" || { echo "ERREUR : rien ne sert $1"; exit 1; }
}
arreter_le_service() {
  kill "$(cat /var/tmp/lot32t-http.pid)" 2>/dev/null || true
  for _ in 1 2 3 4 5; do curl -sf -o /dev/null --max-time 1 "http://192.168.3.1:8099/" || return 0; sleep 1; done
  echo "ERREUR : le serveur HTTP survit au kill"; exit 1
}

etape "0. PRE-VOL (hors creneau) : depot de la sonde, enregistrement de la tache"
servir "$INST"
W '
$ProgressPreference = "SilentlyContinue"
Invoke-WebRequest -Uri "http://192.168.3.1:8099/curseur.ps1" -OutFile "C:\nivuus\curseur.ps1" -UseBasicParsing
"sonde sha256 : " + (Get-FileHash C:\nivuus\curseur.ps1 -Algorithm SHA256).Hash
Set-Content -Path C:\nivuus\cur.cmd -Encoding ASCII -Value @("@echo off",
  "powershell.exe -NoProfile -ExecutionPolicy Bypass -File C:\nivuus\curseur.ps1 > C:\nivuus\cur.txt 2>&1")
schtasks /create /tn lot32t-cur /tr C:\nivuus\cur.cmd /sc once /st 00:00 /it /ru Administrator /rl HIGHEST /f | Out-Null
"tache enregistree"'
arreter_le_service
echo "sonde locale sha256 : $(sha256sum "$INST/curseur.ps1" | tr 'a-f' 'A-F' | cut -c1-64)"

mesurer() { # $1 = etiquette
  etape "MESURE DU CURSEUR — bras $1"
  W 'Remove-Item C:\nivuus\cur.txt -Force -ErrorAction SilentlyContinue
     schtasks /run /tn lot32t-cur | Out-Null
     "sonde lancee"'
  TMPDIR=/var/tmp node "$INST/pilote-curseur.mjs" --etiquette="$1" --sortie="$OUT/curseur-$1.json" || true
  W 'Start-Sleep -Seconds 8; Get-Content C:\nivuus\cur.txt -ErrorAction SilentlyContinue' \
    | tee "$OUT/curseur-$1.txt" | tail -25
}

# ── LA FENETRE COMMENCE ICI ────────────────────────────────────────────────
mesurer rouge-32q

etape "DEPLOIEMENT (binaire deja bati et verifie par sa chaine : voir le rapport)"
W '"agents AVANT : " + (@(Get-Process agent -ErrorAction SilentlyContinue).Count)'
servir /var/tmp/lot32-bin
W '
Stop-ScheduledTask -TaskName guacamole-agent -ErrorAction SilentlyContinue
Start-Sleep -Seconds 2
Get-Process agent -ErrorAction SilentlyContinue | ForEach-Object { Stop-Process -Id $_.Id -Force }
Start-Sleep -Seconds 5
Copy-Item C:\nivuus\agent\agent.exe C:\nivuus\agent\agent.exe.copie-nommee-avant-lot32t -Force
"copie nommee sha256 : " + (Get-FileHash C:\nivuus\agent\agent.exe.copie-nommee-avant-lot32t -Algorithm SHA256).Hash
$ProgressPreference = "SilentlyContinue"
Invoke-WebRequest -Uri "http://192.168.3.1:8099/agent.exe" -OutFile "C:\nivuus\agent\agent.exe" -UseBasicParsing
"deploye sha256 : " + (Get-FileHash C:\nivuus\agent\agent.exe -Algorithm SHA256).Hash
Remove-Item C:\nivuus\state\agent-session.txt -Force -ErrorAction SilentlyContinue
Start-ScheduledTask -TaskName guacamole-agent
Start-Sleep -Seconds 25
"agents : " + (@(Get-Process agent -ErrorAction SilentlyContinue).Count)
"SESSION (attestee par l appliance) : " + (Get-Content C:\nivuus\state\agent-session.txt -ErrorAction SilentlyContinue)'
arreter_le_service
echo "attendu (bati sur l hote) : $(sha256sum /var/tmp/lot32-bin/agent.exe | tr 'a-f' 'A-F' | cut -c1-64)"

mesurer verte-e1
# ── LA FENETRE SE TERMINE ICI ──────────────────────────────────────────────

etape "MENAGE"
W 'schtasks /delete /tn lot32t-cur /f 2>$null | Out-Null
   Remove-Item C:\nivuus\curseur.ps1,C:\nivuus\cur.cmd,C:\nivuus\cur.txt -Force -ErrorAction SilentlyContinue
   "taches lot32 restantes : " + (@(Get-ScheduledTask | Where-Object { $_.TaskName -match "lot32" })).Count'
virsh list --all 2>/dev/null | head -4
