#!/usr/bin/env bash
# Lot 3, item 8 (3.2) — LE PROPRIETAIRE MONO-FENETRE.
#
#     jouer-item8.sh <bras>        bras : temoin | mono | rouge
#
# THE QUESTION: in SINGLE-WINDOW mode, do the clipboard and the accent
# still have an owner? The legacy item asserts they do not.
#
# 🔴 THE CONTROL HAS TWO SIDES, AND IT IS WHAT GIVES THE ZERO ITS MEANING:
#   - `temoin` arm: product AS DELIVERED (SUPERVISEUR=1). The same gesture must return
#     a NON-ZERO count. Without it, the `mono` arm's zero would be returned
#     identically by an instrument that cannot see these messages.
#   - `mono` arm:   SUPERVISEUR=0 CAPTEUR=0. It is the item's answer.
#   - `rouge` arm:  delivered product + PRESSE_PAPIER=0 ACCENT=0. The zero must
#     come back, through a DIFFERENT path than the absence of an owner.
#
# ⚠️ THE TWO STRINGS ARE THOSE OF THE CODE THAT EMITS THEM, reread on September 5th,
# 2026 -- never those of the plan (a trap paid three times that day):
#   agent/src/capteur/sommeil/presse_papier.rs:71  "VM clipboard"
#   agent/src/capteur/fenetre/accent.rs:81         "Windows window accent"
set -uo pipefail
unset -f chpwd 2>/dev/null || true

BRAS="${1:?usage : jouer-item8.sh <temoin|mono|rouge>}"
RACINE="$(git rev-parse --show-toplevel)" || { echo "🔴 hors du depot git" >&2; exit 1; }
J="${RACINE}/docs/superpowers/plans/journaux-lot3-mono"
I="${J}/instrument"
D="${RACINE}/docs/superpowers/plans/journaux-lot3-latence/instrument"
PORT_HTTP=8099

set -a; . "${RACINE}/.env"; set +a
. "${RACINE}/docs/superpowers/plans/journaux-lot3/instrument/harnais-appliance.sh"
etape() { echo; echo "=== $(date -Is) $* ==="; }

servir() {
    local pid
    for pid in $(ss -ltnp 2>/dev/null | grep ":${PORT_HTTP}" | grep -o 'pid=[0-9]*' | cut -d= -f2 | sort -u); do
        kill -9 "${pid}" || true
    done
    sleep 1
    python3 -m http.server "${PORT_HTTP}" --bind 192.168.3.1 --directory "$1" >/dev/null 2>&1 &
    echo $! > /var/tmp/lot3-item8-http.pid
    sleep 2
    curl -sf -o /dev/null "http://192.168.3.1:${PORT_HTTP}/" || { echo "🔴 rien ne sert $1"; exit 1; }
}
arreter_le_service() { kill "$(cat /var/tmp/lot3-item8-http.pid)" 2>/dev/null || true; sleep 1; }

etape "PRE-VOL — depot du copieur, et la tache /it"
vm_prete || exit 1
servir "${I}"
W '$ProgressPreference = "SilentlyContinue"
Invoke-WebRequest -Uri "http://192.168.3.1:'"${PORT_HTTP}"'/copieur-item8.ps1" -OutFile "C:\nivuus\copieur-item8.ps1" -UseBasicParsing
"copieur sha256 : " + (Get-FileHash C:\nivuus\copieur-item8.ps1 -Algorithm SHA256).Hash' 120
arreter_le_service
echo "copieur local sha256 : $(sha256sum "${I}/copieur-item8.ps1" | tr 'a-f' 'A-F' | cut -c1-64)"

MARQUEUR="ITEM8-${BRAS}-$(date +%H%M%S)"
# 🔴 THE .cmd IN POWERSHELL SINGLE QUOTES, WITHOUT `-f`: the format
# operator binds LESS TIGHTLY than the comma and would format the whole array into ONE
# line -- a trap paid for by item 1, with a misleading LastTaskResult=0.
W '$q = [char]34
$ligne = "powershell.exe -NoProfile -ExecutionPolicy Bypass -File C:\nivuus\copieur-item8.ps1 " +
         "-Marqueur '"${MARQUEUR}"' > C:\nivuus\item8-sortie.txt 2>&1"
Set-Content -Path C:\nivuus\item8.cmd -Encoding ASCII -Value @("@echo off", $ligne)
schtasks /create /tn lot3-item8 /tr C:\nivuus\item8.cmd /sc once /st 00:00 /it /ru Administrator /rl HIGHEST /f | Out-Null
"lignes du .cmd (doit valoir 2) = " + @(Get-Content C:\nivuus\item8.cmd).Count
Get-Content C:\nivuus\item8.cmd | ForEach-Object { "  | " + $_ }' 120

