# La page derrière Pomerium — conception

> **21 août 2026.** Lot 1 de la finalisation. Cadrage d'ensemble des quatre
> lots : [`2026-08-21-finalisation-cadrage.md`](2026-08-21-finalisation-cadrage.md).

## 1. Le problème, tel qu'il est — et non tel que le legs le dit

Le chantier `auth-pomerium` a clos sa recette sans jouer son **critère ⑦** :
la page, dans un navigateur, derrière Pomerium. `CLAUDE.md` inscrit ce legs
avec l'avertissement qui compte : **il reste à CONCEVOIR, pas à mesurer.**

**Ce qui a été établi par lecture du code le 21 août 2026**, et non recopié :

| Fait | Preuve |
| --- | --- |
| La plateforme ne sert **aucun** fichier statique | `plateforme/src/http/serveur.ts`, `servirTout` — dix routeurs chaînés, aucun ne prend `/`, puis un `404 introuvable` posé dans le `createServer` |
| C'est **nginx** qui sert la page | `deploiement/nginx.conf` — `root /usr/share/nginx/html`, `try_files $uri $uri/ /index.html` |
| La route nue de Pomerium vise la **plateforme** | `docs/superpowers/specs/2026-08-21-auth-pomerium-design.md` § 7.2, `to: http://192.168.3.1:8080`, commentée « La page et l'API » |
| nginx **efface** l'en-tête d'identité, au niveau `http` — donc globalement | `proxy_set_header X-Pomerium-Claim-Email "";` — invariant ⑤ de `deploiement/README.md` |
| `/auth/moi` rend un jeton interne valide pour **n'importe quel** courriel de l'en-tête | `plateforme/src/http/routes-identite.ts`, `servirIdentite` — aucune garde entre `lireIdentitePomerium` et `signer` |

🔴 **LE SECOND FAIT EST PLUS GRAVE QUE CE QUE LE § « LEGS OUVERTS » EN DIT.**
Il y est écrit « aucun frein sur `/auth/moi` », et la conséquence annoncée est
une table `utilisateur` qui grossit sans borne. C'est vrai, et ce n'est pas le
principal : **qui atteint le port 8080 — donc la VM Windows, que le § 7.1 de la
spec `auth-pomerium` place nommément dans ce périmètre — forge l'en-tête et
obtient un jeton interne pour l'identité de son choix.** C'est un contournement
complet de l'authentification, pas une fuite de ressource. **Un frein ne le
fermerait pas** : il bornerait la cadence d'un contournement qui n'a besoin
d'aboutir qu'une fois.

## 2. Ce que ce lot livre, et ce qu'il ne livre pas

**Il livre** : un servant de fichiers statiques dans la plateforme, les
en-têtes de document qui vont avec, et la garde d'adresse source sur
`/auth/moi`.

**Il ne livre pas** — et c'est nommé ici pour qu'aucun successeur ne le lise
comme un oubli :

- 🔴 **le verdict du critère ⑦ lui-même.** Le flux OAuth Google exige un
  humain ; aucun Chrome sans interface ne le franchit. Ce lot rend le critère
  **jouable** ; il ne le joue pas.
- l'installabilité du hub (legs de ④ : un `<img src>` ne porte pas
  d'`Authorization`). Ouvrir une route aujourd'hui authentifiée est une
  décision de sécurité distincte, laissée où elle est.
- le moindre changement au montage nginx. **Il reste intact**, et l'invariant ⑤
  n'est pas inversé.

## 3. Les voies écartées, et pourquoi

**Router Pomerium vers nginx.** Trois obstacles, dont un rédhibitoire : le
`listen 80` de nginx est un `return 301` vers HTTPS, donc une boucle depuis un
Pomerium qui a déjà terminé TLS ; le `listen 443` exige `deploiement/tls/`,
gitignoré et absent ; et surtout **nginx efface l'en-tête d'identité au niveau
`http`**. Le faire router depuis Pomerium exigerait de rendre cet effacement
**conditionnel** — c'est-à-dire d'inverser une moitié d'un couple dont
`deploiement/README.md` dit qu'il « s'inverse ENSEMBLE ». Ce dépôt a déjà payé
ce patron.

