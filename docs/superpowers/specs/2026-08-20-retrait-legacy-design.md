# Retrait du legacy Guacamole — conditions de mort de l'ancien produit

**Date** : 20 août 2026
**Statut** : ⛔ **Spécification de CONDITIONS — ce chantier n'est PAS exécutable aujourd'hui**
**Portée** : le Guacamole historique (guacd / RDP RemoteApp / FUSE / WinRM) et
ce qui l'entoure dans ce dépôt

---

## 0. Ce que ce document est, et ce qu'il n'est pas

Le cadrage produit écrit une seule phrase sur ce chantier
(`2026-07-27-refonte-produit-design.md:305`, §11) :

> « L'ancien code (guacd/FUSE/WinRM) n'est pas modifié ; il reste fonctionnel
> jusqu'au remplacement, **puis sera supprimé**. »

Cette phrase ne dit ni **quoi**, ni **quand**, ni **à quelle condition**. Elle
est le seul mandat existant, et elle est écrite au futur depuis treize
sous-blocs. Ce document ne programme pas le retrait : **il en écrit les
conditions**, avant qu'on soit tenté de le faire trop tôt — ou, plus
probablement au vu du §3 ci-dessous, trop tard.

**Ce document n'autorise aucune suppression.** Une seule exception est
identifiée et argumentée au §8, et elle ne supprime pas une ligne de code.

**Trois choses en sortent, et elles ne se substituent pas l'une à l'autre :**

1. l'**inventaire** de ce qui devra mourir, relevé par la commande (§2) ;
2. la **matrice de remplacement**, fonction par fonction, avec ce qui n'est
   repris **par personne** (§4) ;
3. les **conditions falsifiables** de chaque retrait (§5), c'est-à-dire des
   énoncés dont l'évaluation peut rendre **NON**.

⚠️ **Convention d'écriture** : `…/` abrège `docs/superpowers/`.

⚠️ **Tous les relevés de ce document ont été pris le 20 août 2026 et ils
DÉRIVENT.** Ce dépôt a payé neuf fois le naufrage du « 487 » (voir `CLAUDE.md`,
§ Conventions de code) : ne recopier aucun nombre d'ici sans relancer sa
commande, qui est donnée à chaque fois.

---

## 1. Pourquoi ce chantier n'est pas exécutable aujourd'hui

La matrice du §4 le détaille, mais le fait tient en une ligne : **le nouveau
produit n'a pas de hub**, et **son pont fichiers est en lecture seule**.

- `client/vite.config.ts` déclare les entrées HTML du client ; le répertoire
  `client/` porte six pages (`connexion.html`, `design.html`, `index.html`,
  `primitives.html`, `probe-coalesced.html`, `shell.html`) et **aucune n'est un
  catalogue d'applications**. Le plan du design system le dit sans détour :
  « le **hub**, que la spec §6 exclut explicitement »
  (`docs/superpowers/plans/2026-08-19-design-system-s3.md:292`).
- Le pont fichiers livré par F1 « vit tout entier » dans l'état
  `ERROR_WRITE_PROTECT` (`docs/superpowers/specs/2026-08-19-pont-fichiers-design.md:702`).
  L'écriture est « un livrable de **F2**, pas une » de F1 (même fichier, l. 297).

Deux autres fonctions du produit historique n'ont, elles, **aucune ligne de
code** dans le nouveau : le **presse-papier** (dans aucun des deux sens) et le
**microphone jusqu'à une application Windows** — le bloc E1 s'arrête au PCM
décodé dans l'agent (`…/plans/2026-08-19-micro-resultats.md:39-40`).

Tant que ces quatre points tiennent, retirer le legacy retire des fonctions que
rien ne remplace. **La condition d'entrée de tout le chantier est donc le §5.**

---

## 2. Inventaire du legacy, relevé par la commande

### 2.1 Le code écrit à la main — 3 170 lignes

```bash
wc -l index.js src/*.js web/*.js assets/*.html test_winrm_*.js \
      Dockerfile docker-compose.yml package.json
```

| Fichier | Lignes | Rôle | Suivi par git ? |
| --- | --- | --- | --- |
| `web/index.js` | **804** | client de session (tunnel Guacamole, souris/clavier/tactile, audio, presse-papier, FSA, WCO) | 🔴 **NON** |
| `src/file.js` | 414 | pont FUSE ⇆ WebSocket, cache LRU | oui |
| `src/iconExtractor.js` | 322 | extraction d'icônes (5 méthodes, PowerShell retenue) | oui |
| `src/session.js` | 304 | sessions guacd, réglages RDP, cycle de vie | oui |
| `src/lnkParser.js` | 200 | parseur `.lnk` maison (MS-SHLLINK) | oui |
| `src/asset.js` | 164 | build Gulp, manifestes PWA, images, `/sw.js` | oui |
| `src/app.js` | 151 | découverte d'apps, couleur, associations de fichiers | oui |
| `assets/index.html` | 148 | page d'accueil (catalogue) | oui |
| `test_winrm_fixed.js` | 135 | essai WinRM | oui |
| `test_winrm_nodejs.js` | 111 | essai WinRM | oui |
| `web/home.js` | 93 | logique de la page d'accueil | oui |
| `assets/app.tpl.html` | 89 | gabarit de la page de session | oui |
| `index.js` | **61** | point d'entrée, `server.listen(3445)`, **secrets en clair** | 🔴 **NON** |
| `src/cleanup.js` | 47 | arrêt gracieux | oui |
| `web/sw.js` | 45 | service worker | oui |
| `package.json` | 30 | dépendances **+ un script du NOUVEAU produit** (§2.5) | oui |
| `docker-compose.yml` | **25** | service `web`, **secrets en clair** | 🔴 **NON** |
| `Dockerfile` | 23 | image `debian:11-slim` + guacd + Node 20 + nodemon | oui |
| `web/fs.js` | 4 | orphelin (voir §8) | oui |
| **Total** | **3 170** | dont **2 280 suivis** et **890 jamais commités** | |

⚠️ **Neuf de ces fichiers n'ont pas de saut de ligne final**, dont `src/app.js` :
`wc -l` y rend **151** alors que `sed -n '152p'` rend bien une ligne. Les
citations de ce document désignent des lignes **au sens de `sed`** ; le tableau
ci-dessus compte des lignes **au sens de `wc`**. Les deux peuvent différer de
un, et c'est le cas ici — ce n'est pas une citation fausse.

### 2.2 🔴 Le fait le plus lourd de l'inventaire : 890 lignes n'existent PAS dans git

```bash
git log --all --oneline -- web/index.js      # → vide
git log --all --oneline -- index.js          # → vide
git log --all --oneline -- docker-compose.yml # → vide
git check-ignore -v index.js web/index.js
#   .gitignore:18:index.js	index.js
#   .gitignore:18:index.js	web/index.js
```

**Les trois commandes `git log` ne rendent aucune ligne : ces fichiers n'ont
jamais été commités, sur aucune branche.** Le motif `.gitignore:18` — écrit
pour protéger le `index.js` de la racine, qui porte des mots de passe Windows
en clair (`index.js:2-4`) — n'a **pas d'ancre de début de chemin** et attrape
donc `web/index.js` **à n'importe quelle profondeur**.

