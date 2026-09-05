#!/usr/bin/env bash
# Lot 3, item 1 (3.7) — UNE exécution complète.
#
#     jouer-item1.sh <etiquette> [--rouge]
#
# Il monte le pont avec une racine locale RÉELLE (OPFS), fait enregistrer le
# Bloc-notes DANS la racine ProjFS depuis la SESSION 1, et relit le poste local.
#
# 🔴 DEUX EXÉCUTIONS NE SE CHEVAUCHENT JAMAIS. F1 en a perdu une : deux se sont
# recouvertes de 2 min 23 s, la seconde a tué l'agent de la première EN PLEINE
# MESURE, et le journal versé sous le nom de la première était celui de la
# seconde.
set -uo pipefail
unset -f chpwd 2>/dev/null || true

ETIQUETTE="${1:?usage : jouer-item1.sh <etiquette> [--rouge]}"
ROUGE="${2:-}"
RACINE="$(git rev-parse --show-toplevel)" || {
    echo "🔴 hors du dépôt git : impossible de dériver RACINE" >&2; exit 1; }
I="${RACINE}/docs/superpowers/plans/journaux-lot3-pont-sauvegarde/instrument"
J="${RACINE}/docs/superpowers/plans/journaux-lot3-pont-sauvegarde"
CONSOLE="$(cd "${RACINE}/../installer" && pwd)"

set -a; . "${RACINE}/.env"; set +a
. "${RACINE}/docs/superpowers/plans/journaux-lot3/instrument/harnais-appliance.sh"

RACINE_PROJFS='C:\Users\Administrator\Mes Fichiers'
FICHIER='item1-sauvegarde.txt'
PORT_HTTP=8099

etape() { echo; echo "=== $(date -Is) $* ==="; }

# 🔴 LIBÉRER LE PORT PAR PID RELEVÉ, jamais par motif. Un orphelin d'une
# exécution précédente RÉPOND À LA PLACE du nôtre, depuis le mauvais
# répertoire, et le symptôme est un 404 qui se lit comme un dépôt raté — payé
# deux fois par ce dépôt (lots 32Q et 32T).
servir() {
    local pid
    for pid in $(ss -ltnp 2>/dev/null | grep ":${PORT_HTTP}" \
                 | grep -o 'pid=[0-9]*' | cut -d= -f2 | sort -u); do
        echo "port ${PORT_HTTP} tenu par le PID ${pid}, je le libère"; kill -9 "${pid}" || true
    done
    sleep 1
    # `--directory` supprime le `cd` : sans lui, `$!` désigne le SOUS-SHELL, le
    # kill le tue, et python survit.
    python3 -m http.server "${PORT_HTTP}" --bind 192.168.3.1 --directory "$1" >/dev/null 2>&1 &
    echo $! > /var/tmp/lot3-item1-http.pid
    sleep 2
    curl -sf -o /dev/null "http://192.168.3.1:${PORT_HTTP}/" || {
        echo "🔴 rien ne sert $1"; exit 1; }
}
arreter_le_service() {
    kill "$(cat /var/tmp/lot3-item1-http.pid)" 2>/dev/null || true
    for _ in 1 2 3 4 5; do
        curl -sf -o /dev/null --max-time 1 "http://192.168.3.1:${PORT_HTTP}/" || return 0
        sleep 1
    done
    echo "🔴 le serveur HTTP survit au kill"; exit 1
}

etape "PRÉ-VOL : la VM, et le dépôt de l'observateur"
vm_prete || exit 1
servir "${I}"
W '
$ProgressPreference = "SilentlyContinue"
Invoke-WebRequest -Uri "http://192.168.3.1:'"${PORT_HTTP}"'/observer-editeurs.ps1" -OutFile "C:\nivuus\observer-editeurs.ps1" -UseBasicParsing
"observateur sha256 : " + (Get-FileHash C:\nivuus\observer-editeurs.ps1 -Algorithm SHA256).Hash' 120
arreter_le_service
# 🔴 LE CONTRÔLE QUI VAUT EST LA COMPARAISON DES DEUX SHA : c'est elle qui a
# attrapé le défaut du serveur orphelin les deux fois, là où aucun code de
# retour ne le pouvait.
echo "observateur local sha256 : $(sha256sum "${I}/observer-editeurs.ps1" | tr 'a-f' 'A-F' | cut -c1-64)"

