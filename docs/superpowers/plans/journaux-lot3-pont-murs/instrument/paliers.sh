#!/usr/bin/env bash
# Batch 3, item 2 (3.6) — THE BRIDGE'S THREE WALLS, re-located on today's
# product.
#
#     paliers.sh <etiquette>
#
# The three walls F4 measured on August 21st, 2026, REREAD in its document:
#   sustained throughput       30 to 33 KiB/s
#   maximum read size          128 KiB  (beyond: fails)
#   maximum enumeration rank   ~3,150 entries (3,200: fails)
#
# 🔴 THIS SCRIPT JUDGES NOTHING: it records duration and OUTCOME at each rung, and
# it is the verdict that locates the wall. A failing rung is only a wall if
# a smaller rung SUCCEEDS — otherwise it is a failure (F4, § 5).
#
# ⚠️ THE CENSUS COUNTERS ARE CUMULATIVE: a measurement is read as the
# DIFFERENCE between two censuses, never on an isolated line.
set -uo pipefail
unset -f chpwd 2>/dev/null || true

ETIQUETTE="${1:?usage : paliers.sh <etiquette>}"
RACINE="$(git rev-parse --show-toplevel)" || { echo "🔴 hors du depot git" >&2; exit 1; }
J="${RACINE}/docs/superpowers/plans/journaux-lot3-pont-murs"
I="${J}/instrument"
PILOTE="${RACINE}/docs/superpowers/plans/journaux-lot3-pont-sauvegarde/instrument/pilote-item1.mjs"

set -a; . "${RACINE}/.env"; set +a
. "${RACINE}/docs/superpowers/plans/journaux-lot3/instrument/harnais-appliance.sh"

VM='C:\Users\Administrator\Mes Fichiers'
etape() { echo; echo "=== $(date -Is) $* ==="; }

etape "PRE-VOL"
vm_prete || exit 1

# The purge is a GATE, not a report (a trap paid for by item 3).
PURGE_RESTANT=9
for t in 1 2 3 4 5; do
    PURGE_RESTANT=$(W 'Get-ChildItem "C:\Users\Administrator\Mes Fichiers" -Force -ErrorAction SilentlyContinue | ForEach-Object { Remove-Item $_.FullName -Recurse -Force -ErrorAction SilentlyContinue }; Start-Sleep -Milliseconds 500; @(Get-ChildItem "C:\Users\Administrator\Mes Fichiers" -Force -ErrorAction SilentlyContinue).Count' 180 | tr -dc '0-9')
    echo "purge, tentative ${t} : restant = ${PURGE_RESTANT:-?}"
    [ "${PURGE_RESTANT:-9}" = "0" ] && break
    sleep 6
done
[ "${PURGE_RESTANT:-9}" = "0" ] || { echo "🔴 racine ProjFS non vidable (${PURGE_RESTANT})" >&2; exit 1; }

etape "ARMER LE RECENSEMENT : PONT_MESURE=1"
agent_arreter
variable_de_banc poser PONT_MESURE 1
W 'Get-Content C:\nivuus\agent\run-agent.ps1 -Encoding UTF8 | Select-String "env:PONT_MESURE|agent.exe" | ForEach-Object { "ligne $($_.LineNumber) : $($_.Line.Trim())" }' 90
REPERE=$(journal_reperer)
agent_relancer 30
echo "--- la trace d'armement (son ABSENCE dirait que la variable n'a pas atteint le processus) ---"
journal_depuis "${REPERE}" | sed 's/\x1b\[[0-9;]*m//g' | grep -a "banc de latence du pont ARME" | head -2

etape "LE PILOTE monte le pont et batit les deux echelles"
nohup node "${PILOTE}" --etiquette="${ETIQUETTE}" --maintien=420 \
      --injection=../../journaux-lot3-pont-murs/instrument/injection-item2.js \
      --prepare=__item2Preparer --relire=__item2Relire \
      --sortie="${J}/opfs-${ETIQUETTE}.json" > "${J}/pilote-${ETIQUETTE}.log" 2>&1 &