⚠️ **Conséquence pour le retrait, et c'est la plus grave de ce document :
« l'historique git reste » est FAUX pour 890 lignes sur 3 170.** Supprimer
`web/index.js` — le client de session, 804 lignes, le seul endroit où vivent
les contournements de presse-papier, de Window Controls Overlay et de File
System Access de l'ancien produit — **le détruit définitivement**. Le §6 en
tire une obligation d'archivage.

⚠️ **Le même motif est une mine pour le NOUVEAU produit.** Aucun fichier du
nouveau produit ne s'appelle `index.js` aujourd'hui :

```bash
find . -name index.js -not -path "*/node_modules/*" -not -path "./target/*" \
       -not -path "./.claude/*"
#   ./index.js  ./web/index.js  ./src/dist/index.js  ./dist/index.js
```

Mais un futur `plateforme/src/index.js` ou `client/src/index.js` serait ignoré
**en silence**. Le retrait du legacy est ce qui permet de supprimer ce motif.

### 2.3 🔴 Le legacy est invisible à la règle des 500 lignes, qui le nomme pourtant

`CLAUDE.md:20` (§ « Portée » des Conventions de code) écrit :

> `agent/src/`, `client/src/`, `plateforme/`, `proto/`, `src/`, `web/`, `scripts/`.

`src/` et `web/` **sont dans la portée**. Or la commande de vérification du même
fichier, lancée le 20 août 2026, rend :

```
  83432 total
   1536 agent/src/encode.rs
    630 agent/src/windows_source.rs
```

**`web/index.js`, 804 lignes, n'y figure pas** — et il devrait, à 304 lignes
au-dessus du plafond. La commande énumère `git ls-files` puis
`git ls-files --others --exclude-standard` : un fichier **ignoré** lui est
structurellement invisible. La table de dette de `CLAUDE.md` a donc **deux**
entrées là où le texte de sa propre portée en impose **trois**.

⚠️ **Ce n'est pas une dette à purger : c'est un constat à inscrire.** La règle
gèle la dette, elle ne la traite pas ; et le remède ici n'est pas de découper
`web/index.js` mais de le retirer. **Le §9 le porte en L1, parce qu'une portée
qui ment est plus coûteuse qu'un fichier trop long.**

### 2.4 Les artefacts, les dépendances, l'image

| Objet | Mesure | Commande |
| --- | --- | --- |
| `dist/` | **3,6 Mo**, 6 fichiers, dont `guacamole-common.min.js` (261 Ko, vendorisé) et `index.js` (2,98 Mo) | `du -sh dist ; ls -la dist` |
| `src/dist/` | **2,0 Mo**, un seul fichier daté du 12 mars 2024 | `du -sh src/dist` |
| `node_modules/` racine | **141 Mo**, **522 paquets** | `du -sh node_modules ; ls node_modules | wc -l` |
| `package-lock.json` | **14 583 lignes** | `wc -l package-lock.json` |
| image `guacamole-web:latest` | **806 Mo**, créée il y a 10 mois | `docker images guacamole-web` |
| conteneur `guacamole-web-1` | **Up 16 h**, `restarts=0` | `docker inspect guacamole-web-1` |
| core dumps `core.*` | **7 fichiers, 390 Mo** | `du -ch core.*` |

Les 21 dépendances de `package.json` se répartissent ainsi (relevé par un
balayage de `require()` sur `index.js`, `src/`, `web/`, `test_winrm_*.js` et
`scripts/`) :

- **legacy seul (20)** : `@babel/core`, `@babel/preset-env`, `colorthief`,
  `express`, `ftp-srv`, `fuse-native`, `guacamole-common-js`, `guacamole-lite`,
  `gulp`, `gulp-babel`, `gulp-browserify`, `lodash`, `mime-types`, `rgb2hex`,
  `sharp`, `sharp-ico`, `to-ico`, `uuid`, `wait-port`, `ws` ;
- 🔴 **partagée avec le nouveau produit (1)** : **`nodejs-winrm`** — voir §2.5.

⚠️ `@babel/core` et `@babel/preset-env` n'apparaissent dans aucun `require()`
direct : ils sont chargés par `gulp-babel` via le nom de preset
(`src/asset.js:21`). Ne pas conclure de l'absence de `require` qu'ils sont
inutiles.

### 2.5 🔴 Ce qui est PARTAGÉ avec le nouveau produit — trois points, et un seul est bénin

**① `nodejs-winrm` et le `package.json` de la racine.** `scripts/winrm.js:3`
fait `require('nodejs-winrm')`, et ce script est l'unique voie d'accès WinRM à
la VM pour **quatre scripts du nouveau produit** :

```bash
grep -rln "scripts/winrm.js" scripts/
#   scripts/build-agent.sh  scripts/check-session.sh
#   scripts/run-agent.sh    scripts/stop-agent.sh
```

`package.json` porte d'ailleurs `"winrm": "node scripts/winrm.js"` dans ses
`scripts`. **Le `package.json` de la racine et son `node_modules` ne peuvent
donc pas être supprimés en bloc** : `nodejs-winrm` doit leur survivre, ou
`scripts/winrm.js` doit changer de socle.

**② `docker-compose.yml` et le relais TURN.** L'en-tête de
`docker-compose.coturn.yml:9` prescrit :

```
docker compose -f docker-compose.yml -f docker-compose.coturn.yml up -d coturn
```

et `CLAUDE.md` reprend cette commande. La spec de la plateforme le relève déjà
(`docs/superpowers/specs/2026-08-19-plateforme-design.md:227`) : le fichier
composé **n'est pas versionné** et ne déclare **aucun** service du nouveau
produit. ✅ **Et il n'est pas nécessaire** — mesuré :

```bash
docker compose -f docker-compose.coturn.yml config >/dev/null ; echo $?
#   0
```

Le fichier coturn se parse **seul**. La dépendance est **documentaire, pas
fonctionnelle** : elle coûte une ligne de correction, pas une refonte. ⚠️ **Un
`config` qui réussit n'est pas un `up` qui réussit** — la condition C7 du §5
exige le démarrage réel, pas la validation.

**③ La VM Windows.** `src/app.js:46`, `src/session.js:65` et
`src/iconExtractor.js:14` parlent à `192.168.3.2:5985` — **la même VM que celle
du nouveau produit**. C'est la seule ressource réellement disputée, et le §3
montre qu'elle l'est aujourd'hui.

### 2.6 Ports et surface réseau

| Port | Qui | Relevé (`ss -ltnp`, 20 août 2026) |
| --- | --- | --- |
| 3445 | legacy, `index.js:61` | `LISTEN *:3445 users:(("node",pid=85245))` |
| 4822 | `guacd`, `src/session.js:116` | `LISTEN 0.0.0.0:4822 users:(("guacd",pid=859471))` |
| 5985 | WinRM sur la VM (sortant) | — |
| 8080 | **plateforme**, `plateforme/src/config.ts:66` | occupé par le nouveau produit |

⚠️ **`docker-compose.yml:19` (`3445:8080/tcp`) est INOPÉRANT** : `network_mode:
host` (l. 9) fait ignorer la clause `ports:` par Docker, et le processus écoute
directement sur 3445. ⚠️ **`docker-compose.yml:14`
(`WINDOWS_ADMIN_USERNAME=Administrator`) est MORT** : `index.js:3` l'écrase à
`Administrateur` au démarrage — et `scripts/winrm.js:8` porte le commentaire
qui explique pourquoi (« `Administrator` échoue à l'authentification sur cette
machine »). **Deux lignes de configuration sur vingt-cinq ne décrivent pas le
comportement du fichier qui les porte.**

