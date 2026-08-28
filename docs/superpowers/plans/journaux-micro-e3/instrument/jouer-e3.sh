#!/usr/bin/env bash
# Joue UNE exécution de la recette du bloc E3.
#
#     instrument/jouer-e3.sh <etiquette>
#
# 🔴 DEUX EXÉCUTIONS NE SE CHEVAUCHENT JAMAIS. F1 en a perdu une : deux se sont
# recouvertes de 2 min 23 s, la seconde a tué l'agent de la première EN PLEINE
# MESURE, et *le journal versé sous le nom de la première était celui de la
# seconde*. Ce script tue AVANT, et vérifie APRÈS.
#
# 🔴 L'ORDRE EST TROIS FOIS CONTRAINT, ET CHAQUE CONTRAINTE A ÉTÉ PAYÉE :
#   1. `.env` EN PREMIER, jamais après les réglages de recette — il porte
#      `SIGNALING_URL` et un `source` postérieur les écrase EN SILENCE ;
#   2. les FENÊTRES avant le SUPERVISEUR — il les trouve par
#      `enumerer_existantes`, et l'ordre inverse lui fait capturer la console
#      PowerShell de la tâche planifiée (D11) ;
#   3. la page-SHELL avant l'AGENT — le signaling ne mémorise que les offres
#      SDP, une annonce `fenetre-ouverte` émise avant est PERDUE SANS TRACE
#      (D1). C'est le pilote qui tient ce troisième ordre, par `APRES_CONNEXION`.
set -uo pipefail
ETIQUETTE="${1:?etiquette}"
RACINE="$(git rev-parse --show-toplevel)" || { echo "🔴 hors du depot git : impossible de deriver RACINE (git rev-parse a echoue)" >&2; exit 1; }
I="$RACINE/docs/superpowers/plans/journaux-micro-e3/instrument"
J="$RACINE/docs/superpowers/plans/journaux-micro-e3"
mkdir -p /tmp/e3

# 🔴 `.env` EN PREMIER, mais les réglages de recette sont MÉMORISÉS avant lui :
# `.env` porte `SIGNALING_URL`, et un `source` postérieur les écraserait EN
# SILENCE. Même garde que `e2-lancer.sh`.
GARDE_FAUTE="${MICRO_FAUTE_ECRITURE:-}"
set -a; source "$RACINE/.env"; set +a
[ -n "$GARDE_FAUTE" ] && export MICRO_FAUTE_ECRITURE="$GARDE_FAUTE" || true
source /tmp/f2/env.sh   # le montage de F2, RÉEMPLOYÉ : même VM, même compte, même préfixe

echo "=== [$ETIQUETTE] $(date -u '+%Y-%m-%dT%H:%M:%SZ') — un compte n'est attribuable qu'assorti de son heure ==="
echo "=== [$ETIQUETTE] espace disque (OPFS et les WAV vivent dans /tmp) ==="
df -h /tmp | tail -1

# 🔴 LA VM MEURT TOUTE SEULE, ET LA CAUSE EST IDENTIFIÉE : `libvirtd --timeout
# 120` reçoit un signal 15 et emporte le domaine. ⚠️ **CE N'EST PAS le
# mécanisme d'hibernation de D1**, et les confondre ferait chercher du mauvais
# côté — D1 voyait un `shutdown.exe` INVITÉ (Kernel-Power 187/42), ici c'est
# l'HÔTE qui tue QEMU. Le compteur libvirt les départage.
#
# Cette exécution en a perdu une : la VM est morte à la fin de l'essai 3, et
# l'essai 4 a échoué sur « L'hôte cible est arrêté ou en panne ».
COMPTEUR_AVANT="$(grep -c 'terminating on signal\|shutting down' /var/log/libvirt/qemu/Windows.log 2>/dev/null || echo '?')"
echo "=== [$ETIQUETTE] compteur libvirt AVANT : $COMPTEUR_AVANT ==="
if [ "$(virsh list --all 2>/dev/null | grep -c 'Windows.*en cours')" -eq 0 ]; then
    echo "=== [$ETIQUETTE] la VM est ETEINTE : demarrage ==="
    virsh start Windows >/dev/null 2>&1 || true
    # ⚠️ On attend l'ACCÈS RÉEL à /media/vm, jamais le seul port 5985 ni la
    # seule présence du montage CIFS — dont l'entrée persiste VM éteinte.
    for _ in $(seq 1 90); do ls /media/vm/dev >/dev/null 2>&1 && break; sleep 5; done