PILOTE_PID=$!
for _ in $(seq 1 90); do
    sleep 4
    grep -aq "pont monté côté navigateur" "${J}/pilote-${ETIQUETTE}.log" 2>/dev/null && break
done
grep -a "racine locale préparée\|pont monté" "${J}/pilote-${ETIQUETTE}.log" | cut -c1-400
sleep 5

{
echo "### BRAS TEMOIN — ARME, AUCUN GESTE : le recensement doit COMPTER ZERO"
# 🔴 This arm is what makes the following ones discriminating. The check that counts
# is not that the line COMES OUT, it is that it COUNTS what it says: two
# census periods (2 x 10 s) without touching the bridge must return n:0.
sleep 22
W 'Get-Content C:\nivuus\agent.log -Encoding UTF8 -Tail 60 | Select-String "recensement" | Select-Object -Last 3 | ForEach-Object { $_.Line }' 180

echo
echo "### 🔴 TEMOIN POSITIF DU RANG, SONDE EN PREMIER"
# At the 1st run, rang-100 failed ("does not exist") AFTER the read failures
# of 192/256/512 KiB, while it EXISTS on the local side — a negative path
# cache of ProjFS is the suspect. We therefore probe it BEFORE any
# read, to separate "the small rank does not work" from "it no longer works
# AFTER a read failure".
W '$d = "C:\Users\Administrator\Mes Fichiers\rang-100"
try { $c = @(Get-ChildItem $d -Force -ErrorAction Stop).Count; "  rang 100 AVANT toute lecture : ABOUTIT, rendues=" + $c }
catch { "  rang 100 AVANT toute lecture : ECHOUE -- " + $_.Exception.Message.Substring(0,[Math]::Min(90,$_.Exception.Message.Length)) }' 180

echo
echo "### ECHELLE DE TAILLE — duree et ISSUE a chaque barreau"
W '$r = @()
foreach ($k in 4,16,32,64,128,192,256,512) {
  $p = "C:\Users\Administrator\Mes Fichiers\taille-${k}k.bin"
  $sw = [Diagnostics.Stopwatch]::StartNew()
  try {
    $o = [IO.File]::ReadAllBytes($p)
    $sw.Stop()
    $kios = if ($sw.Elapsed.TotalSeconds -gt 0) { [math]::Round(($o.Length/1024)/$sw.Elapsed.TotalSeconds,2) } else { 0 }
    $r += "  {0,4} Kio : ABOUTIT  {1,8:N0} ms  lus={2,7} o  {3} Kio/s" -f $k,$sw.Elapsed.TotalMilliseconds,$o.Length,$kios
  } catch {
    $sw.Stop()
    $r += "  {0,4} Kio : ECHOUE   {1,8:N0} ms  -- {2}" -f $k,$sw.Elapsed.TotalMilliseconds,$_.Exception.Message.Substring(0,[Math]::Min(90,$_.Exception.Message.Length))
  }
}
$r' 600

echo
echo "### 🔴 TEMOIN NEGATIF DE LA LECTURE — l'instrument sait-il seulement ECHOUER ?"
# Without this arm, "all rungs succeed" would be returned identically
# by an instrument that cannot report a failure.
W '$p = "C:\Users\Administrator\Mes Fichiers\ce-fichier-n-existe-pas.bin"
try { $o = [IO.File]::ReadAllBytes($p); "  ECHEC DU TEMOIN : la lecture a ABOUTI (" + $o.Length + " o) sur un fichier absent" }
catch { "  temoin OK : la lecture d un fichier absent ECHOUE -- " + $_.Exception.GetType().Name }' 180

echo
echo "### ECHELLE D ENTREES — duree, ISSUE, et le COMPTE RENDU"
# ⚠️ The count returned is the point: a listing that succeeds while returning FEWER
# entries than exist is a DISGUISED wall, not a success.
W '$r = @()
foreach ($n in 100,1000,2000,3000,3150,3200,4000) {
  $d = "C:\Users\Administrator\Mes Fichiers\rang-$n"
  $sw = [Diagnostics.Stopwatch]::StartNew()
  try {
    $c = @(Get-ChildItem $d -Force -ErrorAction Stop).Count
    $sw.Stop()
    $verdict = if ($c -eq $n) { "COMPLET" } else { "TRONQUE" }
    $r += "  rang {0,5} : ABOUTIT  {1,8:N0} ms  rendues={2,5}  {3}" -f $n,$sw.Elapsed.TotalMilliseconds,$c,$verdict
  } catch {
    $sw.Stop()
    $r += "  rang {0,5} : ECHOUE   {1,8:N0} ms  -- {2}" -f $n,$sw.Elapsed.TotalMilliseconds,$_.Exception.Message.Substring(0,[Math]::Min(90,$_.Exception.Message.Length))
  }
}
$r' 900

echo
echo "### 🔴 TEMOIN NEGATIF DE L ENUMERATION — un repertoire absent doit ECHOUER"
W '$d = "C:\Users\Administrator\Mes Fichiers\rang-inexistant"
try { $c = @(Get-ChildItem $d -Force -ErrorAction Stop).Count; "  ECHEC DU TEMOIN : le listage a ABOUTI (" + $c + " entrees) sur un repertoire absent" }
catch { "  temoin OK : le listage d un repertoire absent ECHOUE -- " + $_.Exception.GetType().Name }' 180

echo
echo "### DEBIT SOUTENU — au moins 60 s de lecture continue, FICHIERS DISTINCTS"
# 🔴 DISTINCT FILES, AND THAT IS THE WHOLE POINT. Rereading the same file in
# a loop returned 1,312,669 KiB/s at the 1st run — the Windows file
# cache, not the bridge. Twenty files of 128 KiB, read ONCE each.
W '$dir = "C:\Users\Administrator\Mes Fichiers\debit"
$total = 0; $lus = 0; $echecs = 0
$sw = [Diagnostics.Stopwatch]::StartNew()
foreach ($i in 0..19) {
  $p = Join-Path $dir ("d" + $i.ToString("00") + ".bin")
  try { $o = [IO.File]::ReadAllBytes($p); $total += $o.Length; $lus += 1 } catch { $echecs += 1 }
}
$sw.Stop()
"  fichiers lus={0}  echecs={1}  octets={2:N0}  duree={3:N1} s" -f $lus,$echecs,$total,$sw.Elapsed.TotalSeconds
if ($sw.Elapsed.TotalSeconds -gt 0 -and $lus -gt 0) {
  "  DEBIT SOUTENU = {0:N2} Kio/s   (palier de {1:N1} s, soit {2:N1} periodes de recensement)" -f (($total/1024)/$sw.Elapsed.TotalSeconds), $sw.Elapsed.TotalSeconds, ($sw.Elapsed.TotalSeconds/10)
}' 900

echo
### CENSUS — the DIFFERENCE between two readings, never an isolated line"
W 'Get-Content C:\nivuus\agent.log -Encoding UTF8 -Tail 120 | Select-String "recensement" | Select-Object -Last 4 | ForEach-Object { $_.Line }' 180
} 2>&1 | tee "${J}/paliers-${ETIQUETTE}.log"

etape "ATTENTE de la fin du pilote"
wait "${PILOTE_PID}" 2>/dev/null
grep -a "relecture APRÈS" "${J}/pilote-${ETIQUETTE}.log" | cut -c1-600

etape "RETOUR A L ETAT LIVRE"
agent_arreter
variable_de_banc retirer PONT_MESURE
agent_relancer 30
bash "${RACINE}/docs/superpowers/plans/journaux-lot3/instrument/etat-vm.sh"