✅ **Aucune collision de port entre l'ancien et le nouveau produit.** C'est ce
qui rend leur coexistence tenable, et c'est mesuré, pas supposé.

---

## 3. 🔴 Le legacy TOURNE — et il coûte, aujourd'hui, au nouveau produit

C'est le fait que ce document n'allait pas chercher et qui change son ordre de
priorité.

**Relevé du 20 août 2026 :**

```bash
docker ps -a --format '{{.Names}}\t{{.Image}}\t{{.Status}}' | grep -i guac
#   guacamole-web-1     guacamole-web       Up 16 hours
#   guacamole-coturn-1  coturn/coturn:4.6   Exited (0) 2 weeks ago
ps -o pid,etime,args -p 85245 -p 859471
#   85245   05:40  /usr/bin/node ./index.js      (cwd /opt/server)
#  859471 10:49:37  guacd -b 0.0.0.0 -l 4822 -f
curl -s -o /dev/null -w '%{http_code} %{size_download}\n' http://127.0.0.1:3445/apps
#   200 2                     ← le corps est « [] »
docker logs guacamole-web-1 2>&1 | grep -c "restarting due to changes"
#   1838
```

Quatre faits en sortent, tous mesurés :

**① Le catalogue est VIDE.** `GET /apps` rend `[]` (2 octets). Le journal du
conteneur porte
`Error fetching apps: ENOENT ... '/media/vm/Users/guacamole/Desktop'`, levé par
`src/app.js:59`. ⚠️ **C'est un transitoire, pas une panne** : le répertoire
existe à cet instant (`ls -d /media/vm/Users/guacamole/Desktop` réussit), et
`src/app.js:152` ne réessaie qu'**une fois par heure**. Le catalogue restera
vide jusqu'au prochain tour, ou jusqu'au prochain redémarrage — dont le point ③
montre qu'ils sont fréquents.

**② `guacd` fuit à chaque redémarrage.** Le `guacd` qui tient le port 4822
tourne depuis **10 h 49**, alors que le processus `node` qui l'a censément
engendré n'a que **5 min 40**. Chaque relance journalise
`guacd[…]: ERROR: Unable to bind socket to any addresses.` — le nouveau `guacd`
ne peut pas se lier, l'ancien tenant toujours le port. `src/session.js:134-136`
lui envoie `SIGUSR1`, qui n'est pas un signal de terminaison pour `guacd`.
⚠️ **Le produit reste servi par un `guacd` orphelin dont plus aucun processus
n'est le parent logique.** C'est très exactement la classe de fragilité que
le cadrage §1 invoque pour justifier la refonte.

**③ 🔴 Le conteneur redémarre à chaque écriture dans le dépôt — 1 838 fois en
16 heures**, soit ~115 par heure. `Dockerfile` : `CMD nodemon --ignore dist
./index.js` ; `docker-compose.yml:6` monte `./:/opt/server`. `nodemon` surveille
donc **tout le dépôt**, extensions `js,mjs,cjs,json` (relevé en tête du journal
du conteneur), en n'ignorant que `dist`. **Un `npm install` dans `plateforme/`
ou `client/`, un fichier d'instrument en `.mjs`, un `package-lock.json`
réécrit — chacun relance l'ancien produit.**

**④ Et chaque relance sollicite la VM.** Au chargement,
`src/app.js:149` déclenche `fetchApps()`, qui lit le bureau distant puis, par
application découverte, appelle `getAppFileAssociations` → `runInPowerShell` →
`waitPort` sur 5985 + `winrm.runCommand` (`src/app.js:43-50`). En parallèle,
`src/asset.js:19-37` rejoue le build Gulp/Babel/Browserify des deux bundles.

⚠️ **La conséquence n'est pas théorique** : la VM `192.168.3.2` est la machine
de recette de tous les sous-blocs en cours. **L'ancien produit est un émetteur
non déclaré de trafic WinRM vers la VM sur laquelle on mesure le nouveau**,
au rythme des écritures faites dans le dépôt par les agents eux-mêmes.

✅ **C'est la seule justification d'un geste anticipé, et elle ne demande aucune
suppression de code** — voir L1 au §9.

⚠️ **Ce que ces quatre points N'établissent PAS** : que le legacy soit
inutilisable. Le `guacd` orphelin **écoute**, `guacamole-lite` se connecte à
`127.0.0.1:4822` (`src/session.js:177-180`), et une session pourrait très bien
s'établir dès que `fetchApps` réussira. **Aucune session n'a été tentée** — et
elle ne le sera pas depuis ce chantier, la VM étant occupée.

---

## 4. La matrice de remplacement

**Convention de la colonne « état »**, imposée parce que ce dépôt a payé cher
la confusion entre les quatre :

| Marque | Sens |
| --- | --- |
| **spécifié** | une spec existe |
| **planifié** | un plan de tâches existe |
| **codé** | du code est commité |
| **mesuré** | une recette a tourné sur la VM et un document de résultats la porte |

Relevé de l'état par : `ls docs/superpowers/plans/ | grep -E '…-resultats'`,
`ls -d docs/superpowers/plans/journaux-*`, et
`git log --oneline --since=2026-08-18`.