**Publier nginx en TLS et retirer Pomerium.** Le montage `motdepasse`
fonctionne déjà de bout en bout. Il est écarté parce qu'il jette le chantier
`auth-pomerium` entier, et l'identité Google avec lui.

## 4. Le servant statique

### 4.1 Où il se branche

`plateforme/src/http/routes-page.ts`, chaîné dans `servirTout` **juste avant le
`404`**, donc **après les dix routeurs existants**.

🔴 **L'ORDRE EST LA GARANTIE, PAS UNE COMMODITÉ.** Chaîné en tête, un fichier
nommé `sante` ou `vm` déposé dans la racine volerait le chemin d'un routeur
d'API — et la panne serait la plus discrète possible, le service répondant
`200` avec un corps plausible. Chaîné en queue, un routeur d'API ne peut jamais
être supplanté : il a déjà rendu `true`.

⚠️ **Le `404` lui-même n'est pas touché.** Son corps (`introuvable\n`) et ses
en-têtes restent mot pour mot ceux d'aujourd'hui — « le changer serait un effet
de bord non déclaré » (`serveur.ts`).

### 4.2 `PLATEFORME_PAGE`

**FACULTATIVE.** Le chemin du répertoire bâti (`client/dist`).

🔴 **ABSENTE OU VIDE ⇒ AUCUN SERVANT**, et le comportement du service est celui
d'aujourd'hui **à l'octet près** : `GET /` rend `404 introuvable`. C'est ce qui
rend l'ajout strictement additif — aucun test existant ne bouge — **et c'est ce
qui rend le témoin négatif jouable** : sans lui, un `200` sur `/` ne prouverait
pas que la variable a servi à quelque chose.

⚠️ **Le test de la chaîne VIDE est distinct de celui de l'absence** :
`env.X ?? 'defaut'` ne rattrape pas `''`. P1 a payé cette erreur exacte sur
`PLATEFORME_ICONES` ; la garde est recopiée de là.

⚠️ **Elle n'a AUCUN défaut**, à la différence de `PLATEFORME_ICONES` et
`PLATEFORME_TELEVERSEMENTS`. Un défaut comme `client/dist` ferait servir un
répertoire au hasard du répertoire courant du service, et ferait passer le
montage nginx — où la plateforme ne doit **rien** servir — d'un `404` franc à
un `200` sur des fichiers qu'on n'a pas voulu publier.

### 4.3 La règle de résolution — PURE

Un module sans `fs`, sans `http`, éprouvé sur l'hôte. Il prend un chemin
d'URL et rend soit un chemin relatif de fichier et son type MIME, soit un
refus motivé.

| Cas | Décision |
| --- | --- |
| `%00` n'importe où dans le chemin décodé | **refus** |
| le chemin résolu sort de la racine | **refus** |
| `/` | `index.html` |
| un chemin **avec** extension, connu de la liste MIME | ce fichier |
| un chemin **sans** extension | `index.html` — le `try_files` de nginx, reproduit |
| une extension **hors** de la liste MIME | **refus** |

🔴 **LA TRAVERSÉE SE JUGE SUR LE CHEMIN RÉSOLU, JAMAIS SUR UNE SOUS-CHAÎNE
`..`.** Un filtre par sous-chaîne se contourne par encodage (`%2e%2e`), par
double encodage, et par des séparateurs Windows ; une normalisation suivie d'un
test de préfixe sur le résultat ne se contourne par aucun des trois. C'est la
même leçon que « muter par un motif ancré sur la syntaxe, jamais par une
sous-chaîne ».

🔴 **LA LISTE MIME EST CLOSE, ET UNE EXTENSION INCONNUE REFUSE.** Retomber sur
`application/octet-stream` publierait, avec une invite de téléchargement, tout
fichier qu'un déploiement aurait laissé dans la racine — un `.env`, une clé, un
`.map`. La liste couvre ce que Vite produit réellement (relevé le 21 août 2026
sur `client/dist` : `html`, `js`, `css`, `webmanifest`) plus les formes d'icône
et de police attendues (`ico`, `png`, `svg`, `woff2`).