# La tâche planifiée /it : LA SESSION 1 EST OBLIGATOIRE. SendKeys n'atteint
# aucun bureau depuis la session 0, et l'observateur IMPRIME sa session plutôt
# que de la supposer.
# 🔴 DEUX DÉFAUTS D'ÉCRITURE DE CE `.cmd`, PAYÉS L'UN APRÈS L'AUTRE LE
# 5 SEPTEMBRE 2026, ET LE SECOND SE LISAIT COMME LE PREMIER :
#   ① `\"` n'échappe RIEN en PowerShell — l'échappement est l'accent grave.
#      Le fichier recevait `-Racine \"C:\Users\…` littéralement, cmd.exe
#      coupait l'argument au premier espace. Symptôme : `item1-sortie.txt`
#      ABSENT, journal de l'observateur VIDE, `LastTaskResult` jamais relevé.
#   ② L'opérateur `-f` a une précédence PLUS BASSE que la virgule :
#      `@("a", ("b") -f $x)` se lit `@( ("a","b") -f $x )`, donc il formate
#      L'ARRAY ENTIER et rend UNE SEULE chaîne. Le `.cmd` sortait sur UNE
#      ligne — `@echo off powershell.exe …` — et cmd.exe exécutait `echo`
#      avec tout le reste en arguments. Symptôme : `LastTaskResult = 0`, une
#      sortie qui ressemble à la commande, et RIEN d'exécuté.
# 🔵 Le remède est de n'employer NI `-f` NI d'échappement : une variable
# `$q = [char]34`, interpolée par `${q}`. Et le contrôle qui vaut est de
# RELIRE le fichier déposé en COMPTANT SES LIGNES — le `.cmd` doit en faire
# DEUX. Sans ce comptage, ② est invisible.
W '$q = [char]34
$ligne = "powershell.exe -NoProfile -ExecutionPolicy Bypass -File C:\nivuus\observer-editeurs.ps1 " +
         "-Racine ${q}C:\Users\Administrator\Mes Fichiers${q} -Fichier item1-sauvegarde.txt " +
         "> C:\nivuus\item1-sortie.txt 2>&1"
Set-Content -Path C:\nivuus\item1.cmd -Encoding ASCII -Value @("@echo off", $ligne)
schtasks /create /tn lot3-item1 /tr C:\nivuus\item1.cmd /sc once /st 00:00 /it /ru Administrator /rl HIGHEST /f | Out-Null
"--- le .cmd REELLEMENT depose : il doit faire DEUX lignes ---"
"lignes = " + @(Get-Content C:\nivuus\item1.cmd).Count
Get-Content C:\nivuus\item1.cmd | ForEach-Object { "  | " + $_ }' 120

if [ "${ROUGE}" = "--rouge" ]; then
    etape "BRAS ROUGE : PONT_ECRITURE=0"
    # 🔴 POURQUOI `PONT_ECRITURE=0` ET NON `PONT_MUTATION=0`, QUE LE PLAN
    # PRESCRIVAIT — ET POURQUOI CE N'EST PAS LE PIÈGE QU'IL NOMME.
    #
    # Le plan écrit : « Ne pas employer PONT_ECRITURE=0 à sa place : il pose
    # inscriptible=false, ce qui fait refuser au PRE_ pour une AUTRE raison —
    # une rouge de F3 y a été DISQUALIFIÉE. » Cet avertissement vise le critère
    # de F3, qui est un RENOMMAGE : y désarmer l'écriture ferait rougir pour une
    # raison étrangère au renommage.
    #
    # ⚠️ MAIS LE CRITÈRE DE CET ITEM N'EST PAS UN RENOMMAGE. Il est
    # « la sauvegarde arrive-t-elle ENTIÈRE au poste local ? », et la mesure de
    # l'exécution 1 a établi que le Bloc-notes écrit EN PLACE dans la racine
    # ProjFS — aucun temporaire, aucun renommage, aux TROIS inventaires.
    # `PONT_MUTATION=0` désarme le renommage et la suppression : sur un chemin
    # qui n'en emprunte AUCUN, il ne changerait rien, et la rouge serait VERTE
    # pour une raison qui n'a rien à voir avec ce qu'elle prétend éprouver.
    #
    # 🔵 Le désarmement qui mord sur CE mécanisme est `PONT_ECRITURE=0` : le
    # pont continue de détecter, de journaliser et de compter les écritures
    # dues, ET N'EN POUSSE AUCUNE. C'est exactement l'inverse du chiffre-juge.
    agent_arreter
    variable_de_banc poser PONT_ECRITURE 0
    W 'Get-Content C:\nivuus\agent\run-agent.ps1 -Encoding UTF8 | Select-String "env:PONT_ECRITURE|agent.exe" | ForEach-Object { "ligne $($_.LineNumber) : $($_.Line.Trim())" }' 90
    agent_relancer 30
