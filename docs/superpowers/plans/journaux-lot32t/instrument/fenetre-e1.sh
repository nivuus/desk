#!/usr/bin/env bash
# Batch 32T (E1): THE single window — red run on today's binary,
# deployment, green run.
#
# 🔴 THE EXPECTATION IS SET BEFORE THE MEASUREMENT, AND IT DERIVES FROM NO MEASURED POINT.
# It comes from two lines of the log of the LIVE process, read before
# opening the slot:
#     output duplication established  desktop_width=1860 desktop_height=1080
#     native NVENC session initialised   width=1428 height=1080
# The judge is the SLOPE, which is independent of the origin:
#     gap(A->C) = (0.98 - 0.02) x reference_width
#     today's binary (whole output): 0.96 x 1860 = 1785.6 px
#     E1 binary      (image)       : 0.96 x 1428 = 1370.9 px
# In y both are 1080, so 0.96 x 1080 = 1036.8 px IN BOTH ARMS:
# it is the negative control, inside the same reading.
#
# ⚠️ It is what batch 32R did NOT have: it derived its rectangle from the three
# measured points, then compared the points with that rectangle. See the trap
# << its expectation derived from the measurement itself >> in CLAUDE.md.
#
# ⚠️ THE WINDOW IS CONTINUOUS from the red measurement: the `client` role is
# EXCLUSIVE per session, so the driver takes its place as soon as it connects.
set -euo pipefail
unset -f chpwd 2>/dev/null || true
DESK=/home/mallanic/Projects/Nivuus/packages/desk
INST=$DESK/docs/superpowers/plans/journaux-lot32t/instrument
OUT=$DESK/docs/superpowers/plans/journaux-lot32t
CONS=/home/mallanic/Projects/Nivuus/packages/installer
W() { timeout 250 python3 "$CONS/console/guest/winrm_exec.py" ps "$1" 2>&1 | grep -v CLIXML | grep -v '^<Objs'; }
etape() { echo; echo "=== $(date -Is) $* ==="; }

# 🔴 THE 404 TRAP, PAID TWICE (batch 32Q then batch 32T).
# `(cd D && python3 -m http.server ... & echo $!)` puts the WHOLE `cd && python3`
# in the background: `$!` is the PID of the SUBSHELL, the kill kills it, and
# python survives. The second server can no longer bind to the port, the FIRST
# stays in place, and it returns 404 for a file it does not have -- which reads
# as a failed upload whereas it is the SERVER that is the wrong one.
# `--directory` removes the `cd`, and `$!` then designates python itself.
# ⚠️ The check that counts remains COMPARING THE TWO SHAs, printed below:
# it is what caught this defect both times.
servir() {
  # Frees the port by READ PID, never by pattern: `pkill -f` from a
  # shell whose command line carries the pattern kills the shell (exit 144).
  # An orphan of a previous run is exactly what produced the
  # 404: it answered, but from the wrong directory.
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

# ── THE WINDOW STARTS HERE ────────────────────────────────────────────────
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
# ── THE WINDOW ENDS HERE ──────────────────────────────────────────────

etape "MENAGE"
W 'schtasks /delete /tn lot32t-cur /f 2>$null | Out-Null
   Remove-Item C:\nivuus\curseur.ps1,C:\nivuus\cur.cmd,C:\nivuus\cur.txt -Force -ErrorAction SilentlyContinue
   "taches lot32 restantes : " + (@(Get-ScheduledTask | Where-Object { $_.TaskName -match "lot32" })).Count'
virsh list --all 2>/dev/null | head -4
