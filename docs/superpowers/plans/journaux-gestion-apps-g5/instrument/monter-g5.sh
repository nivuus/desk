#!/usr/bin/env bash
# LE MONTAGE DE LA RECETTE G5 — base neuve, plateforme, client statique.
#
# 🔴 AUCUNE VM, AUCUN AGENT (décision D7 du plan). Les trois critères mesurent
#    un NAVIGATEUR ; la VM est tenue par un chantier voisin, et la mettre dans
#    le chemin critique de G5 rendrait chaque itération coûteuse et chaque
#    échec ambigu.
#
# ⚠️ LES PORTS SONT RELEVÉS, PAS SUPPOSÉS : « un port libre par convention ne
#    l'est pas » — G3 et P4 ont chacun perdu une exécution ainsi.
set -euo pipefail
unset -f chpwd 2>/dev/null || true

RACINE="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../../.." && pwd)"
cd "$RACINE"

TRAVAIL="${1:?usage : monter-g5.sh <repertoire-de-travail>}"
mkdir -p "$TRAVAIL"

libre() {
  local p
  for p in $(seq "$1" "$(( $1 + 200 ))"); do
    if ! ss -ltn 2>/dev/null | grep -q ":${p} "; then echo "$p"; return; fi
  done
  echo "aucun port libre a partir de $1" >&2; exit 1
}

PORT_PLATEFORME="$(libre 8300)"
PORT_CLIENT="$(libre 8500)"

export PLATEFORME_HOTE=127.0.0.1
export PLATEFORME_PORT="$PORT_PLATEFORME"
export PLATEFORME_BASE=sqlite
export PLATEFORME_BASE_URL="$TRAVAIL/g5.sqlite"
# Un secret de recette, engendré à chaque montage : il ne survit pas au
# répertoire de travail, et il n'est écrit dans aucun fichier versionné.
export PLATEFORME_SECRET_JETON="$(openssl rand -hex 32)"
export PLATEFORME_ORIGINE_CLIENT="http://127.0.0.1:$PORT_CLIENT"
export PLATEFORME_ICONES="$TRAVAIL/icones"
export PLATEFORME_TELEVERSEMENTS="$TRAVAIL/televersements"

EMAIL="recette-g5@exemple.test"
MOTDEPASSE="motdepasse-de-recette-g5"

echo "=== ports retenus : plateforme $PORT_PLATEFORME, client $PORT_CLIENT ==="

# ① le compte. Le mot de passe passe par STDIN — jamais par l'argv, que `ps`
#    expose à tout utilisateur de la machine (leçon de P2, tenue par un test).
( cd plateforme && printf '%s\n' "$MOTDEPASSE" | npm run --silent admin:utilisateur -- --email "$EMAIL" ) \
  > "$TRAVAIL/00-utilisateur.log" 2>&1

# ② la VM, et son attribution.
( cd plateforme && npm run --silent admin:agent -- --vm g5 --adresse 127.0.0.1 ) \
  > "$TRAVAIL/01-agent.log" 2>&1
( cd plateforme && npm run --silent admin:attribuer -- --email "$EMAIL" --vm g5 ) \
  > "$TRAVAIL/02-attribuer.log" 2>&1

# ③ l'ensemencement, PAR LES MODULES DU DÉPÔT.
( cd plateforme && npx tsx ../docs/superpowers/plans/journaux-gestion-apps-g5/instrument/ensemencer.mts g5 ) \
  > "$TRAVAIL/03-ensemencer.log" 2>&1

# ④ le client, bâti puis servi en statique. `vite build` d'abord : sans lui,
#    §7.3 et §7.7 jugeraient le build d'avant, et la recette servirait une page
#    qui n'est pas celle qu'on vient d'écrire.
( cd client && npm run --silent build ) > "$TRAVAIL/04-build.log" 2>&1
( cd client/dist && python3 -m http.server "$PORT_CLIENT" --bind 127.0.0.1 ) \
  > "$TRAVAIL/05-client.log" 2>&1 &
PID_CLIENT=$!

# ⑤ la plateforme.
( cd plateforme && npm run --silent start ) > "$TRAVAIL/06-plateforme.log" 2>&1 &
PID_PLATEFORME=$!

echo "$PID_CLIENT $PID_PLATEFORME" > "$TRAVAIL/pids"
cat > "$TRAVAIL/env" <<EOF
PORT_PLATEFORME=$PORT_PLATEFORME
PORT_CLIENT=$PORT_CLIENT
EMAIL=$EMAIL
MOTDEPASSE=$MOTDEPASSE
EOF

# On attend le FAIT — que les deux répondent —, jamais une durée.
for _ in $(seq 1 100); do
  if curl -sf "http://127.0.0.1:$PORT_CLIENT/hub.html" > /dev/null \
     && curl -sf "http://127.0.0.1:$PORT_PLATEFORME/sante" > /dev/null; then
    echo "=== montage pret ==="; exit 0
  fi
  sleep 0.3
done
echo "=== ECHEC : la plateforme ou le client ne repond pas ===" >&2
tail -20 "$TRAVAIL/06-plateforme.log" >&2 || true
exit 1
