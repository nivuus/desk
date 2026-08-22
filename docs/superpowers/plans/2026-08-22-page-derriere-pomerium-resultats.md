# La recette de « page-derriere-pomerium » — les huit critères joués, chaque rouge comprise

**22 août 2026, branche `finalisation`.** Ce document mesure un produit déjà
livré : le service `plateforme/` sert désormais la page bâtie du produit
(`GET /` ne rend plus `404 introuvable` quand `PLATEFORME_PAGE` est posée), et
`GET /auth/moi` ne croit l'en-tête d'identité que d'un pair déclaré de
confiance (`PLATEFORME_PROXY_DE_CONFIANCE`). Aucune ligne de code ni de test
n'a été modifiée pour produire ce document — c'est une recette, pas une
correction.

Les journaux bruts, cités ci-dessous commande par commande, vivent dans
[`docs/superpowers/plans/journaux-page-pomerium/`](journaux-page-pomerium/) et
sont **suivis par git** — ce dépôt a déjà perdu six constats de revue avec un
rapport gitignoré, et ce lot n'ajoute pas un septième cas.

🔴 **LE SECRET S'EST TIRÉ AU SORT À CHAQUE DÉMARRAGE**
(`PLATEFORME_SECRET_JETON="$(openssl rand -hex 24)"`), **jamais écrit en
littéral** dans une commande consignée ci-dessous ou dans un journal. Le garde
`plateforme/src/securite/secrets.test.ts` (qui balaie `git ls-files`, `docs/`
compris) a été rejoué **après** `git add` de ce lot, précisément parce que son
propre commentaire dit qu'un balayage sur des fichiers non encore suivis ne se
mesure pas lui-même.

## Méthode

Huit critères, joués dans l'ordre du cahier des charges. Le service tourne en
local (`PLATEFORME_HOTE=127.0.0.1`, port 8080 par défaut), redémarré entre
chaque critère qui exige une configuration différente — jamais deux critères
sur la même instance sans le dire. Chaque redémarrage est vérifié par
`ss -ltnp | grep '127.0.0.1:8080'` avant de relancer, pour ne jamais mesurer
contre une instance fantôme d'un tour précédent.

`client/dist/` a été bâti par l'étape `npm run build` du critère 0
(`env -u TURN_URL -u TURN_SECRET ./scripts/verify-all.sh`, Step 1
ci-dessous) — aucun rebuild séparé n'a été lancé pour les critères 1 à 8, ce
document le dit explicitement pour ne rien faire passer sous silence.

---

## Step 1 — la suite entière, depuis un shell propre

```
cd /home/mallanic/Projects/Guacamole
env -u TURN_URL -u TURN_SECRET ./scripts/verify-all.sh 2>&1 | tee docs/superpowers/plans/journaux-page-pomerium/verify-all.log
```

Sortie complète : [`verify-all.log`](journaux-page-pomerium/verify-all.log)
(391 991 octets). Dernière ligne : `Les 10 étapes sont passées.`

Contrôles :

```
$ grep -c '^==>' docs/superpowers/plans/journaux-page-pomerium/verify-all.log
18
```

⚠️ **J'ANNONCE LES DIX ÉTAPES** (celles que le script compte et fait passer),
**pas les dix-huit en-têtes `==>` affichés** — les huit de plus viennent de
`client : npm run design:verifier`, comme `CLAUDE.md` le documente. Les deux
comptes sont vrais de choses différentes : dix étapes, dix-huit en-têtes.

**Verdict : VERT.** Exit code `0`, 62 fichiers de test, 648 tests passés,
`typecheck` de `plateforme/` termine la suite sans erreur.

---

## Step 2 — bâtir la page et démarrer le service en mode `pomerium`