fi

REPERE=$(journal_reperer)
etape "REPÈRE du journal de l'agent : ${REPERE}"

etape "LE PILOTE : monte le pont, puis TIENT la session pendant que l'invité écrit"
nohup node "${I}/pilote-item1.mjs" --etiquette="${ETIQUETTE}" --maintien=120 \
      --sortie="${J}/sequence-${ETIQUETTE}.json" > "${J}/pilote-${ETIQUETTE}.log" 2>&1 &
PILOTE=$!
echo "pilote pid=${PILOTE}"

# Attendre que le pont soit RÉELLEMENT monté, jamais une durée : c'est un FAIT
# qu'on attend, et l'agent le publie.
for _ in $(seq 1 40); do
    sleep 3
    if grep -aq "pont monté côté navigateur" "${J}/pilote-${ETIQUETTE}.log" 2>/dev/null; then
        echo "pont monté (vu dans le journal du pilote)"; break
    fi
done
grep -a "racine locale préparée\|pont monté\|clic sur" "${J}/pilote-${ETIQUETTE}.log" || true

etape "L'OBSERVATEUR, en SESSION 1 : le Bloc-notes ouvre, écrit, enregistre, ferme"
W 'Remove-Item C:\nivuus\item1-sortie.txt -Force -ErrorAction SilentlyContinue
   schtasks /run /tn lot3-item1 | Out-Null
   "observateur lance"' 90
sleep 35
# 🔴 LE CODE DE LA TÂCHE AVANT SA SORTIE : une sortie vide a DEUX causes — la
# tâche n'a pas tourné, ou elle a tourné sans rien écrire — et seul
# `LastTaskResult` les sépare.
W '$t = Get-ScheduledTaskInfo -TaskName lot3-item1
   "LastTaskResult = " + $t.LastTaskResult
   "LastRunTime    = " + $t.LastRunTime
   "--- sortie ---"
   Get-Content C:\nivuus\item1-sortie.txt -ErrorAction SilentlyContinue' 120 \
  | tee "${J}/observateur-${ETIQUETTE}.txt"

etape "ATTENTE de la fin du pilote (il relit le poste local APRÈS)"
wait "${PILOTE}" 2>/dev/null
tail -6 "${J}/pilote-${ETIQUETTE}.log"

etape "CE QUE LE PONT A NOTIFIÉ — les notifications ProjFS, dans l'ordre"
journal_depuis "${REPERE}" | sed 's/\x1b\[[0-9;]*m//g' \
  | grep -aiE "pont|projfs|PRE_RENAME|PRE_DELETE|notification|ecriture|poussee|mutation" \
  | tee "${J}/notifications-${ETIQUETTE}.log" | tail -30

if [ "${ROUGE}" = "--rouge" ]; then
    etape "RETOUR À L'ÉTAT LIVRÉ"
    agent_arreter
    variable_de_banc retirer PONT_ECRITURE
    agent_relancer 30
fi

etape "MÉNAGE"
# ⚠️ LA TÂCHE PART, LES PIÈCES RESTENT. La première rédaction effaçait
# `observer-editeurs.ps1`, `item1.cmd` et la sortie — si bien qu'au moment de
# diagnostiquer un échec, IL N'Y AVAIT PLUS RIEN À LIRE. Un ménage qui détruit
# ce qui explique l'échec est un ménage qui coûte une exécution.
W 'schtasks /delete /tn lot3-item1 /f 2>$null | Out-Null
   "taches lot3 restantes : " + (@(Get-ScheduledTask | Where-Object { $_.TaskName -match "lot3" })).Count
   "pieces conservees : " + ((Test-Path C:\nivuus\observer-editeurs.ps1) -and (Test-Path C:\nivuus\item1.cmd))' 90
bash "${RACINE}/docs/superpowers/plans/journaux-lot3/instrument/etat-vm.sh"
