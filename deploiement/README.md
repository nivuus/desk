# Déployer la plateforme — le runbook

Ce répertoire porte tout ce qui sépare « la suite de tests est verte » de « le
service tourne ». Il est court, et il commence par ce qui casse.

---

## Les quatre invariants, et pourquoi

Chacun de ces quatre points, s'il est violé, produit une panne **muette** — le
service démarre, la page se charge, et quelque chose ne marche plus sans que
rien ne le dise. C'est pour cela qu'ils sont en tête et non en annexe.

### ① Une instance de `plateforme`, et une seule

**Ne jamais répliquer le service.** Trois états de routage vivent en mémoire du
processus :

| Fichier | Ce qu'il retient | Ce que deux instances font |
| --- | --- | --- |
| `signaling/appariement.ts` | la table des sessions et leurs deux pairs | **deux pairs de la même session ne s'apparient jamais** — chacun attend l'autre |
| `signaling/propriete.ts` | qui possède quel nom de session | la même session peut être revendiquée deux fois |
| `securite/frein.ts` | les échecs récents | le budget du frein est **multiplié** par le nombre d'instances |

La première ligne est la grave : un usager verrait sa page se charger, son
jeton être accepté, et **le média ne jamais s'établir**. Aucun journal ne le
dirait, parce que du point de vue de chaque instance il ne s'est rien passé
d'anormal — elle attend un pair, ce qui est un état parfaitement normal.

⚠️ **Aucun équilibrage par adresse collante ne referme cela** : les deux pairs
d'une session sont un navigateur et une VM, qui n'ont aucune raison de partager
une adresse.

🔴 **Rien dans le code ne l'empêche.** `deploy.replicas: 1` dans le fichier de
composition et l'unique `server` de l'`upstream` de `nginx.conf` sont des
**déclarations**, pas des gardes : `--scale plateforme=2` fonctionne et produit
la panne. Le remède minimal est nommé et **non livré** — refuser de démarrer si
une seconde instance est détectée, par un verrou consultatif en base
(`pg_advisory_lock` côté Postgres, sans équivalent SQLite) —, ce qui échangerait
la panne muette contre une rupture bruyante. **C'est la dette la plus lourde de
tout le sous-projet ⑤.**

### ② `PLATEFORME_HOTE` n'est jamais une adresse publique

La plateforme **ne termine jamais TLS** : elle écoute en clair. Son écoute doit
donc n'être joignable que par le proxy.

Le fichier de composition pose `PLATEFORME_HOTE: plateforme` — le **nom du
service**. Ce n'est pas une commodité : Node résout le nom passé à `listen()` et
se lie à l'**adresse résolue**, pas à toutes les interfaces. Le nom du service
résolvant à l'adresse du conteneur sur le réseau interne, l'écoute y est bornée.

⚠️ **`PLATEFORME_HOTE` n'a aucun défaut**, et `lireConfig` **lève** s'il manque.
C'est délibéré : une rupture bruyante vaut mieux qu'une écoute universelle
silencieuse. Ne jamais y mettre `0.0.0.0` « pour que ça marche » — sur un hôte
qui publierait un port, cela exposerait le service en clair, jeton compris,
en contournant toute la terminaison TLS.

### ③ `PLATEFORME_PROXY_DE_CONFIANCE` doit porter l'adresse du proxy

C'est l'invariant le plus facile à rater, parce qu'il se trompe **dans les deux
sens** et que ni l'un ni l'autre ne fait de bruit.

`http/adresse-source.ts` ne croit l'en-tête `X-Forwarded-For` que d'un pair
figurant dans cette liste.

- **Absente ou fausse** — l'en-tête n'est pas cru, donc toutes les requêtes
  portent l'adresse du **proxy**. Elles partagent alors **une seule clé de
  frein** : le frein par adresse dégénère en **frein global**, et le premier
  attaquant bloque tous les usagers.
