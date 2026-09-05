#!/usr/bin/env bash
# Lot 3, item 8 (3.2) — LE PROPRIETAIRE MONO-FENETRE.
#
#     jouer-item8.sh <bras>        bras : temoin | mono | rouge
#
# LA QUESTION : en mode MONO-FENETRE, le presse-papier et l'accent ont-ils
# encore un proprietaire ? Le legs affirme que non.
#
# 🔴 LE TEMOIN A DEUX FACES, ET C'EST LUI QUI DONNE SON SENS AU ZERO :
#   - bras `temoin` : produit LIVRE (SUPERVISEUR=1). Le meme geste doit rendre
#     un compte NON NUL. Sans lui, le zero du bras `mono` serait rendu a
#     l'identique par un instrument qui ne sait pas voir ces messages.
#   - bras `mono`   : SUPERVISEUR=0 CAPTEUR=0. C'est la reponse de l'item.
#   - bras `rouge`  : produit livre + PRESSE_PAPIER=0 ACCENT=0. Le zero doit
#     revenir, par un AUTRE chemin que l'absence de proprietaire.
#
# ⚠️ LES DEUX CHAINES SONT CELLES DU CODE QUI LES EMET, relues le 5 septembre
# 2026 -- jamais celles du plan (piege paye trois fois ce jour-la) :
#   agent/src/capteur/sommeil/presse_papier.rs:71  "presse-papier de la VM"
#   agent/src/capteur/fenetre/accent.rs:81         "accent de la fenetre Windows"
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
# 🔴 LE .cmd EN GUILLEMETS SIMPLES POWERSHELL, SANS `-f` : l'operateur de
# format lie MOINS FORT que la virgule et formaterait le tableau entier en UNE
# ligne -- piege paye par l'item 1, avec un LastTaskResult=0 trompeur.
W '$q = [char]34
$ligne = "powershell.exe -NoProfile -ExecutionPolicy Bypass -File C:\nivuus\copieur-item8.ps1 " +
         "-Marqueur '"${MARQUEUR}"' > C:\nivuus\item8-sortie.txt 2>&1"
Set-Content -Path C:\nivuus\item8.cmd -Encoding ASCII -Value @("@echo off", $ligne)
schtasks /create /tn lot3-item8 /tr C:\nivuus\item8.cmd /sc once /st 00:00 /it /ru Administrator /rl HIGHEST /f | Out-Null
"lignes du .cmd (doit valoir 2) = " + @(Get-Content C:\nivuus\item8.cmd).Count
Get-Content C:\nivuus\item8.cmd | ForEach-Object { "  | " + $_ }' 120

etape "ARMEMENT DU BRAS : ${BRAS}"
agent_arreter
# 🔴 LES FENETRES RESIDUELLES D'UN BRAS PRECEDENT NE SONT PAS ADOPTEES (regle
# d'appartenance, lot 32I) ET ELLES OCCUPENT LE VIVIER. Le bras `rouge` a
# d'abord ete REJETE pour cela : `sortie retenue pour cette fenetre` = 0, donc
# aucun accent possible -- son zero ne disait rien du desarmement.
W 'Get-Process notepad -ErrorAction SilentlyContinue | ForEach-Object { Stop-Process -Id $_.Id -Force }
   Start-Sleep 2
   "notepad restants : " + (@(Get-Process notepad -ErrorAction SilentlyContinue)).Count' 90
# On repart d'un run-agent.ps1 propre a chaque bras.
for v in CAPTEUR PRESSE_PAPIER ACCENT WINDOW_TITLE; do variable_de_banc retirer "$v" >/dev/null; done
case "${BRAS}" in
  temoin) echo "produit LIVRE, aucune variable posee" ;;
  mono)
    # ⚠️ SUPERVISEUR est DEJA dans le run-agent.ps1 de l'appliance, a '1' :
    # on ne l'INSERE pas, on change SA VALEUR -- sans quoi l'ancre de
    # `variable_de_banc` disparaitrait avec la ligne qu'elle sert a trouver.
    # 🔴 `.Replace()` LITTERAL, ET SURTOUT PAS `-replace`. La chaine de
    # REMPLACEMENT de `-replace` traite `$` comme un renvoi de groupe : la
    # premiere redaction a produit `$$env:SUPERVISEUR   = 0` -- double dollar,
    # guillemets perdus -- et l'agent n'a plus rien journalise (segment d'UNE
    # ligne, 0 capteur, 0 enrolement). Le bras a du etre REJETE, et le fichier
    # de l'invite REPARE. Mesure le 5 septembre 2026.
    # ⚠️ Le controle qui vaut est la RELECTURE de la ligne, plus bas : elle doit
    # lire exactement `$env:SUPERVISEUR   = '0'`.
    W '$p = "C:\nivuus\agent\run-agent.ps1"
       $b = Get-Content $p -Encoding UTF8 -Raw
       $q = [char]39
       $b = $b.Replace("env:SUPERVISEUR   = " + $q + "1" + $q, "env:SUPERVISEUR   = " + $q + "0" + $q)
       Set-Content -Path $p -Value $b -Encoding UTF8
       "SUPERVISEUR bascule a 0 (Replace litteral)"' 90
    variable_de_banc poser CAPTEUR 0
    # 🔴 SANS FENETRE, L'AGENT MONO-FENETRE MEURT AVANT DE RIEN FAIRE, ET SON
    # ZERO NE VEUT PLUS RIEN DIRE. Mesure le 5 septembre 2026 : le segment ne
    # portait que quatre lignes, dont
    #   `Error: aucune fenetre visible dont le titre contient << firefox >>`
    # -- `WINDOW_TITLE` retombe sur "firefox" (demarrage/source.rs:52), qui
    # n'existe pas sur cette VM. Le bras a ete REJETE, pas encaisse : un zero
    # rendu par un agent qui s'arrete ne dit rien du proprietaire.
    # On ouvre donc une fenetre REELLE en session 1, et on vise SON titre.
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
# 🔴 `APP` EST OBLIGATOIRE, ET SON ABSENCE A COUTE PLUSIEURS BRAS.
# Sans elle, `pilote-latence.mjs` retombe sur son motif par defaut
# `chrome|edge|bloc.?notes|notepad`, qui apparie **Microsoft Edge en
# premier** — or CETTE CAMPAGNE a mesure (item 7) qu'Edge n'est JAMAIS
# adopte : lance par desk, sa fenetre est ECARTEE 60 ms plus tard par la
# regle d'appartenance, et aucune session ne s'ouvre. Le bras rend alors
# « fenetres SERVIES : 0 » pour une raison ETRANGERE a ce qu'il mesure.
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