🔴 **`GET` ET `HEAD` SEULS — ET TOUTE AUTRE MÉTHODE REND `false`, JAMAIS
`405`.** La première rédaction de ce document prescrivait `405`, ce qui est plus
juste au sens de HTTP et **faux ici** : le repli SPA résout *n'importe quel*
chemin sans extension vers `index.html`, si bien qu'un `POST /aplication/x`
— une faute de frappe sur un appel d'API — obtiendrait `405 méthode` au lieu du
`404` qui le désigne. Le service masquerait la faute au lieu de la nommer.
Rendre `false` préserve le `404` générique pour toutes les méthodes, et
maintient la promesse du § 4.2 : **hors `GET`/`HEAD`, le comportement est celui
d'aujourd'hui à l'octet près, que `PLATEFORME_PAGE` soit posée ou non.**

### 4.4 Ce que le servant lit du disque

Le module impur se borne à : ouvrir le fichier résolu **sous la racine**, et le
rendre en flux. Un fichier absent après résolution rend le `404` générique — le
servant rend `false` et laisse la chaîne se terminer, plutôt que d'inventer une
seconde forme de `404`.

## 5. Les en-têtes

### 5.1 Le commentaire qui devient faux

🔴 **`plateforme/src/http/entetes.ts` AFFIRME AUJOURD'HUI QUE LA PLATEFORME NE
SERT PAS LE HTML** — sa table de partage avec le proxy attribue la CSP au proxy
« parce qu'elle porte sur le DOCUMENT, **que la plateforme ne sert pas** ».
**Ce lot rend cette phrase fausse, et la corriger fait partie du lot.** C'est
exactement le patron du « naufrage du 487 » : une affirmation vraie le jour où
elle est écrite, survivant à la réalité qu'elle décrivait.

### 5.2 Trois jeux, par nature de réponse

| Réponse | En-têtes |
| --- | --- |
| **JSON** (les dix routeurs) | **INCHANGÉ** — `X-Content-Type-Options: nosniff`, `Cache-Control: no-store` |
| **document HTML** | `nosniff`, `Cache-Control: no-store`, **`Content-Security-Policy`**, `Referrer-Policy: no-referrer`, `X-Frame-Options: DENY` |
| **ressource** (`assets/`, icônes, polices) | `nosniff`, **`Cache-Control: public, max-age=31536000, immutable`** |

🔴 **`no-store` APPLIQUÉ AUX RESSOURCES TUERAIT LE CACHE DU NAVIGATEUR** sur des
noms que Vite empreinte déjà. Réutiliser `ENTETES_SECURITE` tel quel pour tout
ce que sert le servant est le geste naturel, et c'est le défaut : `no-store` y
est **inconditionnel**, et il est là pour les réponses de `/auth/*`, qui
portent des jetons en clair. Les deux besoins sont opposés ; le lot les sépare.

🔴 **HSTS RESTE AU TERMINATEUR TLS, DONC À POMERIUM — la plateforme ne l'émet
pas.** La ligne de partage devient : *ce qui dépend du DOCUMENT suit le
document ; ce qui dépend de TLS reste chez qui termine TLS.* Émettre
`Strict-Transport-Security` depuis un backend joignable en clair sur
`192.168.3.1:8080` serait une affirmation que ce backend n'est pas en position
de faire.

### 5.3 La CSP, et sa dérive

La valeur est **recopiée** de `deploiement/nginx.conf` :

```
default-src 'self'; connect-src 'self' wss: https:; img-src 'self' data: blob:;
media-src 'self' blob:; script-src 'self'; style-src 'self' 'unsafe-inline';
font-src 'self'; frame-ancestors 'none'; base-uri 'self'; form-action 'self'
```

