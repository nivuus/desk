#!/usr/bin/env bash
# Batch 32Q: THE single window — measurement before, deployment, measurement after.
#
# 🔴 WHAT IS OUTSIDE THE SLOT, AND WHY. The cross-build and the
# upload through the hook do NOT touch the VM: they happen BEFORE, and the
# owner knows nothing about them. Putting them in the slot would cost them two
# minutes for nothing.
#
# ⚠️ WHAT INTERRUPTS THE OWNER STARTS FROM THE MEASUREMENT BEFORE, and not
# at the restart: the `client` role is EXCLUSIVE per session (established in batch 22),
# so the driver takes their place as soon as it connects. The window is therefore
# CONTINUOUS, from the first measurement to the last.
#
# ⚠️ The BEFORE arm only exists once: it disappears at the restart.
# Do not reverse the order, do not skip it.
set -euo pipefail
unset -f chpwd 2>/dev/null || true
DESK=/home/mallanic/Projects/Nivuus/packages/desk
INST=$DESK/docs/superpowers/plans/journaux-lot32q/instrument
CONS=/home/mallanic/Projects/Nivuus/packages/installer
W() { timeout 250 python3 "$CONS/console/guest/winrm_exec.py" ps "$1" 2>&1 | grep -v CLIXML | grep -v '^<Objs'; }

etape() { echo; echo "=== $* ==="; }

etape "0. PRE-VOL (hors creneau) : la sonde est deposee, la tache enregistree"
(cd "$INST" && python3 -m http.server 8099 --bind 192.168.3.1 >/dev/null 2>&1 & echo $! > /var/tmp/lot32q-http.pid); sleep 2
W '
$ProgressPreference = "SilentlyContinue"
Invoke-WebRequest -Uri "http://192.168.3.1:8099/curseur.ps1" -OutFile "C:\nivuus\curseur.ps1" -UseBasicParsing
Set-Content -Path C:\nivuus\cur.cmd -Encoding ASCII -Value @("@echo off",
  "powershell.exe -NoProfile -ExecutionPolicy Bypass -File C:\nivuus\curseur.ps1 > C:\nivuus\cur.txt 2>&1")
schtasks /create /tn lot32q-cur /tr C:\nivuus\cur.cmd /sc once /st 00:00 /it /ru Administrator /rl HIGHEST /f | Out-Null
"sonde deposee"'
kill "$(cat /var/tmp/lot32q-http.pid)" 2>/dev/null || true

mesurer() { # $1 = etiquette
  etape "MESURE DU CURSEUR — bras $1"
  W 'Remove-Item C:\nivuus\cur.txt -Force -ErrorAction SilentlyContinue
     schtasks /run /tn lot32q-cur | Out-Null
     "sonde lancee (40 s)"'
  TMPDIR=/var/tmp node "$INST/pilote-curseur.mjs" --etiquette="$1" \
    --sortie="$DESK/docs/superpowers/plans/journaux-lot32q/curseur-$1.json" || true
  W 'Start-Sleep -Seconds 8; Get-Content C:\nivuus\cur.txt -ErrorAction SilentlyContinue' \
    | tee "/var/tmp/curseur-$1.txt" | tail -20
}

# ── THE WINDOW STARTS HERE ────────────────────────────────────────────────
mesurer avant

etape "DEPLOIEMENT (le binaire est deja bati et depose : voir 0.)"
W '"agents AVANT : " + (@(Get-Process agent -ErrorAction SilentlyContinue).Count)'
(cd /var/tmp/lot32-bin && python3 -m http.server 8099 --bind 192.168.3.1 >/dev/null 2>&1 & echo $! > /var/tmp/lot32q-http.pid); sleep 2
W '
Stop-ScheduledTask -TaskName guacamole-agent -ErrorAction SilentlyContinue
Start-Sleep -Seconds 2
Get-Process agent -ErrorAction SilentlyContinue | ForEach-Object { Stop-Process -Id $_.Id -Force }
Start-Sleep -Seconds 5
Copy-Item C:\nivuus\agent\agent.exe C:\nivuus\agent\agent.exe.copie-nommee-avant-lot32q -Force
"copie nommee sha256 : " + (Get-FileHash C:\nivuus\agent\agent.exe.copie-nommee-avant-lot32q -Algorithm SHA256).Hash
$ProgressPreference = "SilentlyContinue"
Invoke-WebRequest -Uri "http://192.168.3.1:8099/agent.exe" -OutFile "C:\nivuus\agent\agent.exe" -UseBasicParsing
"deploye sha256 : " + (Get-FileHash C:\nivuus\agent\agent.exe -Algorithm SHA256).Hash
Remove-Item C:\nivuus\state\agent-session.txt -Force -ErrorAction SilentlyContinue
Start-ScheduledTask -TaskName guacamole-agent
Start-Sleep -Seconds 20
"agents : " + (@(Get-Process agent -ErrorAction SilentlyContinue).Count)
"SESSION (attestee par l appliance) : " + (Get-Content C:\nivuus\state\agent-session.txt -ErrorAction SilentlyContinue)'
kill "$(cat /var/tmp/lot32q-http.pid)" 2>/dev/null || true
echo "attendu : $(sha256sum /var/tmp/lot32-bin/agent.exe | tr 'a-f' 'A-F' | cut -c1-64)"

mesurer apres
# ── THE WINDOW ENDS HERE ──────────────────────────────────────────────

etape "MENAGE"
W 'schtasks /delete /tn lot32q-cur /f 2>$null | Out-Null
   Remove-Item C:\nivuus\curseur.ps1,C:\nivuus\cur.cmd,C:\nivuus\cur.txt -Force -ErrorAction SilentlyContinue
   "taches lot32 restantes : " + (@(Get-ScheduledTask | Where-Object { $_.TaskName -match "lot32" })).Count'
virsh list --all 2>/dev/null | head -4