| # | Fonction du legacy | Où elle vit | Qui la reprend | État | Verdict |
| --- | --- | --- | --- | --- | --- |
| 1 | **Streaming vidéo** (instructions Guacamole sur canvas) | `web/index.js:111`, `src/session.js` | agent Rust + WebRTC (sous-projet ①/②) | **mesuré** — D1→D11 | ✅ repris et dépassé |
| 2 | **Audio descendant** | `web/index.js:174-179` | chantier A puis D7 (audio par fenêtre) | **mesuré** | ✅ repris |
| 3 | 🔴 **Audio montant (micro) jusqu'à l'application Windows** | `enable-audio-input: true` (`src/session.js:214`) | chantier E — bloc **E2** | **E1 mesuré, E2 NON COMMENCÉ** | 🔴 **NON repris** — voir §4.2 |
| 4 | **Entrée souris / clavier / tactile** | `web/index.js:181-300` | `SendInput`, chantier B, `client/src/input.ts` | **mesuré** | ✅ repris — ⚠️ le **tactile** n'est nommé nulle part dans le nouveau produit (§4.3) |
| 5 | **Presse-papier bidirectionnel** | `web/index.js:331-393` | sous-projet ① presse-papier, P1/P2 | **spécifié ; P1 planifié** — `grep -riE 'clipboard\|presse.papier' agent/src client/src plateforme/src proto/src` → **0** | 🔴 **NON repris, dans AUCUN sens** |
| 6 | **Pont fichiers** (lecture **et** écriture, `mkdir`, `rename`) | `src/file.js` (FUSE) | ProjFS ⇆ FSA, F1 | **codé** ; **recette NON CLOSE** (aucun `-resultats.md`, journaux non suivis) | 🔴 **partiellement repris — 3 verbes sur 8** |
| 7 | **Découverte d'applications** | `src/app.js:52-135` (bureau, horaire) | sous-projet ④ gestion d'apps, G1 | **planifié seulement** — aucun commit | 🔴 **NON repris** |
| 8 | **Parsing `.lnk`** | `src/lnkParser.js` (200 l., maison) | G1 (Shell natif) | **planifié seulement** | 🔴 **NON repris** |
| 9 | **Extraction d'icônes** | `src/iconExtractor.js` (5 méthodes) | G1 (Shell 256×256) | **planifié seulement** | 🔴 **NON repris** |
| 10 | **Associations de fichiers** (registre) | `src/app.js:15-37` | G1 | **planifié seulement** | 🔴 **NON repris** |
| 11 | **Catalogue / page d'accueil (hub)** | `assets/index.html`, `web/home.js`, `/apps` | sous-projet ② | 🔴 **hors périmètre de S3** (`…/plans/2026-08-19-design-system-s3.md:292`) | 🔴 **NON repris** |
| 12 | **Manifeste PWA par app** | `src/asset.js:42-103` | ④ — **G5** (`…/specs/2026-08-19-gestion-apps-design.md:760`) | **spécifié seulement** ; `grep -riE 'webmanifest\|serviceWorker' client/` → **0** | 🔴 **NON repris** |
| 13 | **Service worker** | `web/sw.js`, `src/asset.js:106` | ④ — G5 | **spécifié seulement** | 🔴 **NON repris** |
| 14 | **`file_handlers` PWA** | `src/asset.js:82-100` | ④ — G5 (amendement du cadrage §5) | **spécifié**, obstacles non levés | 🔴 **NON repris** |
| 15 | **Window Controls Overlay / couleur d'accent** | `web/index.js:684-716` | ⑥ **S4** (WCO) et ① **A1** (`…/specs/2026-08-19-presse-papier-design.md:838`) | **spécifiés** ; ni planifiés ni codés | 🔴 **NON repris** |
| 16 | **Sessions et leur cycle de vie** | `src/session.js:59` (en mémoire) | plateforme P1→P3 (Postgres) | **mesuré** | ✅ repris et dépassé |
| 17 | **Authentification** | 🔴 **inexistante** dans le legacy | plateforme **P2** | **mesuré** — `connexion.html`, scrypt, JWT | ✅ **gain net** |
| 18 | **Upload d'installeurs** | 🔴 **inexistant** dans le legacy | ④ — G3 | spécifié seulement | — (pas une régression) |
| 19 | **Orchestration de VMs** | 🔴 **inexistante** | plateforme **P4** (`InventaireStatique`) | **codé (12 tâches sur 19), JAMAIS MESURÉ** | 🟡 gain net **non recetté** |
| 20 | **TURN / traversée NAT** | 🔴 **inexistant** | chantier C volet 2 | **mesuré** | ✅ gain net |

### 4.1 🔴 Ce qui n'est repris PAR PERSONNE — le cœur de ce document

Onze fonctions du legacy — la ligne 3 (micro, §4.2) et les lignes 5 à 15 de la
matrice — n'ont, au 20 août 2026, **aucune ligne de code dans le nouveau
produit**. Trois balayages le vérifient, et rendent tous **zéro** :

```bash
grep -rniE "clipboard|presse.papier"            agent/src client/src plateforme/src proto/src | wc -l   # 0
grep -rniE "IShellLink|\.lnk|installeur|catalogue" agent/src client/src plateforme/src proto/src | wc -l   # 0
grep -rniE "webmanifest|serviceWorker"          client/src client/*.html                        | wc -l   # 0
```
 Trois d'entre elles n'ont
même pas de sous-bloc daté qui les revendique :

- 🔴 **le manifeste PWA par application** (`src/asset.js:42-103`) — c'est
  pourtant l'objectif n°3 du cadrage (« la fenêtre de l'app Windows EST la
  fenêtre PWA ») et le §5 ④ promet « chaque app découverte devient une PWA
  installable ». **Aucun sous-projet ne le porte à ce jour.**
- 🔴 **le service worker** (`web/sw.js`) — cité au cadrage §5 ②, jamais planifié.
- 🔴 **Window Controls Overlay** — cité au cadrage §5 ② et ①, jamais planifié.

⚠️ **Ce n'est pas un défaut du nouveau produit : c'est l'état d'un chantier en
cours.** Mais c'est exactement l'inventaire que le §11 du cadrage n'a jamais
fait, et sans lequel « puis sera supprimé » n'a pas de contenu.

**Deux précisions que la matrice ne peut pas porter, et qui changent la
lecture :**

- 🔴 **Le pont fichiers reprend TROIS verbes sur HUIT.** `proto/src/fichiers.rs:52-54`
  ne déclare que `TYPE_LISTER`, `TYPE_ATTRIBUTS` et `TYPE_LIRE`. Les cinq
  manquants sont **Existence, Créer, Écrire, Renommer, Supprimer**
  (`…/specs/2026-08-19-pont-fichiers-design.md:396-407`, la table du périmètre v1 — huit lignes).
  Et la spec de ③ **argumente que le sous-ensemble sans suppression ne
  fonctionne pas** : l'idiome d'enregistrement Windows est écrire-temporaire →
  renommer → supprimer (`…/specs/2026-08-19-pont-fichiers-design.md:378-382`). Le legacy, lui,
  portait `mkdir` (`src/file.js:286`) et tout le reste. **Le remplacement est
  aujourd'hui un sous-ensemble strict, et un sous-ensemble déclaré
  insuffisant par sa propre spec.**
- 🔴 **Aucune surface du produit n'emploie encore le design system.**
  `CLAUDE.md:9245` : « **AUCUNE SURFACE DU PRODUIT N'EMPLOIE UNE PRIMITIVE.**
  Les dix-huit tokens sortis ont un appelant écrit, **pas un pixel rendu**. »
  C'est S3 qui referme l'écart, et S3 n'a aucun code. **La condition C4 porte
  donc sur une page qui n'existe ni fonctionnellement (④) ni visuellement (⑥).**

### 4.2 🔴 Le microphone est une régression, et je l'avais d'abord classé « repris »

`src/session.js:214` pose `enable-audio-input: true` : le legacy **configurait**
le sens montant jusqu'à l'application Windows, par RDP.

Le chantier E l'a repris **à moitié**, et son propre document de résultats le
dit en gras : « **Ce que E1 ne fait PAS, et qui est le bloc E2** : écrire ce PCM
sur "CABLE Input". **Aucune application Windows n'entend quoi que ce soit à ce
jour.** » (`docs/superpowers/plans/2026-08-19-micro-resultats.md:39-40`).

⚠️ **La comparaison n'est pourtant pas « le legacy le faisait, le nouveau
non »**, et il faut le dire : que le sens montant du legacy ait réellement
fonctionné n'est établi par **aucune pièce de ce dépôt** — c'est un réglage lu
dans un fichier, pas une mesure. **Ce qui est certain, c'est que le nouveau
produit ne le fait pas.** La condition C1 doit donc porter le micro, et le
retrait ne peut pas se faire sur la foi de « E1 est recetté ».

### 4.3 Deux régressions silencieuses, à nommer avant qu'elles ne surprennent

- ⚠️ **Le tactile.** `src/session.js:216` pose `enable-touch: true` et
  `web/index.js:183` instancie `Guacamole.Mouse.Touchscreen`. **Aucune spec du
  nouveau produit ne nomme le tactile** — le chantier B parle de souris
  relative et de manette. Un utilisateur sur tablette perd une fonction qui
  marchait. **Non vérifié plus loin que cette absence de mention** : c'est une
  alerte, pas un constat d'absence de code.