🔴 **DEUX COPIES D'UNE MÊME POLITIQUE DÉRIVENT. LE LOT POSE DONC UN TEST QUI
LIT `deploiement/nginx.conf` ET LA CONSTANTE TYPESCRIPT, ET LES COMPARE.** Sans
lui, un durcissement appliqué d'un seul côté livrerait deux montages aux
sécurités différentes sans qu'aucune suite ne bronche. ⚠️ Le test doit
**échouer** si le fichier nginx est introuvable, jamais se replier sur un vert :
« un `||` de repli transforme *fichier absent* en *contrôle vert* ».

## 6. La garde d'identité sur `/auth/moi`

### 6.1 La règle

Dans `servirIdentite`, **avant** `lireIdentitePomerium` : l'adresse du pair doit
appartenir à `deps.proxyDeConfiance`. Sinon, `401`.

⚠️ **L'ADRESSE SE LIT SUR `req.socket.remoteAddress`, JAMAIS PAR
`adresseSource`.** Cette dernière honore `X-Forwarded-For` **lorsque le pair est
de confiance** — ce qui est correct pour attribuer une requête à un client
derrière le proxy, et circulaire ici : on ne peut pas croire un en-tête fourni
par l'attaquant pour décider si l'on croit l'attaquant. Seule la fonction
`normaliser` d'`adresse-source.ts` est réutilisée, pour le préfixe `::ffff:`
des adresses IPv4 mappées.

⚠️ **La garde se place AVANT la lecture de l'en-tête, pas après.** Après, elle
serait correcte aussi, mais le service aurait déjà lu une identité qu'il refuse
— et un successeur pourrait déplacer la lecture sans voir que la garde en
dépendait.

### 6.2 Le refus de démarrer

🔴 **EN MODE `pomerium`, `PLATEFORME_PROXY_DE_CONFIANCE` DEVIENT OBLIGATOIRE :
LE SERVICE REFUSE DE DÉMARRER SANS ELLE.**

C'est le **troisième** garde lié au mode, et il suit exactement l'idiome
qu'`auth-pomerium` a posé sur `PLATEFORME_HOTE` (`config.ts` : en mode
`pomerium`, les quatre écoutes universelles lèvent). La raison est la même : en
mode `pomerium`, l'identité arrive dans un en-tête **en clair, qu'aucune
signature ne vérifie** ; sans la liste des adresses autorisées à la poser,
l'en-tête est croyable par n'importe qui.

**Pourquoi un refus de démarrer plutôt qu'un `401` à l'exécution** : un refus de
démarrer se lit **avant d'agir**, et nomme la variable et la raison. Un `401`
silencieux pour tout le monde se lit **après**, sur un service qui répond,
écoute et sert les dix autres routeurs — la panne la plus discrète possible.

⚠️ **C'est une rupture** pour un montage qui, faute de page servie, ne peut pas
fonctionner aujourd'hui. `docker-compose.plateforme.yml` pose
`PLATEFORME_AUTH: motdepasse` et n'est donc pas concerné.

### 6.3 Ce que la garde ne promet pas

Elle refuse l'en-tête de qui n'est pas dans la liste. **Elle ne garantit pas que
seul Pomerium porte cette adresse** — c'est à la charge de l'exploitant, comme
la garde d'écoute de `PLATEFORME_HOTE` le dit déjà d'elle-même.

⚠️ **L'adresse depuis laquelle Pomerium se connecte sera MESURÉE sur une
connexion réelle, jamais supposée.** Pomerium tourne en `network_mode: host` et
vise `192.168.3.1:8080` ; c'est le noyau qui choisit l'adresse source, et la
déduire serait une supposition déguisée en fait.

## 7. La recette

### 7.1 Ce qu'elle établit, sans navigateur ni humain

