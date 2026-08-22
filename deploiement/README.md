# Déployer la plateforme — le runbook

Ce répertoire porte tout ce qui sépare « la suite de tests est verte » de « le
service tourne ». Il est court, et il commence par ce qui casse.

---

## Les cinq invariants, et pourquoi

Chacun de ces **cinq** points, s'il est violé, produit une panne **muette** — le
service démarre, la page se charge, et quelque chose ne marche plus sans que
rien ne le dise. C'est pour cela qu'ils sont en tête et non en annexe.

⚠️ **ILS ÉTAIENT QUATRE JUSQU'AU 21 AOÛT 2026** ; le cinquième vient du chantier
`auth-pomerium`, et il est le seul dont la panne ne soit pas seulement muette
mais **ouvrante** — il laisse entrer n'importe qui. ⚠️ Le ⑤ est aussi le seul
qui se répare par **deux** gestes qu'il faut faire ensemble.

### ① Une instance de `plateforme`, et une seule

**Ne jamais répliquer le service.** **Quatre** états de routage vivent en
mémoire du processus :

| Fichier | Ce qu'il retient | Ce que deux instances font |
| --- | --- | --- |
| `signaling/appariement.ts` | la table des sessions et leurs deux pairs | **deux pairs de la même session ne s'apparient jamais** — chacun attend l'autre |
| `agents/registre.ts` | la VM et son socket d'agent courant, plus les ordres en vol | **`POST /session` ne trouve aucun socket** si l'instance qui le traite n'est pas celle qui tient l'agent |
| `signaling/propriete.ts` | qui possède quel nom de session | la même session peut être revendiquée deux fois |
| `securite/frein.ts` | les échecs récents | le budget du frein est **multiplié** par le nombre d'instances |

⚠️ **Ce tableau en a longtemps annoncé TROIS**, et il a été corrigé le 20 août
2026 par la revue transverse de P5. Le manquant — `agents/registre.ts` — est né
du sous-bloc G1, **concurrent** de P5 : aucune revue à l'échelle d'une tâche ne
pouvait le voir apparaître. C'est aussi le plus fréquenté des quatre.

Les deux premières lignes sont les graves : un usager verrait sa page se charger, son
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
silencieuse.

🔴 **« NE JAMAIS Y METTRE `0.0.0.0` » N'EST PLUS UNE DISCIPLINE DE
L'EXPLOITANT : C'EST UNE GARDE DURE, ET ELLE REFUSE LE DÉMARRAGE.** Depuis le
chantier `auth-pomerium` (21 août 2026), `lireConfig` **lève** en mode
`pomerium` sur les quatre écoutes universelles — `0.0.0.0`, `::`, `[::]` et
`*` — au lieu de compter sur la vigilance de qui édite le fichier. Ce
paragraphe a longtemps présenté la chose comme un conseil ; ce n'en est plus
un.

⚠️ **ET CELA MORD SUR LA VALEUR AVEC LAQUELLE LE SERVICE TOURNAIT LA VEILLE.**
Le mode `pomerium` étant le **défaut**, un montage qui posait `0.0.0.0` — ce
qui « marchait » hier — **ne démarre plus du tout** aujourd'hui, et le message
nomme la variable et la raison. Ce n'est pas une régression : c'est le refus
qui remplace l'exposition silencieuse.

🔴 **POURQUOI LA GARDE EST LIÉE AU MODE, ET NON UNIVERSELLE.** En `motdepasse`,
le service s'authentifie lui-même et une écoute large ne le rend pas anonyme.
En `pomerium`, **l'identité arrive dans un en-tête EN CLAIR**
(`X-Pomerium-Claim-Email`), dont aucune signature n'est vérifiée : une écoute
universelle l'offre à quiconque atteint la machine, et c'est un compte pour
n'importe quelle adresse de courriel.

⚠️ **CE QUE LA GARDE NE PROMET PAS**, et son propre commentaire le dit
(`plateforme/src/config.ts`) : elle refuse l'écoute **universelle**, elle ne
garantit pas que « seul le proxy atteint le port ». Ce dernier point reste à la
charge de l'exploitant, et le § 9 de la spec le déclare.

Le reste vaut toujours : sur un hôte qui publierait un port, une écoute large
exposerait le service en clair, jeton compris, en contournant toute la
terminaison TLS.

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