- ⚠️ **L'impression.** `src/session.js:197` pose `enable-printing: true`. Le
  cadrage §11 range l'impression en **v2+**. La régression est donc **connue,
  datée et assumée** — mais elle n'a jamais été rapprochée du fait que
  l'ancien produit l'avait.

---

## 5. Les conditions de retrait — falsifiables, une par fonction

**Règle d'écriture, non négociable** : une condition dont l'évaluation ne peut
pas rendre **NON** n'est pas une condition. Ce dépôt a attrapé une dizaine de
contrôles vacueux sur ses derniers sous-blocs, dont plusieurs écrits par les
plans eux-mêmes. Chaque ligne ci-dessous porte donc **la commande ou la recette
qui l'évalue**, et **l'observation qui la ferait échouer**.

| # | Ce qui peut mourir | Condition | Comment on l'évalue | Ce qui rend NON |
| --- | --- | --- | --- | --- |
| **C1** | `web/index.js`, `assets/app.tpl.html`, `dist/` | La fenêtre de session couvre vidéo, audio descendant, entrée, **presse-papier dans les DEUX sens**, et **micro jusqu'à une application Windows** | documents de résultats versés pour ① (P1 **et** P2) **et** pour E2 | un sens du presse-papier non mesuré ; ou E2 non joué ; ou le préalable éliminatoire de P2 (`paste` sur un `<video>` focalisé, `…/specs/2026-08-19-presse-papier-design.md:814`) non levé |
| **C2** | `src/file.js`, `fuse-native`, `ftp-srv` | Le pont fichiers **écrit** : les huit verbes de `…/specs/2026-08-19-pont-fichiers-design.md:396-407`, suppression comprise | ① `grep -c 'TYPE_' proto/src/fichiers.rs` fait apparaître les types d'écriture ; ② **F1 clos** (un `…-f1-resultats.md` existe) ; ③ recette F2/F3 versée | un verbe absent de `proto/src/fichiers.rs` ; ou F1 toujours sans document de résultats ; ou `ERROR_WRITE_PROTECT` décrit encore le régime nominal |
| **C3** | `src/app.js`, `src/lnkParser.js`, `src/iconExtractor.js` | Le catalogue du nouveau produit rend **au moins autant d'applications** que le legacy sur la **même** VM, avec leurs icônes | recette G1 ; comparer l'ensemble des **noms** rendus par le nouveau catalogue à ceux de `curl http://127.0.0.1:3445/apps` **pris le même jour** | une application présente chez l'ancien et absente chez le nouveau |
| **C4** | `assets/index.html`, `web/home.js` | Un **hub** existe : il liste les applications, les lance, et il est servi par la plateforme | `ls client/*.html` fait apparaître une entrée de catalogue, **et** une recette la montre lançant une session | S3 maintient le hub hors périmètre (état du 20 août) |
| **C5** | `src/asset.js`, `web/sw.js` | Le nouveau produit sert un manifeste PWA par application **et** un service worker | `curl` sur la route de manifeste du nouveau produit rend un JSON avec `icons` et `display` | aucun sous-bloc ne revendique la fonction (état du 20 août) |
| **C6** | `src/session.js`, `guacamole-lite`, image `guacamole-web` | Plus aucune session utilisateur n'emprunte le chemin RDP | `ss -ltnp | grep 4822` ne rend rien **après** arrêt volontaire du conteneur pendant 7 jours, sans réclamation | un utilisateur réclame, ou le port se rouvre |
| **C7** | `docker-compose.yml` | Le relais TURN démarre **sans** ce fichier | `docker compose -f docker-compose.coturn.yml up -d coturn` puis `docker compose … ps` montre le service **Up**, et une session relayée est établie | le service ne démarre pas seul, ou la session ne passe plus par le relais |
| **C8** | `package.json`, `node_modules/` racine | `scripts/winrm.js` fonctionne sans les 20 dépendances legacy | après retrait des 20, `node scripts/winrm.js 'echo ok'` rend `ok`, **et** `scripts/verify-all.sh` reste vert | `nodejs-winrm` casse, ou un script de la VM échoue |
| **C9** | `test_winrm_*.js` | Aucun document de recette en cours ne les nomme | `grep -rn "test_winrm" docs/ scripts/ CLAUDE.md` | une occurrence hors de la présente spec |
| **C10** | La connaissance de `CLAUDE.md` | Ce qui est **vrai du domaine** a été déplacé hors des sections legacy | §6.2 : la liste nommée y est intégralement présente ailleurs | un fait de la liste n'a pas de nouvel emplacement |