`client/dist/` existait déjà avant cette recette et a été **rebâti** par
`npm run build`, invoqué à l'intérieur de `verify-all.sh` (Step 1 ci-dessus,
en-tête `==> npm run build (sans quoi §7.3 et §7.7 jugeraient le build
d'avant)`) — je ne l'ai donc pas rebâti une seconde fois séparément.
`ls client/dist/` : `assets/`, `connexion.html`, `design.html`, `hub.html`,
`hub.webmanifest`, `index.html`, `primitives.html`, `shell.html`.

Le service a été démarré et arrêté **huit fois** au cours de cette recette,
chaque fois avec un secret tiré au sort et une configuration adaptée au
critère en cours. Le premier démarrage armé (`PLATEFORME_PAGE=../client/dist`,
`PLATEFORME_AUTH=pomerium`, `PLATEFORME_PROXY_DE_CONFIANCE=127.0.0.1`) est
journalisé dans
[`service-arme.log`](journaux-page-pomerium/service-arme.log) :

```
> @guacamole/plateforme@0.1.0 start
> tsx src/index.ts

magasin d icones : donnees/icones
magasin de tranches : donnees/televersements
(node:407062) ExperimentalWarning: SQLite is an experimental feature and might change at any time
plateforme à l'écoute sur 127.0.0.1, le port 8080
```

---

## Step 3 — les huit critères

### ① Témoin négatif — sans `PLATEFORME_PAGE`, `GET /` rend `404 introuvable`

**Joué en premier**, avant tout autre critère : c'est lui qui rend le `200`
du critère ② interprétable — sans lui, un `200` pourrait être rendu par un
service qui aurait toujours servi la page, indépendamment de la variable.

Démarrage sans `PLATEFORME_PAGE` :
[`service-sans-page.log`](journaux-page-pomerium/service-sans-page.log).

```
$ curl -si localhost:8080/
```

Sortie ([`critere-1-temoin-negatif.log`](journaux-page-pomerium/critere-1-temoin-negatif.log)) :

```
HTTP/1.1 404 Not Found
content-type: text/plain; charset=utf-8
X-Content-Type-Options: nosniff
Cache-Control: no-store
...

introuvable
```

**Verdict : VERT.** `404`, corps `introuvable` — exactement l'attendu, et
c'est le témoin dont dépend l'interprétation du critère ②.

### ② Octets — `curl -s localhost:8080/ | diff - client/dist/index.html`

Service redémarré avec `PLATEFORME_PAGE=../client/dist`
([`service-arme.log`](journaux-page-pomerium/service-arme.log)).

```
$ curl -s localhost:8080/ | diff - client/dist/index.html ; echo "diff exit=$?"
$ wc -c /tmp/curl-index-body.html client/dist/index.html
$ md5sum /tmp/curl-index-body.html client/dist/index.html
```

Sortie ([`critere-2-octets.log`](journaux-page-pomerium/critere-2-octets.log)) :

```
diff exit=0

 5388 /tmp/curl-index-body.html
 5388 client/dist/index.html
10776 total

b00cf1f63752bf80b487f711445b3907  /tmp/curl-index-body.html
b00cf1f63752bf80b487f711445b3907  client/dist/index.html
```

**Verdict : VERT.** `diff` ne rapporte aucune différence (exit `0`), même
compte d'octets (5388 des deux côtés), même `md5sum`. Jugé sur les **octets**,
pas sur le seul code `200` — la réponse complète, en-têtes compris, est
conservée dans
[`critere-2-page-armee.log`](journaux-page-pomerium/critere-2-page-armee.log).

### ③ En-têtes de sécurité sur `/`

```
$ curl -sI localhost:8080/
```

Sortie ([`critere-3-entetes.log`](journaux-page-pomerium/critere-3-entetes.log)) :

```
HTTP/1.1 200 OK
content-type: text/html; charset=utf-8
X-Content-Type-Options: nosniff
Cache-Control: no-store
Content-Security-Policy: default-src 'self'; connect-src 'self' wss: https:; img-src 'self' data: blob:; media-src 'self' blob:; script-src 'self'; style-src 'self' 'unsafe-inline'; font-src 'self'; frame-ancestors 'none'; base-uri 'self'; form-action 'self'
Referrer-Policy: no-referrer
X-Frame-Options: DENY
...
```

