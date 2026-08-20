# Sous-bloc P5 — le durcissement, le déploiement, et la clôture de ⑤ : résultats

**Date** : 20 août 2026.
**Plan** : `docs/superpowers/plans/2026-08-19-plateforme-p5.md` (`02075bc`, corrigé par `920a1eb`).
**Spécification** : `docs/superpowers/specs/2026-08-19-plateforme-design.md` (`217a765`).
**Commits de la branche** : `bb5a40f` … `21abf7b`.
**Binaire mesuré** : **n/a — service TypeScript**, lancé par `tsx src/index.ts`,
sans étape de compilation. Ce qui tient lieu de « fraîcheur du binaire » est le
commit, nommé dans l'en-tête de chaque journal.

**C'est le DERNIER sous-bloc du sous-projet ⑤.** Le §12 dit ce que ⑤ laisse
ouvert, et c'est la partie à lire si l'on n'en lit qu'une.

---

## 0. Comment lire les journaux

`docs/superpowers/plans/journaux-plateforme-p5/` — **UNE seule famille**, et
c'est le sous-bloc le plus simple du dépôt de ce point de vue : **tout est en
UTF-8, sans séquence ANSI, sans octet de contrôle**, et se `grep`e à plat.

⚠️ **Deux réserves de lecture, relevées plutôt que subies** :

- les journaux capturent la sortie d'un shell dont le profil lance un `ls` :
  quelques listings de répertoire s'y intercalent, sans rapport avec la mesure.
  Ils sont inoffensifs et **non nettoyés** — nettoyer une pièce après coup est
  précisément ce que ce dépôt refuse ;
- `03-critere3-frein.log` fait **2 046 lignes** : il porte les sorties de test
  complètes de deux exécutions vertes et de trois mutations. Les verdicts sont
  en fin de fichier, sous `########## VERDICT ③ ##########`.

Répertoire : `references-t0.log` et les `mutations-t*.log` (tâches 0 à 11),
`cloture-t0-t11.log` et `cloture-finale-t0-t11.log`, `t12-16/` (tâches 12 à 16),
`recette/` (tâche 17, avec son `instrument/`), et `revue-transverse.log`
(tâche 18).

---

## 1. Le fait n°1 : le critère ⑤ a trouvé un défaut que RIEN d'autre ne pouvait voir

**Le profil `deploiement` n'avait jamais été LANCÉ, seulement BÂTI.** À la
première tentative de démarrage, nginx démarre, Postgres est sain, et le
service meurt au chargement :

```
plateforme-1  |   code: 'ERR_MODULE_NOT_FOUND',
plateforme-1  |   url: 'file:///opt/proto/ts/plateforme'
```

Trois fichiers de **production** importent `../../../proto/ts/plateforme` —
`agents/canal.ts`, `agents/registre.ts`, `apps/catalogue.ts` —, c'est-à-dire un
chemin qui **sort** de `plateforme/`. Le contexte de construction était borné à
`plateforme/`. **L'image se construisait sans erreur et ne démarrait pas.**

🔴 **CE QUI REND CE DÉFAUT INSTRUCTIF EST LA LISTE DE CE QUI NE LE VOYAIT PAS :**

| Ce qui était vert | Pourquoi il ne pouvait pas voir |
| --- | --- |
| les 433 tests, sur les deux moteurs | ils tournent depuis l'arbre, où le chemin relatif existe |
| `docker build` | rien n'est résolu à la construction ; l'image se bâtit |
| `docker compose config` | il analyse un fichier, il ne démarre rien |
| `nginx -t` | il n'a aucune opinion sur l'amont |
| `tsc --noEmit` | il résout depuis l'arbre, comme les tests |

**C'est exactement ce que le critère ⑤ existe pour fermer** — « ce qu'un
navigateur exige et qu'un test serveur ne voit pas » —, étendu ici à « ce qu'un
DÉMARRAGE exige et qu'une CONSTRUCTION ne voit pas ».

**Correctif** (`ae7b7f1`) : le contexte devient la racine du dépôt, `proto/ts`
est copié à `/opt/proto/ts` — l'adresse n'est pas libre, c'est celle que Node
nommait —, et le contexte est borné par `plateforme/Dockerfile.dockerignore`,
**nommé d'après son Dockerfile** pour ne pas s'appliquer aussi à l'image du
Guacamole historique bâtie depuis `./Dockerfile`.

⚠️ **AUCUN fichier de `proto/` n'est touché** : il est seulement copié.

### Un second obstacle, de déploiement celui-là, et il est dans le runbook

`GET /` rendait **403** et ses ressources **500**, sur une pile par ailleurs
entièrement saine. Cause : `client/dist` est en `0750 root:root` (l'umask de ce
dépôt) et le **worker** nginx tourne en utilisateur `nginx`. Ni les tests, ni
`nginx -t`, ni `docker compose config` ne le voient ; seul le journal d'erreur
de nginx nomme la permission. `deploiement/README.md` porte désormais le
`chmod -R a+rX dist` et sa raison.

---