⚠️ **C6 est la seule condition qui ne se réduit pas à une commande**, et il faut
le dire : « plus personne ne s'en sert » n'est pas observable depuis le dépôt.
La forme retenue — **arrêt volontaire, fenêtre d'observation, réclamation** —
est un protocole, pas une mesure. Elle **peut** rendre NON (quelqu'un réclame),
ce qui la garde du côté des conditions ; elle n'est pas pour autant une preuve
d'absence d'usage.

⚠️ **C3 porte une exigence de simultanéité qui n'est pas décorative.** Le
catalogue du legacy est un instantané d'un bureau Windows qui change. Comparer
un relevé du nouveau produit à un relevé de l'ancien pris **un autre jour**
compare deux bureaux, pas deux découvertes. Et le §3 ① montre que ce catalogue
peut rendre `[]` sans que rien n'aille mal — **une comparaison contre `[]`
réussirait toujours**, et serait donc vacueuse.

---

## 6. Ce qui se perd — et ce qu'on en garde, où, sous quelle forme

### 6.1 Ce qui disparaît sans repreneur

Au-delà des onze fonctions du §4.1, **cinq choses disparaissent que personne
n'a jamais revendiquées** :

1. 🔴 **890 lignes qui n'existent que sur ce disque** (§2.2). `web/index.js`
   n'est pas seulement du code : c'est le seul endroit du dépôt où sont écrits,
   à l'échelle d'un produit qui a servi, le contournement `getPixelColor` pour
   la couleur de titre (`web/index.js:767`), l'usage de `windowControlsOverlay`
   (l. 684-716) et la lecture du presse-papier client sous permission
   (l. 348-393). **Le nouveau produit refera ces trois choses.**
2. **Le parseur `.lnk` maison** (200 lignes, MS-SHLLINK) et ses cas retors,
   déjà partiellement transcrits dans la spec de gestion d'apps
   (`…/specs/2026-08-19-gestion-apps-design.md:133`, `:412`, `:430`).
3. **La comparaison des cinq voies d'extraction d'icônes** — `.ico`, `icotool`,
   `wrestool`, `sharp-ico`, PowerShell — avec leurs taux de succès. Cette
   comparaison **n'existe nulle part ailleurs que dans `CLAUDE.md`** et dans le
   code de `src/iconExtractor.js`.
4. 🔴 **Les formats riches du presse-papier.** `web/index.js:330-345` transporte
   ce que `navigator.clipboard.read()` rend, types compris. La v1 du nouveau
   presse-papier range **images, fichiers, RTF et HTML hors périmètre**
   (`…/specs/2026-08-19-presse-papier-design.md:994-1016`). C'est une régression de fonction, et
   elle est **déjà décidée** — mais elle n'a jamais été rapprochée du fait que
   l'ancien produit les portait.
5. **Un produit qui marchait.** Le legacy est le seul artefact du dépôt dont on
   sache qu'il a servi des utilisateurs. Le nouveau ne le sait pas encore.

⚠️ **Et le legacy n'est pas seulement un passé : il est la RÉFÉRENCE VIVANTE
d'au moins deux sous-blocs à venir.**

*Un.* La spec du pont fichiers s'appuie sur lui pour établir un fait qu'elle ne
pouvait pas obtenir autrement : `FileSystemHandle.move()` est une extension
Chromium, « l'ancien pont s'en sert (`web/index.js:628` pour le récursif,
`:644` pour un fichier) — et son renommage de répertoire **ne fonctionne
pas** », une `ReferenceError` par zone morte temporelle à `web/index.js:631`
« que personne n'a relevé » (`…/specs/2026-08-19-pont-fichiers-design.md:411-418`). 🔴 **Ce défaut
est décrit dans un fichier de spec qui cite trois lignes d'un fichier qui
n'existe dans aucun commit.** Supprimer `web/index.js` rend cette citation
invérifiable — et c'est la seule pièce qui empêche F2 de réimplémenter le même
bug.

*Deux.* Si l'événement `paste` ne parvient pas sur un `<video>`
focalisé, la spec du presse-papier se replie sur « `readText()` avec
permission, **c'est-à-dire l'ancien produit**, avec son défaut de refus
silencieux à corriger » (`…/specs/2026-08-19-presse-papier-design.md:1022`). **Retirer
`web/index.js` avant que ce préalable ne soit levé, c'est retirer la seule
implémentation de référence du repli.** C'est la raison la plus concrète pour
laquelle L2 précède L3.

### 6.2 🔴 Ce qui est vrai du DOMAINE ne part pas avec le code

`CLAUDE.md` porte **1 113 lignes sur 9 932 (11,2 %)** décrivant l'ancien
produit, réparties en douze blocs (relevé par `grep -n '^## ' CLAUDE.md` puis
différence des bornes) :

| Bloc | Lignes | Sort au retrait |
| --- | --- | --- |
| `## Project Overview` | 7-10 (4) | **réécrit** — il décrit encore RDP |
| Development Commands → Known Constraints | 1012-1196 (185) | **retiré**, sauf ce qui suit |
| Problèmes Résolus / Non Résolus | 1199-1485 (287) | **archivé** (§6.3) |
| Système de Build (Gulp/Babel) | 1487-1529 (43) | **retiré** |
| Configuration Critique | 1532-1578 (47) | 🔴 **traité à part** — il décrit où sont les secrets |
| Parser `.lnk` | 1580-1646 (67) | 🔴 **CONSERVÉ** — vrai du domaine |
| Extraction d'icônes | 1648-1748 (101) | 🔴 **CONSERVÉ** — vrai du domaine |
| Flux de lancement complet | 1750-1905 (156) | **archivé** |
| Commandes de Développement Essentielles | 9709-9777 (69) | **retiré** |
| Checklist de Déploiement Production | 9778-9825 (48) | **transposé** vers la plateforme |
| Roadmap et TODO P0-P3 | 9826-9885 (60) | 🔴 **arbitré ligne à ligne** (§6.4) |
| Ressources et Références | 9886-9931 (46) | **élagué** — Guacamole/FUSE partent, pas Sharp |

**Ce qui est vrai du domaine et qui doit survivre nommément** — la liste est
close, et la condition **C10** l'évalue :

- **le format `.lnk`** : `LocalBasePath` parfois incomplet, `RELATIVE_PATH`
  comme repli, `IconLocation` sous la forme `"chemin,index"` ;
- **les icônes Windows** : `ExtractAssociatedIcon` ne rend que 32×32 ou 48×48 ;
  `icotool` échoue en « reserved non-zero » sur les `.exe` modernes ;
  `sharp-ico` échoue en « Invalid magic bytes » ;
- **WinRM** : `nodejs-winrm` enveloppe **toujours** la commande dans
  `powershell -Command "& { … }"`, ce qui interdit les guillemets doubles
  inline — piège déjà repayé au sous-bloc D3 ;
- **le compte `Administrateur`** : `Administrator` échoue à l'authentification
  sur cette VM (`scripts/winrm.js:6-8`) ;
- **le cycle de vie de la VM** (`CLAUDE.md:1907`) — **déjà partagé**, aucun
  arbitrage nécessaire ;
- **`[Console]::Beep` est un faux négatif audio**, et le défaut d'encodage à
  deux réglages de PowerShell — tous deux **déjà** hors des sections legacy.

⚠️ **Ces faits ne sont pas transportés par ce document : ils sont NOMMÉS.** Les
déplacer est un travail de L4 (§9), et `CLAUDE.md` est hors du périmètre
d'écriture de la présente spec.

### 6.3 Où l'on archive, et sous quelle forme

**Décision : `docs/legacy/`, versionné, et le code y entre par `git add`
explicite avant toute suppression.**

*Pourquoi pas « l'historique git suffit ».* Parce que c'est **faux pour 890
lignes sur 3 170** (§2.2), et parce que même pour les 2 280 autres, « plus
personne ne relira le code mort » — un chemin `docs/legacy/` se `grep`e, un
commit supprimé ne se `grep`e pas.

*Ce qui y entre* : les 19 fichiers du §2.1, **secrets retirés**
(`index.js:2-4`, `docker-compose.yml:15,17`), plus les sections archivées de
`CLAUDE.md` listées au §6.2.

*Ce qui n'y entre pas* : `dist/`, `src/dist/`, `node_modules/`,
`package-lock.json`, les images Docker, les core dumps.

🔴 **Le retrait des secrets est une opération de rédaction, pas de suppression
de fichier.** `index.js` porte quatre lignes dont trois affectent des
identifiants ; le fichier archivé doit porter la structure sans les valeurs, et
**le dire**. ⚠️ **Cet archivage ne purge pas les secrets de la machine** :
`.env` et le `docker-compose.yml` vivant les portent toujours, et ce chantier
n'a pas mandat de les faire tourner.

### 6.4 La roadmap P0-P3 de `CLAUDE.md` — trois lignes survivent, et il faut le dire

`CLAUDE.md:9826-9885` porte vingt entrées de roadmap, dont la quasi-totalité
disparaît avec le legacy (recompilation Gulp, qualité des icônes basse
résolution, détection PWA par `localStorage`). **Trois ne disparaissent pas**,
et leur transposition est un livrable de L4 :

- **P0-2 « Résoudre le crash *double free or corruption* »** : le retrait le
  rend sans objet **par disparition de la chaîne**, jamais par diagnostic. Le
  §7 ci-dessous en tire une conséquence sur les core dumps.
- **P0-3 « Externaliser les credentials de production »** : reste entier —
  `.env` et `docker-compose.yml` vivant portent toujours des mots de passe.
- **P2-10 « Authentification »** : ✅ **reprise et livrée** par la plateforme P4.

---

## 7. Les données et l'état — le §11 du cadrage vérifié, et il est presque juste

Le cadrage écrit « Migration des données de l'ancien système : aucune (pas de
données à migrer) » (`2026-07-27-refonte-produit-design.md:302-303`). **Vérifié
le 20 août 2026 — c'est exact pour l'état SERVEUR, et inexact pour l'état
CLIENT.**