- **Trop large** — n'importe qui peut se déclarer sous l'adresse de son choix,
  et le frein par adresse ne freine plus rien du tout.

Relever la bonne valeur, après le premier démarrage :

```bash
docker compose -f docker-compose.plateforme.yml --profile deploiement \
  exec plateforme getent hosts proxy
```

### ④ Relancer le service **avec** son environnement

**Vérifier l'environnement du processus qui écoute réellement, pas de celui
qu'on croit avoir lancé.** Ce dépôt a déjà payé ce piège au chantier TURN : un
serveur de signaling tournait depuis 36 h sans ses variables, les sessions ne
recevaient aucune configuration ICE, et **rien ne le signalait**.

```bash
# le PID qui écoute réellement, et son environnement
P=$(docker compose -f docker-compose.plateforme.yml --profile deploiement \
      exec -T plateforme sh -c 'echo $$')
docker compose -f docker-compose.plateforme.yml --profile deploiement \
  exec -T plateforme sh -c 'tr "\0" "\n" < /proc/1/environ | grep ^PLATEFORME_ | cut -d= -f1'
```

⚠️ La commande ci-dessus n'imprime que les **noms**, jamais les valeurs.

---

## Mettre en route

### 1. Les deux fichiers de valeurs

Ils ne sont **pas versionnés** (`.gitignore`). Leurs gabarits le sont, et
portent la raison de chaque entrée.

```bash
cp deploiement/postgres.env.exemple   deploiement/postgres.env
cp deploiement/plateforme.env.exemple deploiement/plateforme.env
# puis les remplir — les secrets se TIRENT AU SORT :
openssl rand -base64 32   # POSTGRES_PASSWORD
openssl rand -base64 48   # PLATEFORME_SECRET_JETON
```

Si l'un des deux manque, la commande de déploiement échoue **en le nommant** —
et la commande *sans* profil, celle de la suite de tests, reste intacte.

### 2. Le matériel TLS

`deploiement/tls/` doit porter `fullchain.pem` et `privkey.pem`. Le répertoire
n'est pas versionné.

```bash
mkdir -p deploiement/tls
# Pour une RECETTE seulement — un navigateur refusera ce certificat :
openssl req -x509 -newkey rsa:2048 -nodes -days 30 \
  -keyout deploiement/tls/privkey.pem -out deploiement/tls/fullchain.pem \
  -subj '/CN=localhost'
```

⚠️ **nginx refuse de démarrer** si l'un des deux ne se lit pas, en le nommant.
C'est voulu : une rupture immédiate plutôt qu'un échec au premier handshake.

### 3. Bâtir la page

```bash
cd client && npm ci && npm run build     # produit client/dist/, que le proxy sert
```

### 4. Démarrer

```bash
docker compose -f docker-compose.plateforme.yml --profile deploiement up -d --build
```

⚠️ **`--build` la première fois** : le service est bâti depuis
`plateforme/Dockerfile`, qui part de `node:24-alpine`. Si cette image n'est pas
déjà locale, docker la tire du réseau — **la bâtir avant une recette**, sans
quoi un échec réseau se lira comme un échec du produit.

### 5. Enrôler, et attribuer

```bash
cd plateforme
npm run admin:utilisateur -- --email <adresse>
npm run admin:agent       -- --vm <nom> --adresse <hôte>   # imprime AGENT_SECRET UNE FOIS
npm run admin:attribuer   -- --email <adresse> --vm <id>
```

Si un secret d'agent fuite :

```bash
npm run admin:agent -- --vm <id> --roter
```

Le secret est remplacé, imprimé une seule fois, et **le préfixe de session
n'est pas touché** — le changer couperait toutes les sessions en cours de cette
VM. ⚠️ La rotation ne révoque pas les **jetons déjà délivrés**, qui restent
valides jusqu'à leur expiration (`DUREE_JETON_ACCES_MS`).

