#!/usr/bin/env bash
# Lot 3, item 2 (3.6) — LES TROIS MURS DU PONT, re-situés sur le produit
# d'aujourd'hui.
#
#     paliers.sh <etiquette>
#
# Les trois murs que F4 a mesurés le 21 aout 2026, RELUS dans son document :
#   debit soutenu             30 a 33 Kio/s
#   taille de lecture maximale  128 Kio  (au-dela : echoue)
#   rang d'enumeration maximal  ~3 150 entrees (3 200 : echoue)
#
# 🔴 CE SCRIPT NE JUGE RIEN : il releve duree et ISSUE a chaque barreau, et
# c'est le verdict qui situe le mur. Un barreau qui echoue n'est un mur que si
# un barreau plus petit ABOUTIT — sinon c'est une panne (F4, § 5).
#
# ⚠️ LES COMPTEURS DE RECENSEMENT SONT CUMULATIFS : une mesure se lit par
# DIFFERENCE entre deux recensements, jamais sur une ligne isolee.
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

# La purge est une PORTE, pas un compte rendu (piege paye par l'item 3).
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
# 🔴 Ce bras est ce qui rend les suivants discriminants. Le controle qui vaut
# n'est pas que la ligne SORTE, c'est qu'elle COMPTE ce qu'elle dit : deux
# periodes de recensement (2 x 10 s) sans toucher au pont doivent rendre n:0.
sleep 22
W 'Get-Content C:\nivuus\agent.log -Encoding UTF8 -Tail 60 | Select-String "recensement" | Select-Object -Last 3 | ForEach-Object { $_.Line }' 180

echo
echo "### 🔴 TEMOIN POSITIF DU RANG, SONDE EN PREMIER"
# A la 1re execution, rang-100 a echoue (« does not exist ») APRES les echecs
# de lecture de 192/256/512 Kio, alors qu il EXISTE cote local — un cache de
# chemin negatif de ProjFS est le suspect. On le sonde donc AVANT toute
# lecture, pour separer « le petit rang ne marche pas » de « il ne marche plus
# APRES un echec de lecture ».
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
# Sans ce bras, « tous les barreaux aboutissent » serait rendu a l'identique
# par un instrument qui ne peut pas rapporter d'echec.
W '$p = "C:\Users\Administrator\Mes Fichiers\ce-fichier-n-existe-pas.bin"
try { $o = [IO.File]::ReadAllBytes($p); "  ECHEC DU TEMOIN : la lecture a ABOUTI (" + $o.Length + " o) sur un fichier absent" }
catch { "  temoin OK : la lecture d un fichier absent ECHOUE -- " + $_.Exception.GetType().Name }' 180

echo
echo "### ECHELLE D ENTREES — duree, ISSUE, et le COMPTE RENDU"
# ⚠️ Le compte rendu est le point : un listage qui aboutit en rendant MOINS
# d'entrees qu'il n'en existe est un mur DEGUISE, pas un succes.
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
# 🔴 DES FICHIERS DISTINCTS, ET C EST TOUT LE POINT. Relire le meme fichier en
# boucle rendait 1 312 669 Kio/s a la 1re execution — le cache de fichiers de
# Windows, pas le pont. Vingt fichiers de 128 Kio, lus UNE fois chacun.
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
### RECENSEMENT — la DIFFERENCE entre deux releves, jamais une ligne isolee"
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