| État | Où | Mesure | Verdict |
| --- | --- | --- | --- |
| Sessions | `src/session.js:59`, `guacdInstances = {}` | en mémoire, perdu à chaque redémarrage — 1 838 fois en 16 h (§3 ③) | ✅ rien à migrer |
| Base de données | — | `grep -rn "sqlite\|writeFile" src/ index.js` : aucune écriture de persistance | ✅ aucune |
| Montages FUSE | `src/file.js:401`, `/mnt/ftp-{uuid}` | `ls -d /mnt/ftp-*` → aucun ; `grep ftp- /proc/mounts` → aucun | ✅ aucun résidu |
| Artefacts de build | `dist/`, `src/dist/` | régénérés par `src/asset.js:19-37` à chaque démarrage | ✅ jetables |
| 🟡 Fichiers temporaires sur la VM | `src/iconExtractor.js:162`, `C:\temp\icon_*.png` | nettoyés l. 193, mais l'échec n'est que **journalisé** (l. 195) | 🟡 orphelins possibles — **non mesuré, VM interdite à ce chantier** |
| 🔴 `localStorage` du navigateur | `web/home.js:5`, clé `installedApps` | état par navigateur, hors d'atteinte de toute suppression serveur | 🔴 **survit au retrait** |
| 🔴 Service worker enregistré | `web/index.js:419-423`, scope `/${appName}/` | un SW enregistré survit à la mort de son serveur | 🔴 **survit au retrait** |

⚠️ **Les deux dernières lignes sont un état persistant réel, et le cadrage ne
les voit pas.** Elles ne « se migrent » pas : elles se **désarment**, et le seul
moment où on peut le faire est **pendant que le serveur vit encore** — un
service worker ne se désenregistre que depuis une page servie sous son scope.

**Conséquence opérationnelle, à porter en L2** : si le hub du nouveau produit
doit un jour être servi sous le même hôte, un `/sw.js` d'extinction
(`self.registration.unregister()`) doit être servi **avant** l'arrêt de
l'ancien serveur, pas après. ⚠️ **La gravité réelle n'est pas mesurée** : le
scope est `/${appName}/`, pas la racine, et aucun navigateur porteur d'un SW
legacy n'a été inventorié. **C'est un risque nommé, pas un défaut constaté.**

### 7.1 Les core dumps — 390 Mo, et ce qu'on en fait

```bash
ls -la core.* ; du -ch core.* | tail -1
#   7 fichiers — core.398, core.2454, core.3344, core.4322, core.4384,
#   core.4458, core.4539 — datés du 25 oct. au 18 nov. 2025 — 390 Mo
git check-ignore -v core.398    # → .gitignore:4:core.*
```

Le cadrage les invoque comme **pièce à charge** : « crash récurrent
`double free or corruption` dans la chaîne guacd/fuse-native (7 core dumps à la
racine du projet en attestent) » (`…/specs/2026-07-27-refonte-produit-design.md:16-18`).

⚠️ **Ils attestent d'un crash, pas de sa cause.** Aucun document du dépôt ne
porte de trace d'analyse : ni pile, ni `gdb`, ni attribution. Ils sont, à ce
jour, **sept fichiers de 390 Mo dont personne n'a lu un octet**.

**Décision : ils partent avec le legacy, et pas avant.** Trois raisons :

1. ils sont la **seule preuve matérielle** de la motivation n°1 de la refonte —
   les supprimer avant que le nouveau produit ne soit reçu, c'est effacer la
   justification du chantier pendant qu'on le mène ;
2. ils sont **ignorés par git** (`.gitignore:4`) : leur suppression est
   irréversible et ne se lit dans aucun commit ;
3. ils ne coûtent que de l'espace disque, et **rien ne les relie à un défaut
   ouvert du nouveau produit**.

⚠️ **Ce qui est conservé n'est PAS le fichier, c'est le fait.** Le fait — sept
crashs entre le 25 octobre et le 18 novembre 2025, jamais diagnostiqués — est
écrit ici, daté, et survit à la suppression. **Si quelqu'un veut le diagnostic,
c'est AVANT le retrait qu'il faut le faire**, et ce chantier ne le fera pas.

---

## 8. Ce qui est retirable dès aujourd'hui — un seul candidat, et il ne l'est pas

Le mandat demande de le **vérifier**, pas de le supposer. Vérification :

**Candidat n°1 — `web/fs.js`, 4 lignes.** Il expose `loadFiles()`, qui appelle
`window.showOpenFilePicker()`. Cherchons ses appelants :

```bash
grep -rn "fs.js\|loadFiles\|require('./fs')" web/ assets/ src/ index.js
#   (aucune correspondance hors la définition elle-même)
```

**Aucun appelant.** Mais `web/index.js:679` porte un
`// const [handle] = await window.showOpenFilePicker();` **en commentaire** :
le module a été extrait puis abandonné sur place. **Retirable sans rien
casser** — et pourtant :

⛔ **Non. Ce document ne l'autorise pas.** Retirer 4 lignes du legacy
n'apporte rien, contredit le cadrage §11 (« l'ancien code n'est pas modifié »),
et ouvrirait un précédent : le prochain retrait « évident » n'aura pas été
vérifié aussi soigneusement. **Le gain est nul, le précédent coûteux.**

**Candidat n°2 — `test_winrm_fixed.js` et `test_winrm_nodejs.js`, 246 lignes.**

```bash
grep -rn "test_winrm" docs/ scripts/ CLAUDE.md --include="*.md" --include="*.sh"
#   CLAUDE.md:1178  - `test_winrm_nodejs.js` - NodeJS WinRM testing
#   CLAUDE.md:1179  - `test_winrm_fixed.js` - Fixed WinRM implementation tests
```

Deux occurrences, **toutes deux documentaires**. Aucun script, aucune recette
ne les invoque. **Condition C9 : ✅ satisfaite aujourd'hui.** Ils restent
néanmoins, pour la même raison que ci-dessus : le cadrage interdit de toucher
l'ancien code, et 246 lignes ne justifient pas d'y déroger.

### 8.1 🟡 Le SEUL geste anticipé que ce document recommande, et il ne supprime rien

**Recommandation : borner la surveillance `nodemon` du conteneur legacy à son
propre code.**

*Fait qui la motive* : 1 838 redémarrages en 16 heures (§3 ③), chacun
déclenchant un build Gulp et une passe WinRM vers la VM de recette (§3 ④).

*Forme* : un `nodemon.json` restreignant `watch` à `index.js`, `src/` et
`web/`. **Aucune ligne de code applicatif n'est touchée** ; le comportement du
produit est inchangé ; le fichier est neuf, donc rien n'est « modifié » au sens
du cadrage §11.

*Condition qui la rendrait inutile* : arrêter simplement le conteneur. ⚠️ **Ce
document ne le recommande pas** — l'arrêt est la condition **C6**, elle est
soumise à observation, et personne n'a établi que le legacy ne sert plus.

⚠️ **Ce geste appartient à L1 et n'est pas exécuté ici.** Cette spécification
n'écrit aucun fichier hors elle-même.

---

## 9. Découpage en sous-blocs