etape "ARMEMENT DU BRAS : ${BRAS}"
agent_arreter
# 🔴 THE LEFTOVER WINDOWS OF A PREVIOUS ARM ARE NOT ADOPTED (ownership
# rule, batch 32I) AND THEY OCCUPY THE POOL. The `rouge` arm was
# first REJECTED for that: `output retained for this window` = 0, hence
# no accent possible -- its zero said nothing about disarming.
W 'Get-Process notepad -ErrorAction SilentlyContinue | ForEach-Object { Stop-Process -Id $_.Id -Force }
   Start-Sleep 2
   "notepad restants : " + (@(Get-Process notepad -ErrorAction SilentlyContinue)).Count' 90
# We start again from a clean run-agent.ps1 at each arm.
for v in CAPTEUR PRESSE_PAPIER ACCENT WINDOW_TITLE; do variable_de_banc retirer "$v" >/dev/null; done
case "${BRAS}" in
  temoin) echo "produit LIVRE, aucune variable posee" ;;
  mono)
    # ⚠️ SUPERVISEUR is ALREADY in the appliance's run-agent.ps1, at '1':
    # we do not INSERT it, we change ITS VALUE -- otherwise the anchor of
    # `variable_de_banc` would disappear with the line it serves to find.
    # 🔴 LITERAL `.Replace()`, AND ABOVE ALL NOT `-replace`. The
    # REPLACEMENT string of `-replace` treats `$` as a group back-reference: the
    # first wording produced `$$env:SUPERVISEUR   = 0` -- double dollar,
    # quotes lost -- and the agent logged nothing any more (a segment of ONE
    # line, 0 sensor, 0 enrolment). The arm had to be REJECTED, and the guest's
    # file REPAIRED. Measured on September 5th, 2026.
    # ⚠️ The check that counts is REREADING the line, further down: it must
    # read exactly `$env:SUPERVISEUR   = '0'`.
    W '$p = "C:\nivuus\agent\run-agent.ps1"
       $b = Get-Content $p -Encoding UTF8 -Raw
       $q = [char]39
       $b = $b.Replace("env:SUPERVISEUR   = " + $q + "1" + $q, "env:SUPERVISEUR   = " + $q + "0" + $q)
       Set-Content -Path $p -Value $b -Encoding UTF8
       "SUPERVISEUR bascule a 0 (Replace litteral)"' 90
    variable_de_banc poser CAPTEUR 0
    # 🔴 WITHOUT A WINDOW, THE SINGLE-WINDOW AGENT DIES BEFORE DOING ANYTHING, AND ITS
    # ZERO NO LONGER MEANS ANYTHING. Measured on September 5th, 2026: the segment only
    # carried four lines, including
    #   `Error: no visible window whose title contains << firefox >>`
    # -- `WINDOW_TITLE` falls back to "firefox" (demarrage/source.rs:52), which
    # does not exist on this VM. The arm was REJECTED, not banked: a zero
    # returned by an agent that stops says nothing about the owner.
    # We therefore open a REAL window in session 1, and target ITS title.
    W 'Set-Content -Path C:\nivuus\item8-fenetre.cmd -Encoding ASCII -Value @("@echo off","start notepad.exe")
       schtasks /create /tn lot3-item8-fenetre /tr C:\nivuus\item8-fenetre.cmd /sc once /st 00:00 /it /ru Administrator /rl HIGHEST /f | Out-Null
       schtasks /run /tn lot3-item8-fenetre | Out-Null
       Start-Sleep -Seconds 6
       "notepad ouverts : " + (@(Get-Process notepad -ErrorAction SilentlyContinue)).Count' 120
    variable_de_banc poser WINDOW_TITLE Notepad ;;
  rouge)
    variable_de_banc poser PRESSE_PAPIER 0
    variable_de_banc poser ACCENT 0 ;;
esac
echo "--- les lignes REELLEMENT dans le run-agent.ps1 GENERE, avec leur POSITION ---"
W 'Get-Content C:\nivuus\agent\run-agent.ps1 -Encoding UTF8 | Select-String "env:SUPERVISEUR|env:CAPTEUR|env:PRESSE_PAPIER|env:ACCENT|env:WINDOW_TITLE|agent.exe" | ForEach-Object { "  ligne $($_.LineNumber) : $($_.Line.Trim())" }' 90

REPERE=$(journal_reperer)
agent_relancer 30
echo "REPERE=${REPERE}"