⚠️ **CE QUI PRÉCÈDE DÉCRIT SON RÔLE DANS *CE* MONTAGE (nginx, `motdepasse`),
où elle reste FACULTATIVE.** Depuis le chantier `auth-pomerium` (21 août
2026), la variable porte un **second** rôle — l'autorisation de poser l'en-tête
d'identité `X-Pomerium-Claim-Email` — qui la rend **OBLIGATOIRE** dans l'autre
montage possible de ce service. Voir « Le montage Pomerium », plus bas, qui ne
partage avec celui-ci ni son proxy ni sa méthode de relevé.

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

### ⑤ `PLATEFORME_AUTH: motdepasse` et l'effacement nginx vont ENSEMBLE

🔴 **C'EST LE PLUS DANGEREUX DES CINQ, PARCE QU'IL SE TROMPE EN GRAND ET DANS
LES DEUX SENS À LA FOIS.** Le défaut de `PLATEFORME_AUTH` est **`pomerium`**
(`plateforme/src/config.ts`), et le proxy de CE profil est **nginx**, pas
Pomerium. Deux lignes, et deux seulement, referment l'écart :

| Où | Quoi |
| --- | --- |
| `docker-compose.plateforme.yml`, service `plateforme` | `PLATEFORME_AUTH: motdepasse` |
| `deploiement/nginx.conf`, niveau `http` | `proxy_set_header X-Pomerium-Claim-Email "";` |

**Sans elles, le profil est un contournement COMPLET de l'authentification.**
`GET /auth/moi` échange l'en-tête `X-Pomerium-Claim-Email` contre un jeton
interne **sans vérifier aucune signature** — elle tient pour acquis qu'un
Pomerium l'a posé. nginx ne le pose pas et ne l'efface pas par défaut : celui
qu'un client envoie **traverse verbatim**. Depuis Internet :

```bash
curl -k https://<hôte>/auth/moi -H 'X-Pomerium-Claim-Email: nimporte@qui.tld'
```

rend **un jeton interne valide, et crée le compte**. Et symétriquement, plus
personne ne peut se connecter normalement : le formulaire POSTe
`/auth/connexion`, qui rend `404` en mode `pomerium`.

🔴 **ELLES S'INVERSENT ENSEMBLE, ET C'EST LA PARTIE QU'ON LIT DE TRAVERS.** Il
est **faux** de croire que la directive nginx « resterait juste » le jour où ce
profil passerait derrière Pomerium : `proxy_set_header … "";` efface l'en-tête
**entrant**, donc effacerait aussi celui que Pomerium poserait, et `/auth/moi`
ne verrait plus jamais aucune identité. Ce jour-là, faire les **deux** gestes :
**retirer** la directive nginx **et** passer la variable à `pomerium`. N'en
appliquer qu'une moitié donne, dans un sens, un service que personne ne peut
plus atteindre ; dans l'autre, la porte ouverte ci-dessus.

