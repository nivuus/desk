#!/usr/bin/env bash
# THE SETUP OF ACCEPTANCE RUN G5 — fresh database, platform, static client.
#
# 🔴 NO VM, NO AGENT (decision D7 of the plan). The three criteria measure
#    a BROWSER; the VM is held by a neighbouring workstream, and putting it in
#    G5's critical path would make each iteration costly and each
#    failure ambiguous.
#
# ⚠️ THE PORTS ARE READ, NOT ASSUMED: "a port free by convention
#    is not" — G3 and P4 each lost a run that way.
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
# An acceptance secret, generated at each setup: it does not outlive the
# working directory, and it is written in no versioned file.
export PLATEFORME_SECRET_JETON="$(openssl rand -hex 32)"
export PLATEFORME_ORIGINE_CLIENT="http://127.0.0.1:$PORT_CLIENT"
export PLATEFORME_ICONES="$TRAVAIL/icones"
export PLATEFORME_TELEVERSEMENTS="$TRAVAIL/televersements"

EMAIL="recette-g5@exemple.test"
MOTDEPASSE="motdepasse-de-recette-g5"

echo "=== ports retenus : plateforme $PORT_PLATEFORME, client $PORT_CLIENT ==="

# ① the account. The password goes through STDIN — never through argv, which `ps`
#    exposes to any user of the machine (P2's lesson, held by a test).
( cd plateforme && printf '%s\n' "$MOTDEPASSE" | npm run --silent admin:utilisateur -- --email "$EMAIL" ) \
  > "$TRAVAIL/00-utilisateur.log" 2>&1

# ② the VM, and its assignment.
( cd plateforme && npm run --silent admin:agent -- --vm g5 --adresse 127.0.0.1 ) \
  > "$TRAVAIL/01-agent.log" 2>&1
( cd plateforme && npm run --silent admin:attribuer -- --email "$EMAIL" --vm g5 ) \
  > "$TRAVAIL/02-attribuer.log" 2>&1

# ③ seeding, THROUGH THE REPOSITORY'S MODULES.
( cd plateforme && npx tsx ../docs/superpowers/plans/journaux-gestion-apps-g5/instrument/ensemencer.mts g5 ) \
  > "$TRAVAIL/03-ensemencer.log" 2>&1

# ④ the client, built then served statically. `vite build` first: without it,
#    §7.3 and §7.7 would judge the previous build, and the acceptance run would serve a page
#    that is not the one just written.
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

# We wait for the FACT — that both answer —, never a duration.
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
