#!/usr/bin/env bash
# Copie la mesure sur la VM, la lance PAR TACHE PLANIFIEE /it, et rend son JSON.
#
# 🔴 SESSION INTERACTIVE ET NON WinRM. La racine ProjFS est montee par un
# processus de la session 1 ; l'ecrire depuis la session 0 traverserait quand
# meme le fournisseur, mais le Bloc-notes du critere ② n'y a aucun bureau. Un
# seul chemin pour les deux, c'est un chemin de moins a expliquer.
set -uo pipefail
RACINE="$(git rev-parse --show-toplevel)" || { echo "🔴 hors du depot git : impossible de deriver RACINE (git rev-parse a echoue)" >&2; exit 1; }
I="$RACINE/docs/superpowers/plans/journaux-pont-fichiers-f3/instrument"
set -a; source "$RACINE/.env"; set +a

# 🔴 TUER LES MESURES FIGEES D'ABORD, ET C'EST PAYE SUR PLACE.
#
# Une tentative figee laisse son `powershell` vivant, et ce processus TIENT
# `mesure-f3.json` par la poignee de son `Out-File`. La mesure suivante ecrit
# alors dans le vide : `Set-Content` rend « le fichier est en cours
# d'utilisation par un autre processus », et le releve est perdu **pour une
# raison qui n'a rien a voir avec ce qu'on mesure**. C'est le piege maison de
# D8 — « un pilote qui laisse un processus vivant bloque SILENCIEUSEMENT la
# tentative suivante » — sous une forme neuve.
# ⚠️ LE TUEUR PASSE PAR UN FICHIER, JAMAIS PAR UNE COMMANDE EN LIGNE.
# `nodejs-winrm` enveloppe TOUJOURS la commande dans `powershell -Command
# "& { ... }"` : un script inline portant des guillemets doubles entre en
# collision avec cette enveloppe, **et le symptome est un script qui ne tourne
# jamais** (piege D3). Une premiere redaction de ce tueur l'a paye : il n'a
# tue personne, en silence, et le verrou a survecu a trois tentatives.
cat > /media/vm/dev/tuer-mesures.ps1 <<'PSKILL'
schtasks /end /tn mesure-f3 2>$null | Out-Null
Get-CimInstance Win32_Process | Where-Object { $_.CommandLine -like '*mesure-f3*' } |
    ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }
PSKILL
node "$RACINE/scripts/winrm.js" 'powershell -NoProfile -ExecutionPolicy Bypass -File C:\dev\tuer-mesures.ps1' >/dev/null 2>&1

cp "$I/mesurer-f3.ps1" /media/vm/dev/mesurer-f3.ps1
HORO=$(date +%H%M%S)
SORTIE="C:\\dev\\mesure-f3-$HORO.json"
LOCAL="/media/vm/dev/mesure-f3-$HORO.json"
rm -f "$LOCAL" /media/vm/dev/mesure-f3.trace.txt
cat > /media/vm/dev/lancer-mesure-f3.ps1 <<PS1
# 🔴 LA TRACE ET LE RELEVE NE PARTAGENT PLUS LE MEME FICHIER.
#
# Ce lanceur redirigeait TOUT vers `mesure-f3.json`. Depuis que la mesure
# ecrit elle-meme ses deux points de reprise dans ce fichier, les deux
# ecrivains se disputeraient la meme poignee : `Out-File` le tronque a
# l'ouverture et le tient ouvert jusqu'a la fin, donc un `Set-Content`
# intercalaire est perdu ou refuse. **Le releve serait vide precisement quand
# le point de reprise sert a quelque chose.**
& 'C:\dev\mesurer-f3.ps1' -sortie '$SORTIE' *>&1 | Out-File -FilePath 'C:\dev\mesure-f3.trace.txt' -Encoding utf8
PS1

node "$RACINE/scripts/winrm.js" \
  "schtasks /delete /tn mesure-f3 /f 2>\$null; \
   schtasks /create /tn mesure-f3 /f /it /ru '$WINDOWS_ADMIN_USERNAME' /rp '$WINDOWS_ADMIN_PASSWORD' \
     /sc once /st 00:00 \
     /tr 'powershell -WindowStyle Hidden -NoProfile -ExecutionPolicy Bypass -File C:\dev\lancer-mesure-f3.ps1'; \
   schtasks /run /tn mesure-f3" >/dev/null 2>&1

# Attendre le FAIT, jamais une duree (piege maison D3).
#
# ⚠️ LE FAIT ATTENDU EST LE POINT DE REPRISE N°2 (`apres2`), et l'attente est
# BORNEE : si la mesure se fige sur un geste, le point de reprise n°1 est deja
# ecrit et porte les criteres ① et ②. **On rend alors ce qu'on a, en le
# DISANT** — un releve partiel annonce vaut mieux qu'un releve vide.
COMPLET=non
for i in $(seq 1 90); do
    if [ -s "$LOCAL" ] && grep -qa 'apres2' "$LOCAL"; then COMPLET=oui; break; fi
    sleep 2
done
cp /media/vm/dev/mesure-f3.trace.txt "${TRACE:-/dev/null}" 2>/dev/null || true
if [ "$COMPLET" = non ]; then
    echo "MESURE PARTIELLE : le point de reprise n°2 n'est pas arrive en 180 s." >&2
fi
cat "$LOCAL" 2>/dev/null || echo '{"erreur":"mesure-f3.json absent"}'
