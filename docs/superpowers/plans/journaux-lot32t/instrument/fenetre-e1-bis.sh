#!/usr/bin/env bash
# Batch 32T (E1), second window: the RED run first, by PUTTING BACK the named
# copy of the 32Q binary, then the GREEN run by redeploying E1.
#
# ⚠️ The order is the reverse of the first window because the deployment
# succeeded there whereas the red run, for its part, had failed on an instrument defect
# (`/applications` requires `?vm=`). The 32Q binary is not lost: it lives in
# `agent.exe.copie-nommee-avant-lot32t`, sha 03E2E752...
#
# 🔴 THE EXPECTATION, SET BEFORE THE MEASUREMENT AND DERIVED FROM NO MEASURED POINT:
#   captured output 1860x1080, encoded image 1428x1080 (two lines of the log
#   of the LIVE process, read before opening the slot).
#   Judge = the SLOPE, independent of the origin:
#     gap(A->C) = (0.98 - 0.02) x reference_width
#     32Q (whole output): 0.96 x 1860 = 1785.6 px
#     E1  (image)       : 0.96 x 1428 = 1370.9 px
#   In y: 0.96 x 1080 = 1036.8 px IN BOTH ARMS -- internal negative control.
set -euo pipefail
unset -f chpwd 2>/dev/null || true
DESK=/home/mallanic/Projects/Nivuus/packages/desk
INST=$DESK/docs/superpowers/plans/journaux-lot32t/instrument
OUT=$DESK/docs/superpowers/plans/journaux-lot32t
CONS=/home/mallanic/Projects/Nivuus/packages/installer
W() { timeout 250 python3 "$CONS/console/guest/winrm_exec.py" ps "$1" 2>&1 | grep -v CLIXML | grep -v '^<Objs'; }
etape() { echo; echo "=== $(date -Is) $* ==="; }
servir() {
  for pid in $(ss -ltnp 2>/dev/null | grep ':8099' | grep -o 'pid=[0-9]*' | cut -d= -f2 | sort -u); do
    echo "port 8099 tenu par le PID $pid, je le libere"; kill -9 "$pid" || true; done
  sleep 1
  python3 -m http.server 8099 --bind 192.168.3.1 --directory "$1" >/dev/null 2>&1 &
  echo $! > /var/tmp/lot32t-http.pid; sleep 2
  curl -sf -o /dev/null "http://192.168.3.1:8099/" || { echo "ERREUR : rien ne sert $1"; exit 1; }
}
arreter_le_service() { kill -9 "$(cat /var/tmp/lot32t-http.pid)" 2>/dev/null || true; sleep 1; }

etape "PRE-VOL : depot de la sonde et de la tache"
servir "$INST"
W '$ProgressPreference = "SilentlyContinue"
Invoke-WebRequest -Uri "http://192.168.3.1:8099/curseur.ps1" -OutFile "C:\nivuus\curseur.ps1" -UseBasicParsing
Set-Content -Path C:\nivuus\cur.cmd -Encoding ASCII -Value @("@echo off",
  "powershell.exe -NoProfile -ExecutionPolicy Bypass -File C:\nivuus\curseur.ps1 > C:\nivuus\cur.txt 2>&1")
schtasks /create /tn lot32t-cur /tr C:\nivuus\cur.cmd /sc once /st 00:00 /it /ru Administrator /rl HIGHEST /f | Out-Null
"sonde sha256 : " + (Get-FileHash C:\nivuus\curseur.ps1 -Algorithm SHA256).Hash'
arreter_le_service

# $1 = source du binaire a poser (chemin VM), $2 = sha attendu
poser() {
  W "
Stop-ScheduledTask -TaskName guacamole-agent -ErrorAction SilentlyContinue
Start-Sleep -Seconds 2
Get-Process agent -ErrorAction SilentlyContinue | ForEach-Object { Stop-Process -Id \$_.Id -Force }
Start-Sleep -Seconds 5
Copy-Item '$1' C:\nivuus\agent\agent.exe -Force
'pose sha256 : ' + (Get-FileHash C:\nivuus\agent\agent.exe -Algorithm SHA256).Hash
Remove-Item C:\nivuus\state\agent-session.txt -Force -ErrorAction SilentlyContinue
Start-ScheduledTask -TaskName guacamole-agent
Start-Sleep -Seconds 25
'agents : ' + (@(Get-Process agent -ErrorAction SilentlyContinue).Count)
'SESSION (attestee par l appliance) : ' + (Get-Content C:\nivuus\state\agent-session.txt -ErrorAction SilentlyContinue)"
  echo "attendu : $2"
}

mesurer() {
  etape "MESURE DU CURSEUR — bras $1"
  W 'Remove-Item C:\nivuus\cur.txt -Force -ErrorAction SilentlyContinue
     schtasks /run /tn lot32t-cur | Out-Null; "sonde lancee"'
  TMPDIR=/var/tmp node "$INST/pilote-curseur.mjs" --etiquette="$1" --sortie="$OUT/curseur-$1.json" || true
  W 'Start-Sleep -Seconds 10; Get-Content C:\nivuus\cur.txt -ErrorAction SilentlyContinue' \
    | tee "$OUT/curseur-$1.txt" | tail -30
}

# ── FENETRE ────────────────────────────────────────────────────────────────
etape "RETOUR AU BINAIRE 32Q (la copie nommee) pour la ROUGE"
poser 'C:\nivuus\agent\agent.exe.copie-nommee-avant-lot32t' '03E2E752A4645AA2DD80BF5FB1C21ABDA5731DC41BBEEE9C5F71A4F791E4BED1'
mesurer rouge-32q

etape "REDEPLOIEMENT DE E1"
servir /var/tmp/lot32-bin
W '$ProgressPreference = "SilentlyContinue"
Stop-ScheduledTask -TaskName guacamole-agent -ErrorAction SilentlyContinue
Start-Sleep -Seconds 2
Get-Process agent -ErrorAction SilentlyContinue | ForEach-Object { Stop-Process -Id $_.Id -Force }
Start-Sleep -Seconds 5
Invoke-WebRequest -Uri "http://192.168.3.1:8099/agent.exe" -OutFile "C:\nivuus\agent\agent.exe" -UseBasicParsing
"deploye sha256 : " + (Get-FileHash C:\nivuus\agent\agent.exe -Algorithm SHA256).Hash
Remove-Item C:\nivuus\state\agent-session.txt -Force -ErrorAction SilentlyContinue
Start-ScheduledTask -TaskName guacamole-agent
Start-Sleep -Seconds 25
"agents : " + (@(Get-Process agent -ErrorAction SilentlyContinue).Count)
"SESSION (attestee par l appliance) : " + (Get-Content C:\nivuus\state\agent-session.txt -ErrorAction SilentlyContinue)'
arreter_le_service
echo "attendu : $(sha256sum /var/tmp/lot32-bin/agent.exe | tr 'a-f' 'A-F' | cut -c1-64)"
mesurer verte-e1
# ── FIN ────────────────────────────────────────────────────────────────────

etape "MENAGE"
W 'schtasks /delete /tn lot32t-cur /f 2>$null | Out-Null
   Remove-Item C:\nivuus\curseur.ps1,C:\nivuus\cur.cmd,C:\nivuus\cur.txt -Force -ErrorAction SilentlyContinue
   "taches lot32 restantes : " + (@(Get-ScheduledTask | Where-Object { $_.TaskName -match "lot32" })).Count'
virsh list --all 2>/dev/null | head -4