### 6. Vérifier

```bash
curl -sk https://<hôte>/sante          # le verdict, sans aucun détail divulgué
docker compose -f docker-compose.plateforme.yml --profile deploiement logs -f plateforme
```

---

## Ce que ce déploiement NE couvre PAS

Écrit ici plutôt que découvert.

### TURNS — et donc les réseaux restrictifs

Le relais TURN est **en clair sur 3478**, et il n'y a pas de TURNS. Trois
raisons, dans `docker-compose.coturn.yml` : 443 est occupé par un tiers sur la
machine de mesure ; sur une adresse unique, TURNS/443 et le proxy HTTPS/443 se
disputent le même port, ce qui exigerait une seconde adresse ou un multiplexage
ALPN jamais éprouvé ici ; et TURNS sur 5349 demanderait un certificat pour un
nom public, donc ACME, donc une infrastructure de nommage que ce dépôt n'a pas.

🔴 **Conséquence : la cible « réseaux restrictifs » du cadrage n'est pas
couverte.** Ces réseaux filtrent précisément l'UDP sur 3478. Un client derrière
l'un d'eux n'établira pas sa session, et rien ne le rattrapera.

⚠️ **Et l'inaccessibilité du relais depuis Internet n'est pas établie non
plus** : elle se jugerait sur une sonde extérieure, depuis une machine hors de
ce réseau, qui n'a pas été faite. Ce qui **est** mesuré est que coturn n'ouvre
plus que sur l'adresse nommée — 23 adresses distinctes avant, une seule après.

### La réplication

Voir l'invariant ①. Le service **ne peut pas** être répliqué, et rien ne
l'empêche de l'être.

### Le secret d'enrôlement sur la VM

`scripts/run-agent.sh` écrit `AGENT_SECRET` **en clair** dans
`C:\dev\run-agent.ps1`, sur un partage CIFS lisible depuis l'hôte — comme les
cinquante-sept autres variables. C'est **assumé** pour une VM de développement :
le retirer exigerait de modifier `scripts/`, de faire lire à `agent/` un coffre
Windows (DPAPI), et d'éprouver le résultat sur la VM.

**La contrepartie livrée est la rotation** (`--roter`, §5 ci-dessus) : le vol
d'un secret devient **réparable**, ce qu'il n'était pas — `depot/agent.ts` ne
savait qu'insérer, et réenrôler une VM déjà enrôlée lève sur la clé primaire.
L'exploitant n'avait, jusqu'ici, que le `DELETE` manuel en base.

### La sauvegarde

Le volume `postgres-deploiement` porte toute la base. **Rien dans ce dépôt ne le
sauvegarde**, et `down -v` le détruit. `down` sans `-v` le conserve.

---

## Les deux pièges de commande, mesurés

🔴 **`docker compose … config` imprime les secrets en clair.** Deux fois plutôt
qu'une :

- sur `docker-compose.coturn.yml`, il rend `--static-auth-secret=<valeur>` ;
- sur le profil `deploiement`, il **recopie le contenu des `env_file`** dans le
  bloc `environment:` — `PLATEFORME_SECRET_JETON` et le mot de passe de
  `PLATEFORME_BASE_URL` compris.

**Sa sortie ne se colle donc jamais dans un ticket, un journal ou un rapport.**
Filtrer par **liste blanche** de lignes, jamais par liste noire, puis prouver
l'absence du secret par un `grep -c -F -f` qui rend un **compte**.

⚠️ **`${VAR:?}` engage toutes les sous-commandes.** Sur
`docker-compose.coturn.yml`, `logs`, `ps` et `down` exigent eux aussi
`TURN_LISTENING_IP` et `TURN_RELAY_IP`. Un `logs` lancé sans elles rend le
message d'interpolation **à la place du journal** — et un compteur de lignes
tombe alors à zéro, indiscernable d'un coturn qui n'aurait rien ouvert.