fi
ls /media/vm/dev >/dev/null 2>&1 || { echo "=== [$ETIQUETTE] /media/vm INJOIGNABLE : on s arrete ==="; exit 3; }

echo "=== [$ETIQUETTE] agents et fenetres survivants AVANT ==="
node "$RACINE/scripts/winrm.js" 'powershell -NoProfile -ExecutionPolicy Bypass -File C:\dev\e3-tuer.ps1' 2>&1 | tail -2

echo "=== [$ETIQUETTE] artefacts de l execution precedente ==="
rm -f /media/vm/dev/agent.log /media/vm/dev/e2-juge-e3-*.log \
      "$J/pilote-$ETIQUETTE.json" "$J/agent-$ETIQUETTE.log" "$J/agent-$ETIQUETTE-plat.log"

echo "=== [$ETIQUETTE] DEUX fenetres Bloc-notes en session 1, AVANT le superviseur ==="
node "$RACINE/scripts/winrm.js" 'powershell -NoProfile -ExecutionPolicy Bypass -File C:\dev\e3-fenetres.ps1' 2>&1 | tail -4

echo "=== [$ETIQUETTE] pilote (la shell d abord, l agent ensuite) ==="
# ⚠️ `AUDIO_PERIPHERIQUE` n'est PAS posée, et c'est la Décision 8 du plan : en
# multi-fenêtres `loopback_de_session` vaut `config.audio && fenetre_hwnd.is_none()`,
# et `fenetre_hwnd` est `Some` dans un enfant — la garde de boucle locale est
# INERTE par construction. Toute la recette de E2 la posait ; c'est un artefact
# du mono-fenêtre, et la poser ici ferait mesurer autre chose que le produit.
APRES_CONNEXION="cd $RACINE && set -a && source .env && set +a && export AGENT_VM=$AGENT_VM AGENT_SECRET=$AGENT_SECRET SUPERVISEUR=1 SIGNALING_URL=ws://192.168.3.1:8080 RUST_LOG=${NIVEAU_LOG:-info} ${MICRO_FAUTE_ECRITURE:+MICRO_FAUTE_ECRITURE=$MICRO_FAUTE_ECRITURE} && scripts/run-agent.sh" \
UDD="/tmp/e3/udd-$ETIQUETTE" PORT_CDP="${PORT_CDP:-9470}" WAV="${WAV:-/tmp/e3/ton-440.wav}" \
    node "$I/pilote-e3.mjs" "/tmp/e3/pilote-$ETIQUETTE.json" \
    2>&1 | tee "$J/pilote-$ETIQUETTE.log"
CODE=${PIPESTATUS[0]}

echo "=== [$ETIQUETTE] copie du journal d agent (APRES la fin reelle) ==="
# ⚠️ APRÈS la fin réelle : les enfants meurent quand le navigateur se ferme,
# donc APRÈS la copie, et leurs lignes de libération partiraient avec le journal
# suivant. Une pièce a été perdue ainsi en D4.
sleep 6
cp /media/vm/dev/agent.log "$J/agent-$ETIQUETTE.log" 2>/dev/null || echo 'agent.log introuvable'
sed 's/\x1b\[[0-9;]*m//g' "$J/agent-$ETIQUETTE.log" > "$J/agent-$ETIQUETTE-plat.log" 2>/dev/null || true
cp "/tmp/e3/pilote-$ETIQUETTE.json" "$J/pilote-$ETIQUETTE.json" 2>/dev/null || true
for f in /media/vm/dev/e2-juge-e3-*.log; do
    [ -e "$f" ] || continue
    cp "$f" "$J/juge-$ETIQUETTE-$(basename "$f" .log | sed 's/^e2-juge-e3-//').log" 2>/dev/null || true
done

echo "=== [$ETIQUETTE] agents survivants APRES (y compris si l execution a echoue) ==="
node "$RACINE/scripts/winrm.js" 'powershell -NoProfile -ExecutionPolicy Bypass -File C:\dev\e3-tuer.ps1' 2>&1 | tail -2
echo "=== [$ETIQUETTE] survie de la VM : compteur libvirt AVANT=$COMPTEUR_AVANT APRES=$(grep -c "terminating on signal\|shutting down" /var/log/libvirt/qemu/Windows.log 2>/dev/null || echo '?') ==="
echo "=== [$ETIQUETTE] code du pilote : $CODE ==="
exit "$CODE"
