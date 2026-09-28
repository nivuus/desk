#!/usr/bin/env bash
# Rebâtit le client PUIS rejoue la recette. C'est la commande de contrôle des
# rouges qui portent sur le CODE DU CLIENT : sans le rebuild, la recette
# servirait le build d'AVANT la mutation — c'est-à-dire mesurerait le produit
# intact et se croirait verte.
set -uo pipefail
unset -f chpwd 2>/dev/null || true
RACINE="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../../.." && pwd)"
cd "$RACINE"
SORTIE="${1:?sortie.json}"
. /tmp/g5-travail/env
( cd client && npm run --silent build ) > /tmp/g5-travail/rebuild.log 2>&1 || { echo "BUILD ECHOUE"; tail -5 /tmp/g5-travail/rebuild.log; exit 9; }
node docs/superpowers/plans/journaux-gestion-apps-g5/instrument/recette-g5.mjs \
  "http://127.0.0.1:$PORT_CLIENT" "http://127.0.0.1:$PORT_PLATEFORME" "$EMAIL" "$MOTDEPASSE" "$SORTIE" > /dev/null 2>&1
python3 - "$SORTIE" <<'PY'
import json,sys
r=json.load(open(sys.argv[1]))
if 'echec' in r: print('ECHEC DU PILOTE :', r['echec'])
print('launchQueuePresent :', r.get('launchQueuePresent'))
d=r.get('depot')
if d: print('depot avant :', repr(d['avant'])); print('depot apres :', repr(d['apres']))
mh=r.get('manifesteHub')
if mh: print('manifeste hub url :', mh['url'])
for i in r.get('installabilite',[]):
    ie=i['installabilityErrors']
    print(f"  {i['application']:14} url={str(i['url'])[:24]:24} install={[e['errorId'] for e in ie] if isinstance(ie,list) else ie}")
PY