## 2. Le verdict des cinq critères, avec leur nombre d'exécutions

**Aucun taux n'est revendiqué nulle part.**

| # | Critère | Verdict | Exécutions | Journal |
| --- | --- | --- | --- | --- |
| ① | la suite est verte sur le Postgres **déployé** | **TENU** | **2** (les deux sur postgres) | `recette/01-…` |
| ② | coturn n'ouvre que sur l'adresse nommée | **TENU** | **2** lancements | `recette/02-…` |
| ③ | les tentatives sont freinées, sur les **deux** chemins | **TENU** | **2** (sqlite, postgres) | `recette/03-…` |
| ④ | aucun secret dans un fichier versionné | **TENU** | **2** (sqlite, postgres) | `recette/04-…` |
| ⑤ | le chemin navigateur réel, derrière le proxy | **TENU**, après le correctif du §1 | **2** | `recette/05-…` |

### ① — le discriminant prescrit ne discriminait pas, et la mesure le dit

433 tests sur 49 fichiers, verts, sortie 0, **deux fois**, contre l'instance du
profil `deploiement` (`127.0.0.1:5434`, volume nommé).

🔴 **MAIS LE DISCRIMINANT DE LA DÉCISION D15 EST INSUFFISANT.** D15 prescrivait
`SELECT current_database(), inet_server_port(), version()`. Mesuré :

| Champ | Instance déployée | Instance de test | Discrimine ? |
| --- | --- | --- | --- |
| `inet_server_port()` | 5432 | 5432 | **NON** — port du serveur *dans son conteneur* |
| `version()` | PostgreSQL 16.15 … | PostgreSQL 16.15 … | **NON** — même image, à dessein |
| `current_database()` | `plateforme` | `plateforme_test` | par CONVENTION DE NOM seulement |
| `pg_control_system().system_identifier` | **7676031821417750562** | **7675690416090275874** | **OUI** — identité de grappe |

**La rouge du critère ① n'est pas « casser le service » : c'est pointer la suite
sur l'instance de TEST et vérifier que le discriminant LE DIT.** Jouée : la
suite est **verte aussi** (433/433) et l'identité de grappe rapportée bascule.
C'est le point : *une suite verte, seule, ne dit pas quelle instance elle a
touchée.* **Le discriminant a été VU distinguer.**

⚠️ **Corroboration au passage** : le pilote du service a **refusé** le
`system_identifier` brut — « BIGINT hors de l'entier sûr de JavaScript, converti
nulle part : 7676031821417750562 ». Le discriminant emprunte donc bien le chemin
du service, garde comprise, et non un `psql`.

### ② — une relecture de journal, et rien de plus, mais elle est nette