| # | Critère | Comment il rougit |
| --- | --- | --- |
| ① | `PLATEFORME_PAGE` **absente** ⇒ `GET /` rend `404 introuvable` | **le témoin négatif** — sans lui, le `200` du critère ② ne prouve rien |
| ② | posée ⇒ `GET /` rend l'`index.html` bâti | comparer les octets au fichier de `client/dist`, jamais le seul code `200` |
| ③ | le document porte la CSP, `Referrer-Policy`, `X-Frame-Options`, **et pas HSTS** | les quatre assertions dans des tests **séparés** — `expect` s'arrête au premier échec |
| ④ | `/assets/<empreinte>.js` porte `immutable`, **jamais `no-store`** | idem, et c'est la moitié qui compte |
| ⑤ | une traversée encodée (`%2e%2e%2f`) refuse | ⚠️ éprouver **au moins** la forme encodée, un filtre par sous-chaîne la laisserait passer |
| ⑥ | `GET /sante` reste servi par **son routeur**, même avec un `sante` déposé dans la racine | **la rouge de l'ordre de chaînage** |
| ⑦ | `/auth/moi` : adresse étrangère ⇒ `401` ; adresse déclarée ⇒ `200` + jeton | **les DEUX bras** — un `401` seul serait rendu par une route en panne |
| ⑧ | mode `pomerium` sans `PLATEFORME_PROXY_DE_CONFIANCE` ⇒ le démarrage **lève**, et le message nomme la variable | lire le message, pas seulement le code de sortie |

🔴 **Chaque rouge se joue** : provoquer délibérément l'état que le contrôle doit
dénoncer, et vérifier **quelle assertion** rougit. Un contrôle qu'on n'a jamais
vu rouge n'est pas un contrôle.

### 7.2 Ce qu'elle ne peut pas établir

🔴 **LE CRITÈRE ⑦ D'`auth-pomerium` — la page dans un navigateur réel, derrière
Pomerium.** Le flux OAuth Google exige un humain. Ce lot retire le **blocage
①** (le défaut de conception) ; **le blocage ② demeure**, et il est de nature
ordinaire. La marche à suivre est livrée avec le lot ; le verdict appartient au
propriétaire du dépôt.

## 8. Contraintes de mise en œuvre

🔴 **`serveur.ts` est à 475 lignes sur 500** (relevé le 21 août 2026 ; **le
relever à nouveau avant d'agir**, ce dépôt ayant payé neuf fois un nombre
recopié). Le lot y ajoute un import, une clé de dépendances et un chaînage,
avec la densité de commentaire de ce fichier. **Une tâche d'EXTRACTION dédiée
précède celle qui ajoute** — la forme forte que `CLAUDE.md` prescrit, plutôt
qu'une compression après coup, que le même fichier interdit nommément.

⚠️ `config.ts` est à 281 lignes : il a de la marge pour la variable et la
garde.

⚠️ **La variable neuve n'a pas à rejoindre `scripts/run-agent.sh`** : c'est une
variable de la **plateforme**, pas de l'agent. Elle rejoint
`deploiement/plateforme.env.exemple`, `deploiement/README.md` et le tableau des
variables de `CLAUDE.md`.

## 9. Ce que ce lot change dans la documentation

Chacun de ces points est une affirmation **aujourd'hui vraie** que le lot rend
fausse. Les corriger n'est pas un supplément, c'est le lot.

| Où | Ce qui devient faux |
| --- | --- |
| `plateforme/src/http/entetes.ts` | « le DOCUMENT, **que la plateforme ne sert pas** » |
| `CLAUDE.md`, § Legs ouverts | le blocage ① du critère ⑦ ; « aucun frein sur `/auth/moi` » (à **requalifier** : c'était un contournement, pas un frein manquant) |
| `CLAUDE.md`, tableau des variables serveur | `PLATEFORME_PAGE` absente ; `PLATEFORME_PROXY_DE_CONFIANCE` décrite comme **facultative sans condition** |
| `deploiement/README.md` | la mise en route ne connaît qu'un montage ; le § « Ce que ce déploiement ne couvre pas » ignore Pomerium |
| `docs/superpowers/specs/2026-08-21-auth-pomerium-design.md` § 7 | l'annotation « il reste à concevoir » — **à lever, en nommant ce document** |