| Bloc | Objet | Condition d'entrée | Ce qu'il livre |
| --- | --- | --- | --- |
| **L1** | **Cesser de nuire** — borner `nodemon` (§8.1) ; inscrire au tableau de dette de `CLAUDE.md` que `web/index.js` (804 l.) est **hors de portée de la commande de vérification** (§2.3) | **aucune** — exécutable dès aujourd'hui | un `nodemon.json` ; deux lignes de constat. **Zéro suppression** |
| **L2** | **Archiver avant de toucher** — `docs/legacy/` versionné, secrets retirés (§6.3) ; décider du `/sw.js` d'extinction (§7) | L1 | les 3 170 lignes deviennent **récupérables**, dont les 890 qui ne le sont pas |
| **L3** | **Retirer la surface morte** — `web/index.js`, `assets/`, `web/`, `dist/`, `src/dist/`, `src/session.js`, `src/file.js`, image Docker, `Dockerfile`, `docker-compose.yml` | **C1 ∧ C2 ∧ C4 ∧ C5 ∧ C6 ∧ C7** | l'ancien produit cesse d'exister comme service |
| **L4** | **Retirer la découverte** — `src/app.js`, `src/lnkParser.js`, `src/iconExtractor.js`, `src/asset.js`, `src/cleanup.js`, `index.js` ; **et déplacer la connaissance de domaine** (§6.2) | **C3 ∧ C10** | le dépôt n'a plus qu'un produit ; `CLAUDE.md` perd 1 113 lignes et n'en perd aucun fait vrai |
| **L5** | **Solder** — `package.json` (20 dépendances, `nodejs-winrm` **survit**), `package-lock.json`, `node_modules/`, `test_winrm_*.js`, les motifs `.gitignore:18-19`, la **portée** de `CLAUDE.md:20`, les core dumps (§7.1) | **C8 ∧ C9**, et L3 ∧ L4 achevés | 141 Mo, 806 Mo d'image, 390 Mo de dumps ; la mine du motif `index.js` désamorcée |

⚠️ **L3 avant L4, et l'ordre n'est pas arbitraire.** `index.js:9-12` charge
`session`, `asset`, `app` et `cleanup` : retirer `src/app.js` (L4) sans avoir
retiré `src/session.js` (L3) casse `src/session.js:11`, qui fait
`require('./app')`. **La dépendance est dirigée du service vers la découverte**,
donc le service meurt en premier.

⚠️ **L5 est le seul bloc dont l'échec est visible ailleurs** : retirer les 20
dépendances casse `scripts/winrm.js` si `nodejs-winrm` part avec elles, et donc
**les quatre scripts qui pilotent la VM du nouveau produit** (§2.5 ①). La
condition **C8** existe pour ça, et elle exige `scripts/verify-all.sh` vert —
pas seulement un `echo ok`.

---

## 10. Ce que ce chantier n'établira PAS

- **Il ne diagnostique pas le `double free or corruption`.** Les sept core
  dumps partent non lus (§7.1). Le défaut disparaît **par retrait de la chaîne**,
  jamais par explication. Si le nouveau produit reproduisait un jour une classe
  de bug voisine, **rien de ce chantier ne l'aidera**.
- **Il n'établit pas que personne n'utilisait le legacy.** C6 est un protocole
  d'observation, pas une mesure d'usage (§5).
- **Il ne mesure pas ce que l'ancien produit savait faire.** Aucune session RDP
  ne sera ouverte depuis ce chantier — la VM est disputée (§3 ④). La matrice du
  §4 compare donc **du code lu** à **des recettes documentées**, jamais deux
  produits en fonctionnement.
- **Il ne purge pas les secrets** de la machine (`.env`, `docker-compose.yml`
  vivant) — il retire seulement leurs copies de l'archive (§6.3).
- **Il ne réécrit pas l'historique git.** Les 2 280 lignes suivies restent dans
  les commits ; aucun `filter-branch` n'est envisagé, et les secrets n'y ont
  jamais été (§2.2).
- **Il ne mesure pas la population de service workers legacy** enregistrés dans
  des navigateurs réels (§7).
- **Il ne dit rien de `spike-multifenetres/`, `target/`, `.claude/worktrees/`**
  ni des paquets non-legacy de `node_modules` : ce sont d'autres chantiers.

---

## 11. Risques

| Risque | Gravité | Mitigation |
| --- | --- | --- |
| 🔴 **Suppression de `web/index.js` sans archivage** — 804 lignes détruites, aucun historique | **haute** | L2 est **bloquant** avant L3 ; C10 le vérifie |
| 🔴 **Retrait de `package.json` en bloc** — casse les quatre scripts VM du nouveau produit | **haute** | C8, et l'ordre L5 en dernier |
| 🔴 **Retrait pendant que onze fonctions n'ont pas de repreneur** (§4.1) | **haute** | C1 à C5 ; le §1 en fait la condition d'entrée du chantier entier |
| 🟡 **Un fait vrai du domaine part avec `CLAUDE.md`** — le parseur `.lnk`, les icônes, le piège WinRM | **moyenne** | C10, liste close au §6.2 |
| 🟡 **Rupture du lancement de coturn** (`docker-compose.coturn.yml:9`) | **moyenne** | C7 exige un `up` réel, pas le `config` déjà mesuré |
| 🟡 **Un service worker legacy intercepte le nouveau hub** | **moyenne, non mesurée** | L2 décide du `/sw.js` d'extinction **pendant** que le serveur vit |
| 🟡 **Le legacy continue de perturber les recettes** — 115 redémarrages/heure vers la VM de mesure | **présente aujourd'hui** | L1, exécutable immédiatement |
| ⚪ **Régression tactile et impression** (§4.3) | **faible, assumée** | nommées ici ; l'impression est en v2+ par le cadrage §11 |

---

## 12. Hors périmètre, explicitement

- **Toute suppression.** Ce document n'en autorise aucune, y compris les deux
  candidats « évidents » du §8.
- **Toute modification de l'ancien code.** Le cadrage §11 l'interdit tant que le
  remplacement n'est pas fait ; le seul geste recommandé (§8.1) ajoute un
  fichier de configuration et n'en modifie aucun.
- **`CLAUDE.md`**, `agent/`, `client/`, `plateforme/`, `proto/`, `scripts/` :
  lus, jamais écrits par cette spec. Leurs modifications appartiennent à L1-L5.
- **La rotation des secrets** et la purge de l'historique.
- **Le diagnostic du `double free`** (§10).
- **Le portage de fonctions** : ce document dit ce qui manque, il ne le
  construit pas. Chaque manque du §4.1 appartient à son sous-projet.
- **`spike-multifenetres/`** et le worktree `.claude/worktrees/` : ils ne sont
  pas le Guacamole historique.

---

## 13. Le geste d'ouverture de L1

Avant toute chose, **rejouer l'inventaire** : ce document a une durée de vie, et
son §3 la borne à l'heure près.

```bash
# ① le legacy tourne-t-il toujours, et à quel rythme ?
docker ps -a --format '{{.Names}}\t{{.Status}}' | grep -i guac
docker logs guacamole-web-1 2>&1 | grep -c "restarting due to changes"

# ② le catalogue rend-il autre chose que « [] » ?
curl -s http://127.0.0.1:3445/apps | head -c 200

# ③ les 890 lignes sont-elles toujours hors de git ?
git log --all --oneline -- web/index.js index.js docker-compose.yml

# ④ le compte de lignes du legacy
wc -l index.js src/*.js web/*.js assets/*.html test_winrm_*.js
```

⚠️ **Si ① rend « aucun conteneur », le §3 entier est périmé et L1 est sans
objet** — mais le reste de ce document tient, parce qu'il porte sur du code, pas
sur un processus. ⚠️ **Si ③ rend des commits, le §2.2 est périmé et le §6.3
perd sa justification principale** — vérifier avant de bâtir L2 dessus.