|  | fichier LIVRÉ (t12) | ROUGE (le fichier d'avant t12) |
| --- | --- | --- |
| lignes `listener opened on` | 25 | **575** |
| adresses d'écoute DISTINCTES | **1** (`192.168.3.1`) | **23** |
| lignes portant l'adresse publique `90.87.35.18` | **0** | **25** |
| `Relay address to use` | `192.168.3.1` | l'hôte entier |
| `--env-file /dev/null config` | **sortie 1**, en NOMMANT `TURN_LISTENING_IP` | sortie 0 |

Les deux lancements du fichier livré sont identiques.

🔴 **CE QUE ② N'ÉTABLIT PAS, et c'est écrit depuis le plan** : l'inaccessibilité
du relais **depuis Internet**. C'est une relecture du journal de coturn, pas une
sonde. Elle établit ce que le processus DIT avoir ouvert, pas ce qu'une machine
extérieure peut joindre — et une telle sonde exigerait une machine hors de ce
réseau, que ce chantier n'a pas.

### ③ — deux rouges DISTINCTES, et une trouvaille

Deux exécutions vertes, 33 tests sur 2 fichiers, sur chaque moteur.

| Rouge | Ce qu'elle neutralise | Ce qui tombe |
| --- | --- | --- |
| A | la consultation du frein dans `http/routes-auth.ts` | **8** échecs, **tous** dans `routes-auth.test.ts`, **aucun** dans `canal.test.ts` |
| B | la consultation du frein dans `agents/canal.ts` | **2** échecs, **tous** dans `canal.test.ts`, **aucun** dans `routes-auth.test.ts` |

**Aucune ne fait tomber le chemin de l'autre** : c'est la forme attendue, et
c'est elle qui établit que les deux chemins sont freinés SÉPARÉMENT.

🔴 **CE QUE LA ROUGE B A TROUVÉ, ET QUE PERSONNE NE CHERCHAIT** : le test
`canal.test.ts` « (a) la n+1ᵉ tentative sur la MÊME VM est refusée par le
frein » **RESTE VERT** quand l'unique site d'application du frein est
neutralisé. Il n'affirme que `type === 'refus'`, or le refus freiné et le refus
par mauvais secret sont **identiques par conception** — c'est l'oracle
d'énumération que le test (c) existe pour verrouiller. Ce test ne peut donc
**structurellement** pas distinguer les deux, quel que soit le produit ; son
titre promet plus que sa mesure.

✅ **Le comportement EST couvert** : par (b) et (e), tous deux par le même
discriminant `compteur.acces() === 0` — « le refus freiné n'a RIEN lu en base ».
C'est ce discriminant, et lui seul, qui porte ③ côté `/agent`.

### ④ — la rouge nomme le fichier, la ligne et le NOM, jamais la valeur

Deux exécutions vertes. La rouge, jouée par la procédure de la tâche 11 :

```
+   "essai-secret.env:1 affecte TURN_SECRET [empreinte 59444af30f860ed4]",
```

`git status --porcelain` **avant** et **après** : identiques, vérifiés ligne à
ligne. Le retrait s'est fait par `git rm --cached` puis suppression **nommée** —
jamais `git clean`, un chantier concurrent ayant des fichiers non suivis dans
cet arbre.

### ⑤ — quatre relevés, deux rouges, et les trois assertions savent échouer

Montage : profil `deploiement` **complet** (postgres + plateforme + nginx), TLS
terminé par le proxy, certificat auto-signé **engendré dans `/tmp`** puis copié
dans `deploiement/tls/` (**gitignoré**, vérifié par `git check-ignore`).
Navigateur : Chrome 151, piloté par CDP, avec `--ignore-certificate-errors-spki-list`
— **jamais** un `--ignore-certificate-errors` global, qui rendrait le contrôle
vacueux ailleurs.

| Relevé | Exécution 1 | Exécution 2 |
| --- | --- | --- |
| (a) la page se charge en `https:` | `https:`, `isSecureContext = true` | idem |
| (b) aucune violation CSP | **0** dans la page, **0** au journal de console | idem |
| (c) la montée `wss://` s'établit | **WS-OUVERT** sur `wss://localhost:8443/` | idem |
| (d) `POST /auth/connexion` lisible | **200**, corps lisible, champs `["acces","expire_a","rafraichissement"]` | idem |

| Rouge | Forme |
| --- | --- |
| n°1 — le client d'avant la tâche 16 (`ws://…:8080` depuis une page `https://`) | **1** violation, **WS-REFUSE** ; (d) reste VERT — elle ne touche que (b) et (c) |
| n°2 — CSP `connect-src 'none'` | **2** violations, **WS-REFUSE**, ET `Failed to fetch` — (b), (c) et (d) tombent ENSEMBLE |

**Les trois assertions sont donc capables d'échouer, et la n°1 montre qu'elles
ne tombent pas toutes ensemble par construction.**

🔴 **LA ROUGE QUE LE PLAN PRESCRIVAIT POUR LA TÂCHE 14 EST VACUEUSE, et la
tâche 14 l'avait déjà mesuré** : `'self'` couvre déjà une montée `wss://` de
même origine, donc retirer `wss:` ne casse rien (3 exécutions sur 3). La rouge
qui discrimine est `connect-src 'none'`, nommée dans `deploiement/nginx.conf`
à cet effet, et c'est celle-là qui a été jouée.

### Un contrôle supplémentaire que le plan ne demandait pas : `X-Forwarded-For` mord-il ?

C'est l'invariant ③ du runbook, et le défaut n°2 que la tâche 14 avait trouvé en
exécutant (nginx **remplace** les `proxy_set_header`, il ne les fusionne pas) —
fermé alors sur un amont d'ESSAI. Mesuré ici sur le **service réel** : 51
tentatives à courriels tous distincts, puis

```
frein route=/auth/connexion adresse=172.18.0.1 cles="compte:balayage-38@p5.local adr:172.18.0.1" retry_apres_s=866 entrees=51 evictions=0
```

**L'adresse journalisée est `172.18.0.1` — le client tel que le proxy le voit —
et NON `172.18.0.5`, le proxy lui-même.** Le frein par adresse n'a pas dégénéré
en frein global. La 52ᵉ tentative rend **429**.

⚠️ **Ce que ce contrôle n'établit pas** : que le frein distingue DEUX clients.
Toutes les requêtes arrivent par la même passerelle Docker.

---

## 3. La revue transverse — dix constats, dont trois avec une conséquence

Détail complet : `journaux-plateforme-p5/revue-transverse.log`.

### Six affirmations de code devenues fausses DANS LEUR PROPRE BRANCHE

| Place | Ce qu'elle disait | Pourquoi c'est faux |
| --- | --- | --- |
| `config.ts:25` | « les tentatives de secret ne sont bridées par rien à ce jour (c'est le sujet de P5) » | elles le sont, par `canal.ts:270`, avant tout `scrypt` |
| `agents/canal.ts:105` | « le déni de service que P5 doit freiner … le jour où on voudra le brider » | ce jour est arrivé **cent soixante lignes plus bas dans le même fichier**, qui l'écrit en toutes lettres |
| `agents/canal.test.ts:131` | la même phrase, autre fichier | idem |
| `signaling/resilience.test.ts:26` | « Le frein est P5 ③ » | **faux deux fois** : le futur est passé, ET c'était le mauvais remède |
| `client/src/jeton.ts:18` | « il se rouvrira[a] [au] sous-bloc P5 » | P5 est passé, les en-têtes sont livrés, **et le stockage n'a pas changé** |
| `obs/journal.ts:6-9` | « DIX-NEUF occurrences … `agents/canal.ts` (5) » | périmé par la **tâche 8 de la même branche** : 21 et 6 |

⚠️ **Le cas de `resilience.test.ts` mérite d'être retenu pour sa forme.** Ce qui
ferme le déni de service en une trame n'est pas le frein — `relais.ts` n'importe
même pas `Frein` — mais `TRAME_MAX_OCTETS` posé en `maxPayload` sur les deux
serveurs WebSocket, **et il est éprouvé par ce fichier même, plus bas**. La
phrase renvoyait à un remède qui n'est pas venu, à côté du remède qui est venu.

⚠️ **Le cas d'`obs/journal.ts` aussi** : ce commentaire a été écrit **précisément
pour chiffrer une dette**, il s'auto-annotait déjà (« ils dériveront encore »),
et il a dérivé **deux tâches plus tard, par la faute de sa propre branche**.
Et le chiffre qui compte n'a pas bougé : la dette de migration vaut toujours
**18 sites de forme libre**, les deux additions étant structurées.

### Quatre états de routage en mémoire, pas trois

`docker-compose.plateforme.yml` et `deploiement/README.md` annonçaient **trois**
porteurs. Il y en a **quatre** : `agents/registre.ts` porte la carte
VM → socket d'agent, plus les ordres en vol.

🔴 **C'est le plus coûteux des quatre.** À deux instances, un `POST /session`
traité par l'instance qui NE tient PAS le socket de la VM ne trouve rien — même
panne muette que pour l'appariement, sur le chemin le plus fréquenté du produit.

**Personne ne pouvait le voir à l'échelle d'une tâche** : `agents/registre.ts`
est né du sous-bloc **G1**, concurrent de P5.

### Le plan a vieilli sous G1 — trois affirmations, une seule cause

- **E1 du plan est FAUX** : « la spec nomme `agents/registre.ts` ; ce fichier
  n'existe pas ». Il existe, créé par `0a719a2`. **La spec avait raison.**
- **D12 chiffrait 15 occurrences dans 9 fichiers et 5 fichiers de test** ; c'est
  21, 11 et **8**. `obs/journal.ts` le disait déjà, et son propre compte a
  vieilli à son tour (ci-dessus) : **deux niveaux d'annotation, tous deux
  périmés.**
- **La table de tailles du plan portait deux chiffres FAUX À SA DATE** :
  `canal.ts` annoncé 198 pour **287** réels, `serveur.ts` 207 pour **236**. Et
  les cinq croissances annoncées sont sous-estimées d'un facteur 2 à 4 —
  `routes-auth.ts` : « ~45 » annoncé, **+156** réel.

  ✅ **La porte à 450 a tenu**, et c'est un fait : le plus gros fichier que P5
  touche est `agents/canal.ts` à **417** (marge 83). Mais elle a tenu par 33
  lignes, pas par la marge confortable que le plan annonçait.

---

## 4. Le sort des douze divergences E1…E12

| # | Ce que le plan annonçait | Sort |
| --- | --- | --- |
| E1 | `agents/registre.ts` n'existe pas | ❌ **FAUX** — il existe (G1). Corrigé au §3 |
| E2 | le client parle `ws://` et `http://` en dur | ✅ **FERMÉ** — `client/src/adresse-plateforme.ts`, et la rouge n°1 de ⑤ le mesure |
| E3 | TURNS sur 443 n'est pas livrable ici | ✅ **CONFIRMÉ** — non livré, les deux raisons écrites dans `docker-compose.coturn.yml` |
| E4 | le critère ③ ne dit pas sur quoi porte le frein | ✅ **TRANCHÉ** — deux clés, compte et adresse (D1) |
| E5 | la spec range `/agent` hors du critère ; P3 l'y a mis | ✅ **TRANCHÉ** — `/agent` est freiné, et la rouge B le mesure |
| E6 | `enroler` ne sait qu'insérer | ✅ **FERMÉ** — `--roter` (tâche 15) |
| E7 | le lint SQL ne porte que sur les migrations | ✅ **SANS OBJET** — P5 n'écrit aucune migration |
| E8 | les routeurs traitent déjà `OPTIONS` | ✅ **CONFIRMÉ**, et mesuré à travers le proxy : **204** sur `/auth/connexion` et sur `/session` |
| E9 | le fichier de composition se dit de TEST | ✅ **FERMÉ** — profil `deploiement`, sans changer le sens des commandes existantes |
| E10 | `verify-all.sh` dépend d'une instance Postgres | ✅ **CONFIRMÉ**, et P5 n'en ajoute pas au script : la seconde instance est employée hors de lui |
| E11 | `frein.ts` doit documenter son coût | ✅ **FAIT** — `ENTREES_MAX`, la purge, l'éviction, tous documentés et éprouvés |
| E12 | le balayage des sessions au démarrage existe | ✅ **CONFIRMÉ** — P5 n'y touche pas |

⚠️ **E4 et E5 sont « tranchés », pas « fermés »** : ce sont des ambiguïtés de
spécification que P5 a levées par décision, pas des défauts qu'il a corrigés.

---

## 5. Le relevé de tailles, PAR LA COMMANDE, APRÈS la dernière édition

**Relancé après le commit `21abf7b`**, avec la commande de `CLAUDE.md` :

```
1536  agent/src/encode.rs
 630  agent/src/windows_source.rs
 561  proto/src/plateforme/tests.rs
 512  proto/ts/plateforme.test.ts
```

🔴 **LES DEUX DERNIERS NE FIGURENT DANS AUCUN TABLEAU DE DETTE**, et c'est la
première fois qu'ils sont inscrits. Ils sont nés du sous-bloc **G1**
(`457a7f8`). P5 ne les corrige pas — il ne touche ni `proto/` ni `agent/` — mais
**une dette non écrite est une dette qu'on redécouvre**.

**Marges étroites, relevées au même moment, dont plusieurs qu'aucun tableau ne
signalait :**

| Fichier | Lignes | Marge |
| --- | --- | --- |
| `agent/src/encode/arret.rs` | 500 | **0** |
| `client/verify-webrtc.mjs` | **494** | 6 — *il valait 497 au relevé D10 : il a MAIGRI* |
| `agent/src/capture.rs` | 492 | 8 |
| `agent/src/demarrage.rs` | 491 | 9 |
| `agent/src/superviseur/lanceur.rs` | 488 | 12 |
| `agent/src/pont/projfs/rappels.rs` | 488 | 12 |
| `plateforme/src/http/routes-applications.test.ts` | **480** | 20 |

Le dernier est **le seul du sous-projet ⑤**. Il vient de G1, P5 ne le touche
pas, et il est **déjà au-dessus de la porte à 450** que le plan de P5
s'imposait — ce que la clôture des tâches 0 à 11 avait relevé et déclaré.

**Fichiers que P5 crée ou modifie, aucun n'approchant 450 :**
`securite/frein.ts` **268**, `securite/secrets.test.ts` **356**,
`http/adresse-source.ts` **94**, `http/entetes.ts` **45**,
`http/entetes-routeurs.test.ts` **144**, `obs/journal.ts` **76**,
`http/routes-sante.ts` **157**, `http/routes-auth.ts` **397**,
`agents/canal.ts` **417**, `http/serveur.ts` **371**, `config.ts` **176**,
`admin/enroler-agent.ts` **260**, `depot/agent.ts` **128**,
`client/src/adresse-plateforme.ts` **75**, `deploiement/nginx.conf` **300**,
`deploiement/README.md` **258**, `docker-compose.plateforme.yml` **263**,
`plateforme/Dockerfile` **79**.

---

## 6. Ce que P5 n'établit PAS

- **Aucun taux, nulle part.** Deux exécutions par critère au mieux, une par
  rouge.
- **L'inaccessibilité du relais coturn depuis Internet** : ② est une relecture
  de journal, pas une sonde extérieure. Le plan ne la prescrivait pas, et elle
  exigerait une machine hors de ce réseau.
- **L'inaccessibilité effective du service** depuis une autre interface : le
  profil `deploiement` ne publie aucun port pour la plateforme, mais c'est une
  **configuration**, pas une **mesure**. Le legs de P1 reste dû.
- **Le comportement de Postgres sous CHARGE, en concurrence, ou après
  redémarrage.** Le critère ① éprouve le **sous-ensemble SQL** contre l'instance
  déployée ; il ne mesure ni charge, ni concurrence, ni redémarrage. Le legs de
  P1 reste dû, **et il aurait été facile de le croire fermé.**
- **Que le déploiement FONCTIONNE**, au sens du produit : ① est un dialecte
  SQL, ⑤ est un chemin navigateur. Aucune session WebRTC n'est négociée de bout
  en bout, aucune VM n'est pilotée.
- **`GET /vm` et `POST /session` ne sont pas freinées** (voir §12).
- **Le relais de signaling n'est couvert par aucun frein**, et des connexions
  **muettes** ne sont comptées par rien — ni par le frein, qui compte des
  tentatives, ni par nginx, qui ne pose ni `limit_conn` ni `limit_req`.
- **TURNS sur 443 n'est pas livré**, donc **la cible « réseaux restrictifs » du
  cadrage produit n'est PAS couverte**.
- **La réplication n'est pas livrée**, et ⑤ ne la livrera pas : quatre états de
  routage vivent en mémoire, une instance et une seule est la règle. Le remède
  minimal est nommé et non livré — un verrou consultatif en base
  (`pg_advisory_lock`, sans équivalent SQLite) pour échanger la panne muette
  contre une rupture bruyante.
- **Le secret d'enrôlement en clair sur la VM n'est PAS retiré** : il est rendu
  **réparable** (rotation). Le coût reste, la contrepartie est nommée.
- **Le certificat de la recette est auto-signé et sa clé publique est épinglée
  au navigateur.** Aucune chaîne de confiance réelle n'est éprouvée, et HSTS
  n'est vérifié à l'usage par rien.
- **Le proxy écoute sur `127.0.0.1:8443`, pas sur 443** : sur cette machine 443
  est occupé par un tiers. Rien n'est mesuré d'une exposition publique.
- **Le chemin CORS avec une origine tierce autorisée n'est pas exercé par un
  navigateur** : la page et l'API sont sur la MÊME origine — configuration
  nominale —, donc le navigateur n'émet **aucune** requête préalable. Ce chemin
  n'est couvert que par les tests d'hôte.
- **Un seul navigateur** (Chrome 151), sans interface, en `--no-sandbox` imposé
  par l'exécution en root.
- **Aucune migration vers la journalisation structurée** : 18 sites de forme
  libre restent, et leur coût est chiffré.
- **Aucune calibration.** `FENETRE_MS`, `ECHECS_MAX_COMPTE`,
  `ECHECS_MAX_ADRESSE`, `ENTREES_MAX`, `TRAME_MAX_OCTETS`, `PERIODE_SANTE_MS`
  rejoignent la liste que ce dépôt tient depuis le chantier C. **Aucun jugement
  d'usage n'a été porté sur aucune d'elles.**
- **Le cookie `HttpOnly` n'est pas livré**, et l'arbitrage `localStorage` de
  `client/src/jeton.ts` n'a pas été rouvert — il passe d'ANNONCÉ à **DÛ**.

---

## 7. Contrôle explicite qu'aucune preuve ne vit hors de git

`git ls-files docs/superpowers/plans/journaux-plateforme-p5 | wc -l` rend
**36** au moment d'écrire ce document, et le détail est consigné dans
`revue-transverse.log`, §L.

**Aucune preuve de ce sous-bloc ne vit dans un fichier gitignoré.** Les seuls
fichiers non versionnés employés sont ceux que le dépôt **impose** de ne pas
versionner — `.env`, `deploiement/postgres.env`, `deploiement/plateforme.env`,
`deploiement/tls/` —, et **aucun n'est cité comme preuve**.

⚠️ **C'est la contre-mesure que D9 a payée** : son espace de travail
`.superpowers/sdd/` a disparu avec la session, emportant six constats de revue
définitivement perdus. Ici, l'analyse vit dans ce document et les pièces brutes
sous `journaux-plateforme-p5/`.

### Le contrôle des secrets, joué DEUX fois

Il rend un **COMPTE**, jamais une valeur, sur cinq motifs tirés de `.env` et des
secrets engendrés pour la recette :

```
  TOTAL = 0
```

sur **37** pièces — les 36 de `journaux-plateforme-p5/`, tâches 0 à 16
comprises, **plus ce document de résultats lui-même**. Le contrôle a été rejoué
une dernière fois après sa rédaction. Il a été **rejoué après
indexation**, le balayage `securite/secrets.test.ts` lisant `git ls-files` :
4 tests verts sur des pièces désormais suivies.

---

## 8. Le témoin de clôture

**Relancé APRÈS la dernière édition de code** — c'est-à-dire après le commit
`21abf7b` de la revue transverse, qui touche cinq fichiers de `plateforme/src`
et un de `client/src`. Pièce versée : `journaux-plateforme-p5/verify-all-cloture.log`.

```
./scripts/verify-all.sh   ->   SORTIE = 0
                               « Les 10 étapes sont passées. »
```

| Étape | Verdict |
| --- | --- |
| `cargo test --workspace` | ✅ **671** + **83** + 0 |
| `cargo clippy --workspace` | ✅ — sortie 0. ⚠️ **311** lignes `warning` dans la sortie, chiffre **relevé et non jugé** : ce dépôt écrit depuis D3 qu'il ne faut « pas chasser le compte absolu d'avertissements clippy », qui dérive avec la fraîcheur du cache. **Vérifier la nature, jamais le nombre** — et P5 ne touche aucun fichier Rust |
| `client : npm test` | ✅ **304** tests / 33 fichiers |
| `client : npm run typecheck` | ✅ |
| `client : npm run design:verifier` | ✅ |
| `proto : npm test` | ✅ **142** tests / 5 fichiers |
| `proto : npm run typecheck` | ✅ |
| `plateforme : npm run test:sqlite` | ✅ **433** tests / 49 fichiers |
| `plateforme : npm run test:postgres` | ✅ **433** tests / 49 fichiers |
| `plateforme : npm run typecheck` | ✅ |

🔴 **DIX ÉTAPES DANS LE SCRIPT, DIX-HUIT EN-TÊTES À L'ÉCRAN, ET JE DIS LEQUEL JE
COMPTE.** `grep -c '^ *etape "' scripts/verify-all.sh` rend **10** ; le script
ne contient qu'**UNE** occurrence littérale de `==>`, dans le corps de la
fonction. La sortie porte **18** lignes `==>` : les 10 étapes, plus **8** émises
par `client : npm run design:verifier`, qui imprime ses propres sous-titres.
**Le chiffre que je retiens est DIX**, celui des étapes ; le 18 est celui de la
SORTIE, et il dépend d'un outil tiers.

⚠️ **P3 et P4 annonçaient DIX-SEPT en-têtes ; il y en a DIX-HUIT aujourd'hui.**
Le script n'a pas changé — 10 étapes aux deux dates. C'est `verifier-design.mjs`
qui en a gagné un. **Un nombre juste à sa date.**

---

## 9. Pièges neufs — à connaître avant de toucher à ce terrain

- 🔴 **Une image qui SE CONSTRUIT n'est pas une image qui DÉMARRE.** Un contexte
  de construction trop étroit ne se voit ni au `build`, ni aux tests, ni au
  `typecheck`, ni à `compose config`. **La seule mesure est de la lancer.**
- 🔴 **`docker compose config` AVEC un profil recopie le contenu des `env_file`
  en clair** dans le bloc `environment:` qu'il imprime. Jumeau exact du
  `--static-auth-secret` de coturn. **Filtrer par liste blanche, puis prouver
  l'absence par un `grep -c -F -f` qui rend un compte.**
- 🔴 **`${VAR:?}` engage TOUTES les sous-commandes**, profils non demandés
  compris : un `logs` sans les variables rend le message d'interpolation à la
  place du journal. C'est pourquoi le profil `deploiement` emploie `env_file`,
  évalué PAR PROFIL, et non `${VAR:?}`.
- ⚠️ **Un `.dockerignore` à la racine s'applique à TOUS les Dockerfile du
  dépôt.** BuildKit lit `<chemin-du-dockerfile>.dockerignore` en priorité :
  c'est ce qui permet de borner un contexte sans changer le sens d'un build
  qui ne nous appartient pas.
- ⚠️ **`client/dist` doit être lisible par l'utilisateur `nginx`** : le master
  tourne en root, les workers non. `403` sur la page et `500` sur ses
  ressources, sur une pile entièrement saine.
- ⚠️ **Chrome refuse de démarrer en root sans `--no-sandbox`**, et le message
  ne sort que sur son stderr — le pilote voit « CDP injoignable ».
- ⚠️ **`pkill -f <motif>` depuis un shell dont la ligne de commande contient le
  motif tue le shell** (exit 144). Piège déjà documenté par ce dépôt, rejoué
  ici tel quel.
- ⚠️ **`inet_server_port()` rend le port du serveur DANS son conteneur**, jamais
  le port publié. Un discriminant bâti dessus ne discrimine rien.
- ⚠️ **Un `sed` glouton sur une ligne `clé : valeur:port` mange jusqu'au dernier
  `:`** : la première extraction d'adresse coturn rendait « 3478 » pour adresse.
  Passe écartée et rejouée.
- ⚠️ **Une mutation qui casse plus que ce qu'elle vise ne mesure rien.** La
  « ROUGE B bis » rougissait (12 échecs) en cassant le module au-delà du frein ;
  écartée, et **inutile de surcroît** — les deux sites qu'elle ajoutait ne sont
  pas des sites d'application du frein mais de sa **journalisation**.
- ⚠️ **Un `grep -c` après correction ne prouve rien dans les deux sens** : un
  compte non nul n'est pas une correction manquée (le dépôt annote en CITANT la
  phrase réfutée), et un compte nul n'est pas une preuve de complétude. **Les
  places se RELISENT, pas se comptent.**

---

## 10. Ce que le plan prescrivait et qui ne s'est pas passé comme écrit

| Prescription | Ce qui s'est passé |
| --- | --- |
| la rouge de la tâche 14 : « retirer `connect-src wss:` » | **VACUEUSE** — mesurée telle par la tâche 14 elle-même. La rouge jouée est `connect-src 'none'` |
| le discriminant de D15 | **N'EN EST PAS UN** — corrigé par l'identité de grappe |
| E1 : « `agents/registre.ts` n'existe pas » | **FAUX** — il existe |
| « les quatre routeurs » (tâche 10) | il y en a **CINQ** — déjà relevé et fermé par l'implémenteur |
| D12 : « 15 occurrences, 9 fichiers, 5 tests » | **21, 11 et 8** |
| la table de tailles | deux chiffres faux à sa date, cinq croissances sous-estimées d'un facteur 2 à 4 |
| aucun Dockerfile pour le service `plateforme` | **manque du plan**, comblé à la tâche 13 — puis **corrigé par le critère ⑤**, qui a montré qu'il ne démarrait pas |

⚠️ **Six des sept lignes ont la même cause** : le plan a été écrit pendant que
le sous-bloc **G1**, concurrent, changeait le code sous lui. C'est le prix d'un
plan long dans un arbre partagé, et il se paie en constats, pas en défauts.

---

## 11. Ce que P5 lègue

1. 🔴 **`GET /vm` et `POST /session` ne sont pas freinées.** Le legs n°7 de P4
   nommait « /auth/connexion, /agent, ET les DEUX routes neuves » ; il se ferme
   sur les deux premiers et **reste dû** sur les deux dernières. Elles exigent
   un jeton porteur valide, ce qui n'est pas rien — mais l'énoncé du legs ne
   dit pas cela.
2. 🔴 **Le relais de signaling n'est couvert par aucun frein**, et les
   connexions **muettes** ne sont comptées par rien. Le remède naturel est côté
   proxy (`limit_conn`), non livré.
3. 🔴 **Le comportement de Postgres sous charge, en concurrence, après
   redémarrage** — legs de P1, **non levé** par le critère ①, qui éprouve un
   sous-ensemble SQL.
4. 🔴 **L'inaccessibilité effective du service et du relais** : deux
   configurations, aucune mesure. Elles exigeraient une machine extérieure.
5. ⚠️ **`canal.test.ts` « (a) … refusée par le frein » ne peut pas échouer.**
   Titre à resserrer, ou assertion à rendre discriminante.
6. ⚠️ **L'arbitrage `localStorage` de `client/src/jeton.ts` passe d'ANNONCÉ à
   DÛ** : les en-têtes sont livrés, le stockage n'a pas changé, et le remède
   réel — un cookie `HttpOnly` — est hors du périmètre de ⑤.
7. ⚠️ **18 sites de journalisation de forme libre**, chiffrés et non migrés.
8. ⚠️ **`client/src/main.ts:79` affirme que `borner_a_la_taille_max` « n'est
   plus branchée nulle part »** : faux, deux appelants vérifiés par la commande.
   **Non corrigé** — c'est D10 qui l'a rendue fausse, et le fichier est tenu par
   un chantier concurrent.
9. ⚠️ **`plateforme/src/base/pilote-postgres.ts:69`** parle de « ses trois
   fichiers de test » pour P5, qui en a ajouté sept. Ambigu tel qu'écrit.
10. ⚠️ **`plateforme/src/agents/fraicheur.ts:21-26`** se contredit à sept lignes
    d'intervalle. Antérieur à P5.
11. 🔴 **Deux dettes de taille inscrites pour la première fois** :
    `proto/src/plateforme/tests.rs` (**561**) et `proto/ts/plateforme.test.ts`
    (**512**), nées de G1.

---

## 12. 🔴 Ce que le sous-projet ⑤ laisse ouvert, après son DERNIER sous-bloc

**C'est le paragraphe à lire si l'on n'en lit qu'un.**

### Deux promesses du cadrage produit que ⑤ ne tient pas

- 🔴 **« Scalabilité horizontale possible (plus d'état en mémoire) »**
  (`2026-07-27-refonte-produit-design.md:225`). **La phrase est trompeuse.** Une
  base retire de la mémoire l'état **durable** ; elle ne retire pas l'état de
  **routage**, parce qu'un WebSocket vit dans un processus et un seul. **QUATRE**
  états de routage vivent en mémoire du processus — `signaling/appariement.ts`,
  `agents/registre.ts`, `signaling/propriete.ts`, `securite/frein.ts` —, et
  **une instance et une seule** est la règle, écrite dans le proxy (un seul
  amont), dans le fichier de composition (`deploy.replicas: 1`) et dans le
  runbook (invariant ①).
  ⚠️ **Rien dans le code ne l'empêche** : `--scale plateforme=2` donne la panne
  muette. Le remède minimal est nommé et **non livré**.
- 🔴 **« Serveur TURN pour les réseaux restrictifs »**
  (`2026-07-27-refonte-produit-design.md:223`). **TURNS sur 443 n'est pas
  livré**, pour deux raisons écrites dans `docker-compose.coturn.yml` : 443 est
  occupé par un tiers sur la machine de mesure, et il y a un conflit structurel
  sur une adresse unique. **La cible « réseaux restrictifs » n'est donc PAS
  couverte** — un client derrière un pare-feu qui ne laisse passer que 443 ne
  joindra pas ce relais.

### Ce que ⑤ a livré, et qui tient

Le service, sa base sur deux moteurs, l'identité des humains et celle des
agents, le canal plateforme ↔ agent, l'orchestration et l'attribution d'une VM,
le catalogue d'applications, et — par P5 — le frein, l'adresse source, les
en-têtes, `/sante`, `maxPayload`, le balayage de secrets, la rotation, coturn
borné, le profil de déploiement et son runbook.

### Les cinq choses qu'un successeur doit savoir avant de reprendre ⑤

1. **Une instance, et une seule.** Ce n'est pas un réglage de capacité.
2. **Le secret d'enrôlement est en clair sur la VM.** Il est réparable, pas
   protégé.
3. **Le déploiement n'a jamais servi un usager.** Il a servi une recette.
4. **Aucune constante de sécurité n'est calibrée.**
5. **La chaîne `X-Forwarded-For` n'a QU'UN saut.** `PLATEFORME_PROXY_DE_CONFIANCE`
   mal posée fait dégénérer le frein par adresse en frein **global** — et la
   ligne de journal du frein est le seul endroit où cela se voit.