etape "UNE FENETRE, pour que l'accent ait un objet — et un navigateur connecte"
# 🔴 `APP` IS MANDATORY, AND ITS ABSENCE COST SEVERAL ARMS.
# Without it, `pilote-latence.mjs` falls back to its default pattern
# `chrome|edge|bloc.?notes|notepad`, which matches **Microsoft Edge
# first** — yet THIS CAMPAIGN measured (item 7) that Edge is NEVER
# adopted: launched by desk, its window is SET ASIDE 60 ms later by the
# ownership rule, and no session opens. The arm then returns
# "windows SERVED: 0" for a reason FOREIGN to what it measures.
APP='^Notepad$' nohup node "${D}/pilote-latence.mjs" --etiquette="item8-${BRAS}" --fenetres=1 --duree=95 --animer=0 \
      --sortie="/var/tmp/lot3-item8-${BRAS}.json" > "${J}/pilote-${BRAS}.log" 2>&1 &
PILOTE_PID=$!
sleep 45

etape "LE GESTE : trois ecritures du presse-papier, en SESSION 1"
W 'Remove-Item C:\nivuus\item8-sortie.txt -Force -ErrorAction SilentlyContinue
   schtasks /run /tn lot3-item8 | Out-Null; "copieur lance"' 90
sleep 25
W '$t = Get-ScheduledTaskInfo -TaskName lot3-item8
   "LastTaskResult = " + $t.LastTaskResult
   Get-Content C:\nivuus\item8-sortie.txt -ErrorAction SilentlyContinue' 120 \
  | tee "${J}/copieur-${BRAS}.txt"

wait "${PILOTE_PID}" 2>/dev/null

etape "LE COMPTE DE MESSAGES — le seul controle qui vaille"
{
echo "bras=${BRAS} marqueur=${MARQUEUR}"
journal_depuis "${REPERE}" | sed 's/\x1b\[[0-9;]*m//g' > /var/tmp/lot3-item8-segment.log
echo "lignes du segment                        : $(wc -l < /var/tmp/lot3-item8-segment.log)"
echo "messages 'presse-papier de la VM'         : $(grep -ac 'presse-papier de la VM' /var/tmp/lot3-item8-segment.log)"
echo "annonces 'accent de la fenetre Windows'   : $(grep -ac 'accent de la fenetre Windows' /var/tmp/lot3-item8-segment.log)"
echo "--- traces de DESARMEMENT (elles prouvent l'arrivee de la variable, JAMAIS la coupure) ---"
grep -a "DESARME" /var/tmp/lot3-item8-segment.log | head -4
echo "--- TEMOIN DE VIE DE L'AGENT dans CE bras : il tourne, meme si rien n'est pousse ---"
echo "  lignes 'enrole' / 'session'   : $(grep -ac 'enrôlé\|enrole' /var/tmp/lot3-item8-segment.log)"
echo "  lignes 'capteur'              : $(grep -ac 'capteur' /var/tmp/lot3-item8-segment.log)"
echo "  lignes ERROR                  : $(grep -ac 'ERROR' /var/tmp/lot3-item8-segment.log)"
echo "  🔴 'aucune fenetre visible'    : $(grep -ac 'aucune fenêtre visible' /var/tmp/lot3-item8-segment.log)   (non nul = l'agent est MORT, le bras est a REJETER)"
echo "  🔴 fenetres SERVIES            : $(grep -ac 'sortie retenue pour cette fenetre' /var/tmp/lot3-item8-segment.log)   (zero en bras temoin/rouge = aucun accent possible, bras a REJETER)"
} 2>&1 | tee "${J}/comptes-${BRAS}.log"
cp /var/tmp/lot3-item8-segment.log "${J}/segment-${BRAS}.log"

etape "RETOUR A L ETAT LIVRE — sans quoi le bras suivant ne mesurerait pas le produit"
agent_arreter
for v in CAPTEUR PRESSE_PAPIER ACCENT WINDOW_TITLE; do variable_de_banc retirer "$v" >/dev/null; done
W '$p = "C:\nivuus\agent\run-agent.ps1"
   $b = Get-Content $p -Encoding UTF8 -Raw
   $q = [char]39
   $b = $b.Replace("env:SUPERVISEUR   = " + $q + "0" + $q, "env:SUPERVISEUR   = " + $q + "1" + $q)
   Set-Content -Path $p -Value $b -Encoding UTF8
   "--- etat livre restaure, relu ---"
   Get-Content $p -Encoding UTF8 | Select-String "env:SUPERVISEUR|env:CAPTEUR|env:PRESSE_PAPIER|env:ACCENT" | ForEach-Object { "  ligne " + $_.LineNumber + " : " + $_.Line.Trim() }' 90
agent_relancer 30

etape "MENAGE (la tache part, les pieces restent)"
W 'schtasks /delete /tn lot3-item8 /f 2>$null | Out-Null; "tache retiree"' 90