Contrôle de l'absence :

```
$ grep -ci 'strict-transport-security' docs/superpowers/plans/journaux-page-pomerium/critere-3-entetes.log
0
```

**Verdict : VERT.** `Content-Security-Policy`, `Referrer-Policy` et
`X-Frame-Options` présents ; `strict-transport-security` compte **0**
occurrence — absent, comme attendu d'un service qui n'a pas terminé TLS
lui-même (c'est le proxy qui le ferait, en amont).

### ④ Cache sur un asset réel

```
$ ASSET=$(ls client/dist/assets | grep '\.js$' | head -1)   # adresse-plateforme-uutwZeXQ.js
$ curl -sI "localhost:8080/assets/$ASSET"
```

Sortie ([`critere-4-cache.log`](journaux-page-pomerium/critere-4-cache.log)) :

```
HTTP/1.1 200 OK
content-type: text/javascript; charset=utf-8
X-Content-Type-Options: nosniff
Cache-Control: public, max-age=31536000, immutable
...
```

**Verdict : VERT.** `Cache-Control: public, max-age=31536000, immutable` —
exactement la chaîne attendue, sur un fichier réellement présent dans
`client/dist/assets/` (`adresse-plateforme-uutwZeXQ.js`), pas un chemin
inventé.

### ⑤ Traversée de chemin

```
$ curl -si 'localhost:8080/%2e%2e%2f%2e%2e%2fetc%2fpasswd'
```

Sortie ([`critere-5-traversee.log`](journaux-page-pomerium/critere-5-traversee.log)) :

```
HTTP/1.1 404 Not Found
content-type: text/plain; charset=utf-8
...

introuvable
```

**Verdict : VERT.** `404`, même corps générique que le témoin négatif du
critère ① — aucune fuite du système de fichiers hôte.

### ⑥ Un fichier nommé `sante` ne masque pas la route `/sante`

Pour ne rien déposer de durable dans `client/dist/` réel, j'ai travaillé sur
une **copie isolée** : `cp -r client/dist /tmp/dist-copie-c6`, `touch
/tmp/dist-copie-c6/sante`, puis démarré le service avec
`PLATEFORME_PAGE=/tmp/dist-copie-c6`
([`service-critere6.log`](journaux-page-pomerium/service-critere6.log)).

```
$ curl -si localhost:8080/sante
```

Sortie ([`critere-6-sante.log`](journaux-page-pomerium/critere-6-sante.log)) :

```
HTTP/1.1 200 OK
content-type: application/json; charset=utf-8
Cache-Control: no-store
...

{"etat":"ok"}
```

**Verdict : VERT.** `content-type: application/json`, corps `{"etat":"ok"}`
— la route de santé applicative l'emporte sur le fichier statique homonyme,
**jamais** le fichier `sante` (qui aurait rendu `text/plain` ou un type
générique, vide, avec `Cache-Control: no-store` mais un corps différent).
Nettoyage vérifié après coup :
`ls client/dist/sante` → `Aucun fichier ou dossier de ce nom` ; la copie
`/tmp/dist-copie-c6` a été supprimée.

### ⑦ Les deux bras de la garde d'identité sur `/auth/moi`

**Bras 1 — pair déclaré de confiance** (`PLATEFORME_PROXY_DE_CONFIANCE=127.0.0.1`,
`curl` lancé depuis `127.0.0.1` —
[`service-critere7-bras1.log`](journaux-page-pomerium/service-critere7-bras1.log)) :

```
$ curl -si -H 'X-Pomerium-Claim-Email: a@b.c' localhost:8080/auth/moi
```

Sortie ([`critere-7-bras1-confiance.log`](journaux-page-pomerium/critere-7-bras1-confiance.log)) :

```
HTTP/1.1 200 OK
content-type: application/json; charset=utf-8
...

{"acces":"eyJhbGciOiJIUzI1NiIs...fVJNA0SDS9UwzXUTiCv36-vQKKR9-ALZxCQ5y_OufyA"}
```

**Bras 2 — pair NON déclaré de confiance**
(`PLATEFORME_PROXY_DE_CONFIANCE=10.9.9.9`, même `curl` depuis `127.0.0.1` —
[`service-critere7-bras2.log`](journaux-page-pomerium/service-critere7-bras2.log)) :

Sortie ([`critere-7-bras2-etranger.log`](journaux-page-pomerium/critere-7-bras2-etranger.log)) :

```
HTTP/1.1 401 Unauthorized
content-type: application/json; charset=utf-8
...

{"refus":"pair-non-de-confiance"}
```

**Verdict : VERT sur les deux bras.** Bras 1 : `200` + un jeton (JWT signé,
trois segments séparés par `.`). Bras 2 : `401`, corps
`{"refus":"pair-non-de-confiance"}` — **la même adresse cliente**
(`127.0.0.1`), la seule chose qui change entre les deux tours est la liste des
pairs de confiance déclarée au service. Un `401` seul n'aurait rien prouvé
(une route entièrement en panne le rendrait aussi) ; c'est le bras 1, qui rend
`200` sous la configuration adjacente, qui rend le `401` du bras 2
discriminant.

Le jeton du bras 1 n'est pas le secret : c'est un JWT signé par un secret tiré
au sort à ce démarrage, et jamais consigné.

### ⑧ Démarrage en `pomerium` sans `PLATEFORME_PROXY_DE_CONFIANCE`

```
$ PLATEFORME_HOTE=127.0.0.1 PLATEFORME_SECRET_JETON="$(openssl rand -hex 24)" \
  PLATEFORME_AUTH=pomerium PLATEFORME_PAGE=../client/dist \
  timeout 8 npm start
$ echo "EXIT=$?"
```

Sortie ([`critere-8-sans-proxy-confiance.log`](journaux-page-pomerium/critere-8-sans-proxy-confiance.log)) :

```
EXIT=1

> @guacamole/plateforme@0.1.0 start
> tsx src/index.ts

/home/mallanic/Projects/Guacamole/plateforme/src/config.ts:303
        throw new Error(
              ^

Error: PLATEFORME_PROXY_DE_CONFIANCE est obligatoire en mode pomerium : l'identité arrive dans un en-tête en clair qu'aucune signature ne vérifie, et sans la liste des adresses autorisées à le poser, quiconque atteint le port obtient un jeton pour l'identité de son choix. Poser l'adresse du proxy, ou PLATEFORME_AUTH=motdepasse.
    at lireConfig (/home/mallanic/Projects/Guacamole/plateforme/src/config.ts:303:15)
    ...

Node.js v24.9.0
```

Contrôle : `ss -ltnp | grep '127.0.0.1:8080'` rend une sortie **vide** — le
service n'a jamais ouvert de port d'écoute.

**Verdict : VERT.** Le démarrage **lève** (`Error`, `EXIT=1`), et le message
**nomme la variable** (`PLATEFORME_PROXY_DE_CONFIANCE est obligatoire en mode
pomerium`) ainsi que la raison (identité en clair, non signée) et le remède
(poser l'adresse, ou basculer en `motdepasse`).

---

## Tableau récapitulatif

| # | Critère | Verdict | Journal |
| --- | --- | --- | --- |
| ① | témoin négatif, sans `PLATEFORME_PAGE` | VERT — `404 introuvable` | [critere-1-temoin-negatif.log](journaux-page-pomerium/critere-1-temoin-negatif.log) |
| ② | octets de `GET /` == `client/dist/index.html` | VERT — `diff` exit 0, mêmes octets, même md5sum | [critere-2-octets.log](journaux-page-pomerium/critere-2-octets.log) |
| ③ | en-têtes de sécurité, `strict-transport-security` absent | VERT | [critere-3-entetes.log](journaux-page-pomerium/critere-3-entetes.log) |
| ④ | `cache-control` sur un asset réel | VERT — chaîne exacte | [critere-4-cache.log](journaux-page-pomerium/critere-4-cache.log) |
| ⑤ | traversée de chemin encodée | VERT — `404` | [critere-5-traversee.log](journaux-page-pomerium/critere-5-traversee.log) |
| ⑥ | fichier `sante` vs route `/sante` | VERT — JSON de santé rendu, jamais le fichier | [critere-6-sante.log](journaux-page-pomerium/critere-6-sante.log) |
| ⑦ | les deux bras de la garde `/auth/moi` | VERT — `200`+jeton puis `401 pair-non-de-confiance` | [critere-7-bras1-confiance.log](journaux-page-pomerium/critere-7-bras1-confiance.log), [critere-7-bras2-etranger.log](journaux-page-pomerium/critere-7-bras2-etranger.log) |
| ⑧ | démarrage sans `PLATEFORME_PROXY_DE_CONFIANCE` | VERT — lève, message nomme la variable | [critere-8-sans-proxy-confiance.log](journaux-page-pomerium/critere-8-sans-proxy-confiance.log) |

**Les huit critères sont VERTS**, chacun avec sa commande, sa sortie brute et
son verdict lu sur l'assertion — pas sur le seul code de sortie.

---

## Ce que cette recette n'établit PAS

🔴 **LE CRITÈRE ⑦ D'`auth-pomerium` — la page dans un navigateur réel, derrière
Pomerium — N'EST TOUJOURS PAS JOUÉ.** Ce lot retire le **blocage ①**, qui
était un défaut de **conception** : la route nue de Pomerium visait un
backend (la plateforme) qui ne servait aucun fichier statique, `GET /` y
rendant `404 introuvable` avant ce lot. Ce blocage a disparu — `GET /` sert
maintenant la page bâtie, mesuré ci-dessus au critère ②. Le **blocage ②**
demeure **entier** : le flux OAuth Google, en amont de Pomerium, exige **un
humain** dans un navigateur avec interface, et aucun Chrome sans tête ne le
franchit. Il relève du **lot 4** du cadrage, pas de celui-ci.

⚠️ **NE PAS CONFONDRE CE « CRITÈRE ⑦ » (celui d'`auth-pomerium`, la page vue
par un navigateur réel derrière Pomerium) AVEC LE « CRITÈRE ⑦ » DE CETTE
RECETTE-CI** (les deux bras de la garde d'identité sur `/auth/moi`, mesurés
ci-dessus et VERTS). Ce sont deux affirmations distinctes qui portent le même
numéro dans deux documents différents — la première reste ouverte, la seconde
est close par cette recette.

Ce que cette recette ne couvre pas non plus :

- **Aucun navigateur réel n'a chargé la page** : toute la mesure est faite par
  `curl`, en ligne de commande. Ce qu'un navigateur exige — CORS, exécution de
  script, CSP appliquée réellement — n'est pas vu par cette classe
  d'instrument (`CLAUDE.md`, « ce que le montage de recette ne peut pas
  voir »).
- **Aucune mesure derrière un VRAI proxy Pomerium** : le critère ⑦ de cette
  recette simule la confiance de pair par `PLATEFORME_PROXY_DE_CONFIANCE`, pas
  par un Pomerium réellement interposé qui poserait l'en-tête lui-même après
  un flux OAuth authentique.
- **`onze pilotes de recette qui visent encore l'ancienne racine du relais de
  signaling`** (legs déjà consigné dans `CLAUDE.md`, chantier `auth-pomerium`)
  ne sont pas concernés par ce lot et restent dus.
- **La VM Windows n'a pas été démarrée** : rien dans ce lot n'en dépend, et
  cette recette ne l'a donc ni sondée ni requise.