⚠️ **LA DIRECTIVE VA AU NIVEAU `http`, JAMAIS DANS UN `location`** — nginx
n'hérite pas par fusion mais par **remplacement**, et un seul `proxy_set_header`
dans un bloc plus profond efface **tous** ceux du parent. Ce fichier l'a déjà
payé une fois (voir son encadré, et l'invariant ③).

⚠️ **`nginx -t` NE VOIT RIEN DE TOUT CECI** : la configuration sans la directive
est parfaitement « ok ». Ce qui le juge est de lire la configuration
**réellement chargée**, et de la comparer à un témoin sans la ligne :

```bash
docker run --rm -v "$PWD/deploiement/nginx.conf:/etc/nginx/nginx.conf:ro" \
  -v "$PWD/deploiement/tls:/etc/nginx/tls:ro" nginx:alpine nginx -T \
  | grep -c '^\s*proxy_set_header X-Pomerium-Claim-Email ""'
```

doit rendre **1**. ⚠️ **Chercher le NOM de l'en-tête plutôt que la DIRECTIVE
rend 3** — les commentaires du fichier le citent : le motif est ancré sur la
syntaxe, à dessein.

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
chmod -R a+rX dist                       # ⚠️ obligatoire — voir juste en dessous
```

🔴 **`client/dist/` doit être lisible par l'utilisateur `nginx`, et ce n'est pas
automatique.** Le *master* nginx tourne en `root`, mais ses **workers** tournent
en `nginx` : sur un dépôt cloné avec un umask restrictif, `client/dist` est en
`0750 root:root` et le worker ne peut rien lire. **Mesuré le 20 août 2026, au
critère ⑤ de la recette P5** : `GET /` rendait **403** et ses ressources
**500**, sur une pile par ailleurs entièrement saine — le service, la base et
le routage fonctionnaient tous. Rien dans les journaux d'agent ne le dit ; seul
le journal d'erreur de nginx nomme la permission.

⚠️ **C'est un piège de DÉPLOIEMENT, pas un défaut du produit**, et il ne se voit
ni aux tests, ni à `nginx -t`, ni à `docker compose config`.

### 4. Démarrer

```bash
docker compose -f docker-compose.plateforme.yml --profile deploiement up -d --build
```

⚠️ **`--build` la première fois** : le service est bâti depuis
`plateforme/Dockerfile`, qui part de `node:24-alpine`. Si cette image n'est pas
déjà locale, docker la tire du réseau — **la bâtir avant une recette**, sans
quoi un échec réseau se lira comme un échec du produit.

### 5. Enrôler, et attribuer

✅ **`npm run admin:utilisateur` EST BIEN L'ÉTAPE JUSTE POUR CE PROFIL, ET
C'EST À VÉRIFIER CHAQUE FOIS QUE LE MODE CHANGE.** Le fichier de composition
pose `PLATEFORME_AUTH: motdepasse` pour le service `plateforme` : ce profil
authentifie donc **par mot de passe**, et c'est bien par la création d'un compte
qu'on y ouvre l'accès. 🔴 **Sous le DÉFAUT (`pomerium`), cette étape n'aurait
plus de sens** — `POST /auth/connexion` rendrait `404`, les comptes se
créeraient tout seuls au premier passage de `GET /auth/moi`, et le mot de passe
posé ici ne servirait à rien. Voir l'invariant ⑤ **plus haut** (il ouvre ce
fichier ; ce n'est pas une note de bas de page).

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

## Le montage Pomerium

**Un second montage possible du même service, EXCLUSIF du premier.** Celui
décrit dans « Mettre en route » ci-dessus s'authentifie **lui-même**
(`PLATEFORME_AUTH=motdepasse`) derrière **nginx**, qui termine TLS et sert la
page depuis `client/dist` monté en volume. Celui-ci délègue l'identité à
**Pomerium** (`PLATEFORME_AUTH=pomerium`, le **défaut**), qui termine TLS,
authentifie l'utilisateur par OAuth Google et pose l'en-tête
`X-Pomerium-Claim-Email` — et c'est la plateforme **elle-même** qui sert
désormais la page, `nginx` n'étant plus dans la chaîne. Introduit par le
chantier `auth-pomerium` (21 août 2026) et complété par la variable
`PLATEFORME_PAGE` (22 août 2026), voir `CLAUDE.md` et
`docs/superpowers/specs/2026-08-21-auth-pomerium-design.md` § 7.

🔴 **LES DEUX MONTAGES SONT EXCLUSIFS, ET LE MÉLANGE EST LA PANNE.** Un service
ne peut porter qu'une valeur de `PLATEFORME_AUTH` à la fois. Faire tourner ce
montage-ci en laissant `deploiement/nginx.conf` en face (avec sa directive
`proxy_set_header X-Pomerium-Claim-Email "";` de l'invariant ⑤) effacerait
l'en-tête que Pomerium vient de poser et couperait toute authentification ;
faire tourner le montage nginx sans cette directive, comme l'invariant ⑤ le
dit déjà, est le contournement complet inverse. **Un déploiement choisit l'un
des deux, jamais les deux à la fois sur la même écoute.**

### Les quatre variables du montage

| Variable | Valeur | Pourquoi |
| --- | --- | --- |
| `PLATEFORME_AUTH` | `pomerium` | c'est le **défaut** — l'écrire est une clarté, pas une nécessité. Une valeur inconnue LÈVE |
| `PLATEFORME_HOTE` | `192.168.3.1` | ni `127.0.0.1` (Pomerium tourne en `network_mode: host` et atteint n'importe quelle adresse de l'hôte, mais l'agent Windows, depuis `192.168.3.2`, n'atteint JAMAIS la boucle locale de l'hôte), ni `0.0.0.0` (la garde du § ② ci-dessus **refuse de démarrer** en mode `pomerium`) — voir spec § 7.1, qui pose et vérifie les trois contraintes ensemble |
| `PLATEFORME_PAGE` | le chemin **absolu** de `client/dist` **bâti** (`cd client && npm ci && npm run build`) | **AUCUN DÉFAUT** : absente ou vide, le service ne sert toujours rien et `GET /` rend `404` — c'est ce montage-ci qui a besoin qu'elle soit posée, puisque nginx n'est plus là pour servir la page |
| `PLATEFORME_PROXY_DE_CONFIANCE` | l'adresse **mesurée** de Pomerium (ci-dessous) | **OBLIGATOIRE dans ce montage : le service REFUSE DE DÉMARRER sans elle** en mode `pomerium` (`plateforme/src/config.ts::lireConfig`) — voir l'invariant ③, qui documente son AUTRE rôle |

### Mesurer l'adresse de Pomerium, ne jamais la déduire

🔴 **NE PAS ÉCRIRE UNE ADRESSE EN DUR ICI.** Pomerium tourne en
`network_mode: host` : c'est le **noyau**, pas ce document, qui choisit
l'adresse source d'une connexion sortante vers le port `8080`. Toute valeur
recopiée d'une exécution précédente peut être fausse sur la suivante. La
mesurer sur une connexion **réelle**, pas par déduction :

```bash
# Provoquer une requête RÉELLE depuis Pomerium au préalable (par exemple
# GET /auth/moi), puis, pendant qu'une connexion est établie ou vient de
# l'être :
ss -tn state established '( dport = :8080 or sport = :8080 )'
```

Poser `PLATEFORME_PROXY_DE_CONFIANCE` à l'adresse ainsi lue, jamais à un nom
d'hôte — voir l'avertissement ci-dessous.

⚠️ **PIÈGE MESURÉ PAR LA REVUE : UN NOM D'HÔTE AU LIEU D'UNE ADRESSE REFUSE
TOUT LE MONDE, SANS AUCUNE TRACE.** `http/adresse-source.ts::pairDeConfiance`
compare des **chaînes**, sans jamais résoudre de nom — ni `PLATEFORME_
PROXY_DE_CONFIANCE`, ni `req.socket.remoteAddress` (qui est toujours une
adresse) ne passent par une résolution DNS. Poser un nom d'hôte fait donc
échouer **toute** comparaison, pour **toute** requête, y compris les requêtes
légitimes de Pomerium : `GET /auth/moi` rend `401 pair-non-de-confiance` en
boucle, sans qu'aucune ligne ne soit journalisée côté service (`routes-
identite.ts` ne trace pas ce refus). **Le service répond, la page se charge,
et personne ne peut se connecter** — c'est la même classe de panne muette que
les cinq invariants ci-dessus.

### Lancer le service

Ce montage n'a pas de profil `docker compose` dédié : il tourne sur l'hôte,
directement, pour que Pomerium (lui aussi sur l'hôte, en `network_mode: host`)
et l'agent Windows (sur `192.168.3.0/24`) l'atteignent tous deux à la même
adresse — voir spec § 7.1.

```bash
cd client && npm ci && npm run build && cd ..   # produit client/dist/, servi CETTE FOIS par la plateforme
cd plateforme
set -a && source ../deploiement/plateforme.env && set +a   # secret, URL de base, proxy, PAGE
PLATEFORME_AUTH=pomerium \
PLATEFORME_HOTE=192.168.3.1 \
npm start
```

⚠️ **`deploiement/plateforme.env` reste le même gabarit que pour le montage
nginx** (`deploiement/plateforme.env.exemple`) : il porte le secret, l'URL de
base, `PLATEFORME_PROXY_DE_CONFIANCE` — **obligatoire ici** — et
`PLATEFORME_PAGE`, **commentée dans le gabarit** parce que le montage nginx ne
doit rien servir lui-même : **décommenter cette ligne et la faire pointer vers
`client/dist` bâti, en chemin ABSOLU**, est le seul geste propre à ce montage
sur ce fichier. `PLATEFORME_AUTH` et `PLATEFORME_HOTE`, eux, restent hors du
gabarit — ce sont des littéraux qui CHOISISSENT le montage, pas des secrets ni
des chemins propres à une machine, à l'image de ce que
`docker-compose.plateforme.yml` pose déjà en clair pour le premier montage.

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
