#!/usr/bin/env bash
# Sets up the environment of acceptance run A1: a SECOND platform instance,
# a vite for the client, a user, a VM, an enrolled agent.
#
# 🔴 A SECOND INSTANCE, NEVER A RESTART OF THE NEIGHBOUR'S: a
# concurrent workstream already listens on 8080, and restarting it would invalidate its
# tokens. It is the decision the clipboard's P1 took for the same reason.
set -euo pipefail
unset -f chpwd 2>/dev/null || true
cd "$(git rev-parse --show-toplevel)"

PORT="${PORT_A1:-8092}"
PORT_CLIENT="${PORT_CLIENT_A1:-5176}"
BASE="/tmp/a1-plateforme.sqlite"
IDENT="/tmp/a1-identite.env"
EMAIL="a1@recette.local"
MDP="a1-recette-$(head -c 6 /dev/urandom | od -An -tx1 | tr -d ' \n')"
VM="a1vm"

rm -f "$BASE" "$IDENT"
export PLATEFORME_HOTE=0.0.0.0
export PLATEFORME_PORT="$PORT"
export PLATEFORME_BASE=sqlite
export PLATEFORME_BASE_URL="$BASE"
export PLATEFORME_SECRET_JETON="$(head -c 32 /dev/urandom | od -An -tx1 | tr -d ' \n')"
export PLATEFORME_ORIGINE_CLIENT="http://127.0.0.1:${PORT_CLIENT}"

cd plateforme
# Migrations run at startup: we launch, we wait, we administer.
npm start > /tmp/a1-plateforme.log 2>&1 &
echo $! > /tmp/a1-plateforme.pid
cd ..
for _ in $(seq 1 60); do
    grep -q "le port ${PORT}" /tmp/a1-plateforme.log 2>/dev/null && break
    sleep 1
done
grep -q "le port ${PORT}" /tmp/a1-plateforme.log || { echo "🔴 plateforme non démarrée"; tail -5 /tmp/a1-plateforme.log; exit 1; }
echo "plateforme sur $PORT"

cd plateforme
printf '%s\n' "$MDP" | npm run --silent admin:utilisateur -- --email "$EMAIL" > /tmp/a1-user.log 2>&1
ENROL=$(npm run --silent admin:agent -- --vm "$VM" --adresse 192.168.3.2 2>&1 | tee /tmp/a1-enrol.log)
cd ..

VMID=$(grep -oE 'vm_id=[0-9a-f-]+' /tmp/a1-enrol.log | head -1 | cut -d= -f2)
SECRET=$(grep -oiE 'secret[^A-Za-z0-9]+[A-Za-z0-9_-]{16,}' /tmp/a1-enrol.log | head -1 | grep -oE '[A-Za-z0-9_-]{16,}$')
PREFIXE=$(grep -oiE 'prefixe[^A-Za-z0-9]+[A-Za-z0-9_-]{16,}' /tmp/a1-enrol.log | head -1 | grep -oE '[A-Za-z0-9_-]{16,}$')

cd plateforme && npm run --silent admin:attribuer -- --email "$EMAIL" --vm "$VM" > /tmp/a1-attrib.log 2>&1; cd ..

cat > "$IDENT" <<EOF
AGENT_VM=$VMID
AGENT_SECRET=$SECRET
PREFIXE_VM=$PREFIXE
RECETTE_EMAIL=$EMAIL
RECETTE_MOTDEPASSE=$MDP
PLATEFORME_URL=http://127.0.0.1:$PORT
CLIENT_URL=http://127.0.0.1:$PORT_CLIENT
SIGNALING_WS=ws://192.168.3.1:$PORT
EOF
echo "identité : $IDENT"

cd client
npx vite --port "$PORT_CLIENT" --host 127.0.0.1 > /tmp/a1-vite.log 2>&1 &
echo $! > /tmp/a1-vite.pid
cd ..
for _ in $(seq 1 40); do
    curl -sf "http://127.0.0.1:${PORT_CLIENT}/shell.html" -o /dev/null && break
    sleep 1
done
curl -sf "http://127.0.0.1:${PORT_CLIENT}/shell.html" -o /dev/null \
    && echo "vite sur $PORT_CLIENT" || { echo "🔴 vite non démarré"; tail -5 /tmp/a1-vite.log; exit 1; }
