# Sous-bloc P5 — la production : Postgres déployé, et le durcissement : plan d'implémentation

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** rendre le service déployable sans lui ajouter de dépendance ni de
promesse : freiner ce qui se devine (mots de passe, secrets d'enrôlement),
borner ce qu'un anonyme peut consommer (une trame de 100 Mio est acceptée
aujourd'hui, **mesuré**), restreindre le relais TURN à l'adresse qu'on lui
nomme, déléguer TLS à un proxy inverse **versionné**, et dire — sans le
contourner — tout ce que cette version ne couvre pas.

**Architecture:** cinq étages, dont **quatre sont purs**.

1. **L'adresse du client** (`http/adresse-source.ts`) — derrière un proxy
   inverse, `req.socket.remoteAddress` est l'adresse du **proxy**, la même pour
   tout le monde ; et `X-Forwarded-For` est **forgeable** par le demandeur.
   Une fonction PURE tranche : on ne croit l'en-tête que d'une source
   explicitement déclarée de confiance, et on y lit le **dernier** élément,
   jamais le premier. **PUR.**
2. **Le frein** (`securite/frein.ts`) — une fenêtre glissante par clé, une
   horloge injectée, **et un plafond d'entrées** : un frein dont la table
   grandit sans borne est lui-même le déni de service qu'il prétend fermer.
   **PUR.**
3. **Les en-têtes** (`http/entetes.ts`) — `nosniff` et `no-store` sur toute
   réponse JSON du service ; CSP, HSTS et `frame-ancestors` appartiennent au
   proxy, qui sert le HTML. **PUR.**
4. **La santé** (`http/routes-sante.ts`) — `GET /sante` interroge la base, avec
   un verdict **mis en cache pour une durée bornée** : sans ce cache, un
   anonyme transforme une requête HTTP en requête SQL, à volonté.
5. **Le déploiement** (`docker-compose.plateforme.yml`, `docker-compose.coturn.yml`,
   `deploiement/nginx.conf`) — aucune valeur de secret dans un fichier
   versionné, et **aucun défaut permissif** : une variable manquante fait
   échouer `docker compose` avec son motif, elle ne produit pas une écoute
   universelle silencieuse.

**Tech Stack:** inchangée — Node/TypeScript, Vitest, `ws`, `node:sqlite`, `pg`.
🔴 **P5 N'AJOUTE AUCUNE DÉPENDANCE de production.** Le contrôle
`plateforme/src/base/pilote.test.ts:49` (`expect(deps).toEqual(['pg', 'ws'])`)
reste **inchangé** et doit rester **vert** — c'est le témoin de cette
propriété, et la décision **D13** dit pourquoi aucune bibliothèque de freinage
n'est nécessaire.

🔴 **P5 NE TOUCHE PAS `agent/`, NI `proto/`.** Aucune variante de message,
aucun vecteur, aucune constante de version. `PLATEFORME_VERSION` ne monte pas.

**Spec :** `docs/superpowers/specs/2026-08-19-plateforme-design.md` (commit
`217a765`), §2.7, §3.5, §4 « P5 », §7.2, §8, §9.
**Sous-blocs précédents :** `…-p1-resultats.md`, `…-p2-resultats.md`,
`…-p3-resultats.md`, et le plan `…-p4.md`, **qui font autorité sur l'état du
code** — la spec, elle, a vieilli, et **quatre de ses points portent sur P5
lui-même** (E1, E2, E3, E9).

**Ce plan ne couvre QUE P5**, et P5 est **le dernier sous-bloc du sous-projet
⑤**. Il ne livre donc rien de ce que la spec range hors périmètre v1 (§9) :
ni OIDC, ni inscription publique, ni provisionnement de VM, ni backend
d'hyperviseur, ni scalabilité horizontale, ni révocation immédiate d'un jeton
d'accès, ni hub visuel, ni upload d'installeurs, ni pont fichiers, ni
découverte d'applications. **Ce qu'il laisse ouvert est énuméré au §« Ce que
P5 lègue », et c'est la dernière liste de ⑤ : après elle, plus aucun sous-bloc
ne la reprendra.**

---

## Contraintes globales

### Ce qui a été relevé PAR LA COMMANDE avant d'écrire une ligne

Toutes les valeurs ci-dessous ont été obtenues le **20 août 2026**, sur cette
machine, en lançant réellement la commande. Aucune n'est recopiée d'un document
antérieur. **Ce qui n'a pas été mesuré est marqué comme tel partout ailleurs
dans ce plan.**

| # | Commande | Résultat relevé |
| --- | --- | --- |
| ① | `cd plateforme && npm run test:sqlite` | `Test Files 37 passed (37)`, `Tests 284 passed (284)` |
| ② | `cd plateforme && npm run test:postgres` | `Test Files 37 passed (37)`, `Tests 284 passed (284)` |
| ③ | `cd plateforme && npm run typecheck` | sortie **0** |
| ④ | `cd client && npm test` | `Test Files 24 passed (24)`, `Tests 219 passed (219)` |
| ⑤ | `cd proto && npm test` | `Test Files 5 passed (5)`, `Tests 111 passed (111)` |
| ⑥ | `node --version` | `v24.9.0` |
| ⑦ | `grep -cE '^etape ' scripts/verify-all.sh` | **10** |
| ⑧ | `docker ps` | `guacamole-postgres-plateforme-1 … Up 13 hours (healthy) 127.0.0.1:5433->5432/tcp` |
| ⑨ | `docker ps` \| `ss -lntup` | **aucun conteneur `coturn` ne tourne, et rien n'écoute sur 3478** |
| ⑩ | `docker images` | `nginx:alpine`, `coturn/coturn:4.6`, `postgres:16-alpine` sont **déjà présentes localement** ; **aucune image `caddy` ni `traefik`** |
| ⑪ | `ss -lntp` | **443/tcp est occupé par `envoy`** (pomerium), un tiers étranger à ce dépôt |
| ⑫ | la commande des 500 lignes de `CLAUDE.md` | **deux** fichiers au-dessus du plafond : `agent/src/encode.rs` **1536**, `agent/src/windows_source.rs` **630** |
| ⑬ | `git status --porcelain` | **cinq** lignes, toutes non suivies, toutes du chantier concurrent F1 (`.playwright-mcp/`, `journaux-pont-fichiers/…`). ⚠️ **Ce nombre a vieilli PENDANT la rédaction de ce plan** : relancé une heure plus tard, il en rend **seize** — F1 versait ses journaux. **Le nombre est sans importance ici ; ce qui compte est qu'il y en a, et que `git clean` les emporterait** (voir la tâche 11) |
| ⑭ | `git log --oneline -1` | `0289a27 plateforme(p4): npm run admin:attribuer, et --detacher` |
| ⑮ | `grep maxPayload plateforme/node_modules/ws/lib/websocket-server.js` | `:74` → `maxPayload: 100 * 1024 * 1024` — **100 Mio par défaut** |
| ⑯ | `node -e` sur `http.createServer()` | `requestTimeout 300000`, `headersTimeout 60000`, `keepAliveTimeout 5000`, `maxHeaderSize 16384` |
| ⑰ | `which google-chrome` | `/usr/bin/google-chrome` — **un navigateur réel est disponible** |

⚠️ **Les relevés ① à ⑤ sont les références de non-régression de P5**, **à la
date d'aujourd'hui et pas à celle du dispatch** : P4 n'est pas clos (ses tâches
13 à 19 restent, dont deux qui touchent `client/src/`), et il les fera monter.
**La tâche 0 de ce plan les remesure**, et c'est elle qui fait foi. Toute tâche
qui les fait baisser a cassé quelque chose ; toute tâche qui les fait monter
doit dire **de combien et pourquoi**, et l'annoncer **avant** de lire le
compte — c'est ainsi que D10 a rattrapé un test supprimé par un `Write`
d'écrasement.

🔴 **`cargo test --workspace` N'A PAS ÉTÉ LANCÉ pour établir cette référence**,
et c'est déclaré plutôt que dissimulé : le chantier F1 (pont fichiers / ProjFS)
travaille dans `agent/` et **emploie la VM Windows** en ce moment même. **P5 ne
touche aucun fichier Rust** : aucune de ses tâches n'a donc à relever cette
référence, et la tâche de recette ne revendiquera **aucun** chiffre `cargo`.

⚠️ **`verify-all.sh` : « dix » et « dix-sept » sont vrais de deux choses
différentes, et je dis lequel j'ai compté.** Le relevé ⑦ est le nombre
d'**appels de la fonction `etape` dans le script** : **dix**, mesuré par moi ce
jour. Le nombre d'**en-têtes `==>` à l'écran** est **dix-sept** — sept d'entre
eux venant de l'intérieur de l'étape `client : npm run design:verifier` —, et
**je ne l'ai PAS re-relevé** : c'est le chiffre mesuré par la revue transverse
de P3 sur une exécution complète. La tâche de recette relancera le script et
**dira lequel des deux elle compte**.

🔴 **P5 N'AJOUTE AUCUNE ÉTAPE à `verify-all.sh`, et c'est une décision, pas un
oubli** : tous ses contrôles neufs sont des tests de `plateforme/` ou de
`client/`, donc déjà couverts par des étapes existantes. Le compte doit rester
**10**. Une tâche qui le ferait monter doit s'expliquer.

### La règle des 500 lignes : marges relevées, et la porte à 450

**Relevé par la commande le 20 août 2026** (`{ git ls-files; git ls-files
--others --exclude-standard; } | grep -vE 'node_modules|package-lock|Cargo.lock|/dist/|testdata/|^docs/|^CLAUDE.md' | xargs wc -l | sort -rn`),
pour les seuls fichiers que P5 modifie ou qui bordent son périmètre :

| Fichier | Lignes | Marge | Ce que P5 y ajoute |
| --- | --- | --- | --- |
| `plateforme/src/signaling/relais.ts` | **327** | 173 | rien (le `maxPayload` est posé chez l'appelant) |
| `plateforme/src/http/routes-vm.test.ts` | **323** | 177 | quelques assertions d'en-têtes (~20) |
| `plateforme/src/http/routes-session.test.ts` | **289** | 211 | idem (~20) |
| `plateforme/src/http/serveur.test.ts` | **241** | 259 | le test de la trame trop grande, celui de `/sante` (~60) |
| `plateforme/src/http/routes-auth.ts` | **241** | 259 | le câblage du frein (~45) |
| `plateforme/src/admin/attribuer-vm.ts` | **224** | 276 | rien |
| `plateforme/src/http/routes-auth.test.ts` | **215** | 285 | les tests du frein (~90) |
| `plateforme/src/http/routes-vm.ts` | **212** | 288 | les en-têtes de sécurité (~5) |
| `plateforme/src/http/serveur.ts` | **207** | 293 | `maxPayload`, le frein construit, `/sante` chaîné (~35) |
| `plateforme/src/agents/canal.ts` | **198** | 302 | le frein d'enrôlement (~30) |
| `plateforme/src/http/routes-session.ts` | **157** | 343 | les en-têtes (~5) |
| `plateforme/src/admin/enroler-agent.ts` | **162** | 338 | `--roter` (~45) |
| `plateforme/src/config.ts` | **121** | 379 | `PLATEFORME_PROXY_DE_CONFIANCE` (~30) |
| `plateforme/src/depot/agent.ts` | **96** | 404 | `remplacerEmpreinte` (~15) |
| `plateforme/src/http/cors.ts` | **50** | 450 | **rien** — les en-têtes de sécurité vont dans un module SÉPARÉ (D7) |
| `client/src/connexion.ts` | **75** | 425 | l'appel au module d'adresse (~5) |
| `client/src/shell-page.ts` | **96** | 404 | idem (~3) |
| `client/src/main.ts` | (non relevé — hors périmètre de la porte, voir ci-dessous) | | idem (~3) |

🔴 **AUCUNE EXTRACTION N'EST REQUISE PAR P5, et ce n'est pas une omission :
c'est un relevé.** Le plus gros fichier du périmètre est `relais.ts` à **327**,
que P5 ne touche pas ; le plus gros qu'il touche est `routes-auth.ts` à
**241**, qui gagne ~45 lignes. **Aucun fichier du périmètre n'approche 450.**
Le dépôt n'a jamais eu beaucoup de sous-blocs où la question ne se posait pas ;
celui-ci en est un — comme P4 —, et le dire est plus utile que d'inventer une
extraction pour se conformer à une habitude.

⚠️ **Porte chiffrée, à jouer après CHAQUE tâche qui touche un fichier
existant** : relancer `wc -l` sur les fichiers de la table. Si l'un d'eux
dépasse **450**, extraire **avant** de continuer, jamais après. Une compression
de commentaire pour repasser sous la ligne est **interdite** — `CLAUDE.md` le
dit nommément, et D9 l'a payée deux fois dans le même sous-bloc.

⚠️ **À REMESURER, et non recopié** : les marges serrées du dépôt hors périmètre
P5, **relevées par la commande le 20 août 2026** — `agent/src/encode/arret.rs`
**500** (marge 0), `client/verify-webrtc.mjs` **494**, `agent/src/capture.rs`
**492**, `agent/src/demarrage.rs` **491**, `agent/src/pont/projfs/rappels.rs`
**489** (fichier du chantier concurrent F1). ⚠️ **`agent/src/transport.rs` vaut
440, et non 495** : le document de résultats de P3 le donnait à 495 (« marge
5 ») et le plan de P4 le **confirmait** à 495 le 20 août 2026 au matin ; **la
commande, relancée pour ce plan le même jour, rend 440** — un chantier l'a
raccourci entre les deux, et le chiffre de P4 est déjà périmé. C'est le
naufrage du « 487 » sous sa forme la plus douce : un nombre juste à sa date,
faux le lendemain. **Ne pas recopier ce 440 non plus : le remesurer.**
**P5 ne touche aucun de ces fichiers.**

### Les règles de méthode, héritées et non négociables

- **Jamais `git add -A`** : nommer les fichiers, un par un. Un `git add -A` a
  déjà emporté le travail concurrent d'une autre tâche dans un commit qui ne
  compilait pas. 🔴 **Et un autre agent travaille dans le même arbre git.**
- 🔴 **Jamais `git commit --amend`**, et jamais de commit sans pathspec
  explicite : `git commit` valide TOUT l'index. Si le message porte des accents
  graves ou des `$`, employer `git commit -F <fichier>`.
- 🔴 **`git checkout` NE RESTAURE PAS UN FICHIER NON SUIVI — piège neuf,
  mesuré le 20 août 2026, et deux mutations y ont survécu dans ce dépôt le jour
  même, dont un `setTimeout(2000)` resté dans une route de production.**
  Conséquence, non négociable pour **toute** rouge de ce plan :
  1. **commiter avant de muter** — la restauration d'un fichier suivi est
     décidable, celle d'un fichier non suivi ne l'est pas ;
  2. relever l'empreinte AVANT (`sha256sum` des fichiers touchés), la
     reprendre APRÈS restauration, et **verser les deux dans le journal de la
     rouge** ;
  3. clore chaque rouge par `git status --porcelain`, dont la sortie doit être
     **identique** à celle d'avant la mutation.
  ⚠️ **Une rouge qui crée un fichier NEUF est le cas dangereux** : `git
  checkout` ne le voit pas, `git clean` l'emporterait avec le travail
  concurrent non suivi de F1. **Supprimer le fichier neuf NOMMÉMENT**, jamais
  par `git clean`.
- **Un contrôle qu'on n'a jamais vu rouge n'est pas un contrôle.** Chaque test
  neuf est exécuté **avant** l'implémentation et **vu échouer**. Chaque tâche
  nomme la mutation qui le rend rouge, **et ce plan affirme pour chacune que
  l'état rouge est ATTEIGNABLE** — voir le paragraphe suivant.
  🔴 **ET CHAQUE ASSERTION D'UN CRITÈRE À PLUSIEURS ASSERTIONS EXIGE SA PROPRE
  ROUGE, ET SON PROPRE `it()`.** C'est la leçon que P2 a payée : sa rouge ①A
  devait faire tomber « les DEUX assertions » du critère ① ; elle n'en faisait
  tomber qu'une, `expect` interrompant le test à la première.
- 🔴 **Ce plan est lui-même une source de contrôles vacueux.** D10 en a attrapé
  quatre, dont **trois écrits par son propre plan** ; le 19 août 2026 un plan
  de ce dépôt a prescrit une rouge **impossible** ; le 20 août 2026 un autre
  s'est contredit entre deux de ses propres tableaux, et un troisième portait
  un contrôle dont le chemin faux traduisait un fichier absent en succès.
  **Le doute porte sur ce document.** Chaque tâche ci-dessous porte une ligne
  « **la rouge est atteignable parce que…** », et si un implémenteur ne
  parvient pas à l'atteindre, **c'est le plan qui a tort**, pas la tâche : le
  signaler, ne pas contourner.
- **Ne jamais fabriquer une sortie de commande.** D10 a attrapé deux pièces
  fabriquées, dont une inscrite dans `CLAUDE.md` **à l'intérieur d'une
  correction qui dénonçait une affirmation non étayée**. Le mécanisme nommé par
  son auteur : *réutiliser la sortie d'une commande antérieure pour répondre à
  la question d'une AUTRE, sans la relancer.*
- **Un saut est un échec.** `npm run test:postgres` ne se saute pas quand
  l'instance manque : il échoue. Aucun `skipIf`, aucun `runIf`, aucun
  `it.todo` — la suite de `plateforme/` n'en contient aucun aujourd'hui, et P5
  n'en introduit pas. ⚠️ **Y compris le contrôle navigateur** : s'il ne peut
  pas tourner, la tâche **échoue et le dit**, elle ne se saute pas.
- **Les valeurs d'essai sont RÉALISTES, jamais commodes.** Tout horodatage
  d'essai est de la magnitude d'une époque en millisecondes —
  `plateforme/src/base/harnais.ts:32` porte `INSTANT_MIGRATION =
  1_700_000_000_000`, et c'est la référence. **Toute adresse IP d'essai est
  une adresse plausible** (`203.0.113.7`, `198.51.100.4`, `172.18.0.5`), jamais
  `a`/`b`/`c` : un frein qui normalise les adresses ne peut pas être éprouvé
  sur des étiquettes.
- **L'horloge est TOUJOURS un paramètre**, jamais `Date.now()` lu dans un
  module. Précédents : `agents/fraicheur.ts`, `identite/jeton.ts`,
  `signaling/ice.ts`, `depot/session.ts`. Le frein et le cache de `/sante` la
  reçoivent tous deux.
- **Aucun taux ne sera revendiqué.** Chaque énoncé porte son nombre
  d'exécutions. **Deux exécutions par critère de recette, pas une**, et la
  convention de P2/P3/P4 est reconduite : **exécution 1 = `sqlite`, exécution
  2 = `postgres`**, déclaré dans l'en-tête de chaque journal — sauf pour le
  critère ① dont les deux exécutions sont **toutes deux** sur Postgres
  déployé, ce que sa tâche dit explicitement.
- **Toute preuve d'une affirmation portée dans `CLAUDE.md` est versée dans
  git.** D10 a établi par la commande que l'espace de travail de D9
  (`.superpowers/sdd/`, gitignoré) **a disparu**, emportant six constats de
  revue définitivement perdus. **Aucun rapport de tâche ne sera cité comme
  preuve** ; tout ce que le document de résultats affirme est adossé à un
  fichier de `journaux-plateforme-p5/`, à un commit, ou à une commande
  relancée à la clôture.
- 🔴 **JAMAIS UN SECRET EN CLAIR DANS UNE PIÈCE VERSÉE, et ce n'est pas
  théorique ici — c'est mesuré.** `docker compose -f docker-compose.coturn.yml
  config` **imprime la valeur de `TURN_SECRET` en clair** (relevé le 20 août
  2026 ; la valeur n'est pas recopiée dans ce plan). La recette de P5 emploie
  cette commande **et** le journal de coturn : les deux doivent être filtrés
  **par liste blanche de lignes** avant d'être versés, jamais par liste noire,
  et la tâche 17 impose un contrôle qui le prouve. Voir **D10**.
- **Relire chaque citation `fichier:ligne` APRÈS l'avoir écrite**, et **une
  seconde fois APRÈS avoir exécuté ce qui la déplace** : c'est la leçon de P3,
  dont une citation était exacte à l'écriture du plan et fausse à la fin de son
  exécution, parce que le plan lui-même prescrivait de déplacer la ligne citée.
  Les citations de ce plan ont été relevées par `grep -n` et relues une par
  une ; celles que P5 déplacera sont signalées dans leur tâche.

### Périmètre concurrent — à lire AVANT de démarrer quoi que ce soit

**Trois chantiers vivent dans le même arbre.**

**① P4 lui-même n'est PAS clos.** Relevé le 20 août 2026 : `git log` s'arrête à
`0289a27 plateforme(p4): npm run admin:attribuer, et --detacher`, qui est la
**tâche 12** de son plan. Ses tâches **13 à 19** restent — dont **13** et **14**
qui touchent `client/src/prefixe.ts` et `client/src/connexion.ts`, **18** qui
écrit `CLAUDE.md`, et **17** qui est sa revue transverse.

🔴 **P5 NE DÉMARRE PAS AVANT LA CLÔTURE DE P4** (son document de résultats
existe, et `git status --porcelain` ne porte plus de fichier `plateforme/` ni
`client/` modifié). Le sous-bloc en dépend explicitement (spec §4). Sa
**tâche 0** vérifie ce fait et remesure les références.

**② F1 — pont fichiers / ProjFS.** Il travaille dans `agent/src/pont/` et **il
emploie la VM Windows**. ⛔ **P5 ne touche aucun fichier de `agent/`, et
n'emploie PAS la VM** : aucun de ses critères n'en a besoin, et c'est une
propriété héritée de tout ⑤ (spec §4).

**③ G1 — gestion d'apps.** Son plan existe
(`docs/superpowers/plans/2026-08-19-gestion-apps-g1.md`) et n'a pas encore été
exécuté (aucun commit `(g1)`). Il collisionne avec `plateforme/src/http/` et
avec `proto/`. **Avant de démarrer une famille qui touche `http/`, relancer
`git log --oneline -5 -- plateforme/src/http/` et `git status --porcelain`, et
ne pas démarrer si un fichier du périmètre y figure comme modifié.**

⛔ **Interdits de MODIFICATION pour toute tâche de ce plan** : `agent/`,
`proto/`, `src/`, `web/`, `index.js`, `docker-compose.yml` (non versionné),
`docs/superpowers/specs/`, `docs/superpowers/plans/2026-08-19-gestion-apps-*`,
`docs/superpowers/plans/2026-08-19-pont-fichiers-*`.

⚠️ **`scripts/` est autorisé en LECTURE et en EXÉCUTION, et interdit en
ÉCRITURE sauf pour la seule tâche qui en aurait besoin — or aucune n'en a
besoin** (P5 n'ajoute aucune étape à `verify-all.sh`, voir plus haut). C'est
aussi ce qui rend la décision **D11** inévitable : `scripts/run-agent.sh` est
le fichier qui écrit le secret d'enrôlement en clair sur la VM, et P5 n'a le
droit ni d'y toucher, ni d'atteindre la VM.

⚠️ **`CLAUDE.md` n'est écrit que par la tâche 21**, jamais en cours de route,
et **jamais en même temps que la tâche 18 de P4**.

---

## Décisions tranchées — les quinze questions que ce plan devait trancher

### D1 — Le frein : deux clés, en mémoire, un REFUS immédiat, jamais un délai

**Question :** par IP, par compte, ou les deux ? Où vit l'état ?

**Tranché : les deux clés, indépendamment, et l'état vit EN MÉMOIRE.**

- **par compte** (le courriel présenté, normalisé en minuscules) : c'est le
  seul frein qui ferme la force brute **ciblée** — un attaquant disposant de
  mille adresses source n'en est pas ralenti autrement ;
- **par adresse source** (celle que `http/adresse-source.ts` décide, D3) :
  c'est le seul frein qui ferme le **balayage de comptes** — mille courriels
  essayés une fois chacun ne consomment aucun budget de compte.

**L'état est en mémoire**, pas en base, pour trois raisons **dont la première
est décisive** :

1. **un frein en base fait ÉCRIRE l'attaquant.** Chaque tentative devient un
   `INSERT`/`UPDATE` : le frein devient un amplificateur de charge, exactement
   ce qu'il existe pour empêcher. C'est la même famille de raisonnement que
   `signaling/propriete.ts`, qui explique déjà pourquoi la décision
   d'appartenance ne consulte pas la base ;
2. le gestionnaire `message` de `ws` est **synchrone** (`signaling/relais.ts`),
   et une promesse rejetée y abat le processus — le frein de `/agent` doit donc
   pouvoir répondre sans `await` ;
3. **P5 ne livre pas la scalabilité horizontale** (spec §9), et un WebSocket
   vit dans un processus et un seul (spec §3.1) : il n'y a pas de seconde
   instance avec qui partager cet état. **Voir D14 pour ce que cela coûte, et
   ce que le déploiement doit donc déclarer.**

**Le coût est nommé et NON corrigé : le frein ne survit pas à un redémarrage du
service.** Un attaquant qui parviendrait à le faire redémarrer remettrait les
compteurs à zéro — mais s'il le peut, il a déjà mieux à faire. Ce qui reste
vrai sans réserve : **le frein d'une instance ne protège que cette instance.**

🔴 **Le frein REFUSE, il ne RETARDE PAS.** Un frein par temporisation garde le
socket ouvert pendant l'attente : c'est un second déni de service offert à
l'attaquant, et le dépôt vient d'en payer une variante (un `setTimeout(2000)`
resté dans une route de production le 20 août 2026). La réponse est un **429
immédiat**, avec `Retry-After` en secondes. C'est aussi la propriété que le
critère ③ de P4 exige déjà sur une autre route — « pas d'attente, pas de délai
d'expiration » —, et l'homogénéité vaut mieux qu'une seconde convention.

**Constantes, TOUTES NON CALIBRÉES et déclarées comme telles** — elles
rejoignent `BPP_MIN`, `FACTEUR_FOCUS`, `PART_DORMANTE_BPS`, `HYSTERESIS`,
`TAILLE_MAX_SORTIE`, `SEUIL_INJOIGNABLE_MS`, `DUREE_JETON_ACCES_MS` et les
86 400 s de TURN :

| Constante | Valeur | Ce qu'elle borne |
| --- | --- | --- |
| `FENETRE_MS` | `15 * 60_000` | la mémoire d'un échec |
| `ECHECS_MAX_COMPTE` | `5` | les essais contre UN compte |
| `ECHECS_MAX_ADRESSE` | `50` | les essais depuis UNE adresse |
| `ENTREES_MAX` | `10_000` | la taille de la table (D2) |

⚠️ **Arbitrage assumé, et il faut l'écrire parce qu'il se retourne contre
l'utilisateur légitime** : un attaquant peut brûler le budget d'un compte
qu'il vise et **en refuser l'accès à son propriétaire pendant la fenêtre**.
C'est l'arbitrage classique du verrouillage de compte. Il est accepté parce
que (a) la fenêtre est courte, (b) le déblocage par courriel exigerait un SMTP,
que la spec §9 range hors périmètre v1, et (c) l'alternative — ne freiner que
par adresse — laisse passer la force brute ciblée, qui est la menace nommée.
**Un échec réussi remet le compteur du COMPTE à zéro**, jamais celui de
l'adresse : sinon une connexion légitime depuis une adresse d'attaque
blanchirait l'attaquant.

⚠️ **`/auth/rafraichir` n'est freiné QUE par adresse**, pas par compte : le
demandeur n'y présente aucun courriel, seulement un jeton opaque, et prendre
ce jeton pour clé reviendrait à indexer une table sur un secret.

### D2 — Le frein est BORNÉ en mémoire, et c'est la moitié qui compte

Un frein qui retient une entrée par clé vue est un **vecteur d'épuisement
mémoire** : un attaquant essaie un million de courriels distincts, chacun une
seule fois, et la table grandit sans jamais qu'aucun budget ne soit dépassé.
**Le frein devient l'attaque.**

**Tranché :** `securite/frein.ts` porte `ENTREES_MAX`. À l'insertion, si la
table est pleine, il **purge d'abord toutes les entrées expirées** ; si elle
est encore pleine, il **évince la plus ancienne** (l'entrée dont la fenêtre se
referme le plus tôt). Une éviction n'est **jamais** silencieuse pour
l'exploitant : elle incrémente un compteur exposé par le module, que la trace
lit.

⚠️ **Ce que l'éviction coûte, et qui n'est pas rattrapable ici** : sous
saturation, un attaquant peut faire évincer l'entrée d'un compte qu'il vise
pour lui rendre son budget. C'est le prix de la borne, et la borne vaut mieux
que la mémoire. **`ENTREES_MAX = 10_000` n'est pas calibrée** ; l'ordre de
grandeur du coût est de quelques centaines de kilo-octets, **calculé et non
mesuré**.

**C'est la propriété la plus facile à oublier et la plus facile à éprouver** :
insérer `ENTREES_MAX + 1` clés distinctes et vérifier que la taille ne dépasse
pas la borne.

### D3 — `X-Forwarded-For` : on ne croit rien par défaut, et on lit le DERNIER élément

**Question :** derrière un proxy inverse, toutes les requêtes viennent de la
même adresse. Croire l'en-tête aveuglément est une usurpation d'identité ; ne
pas le lire casse le frein par IP.

**Tranché :** une fonction PURE, `adresseSource(remote, entete, confiance)`.

1. **si `remote` n'est pas dans l'ensemble de confiance, l'en-tête est
   IGNORÉ**, quel qu'il soit. C'est le défaut, et l'ensemble de confiance est
   **vide** quand `PLATEFORME_PROXY_DE_CONFIANCE` est absente — même doctrine
   que `PLATEFORME_ORIGINE_CLIENT`, dont l'absence produit un refus et non une
   permission ;
2. **si `remote` est de confiance, on prend le DERNIER élément non vide de
   `X-Forwarded-For`.** 🔴 **Prendre le PREMIER est la faute classique**, et
   c'est exactement ce qui rend l'adresse forgeable : `nginx` avec
   `$proxy_add_x_forwarded_for` **ajoute** l'adresse de son pair à ce que le
   client a envoyé, donc un client qui envoie `X-Forwarded-For: 203.0.113.7`
   produit `203.0.113.7, <sa vraie adresse>`. Le dernier élément est le seul
   que le proxy ait écrit lui-même ;
3. **si l'en-tête est absent ou vide**, on rend `remote` ;
4. **les adresses IPv4 encapsulées en IPv6 sont normalisées** :
   `::ffff:203.0.113.7` devient `203.0.113.7`. Sans quoi le même client compte
   deux fois selon la pile employée, et son budget double.

⚠️ **Cette règle suppose EXACTEMENT UN proxy de confiance en tête de chaîne.**
Avec deux proxies enchaînés, le dernier élément est l'adresse du premier proxy,
pas celle du client. **P5 ne livre pas la chaîne à N sauts** : la configuration
versionnée en pose **un seul**, `deploiement/nginx.conf` le documente, et le
module le dit dans son en-tête. C'est une limite déclarée, pas une omission.

⚠️ **Le mode de défaillance de l'oubli est nommé** : si l'opérateur pose un
proxy sans déclarer sa confiance, **toutes** les requêtes portent l'adresse du
proxy, le frein par adresse dégénère en frein **global**, et le service se
refuse à lui-même au 51ᵉ échec. Le remède **n'est pas** de croire l'en-tête par
défaut — ce serait échanger une panne bruyante contre un contournement
silencieux —, c'est que **la trace du frein nomme l'adresse retenue** : un
exploitant qui lit `frein adresse=172.18.0.5` sur toutes les lignes reconnaît
l'adresse de son proxy. Le runbook le dit aussi.

### D4 — Le frein sur `/agent` : par VM demandée, et par adresse

Le canal `/agent` ferme déjà le socket sur un refus d'enrôlement
(`agents/canal.ts`, `MOTIFS_FERMANTS`), et son commentaire annonce en toutes
lettres que « fermer ne l'empêche pas de se reconnecter — cela lui en fait
payer le coût, et rend le nombre de tentatives comptable à l'étage au-dessus le
jour où on voudra le brider ». **P5 est ce jour-là.**

**Tranché :** le **même** module `securite/frein.ts`, deux clés :
`agent:<vm_id>` et `adr:<adresse>` — une VM ciblée et une source. Budgets
identiques à ceux de D1, mêmes constantes, **pas de seconde table** : deux
freins distincts divergeraient le jour où l'un serait durci.

🔴 **Le frein est consulté AVANT `verifierEnrolement`, donc avant la
dérivation `scrypt`.** C'est le point : `scrypt` est à mémoire dure et coûte
délibérément cher, et un attaquant qui le déclenche à volonté épuise le
service sans jamais deviner un secret. **Un frein posté après la vérification
ne protège rien** — il compte des échecs qu'il a déjà payés.

⚠️ **Le refus freiné réemploie le motif `enrolement` et RIEN D'AUTRE.** Un
motif `frein` distinct rendrait à l'attaquant l'information « cette VM existe
et je l'ai fait déclencher » : c'est l'oracle d'énumération que
`agents/enrolement.ts` documente sur trois paragraphes. **Le journal, lui,
distingue les deux** — même partage que `garde.ts` (`message` sur le fil,
`journal` chez nous).

### D5 — Le déni de service en une trame : `maxPayload`, mesuré à 100 Mio

**Relevé** : `plateforme/node_modules/ws/lib/websocket-server.js:74` →
`maxPayload: 100 * 1024 * 1024`. Les deux serveurs du service
(`http/serveur.ts:119` et `:126`) sont construits **sans** cette option ; celui
de `signaling/relais.ts:150` aussi, sur son chemin de test.

Conséquence, et elle ferme le legs **E15 de P3** : **un pair anonyme peut
pousser une trame de 100 Mio avant toute authentification**, parce que le
contrôle de forme court avant la garde — `signaling/relais.ts:84-86` le dit
lui-même : « `isJsonObject` est appelé une trentaine de lignes avant
`garde.verifier` ». `JSON.parse` sur 100 Mio est une allocation, puis un pic
CPU, par socket et par trame.

**Tranché :** `maxPayload: TRAME_MAX_OCTETS` sur **les deux** serveurs, valeur
**256 Kio**. `ws` ferme alors le socket avec le code 1009 sans jamais
transmettre la trame au gestionnaire.

⚠️ **La valeur n'est PAS calibrée, et son plancher est raisonné, pas mesuré :**
le plus gros message légitime est une offre ou une réponse SDP, dont les
sessions de ce dépôt tiennent en quelques kilo-octets ; 256 Kio laisse **deux
ordres de grandeur** de marge. **Aucune SDP réelle n'a été mesurée pour poser
ce chiffre**, et le dire vaut mieux que de laisser croire à un calibrage.

⚠️ **Ce que `maxPayload` NE ferme PAS, et qu'aucune tâche de P5 ne ferme** :
un pair peut toujours ouvrir **beaucoup de connexions**. Le frein de D4 en
compte les tentatives d'enrôlement ; il ne compte pas les sockets ouverts et
muets. **Legs déclaré.**

⚠️ **Les délais HTTP de Node sont relevés et jugés suffisants pour v1**, sans
être mesurés sous attaque : `requestTimeout 300000`, `headersTimeout 60000`,
`keepAliveTimeout 5000`, `maxHeaderSize 16384` (relevé ⑯). `CORPS_MAX_OCTETS`
existe déjà à 4 Kio (`http/routes-auth.ts:44`). **P5 n'y touche pas.**

### D6 — `/sante` : la base, un cache borné, et rien qui se divulgue

**Tranché :** `GET /sante` rend **200** `{"etat":"ok"}` si la base répond, et
**503** `{"etat":"degrade"}` sinon. Rien d'autre : **ni version, ni compte de
sessions, ni URL de base, ni nom de moteur**. Une page de santé bavarde est un
inventaire offert à un anonyme, et elle est **par construction** la seule route
non authentifiée qui reste après P2.

🔴 **Le verdict est mis en CACHE pour `PERIODE_SANTE_MS = 1000`**, horloge
injectée. Sans ce cache, `/sante` traduit une requête HTTP anonyme en requête
SQL, à volonté : c'est une amplification, sur la route même qu'un équilibreur
de charge appelle en boucle. Le cache est **le point de cette route**, pas un
raffinement.

⚠️ **`/sante` n'est PAS freiné**, et c'est délibéré : une sonde d'équilibreur
freinée déclarerait le service mort. Le cache est ce qui la rend sûre sans
frein — c'est pourquoi les deux décisions vivent dans le même paragraphe.

⚠️ **La sonde de santé n'est pas une sonde de correction** : elle dit que la
base répond, jamais que le service sert. Écrit dans le module.

### D7 — En-têtes de sécurité : la plateforme en pose DEUX, le proxy pose le reste

**Tranché**, et le partage suit ce que chacun sert :

| Qui | En-tête | Pourquoi lui |
| --- | --- | --- |
| **plateforme** (toute réponse JSON) | `X-Content-Type-Options: nosniff` | elle seule connaît son type de contenu |
| **plateforme** (toute réponse JSON) | `Cache-Control: no-store` | 🔴 les réponses de `/auth/*` **portent des jetons** : un cache intermédiaire ou un disque de navigateur les retiendrait |
| **proxy** (le HTML) | `Content-Security-Policy` | elle porte sur le document, que la plateforme ne sert pas |
| **proxy** | `Strict-Transport-Security` | c'est lui qui termine TLS |
| **proxy** | `Referrer-Policy`, `X-Frame-Options` / `frame-ancestors` | idem |

Le module `http/entetes.ts` est **PUR** et rend un objet ; les quatre routeurs
l'étalent dans leur `writeHead`. Il est **séparé de `cors.ts`** : `cors.ts`
rend `undefined` quand l'origine n'est pas autorisée, et les en-têtes de
sécurité, eux, sont **inconditionnels**. Les fusionner ferait dépendre la
sécurité d'une configuration CORS facultative.

### D8 — Le proxy inverse est NGINX, et le WebSocket sur `/` se route par `Upgrade`

**Question :** lequel ?

**Tranché : nginx**, pour trois raisons dont deux sont des relevés :

1. **`nginx:alpine` est déjà présente sur cette machine** (relevé ⑩), quand
   `caddy` et `traefik` ne le sont pas : la recette n'a rien à télécharger, et
   une recette qui dépend du réseau échoue pour une raison étrangère à ce
   qu'elle mesure ;
2. c'est le proxy que la plupart des exploitants savent déjà lire ;
3. son idiome `map $http_upgrade` est **exactement** ce dont ce service a
   besoin — voir ci-dessous.

🔴 **LE PIÈGE DE DÉPLOIEMENT, ET IL EST STRUCTUREL : LA PLATEFORME MONTE SON
WEBSOCKET SUR `/`, LA RACINE.** `http/serveur.ts` route la montée sur `/` et
sur `/agent` ; le client statique veut la **même** racine pour sa page. Sur une
origine unique, `GET /` doit donc aller **au fichier** et `GET /` portant
`Upgrade: websocket` doit aller **à la plateforme**. nginx le fait par
`map $http_upgrade $vers_plateforme` et un `if`/`try_files` distinct — c'est
écrit et commenté dans `deploiement/nginx.conf`.

⚠️ **L'alternative propre serait de déplacer le chemin du relais** (de `/` vers
`/signal`). **Elle est écartée, et pour une raison de périmètre, pas de
goût** : `agent/src/signaling.rs` vise la racine, et P5 n'a pas le droit de
toucher `agent/` — le changement casserait tout agent non recompilé, ce que
la spec §10 s'engage à ne pas faire hors de P3. **Legs déclaré** : le jour où
`agent/` sera rouvert, déplacer le chemin simplifierait ce fichier de
configuration.

### D9 — TLS jamais dans le processus, et TURNS sur 443 N'EST PAS LIVRÉ

**TLS** : la plateforme ne termine jamais TLS (spec §9). Elle écoute en clair
sur `PLATEFORME_HOTE`, qui doit être une adresse **que seul le proxy atteint** —
`127.0.0.1` en mono-hôte, l'adresse du réseau docker interne sinon. Le proxy
termine TLS et parle en clair au service. **Le runbook le dit, et
`PLATEFORME_HOTE` n'a toujours aucun défaut** : c'est la défense de premier
rang que `config.ts` documente déjà.

**TURNS : NON LIVRÉ, et le constat est écrit — c'est exactement ce que la spec
§2.7 et §8 autorisent (« si P5 ne livre pas TURN over TLS sur 443, le constat
sera écrit, pas contourné »).** Deux raisons, la première mesurée :

1. **sur cette machine, 443/tcp est déjà occupé par un tiers** — `envoy`
   (pomerium), relevé ⑪ —, étranger à ce dépôt et que P5 n'a pas à déplacer ;
2. **et le conflit n'est pas local : sur une seule adresse IP, TURNS/443 et le
   proxy HTTPS/443 se disputent le même port.** Les faire coexister exige soit
   une seconde adresse, soit un multiplexage par ALPN (`stun.turn`, RFC 7443)
   dans un étage `stream` — technique réelle, **que ce dépôt n'a jamais
   éprouvée** et qu'un sous-bloc de clôture n'est pas l'endroit d'inventer.

**Et TURNS sur 5349 n'est pas livré non plus** : il exigerait un certificat
valide pour un nom public, donc ACME, donc le port 80 ou un défi DNS —
c'est-à-dire l'infrastructure de nommage que ce dépôt n'a pas. **Un certificat
auto-signé serait refusé par les navigateurs**, et le livrer donnerait
l'apparence d'une couverture sans la couverture.

🔴 **La conséquence produit, en toutes lettres et à porter dans `CLAUDE.md` :
la cible « réseaux restrictifs » du cadrage §5 ⑤ N'EST PAS COUVERTE.** Le
relais reste en clair sur 3478, ce que ces réseaux filtrent précisément.
`--no-tls --no-dtls` reste dans le fichier (`docker-compose.coturn.yml:36-37`),
**assumé et commenté**, plutôt que remplacé par un TLS qui ne servirait
personne.

### D10 — coturn : `--listening-ip` obligatoire, `--relay-ip`, et DEUX contrôles

**Relevé le 20 août 2026 : aucun conteneur coturn ne tourne, et rien n'écoute
sur 3478** (relevé ⑨). L'avertissement de `CLAUDE.md` — « coturn écoute sur
toutes les interfaces de l'hôte, dont l'adresse publique », constaté
`UDP listener opened on: 90.87.35.18:3478` le 30 juillet 2026 — porte donc sur
le **fichier**, pas sur un processus vivant. **C'est la rouge gratuite du
critère ②, et elle est disponible : il suffit de lancer le fichier
d'aujourd'hui.**

**Tranché :**

- `--listening-ip=${TURN_LISTENING_IP:?…}` — **obligatoire, sans défaut**.
  L'idiome `${VAR:?message}` de docker compose **fait échouer la commande** si
  la variable manque, avec le message. **Vérifié le 20 août 2026 par une
  maquette hors du dépôt** : `error while interpolating … required variable …
  is missing a value: <message>`, **sortie 1**. C'est la même doctrine que
  `PLATEFORME_HOTE` : une rupture bruyante vaut mieux qu'une écoute universelle
  silencieuse ;
- `--relay-ip=${TURN_RELAY_IP:?…}` — **aussi obligatoire**, et c'est le second
  demi-tour que personne ne fait : `--listening-ip` borne où coturn **écoute**,
  `--relay-ip` borne l'adresse depuis laquelle il **relaie**. Restreindre l'une
  sans l'autre laisse la moitié du problème ouverte ;
- `--no-cli` et la plage `49160-49200` restent inchangés. **Les 41 ports de
  relais restent un plafond dont ni la valeur effective ni le comportement au
  dépassement ne sont mesurés** (spec §8) — P5 ne les mesure pas davantage.

**DEUX contrôles, et le premier ne démarre RIEN :**

| Contrôle | Ce qu'il éprouve | Comment il rougit |
| --- | --- | --- |
| **statique** — `docker compose -f docker-compose.coturn.yml --env-file /dev/null config` | que les variables sont **obligatoires** | avec un défaut, la commande réussirait |
| **dynamique** — coturn lancé, `docker compose … logs coturn` | qu'il **n'ouvre** que sur l'adresse nommée | le fichier d'aujourd'hui ouvre partout |

🔴 **ET LE CONTRÔLE STATIQUE PORTE SON PROPRE PIÈGE, MESURÉ :
`docker compose … config` IMPRIME `--static-auth-secret=<valeur>` EN CLAIR.**
Vérifié le 20 août 2026 ; la valeur n'est pas recopiée ici. **Sa sortie ne se
verse donc JAMAIS telle quelle.** La tâche 17 impose deux gestes : filtrer par
**liste blanche de lignes** (garder `--listening-ip`, `--listening-port`,
`--relay-ip`, `--min-port`, `--max-port`, et le message d'erreur ; jeter tout
le reste), puis **prouver** que la pièce versée ne contient pas le secret, par
un `grep -c -F -f` alimenté depuis `.env` — qui rend un **compte**, jamais la
valeur. Le journal `--verbose` de coturn est soumis au même traitement.

⚠️ **`--verbose` reste posé** : c'est lui qui produit les lignes
`listener opened`, sans lesquelles le critère ② n'a rien à lire. Son commentaire
existant le dit déjà, et P5 lui ajoute la mise en garde sur le secret.

⚠️ **Ce que le critère ② n'établit PAS, et la spec §4 le dit avant moi** :
l'inaccessibilité effective du relais **depuis Internet**. Elle se jugerait sur
une sonde extérieure, qui exigerait une machine hors de ce réseau. **Elle n'est
pas prescrite, et ce sous-projet ne l'établit donc pas.**

⚠️ **Recommandation opérationnelle, hors critère et clairement séparée :
`TURN_SECRET` a été affiché en clair pendant la rédaction de ce plan** (par la
commande `config` ci-dessus, sur cette machine, dans la transcription d'une
session d'agent). La tâche 12 prévoit sa **rotation** — un tirage neuf dans
`.env`, sans rien de versionné. Ce n'est pas un critère : c'est de l'hygiène,
et c'est déclaré plutôt que passé sous silence.

### D11 — Le secret d'enrôlement en clair sur la VM : P5 ne le retire pas, il rend le vol RÉPARABLE

**Question :** P5 corrige-t-il, ou déclare-t-il assumé ?

**Tranché : ASSUMÉ, avec sa raison — ET une contrepartie qui n'existe pas
aujourd'hui.**

**Pourquoi assumé.** Le secret vit en clair dans `C:\dev\run-agent.ps1`, écrit
par `scripts/run-agent.sh`, sur un partage CIFS. Le retirer de là exige (a) de
modifier `scripts/`, (b) de modifier `agent/` pour lire un coffre Windows
(DPAPI ou le magasin d'identifiants), et (c) d'éprouver le résultat **sur la
VM**. **Les trois sont hors du périmètre de P5** : `agent/` et la VM sont
tenus par le chantier concurrent F1, et aucun critère de ⑤ n'emploie la VM
(spec §4). Livrer un demi-remède non éprouvé serait pire que de le déclarer.

**La contrepartie, elle, est entièrement dans le périmètre, et elle manque :
IL N'EXISTE AUCUN MOYEN DE FAIRE TOURNER UN SECRET D'ENRÔLEMENT COMPROMIS.**
Relevé : `depot/agent.ts:54` ne fait qu'un `INSERT INTO agent_enrole`,
`0003-agents.sql:37` pose `vm_id TEXT PRIMARY KEY`, et
`admin/enroler-agent.ts:105` insère aussi dans `vm` — donc **réenrôler une VM
déjà enrôlée LÈVE**, ce que l'en-tête du fichier annonce lui-même. Un
exploitant qui apprend qu'un secret a fuité n'a, aujourd'hui, que le `DELETE`
manuel en base.

**P5 livre donc `npm run admin:agent -- --vm <id> --roter`** : un secret neuf
est tiré, son empreinte remplace l'ancienne, le secret est **imprimé une seule
fois**, et **le préfixe de session n'est PAS touché**. 🔴 **Le préfixe ne
tourne pas, et c'est délibéré** : il compose le nom des sessions vivantes
(`agents/prefixe.ts`, spec §3.4), et le changer couperait toute session en
cours de la VM. Rotation du secret ≠ rotation de l'identité.

⚠️ **Ce que la rotation ne fait pas** : elle ne révoque pas les jetons d'agent
**déjà délivrés**, qui restent valides jusqu'à leur expiration. C'est la même
propriété que la spec §3.5 écrit pour les jetons humains — « un jeton d'accès
court n'est pas révocable avant son expiration » —, et elle borne la fenêtre à
`DUREE_JETON_ACCES_MS`.

### D12 — Journalisation structurée : NON, sauf pour les événements neufs, et voici le compte

La spec liste « journalisation structurée » dans ce que P5 **livre**, et **ne
lui donne aucun critère** (§4, P5). Avant de trancher, j'ai compté.

**Relevé le 20 août 2026 :** `grep -rn 'console\.' plateforme/src` hors tests
rend **15 occurrences dans 9 fichiers**, dont une est une **chaîne de code**
dans `base/pilote-sqlite.ts` et non un appel. Et `grep -rn 'spyOn(console'`
rend **cinq fichiers de test** qui capturent la console **et assertent sur le
contenu du message** (`agents/canal.test.ts:222`,
`signaling/trace.test.ts:225`, `signaling/garde-fil.test.ts:154`,
`orchestration/inventaire-statique.test.ts:68`,
`http/routes-auth.test.ts:193`). Enfin, `index.ts` porte un **couplage nommé** :
sa ligne d'annonce doit contenir `le port <n>`, sans quoi
`signaling/resilience.test.ts` expire au bout de 10 s **sans que rien ne
désigne la cause**.

**Tranché : P5 n'entreprend PAS la migration générale.** Quatorze sites
d'appel, cinq fichiers de test qui assertent sur des messages, un couplage
nommé, et **aucun critère pour juger le résultat** : c'est exactement la
churn non mesurée que ce dépôt punit, au dernier sous-bloc d'une branche.

**Ce que P5 livre à la place** : `obs/journal.ts`, **PUR** — il *rend* la
ligne, il ne l'écrit pas —, au format `evenement k=v k=v`, et **les événements
neufs de P5 l'emploient** : `frein`, `sante`, `enrolement freine`. Un test
interdit à ces modules-là d'appeler `console` autrement que par lui.

**Ce que cela laisse ouvert est écrit dans `obs/journal.ts` et légué** : les
quatorze sites existants gardent leur forme libre. **Le coût de la migration
est chiffré ici pour que le chantier qui la fera n'ait pas à le recompter.**

### D13 — Aucune dépendance, et pourquoi la bibliothèque standard suffit

Le contrôle `plateforme/src/base/pilote.test.ts:49` échouerait sur toute
dépendance neuve. **Aucune n'est nécessaire**, et il faut dire pourquoi plutôt
que l'affirmer :

- **le frein** est une `Map` de compteurs et une soustraction d'horodatages —
  la totalité de `express-rate-limit` qui vaille ici tient en cinquante
  lignes, et la partie qui ne tient pas (les magasins Redis) est précisément ce
  que D1 refuse ;
- **les en-têtes de sécurité** sont un objet littéral — `helmet` ne fait rien
  d'autre, et sa politique par défaut casserait la CSP que D7 confie au proxy ;
- **TLS** n'est pas terminé dans le processus (D9), donc aucune bibliothèque
  de certificats ;
- **le proxy** et **coturn** sont des images, pas des paquets npm.

🔴 **Ce contrôle reste inchangé et doit rester vert.** S'il devenait rouge,
c'est qu'une tâche a dévié du plan ; le signaler, ne pas mettre à jour la
liste.

### D14 — L'état de routage en mémoire : P5 ne le corrige pas, il le DÉCLARE, et le déploiement en dépend

⚠️ **« Plus d'état en mémoire ⇒ scalabilité horizontale » est trompeur, et la
spec le range hors périmètre v1 (§9), en le disant elle-même au §8 : « rien de
la scalabilité horizontale : elle n'est pas obtenue par la persistance,
contrairement à ce que le cadrage laisse entendre ».**

**Relevé, plutôt que supposé.** ⚠️ **Le fichier `agents/registre.ts` que le
cahier des charges de cette tâche nomme N'EXISTE PAS** — vérifié par
`grep -rln registre plateforme/src` et par la liste des fichiers de
`plateforme/src/agents/` (`canal.ts`, `enrolement.ts`, `fraicheur.ts`,
`prefixe.ts`). Les deux états de routage réellement en mémoire sont :

| Fichier | Ce qu'il retient | Effet à deux instances |
| --- | --- | --- |
| `signaling/appariement.ts:55` | la table des sessions et leurs deux pairs | **deux pairs de la même session sur deux instances ne s'apparient JAMAIS** — et rien ne le dit : chacun attend l'autre |
| `signaling/propriete.ts:42` | qui possède quel nom de session | la même session peut être revendiquée deux fois |
| (neuf) `securite/frein.ts` | les échecs récents | le budget est **multiplié par le nombre d'instances** |

**Tranché : P5 ne rend rien de cela partageable** — ce serait un bus
inter-instances ou un routage collant, c'est-à-dire un changement de conception
au dernier sous-bloc d'une branche, sans critère pour le juger. **P5 fait trois
choses, toutes décidables :**

1. `deploiement/nginx.conf` déclare **une seule** cible en amont, avec le
   commentaire qui dit pourquoi ;
2. `securite/frein.ts` porte le coût dans son en-tête, sur le patron de
   `signaling/propriete.ts` qui le fait déjà pour le sien ;
3. le runbook porte l'invariant en tête : **une instance, et une seule**.

**Ce que cela laisse ouvert est le legs le plus lourd de tout ⑤**, et il est
inscrit comme tel : **le service ne peut pas être répliqué, et rien dans le
code ne l'empêche de l'être.** Un exploitant qui doublerait l'instance
obtiendrait des sessions qui ne s'établissent jamais — panne muette, la classe
exacte contre laquelle ce dépôt est écrit. **Le remède minimal, nommé et non
livré : refuser de démarrer si une seconde instance est détectée** (un verrou
consultatif en base, `pg_advisory_lock` côté Postgres, sans équivalent SQLite),
ce qui échangerait la panne muette contre une rupture bruyante.

### D15 — « La configuration DÉPLOYÉE » : une seconde instance Postgres, et le discriminant qui prouve laquelle a été mesurée

Le critère ① exige que la suite soit verte **contre l'instance de déploiement**,
et son énoncé prévient : « une configuration de test qui diffère de celle de
déploiement ne prouve rien ».

**Tranché :** `docker-compose.plateforme.yml` gagne un **profil `deploiement`**
portant :

- `postgres-deploiement` — **même image** `postgres:16-alpine` que l'instance
  de test, pour que le mot « diffère » porte sur la configuration et non sur le
  moteur ; **identifiants par `${…:?}`**, aucun littéral (critère ④) ; **volume
  nommé**, contrairement à l'instance de test qui est jetable ; publié sur
  **`127.0.0.1:5434`**, port **distinct** de 5433 ;
- `plateforme` — le service lui-même, avec `PLATEFORME_HOTE` sur le réseau
  interne, sans port publié : seul le proxy l'atteint ;
- `proxy` — nginx, avec `deploiement/nginx.conf` monté.

🔴 **LE PORT DISTINCT EST LE POINT, ET IL Y A UN DISCRIMINANT.** Les deux
instances sont des Postgres 16 : une suite verte ne dit pas laquelle elle a
touchée. La recette du critère ① **journalise, avant et après la suite**, le
résultat de `SELECT current_database(), inet_server_port(), version()` **par le
pilote du service**, et l'URL employée **avec son mot de passe masqué**. **La
rouge du critère ① n'est pas « casser le service » : c'est pointer la suite sur
l'instance de TEST et vérifier que le discriminant le DIT.** Un discriminant
qu'on n'a pas vu distinguer ne distingue rien.

⚠️ **Ce que le critère ① n'établit pas** : que le déploiement fonctionne. Il
établit que **le sous-ensemble SQL** du service est vert contre l'instance de
déploiement. Le service, lui, est éprouvé par le critère ⑤.

---

## Divergences relevées entre la spec, le code réel, et ce que P5 doit faire

### E1 — La spec nomme `agents/registre.ts` ; ce fichier n'existe pas

Voir **D14**. Le cahier des charges de cette tâche l'appelle en exemple d'état
en mémoire. **`grep -rln registre plateforme/src` ne le trouve nulle part**, et
`plateforme/src/agents/` n'a que quatre modules. Les deux vrais porteurs
d'état de routage sont `signaling/appariement.ts` et `signaling/propriete.ts`.
**Corrigé ici ; ne pas chercher ce fichier.**

### E2 — 🔴 Le client parle `ws://` et `http://` en dur — sous TLS, le navigateur BLOQUE

**C'est la divergence la plus lourde de P5, et elle est bloquante pour le
déploiement même.** Relevé le 20 août 2026 :

| Fichier | Ligne | Valeur par défaut |
| --- | --- | --- |
| `client/src/connexion.ts` | **23** | `http://${window.location.hostname}:8080` |
| `client/src/main.ts` | **31-32** | `ws://${window.location.hostname}:8080` |
| `client/src/shell-page.ts` | **12** | `ws://${window.location.hostname}:8080` |

Une page servie en **`https://`** par le proxy qui ouvrirait `ws://…:8080` est
du **contenu mixte** : le navigateur refuse la connexion, et **aucun test Node
ne peut le voir** — `cors.ts` écrit déjà cette phrase pour son propre sujet.
Le déploiement de P5 serait donc livré non fonctionnel, et vert.

**P5 le corrige** (tâche 16) par un module **PUR** `client/src/adresse-plateforme.ts`
qui dérive schéma et autorité d'un `location` **passé en paramètre** — patron
de `client/src/resize.ts` et `client/src/prefixe.ts` —, et par le branchement
des trois sites. **Le paramètre d'URL explicite (`?plateforme=`, `?signaling=`)
reste prioritaire**, parce que c'est lui qui rend les essais locaux possibles
et que P3 a déjà payé la disparition d'un mode d'essai.

### E3 — La spec fait de « TURNS sur 443 » une livraison possible ; deux relevés l'excluent

Voir **D9** : 443/tcp est pris par un tiers sur cette machine (relevé ⑪), et le
conflit avec le proxy HTTPS est structurel sur une adresse unique. **La spec
prévoit ce cas** (§2.7, §8) et demande alors un **constat écrit**. C'est ce que
P5 livre.

### E4 — Le critère ③ de la spec ne dit pas SUR QUOI porte le frein

Son énoncé — « après *n* échecs, la *n+1*ᵉ est refusée par le frein » — ne
nomme ni la clé, ni la fenêtre, ni le chemin. **P5 le durcit** : deux clés
(D1), deux chemins (`/auth/*` **et** `/agent`, legs n°5 de P3), et **quatre
assertions distinctes, chacune avec son `it()` et sa rouge**. Le durcissement
est déclaré comme tel : ce n'est pas une transcription de la spec.

### E5 — La spec range le frein de `/agent` hors de son critère ; P3 l'y a mis

Le legs n°5 de P3 dit « le frein sur les routes d'authentification **ET** sur
`/agent` ». La spec §4 P5 ③ ne parle que d'« authentification ». **Le legs
gagne** — c'est un document de résultats, et il fait autorité sur l'état du
code.

### E6 — `enroler` ne sait qu'INSÉRER : il n'y a aucune rotation

Relevé : `depot/agent.ts:54`, `0003-agents.sql:37`, `admin/enroler-agent.ts:105`.
Voir **D11**. La spec ne nomme pas ce manque ; il tombe pourtant exactement
dans « le durcissement » qu'elle confie à P5.

### E7 — Le lint SQL ne porte que sur les MIGRATIONS, et P5 n'en écrit aucune

`base/sous-ensemble.test.ts` lit `src/base/migrations/*.sql` et rien d'autre :
ses deux interdits — les jetons hors sous-ensemble et **toute chaîne
littérale** — ne s'appliquent donc pas aux requêtes en ligne des dépôts.
**P5 n'ajoute aucune migration** (le frein est en mémoire, D1 ; la rotation
réécrit une colonne existante), donc **aucun risque de ce côté** — et le
`SELECT 1` de `/sante` (D6) n'est pas concerné. ⚠️ **La règle du dépôt reste
entière par ailleurs** : aucune valeur littérale dans une requête paramétrée,
et cette moitié-là ne lève que sous Postgres.

### E8 — Les trois routeurs traitent déjà `OPTIONS` ; la classe navigateur reste ouverte ailleurs

Relevé : `routes-auth.ts`, `routes-vm.ts:105`, `routes-session.ts:78` traitent
tous `OPTIONS`, et `cors.ts:48` autorise déjà `content-type, authorization`.
**Les deux défauts CORS de P4 sont donc fermés.** Ce qui reste ouvert de cette
classe, et que P5 doit traiter, est **ailleurs** : le contenu mixte (E2) et la
CSP (D7). **C'est pourquoi le critère ⑤ passe par un navigateur RÉEL** (relevé
⑰ : `/usr/bin/google-chrome` existe), et non par un `fetch` Node de plus.

### E9 — La spec écrit `docker-compose.plateforme.yml` « complet » ; le fichier actuel se dit de TEST

Son en-tête dit « instance Postgres de **TEST** », « ⚠️ NE PAS le composer avec
`docker-compose.yml` », et « une base de production n'emploiera JAMAIS ce
fichier ». **P5 ne contredit pas cet en-tête, il l'étend** : les services de
déploiement vivent sous un **profil** (`--profile deploiement`), ce qui laisse
`docker compose -f docker-compose.plateforme.yml up -d` inchangé pour la suite
de tests. **Aucune commande existante ne change de sens** — la propriété qui a
manqué au dépôt à chaque fois qu'un fichier partagé a changé.

### E10 — `verify-all.sh` dépend déjà d'une instance Postgres ; P5 en ajoute une seconde

Son en-tête dit « ce script dépend d'une instance Postgres, et c'est VOULU ».
Il vise **l'instance de test** (5433). **Le critère ① de P5 emploie l'instance
de déploiement** (5434), et **`verify-all.sh` ne la connaît pas et n'a pas à la
connaître** : le critère ① est une recette, pas une étape de vérification
continue. **Aucune étape neuve** (voir les contraintes globales).

### E11 — `ProprieteDeSession` documente son propre coût ; `frein.ts` doit faire pareil

`signaling/propriete.ts` porte quatre paragraphes sur ce que son état en
mémoire ne survit pas. **C'est le patron exact que `securite/frein.ts` doit
suivre** (D1, D14), et le dire ici évite qu'un implémenteur invente une
troisième façon de documenter la même limite.

### E12 — La spec §6 promet un balayage des sessions au démarrage ; il existe, et P5 n'y touche pas

`demarrage.ts` appelle `balayerLesOuvertes` avant d'ouvrir le port. Relevé et
cité ici pour une seule raison : **`/sante` ne doit pas devenir un second
endroit qui décide si le service est prêt.** L'ordre de démarrage est déjà
établi et commenté « NON NÉGOCIABLE » ; `/sante` ne fait que rapporter.

---

## Structure des fichiers

```
plateforme/
  src/
    securite/frein.ts              NEUF — PUR : fenêtre glissante, deux clés, plafond
    securite/frein.test.ts         NEUF
    http/adresse-source.ts         NEUF — PUR : remote + X-Forwarded-For + confiance
    http/adresse-source.test.ts    NEUF
    http/entetes.ts                NEUF — PUR : nosniff + no-store
    http/entetes.test.ts           NEUF
    http/routes-sante.ts           NEUF — GET /sante, verdict en cache borné
    http/routes-sante.test.ts      NEUF
    obs/journal.ts                 NEUF — PUR : rend la ligne, n'écrit pas
    obs/journal.test.ts            NEUF
    securite/secrets.test.ts       NEUF — le balayage du critère ④
    config.ts                      + PLATEFORME_PROXY_DE_CONFIANCE
    http/serveur.ts                + maxPayload, + le frein construit, + /sante chaîné
    http/routes-auth.ts            + le frein, avant tout hachage
    http/routes-vm.ts              + les en-têtes de sécurité
    http/routes-session.ts         + les en-têtes de sécurité
    agents/canal.ts                + le frein d'enrôlement, avant scrypt
    depot/agent.ts                 + remplacerEmpreinte
    admin/enroler-agent.ts         + --roter
client/
  src/adresse-plateforme.ts        NEUF — PUR, sans DOM
  src/adresse-plateforme.test.ts   NEUF
  src/connexion.ts                 + l'appel
  src/main.ts                      + l'appel
  src/shell-page.ts                + l'appel
deploiement/
  nginx.conf                       NEUF — TLS, aiguillage Upgrade, X-Forwarded-For, CSP
  README.md                        NEUF — le runbook : une instance, et pourquoi
docker-compose.coturn.yml          + --listening-ip, + --relay-ip (obligatoires)
docker-compose.plateforme.yml      + le profil `deploiement`
docs/superpowers/plans/
  2026-08-19-plateforme-p5-resultats.md      NEUF
  journaux-plateforme-p5/                    NEUF — toutes les pièces
```

---

## Interfaces partagées

```ts
// securite/frein.ts — PUR. Aucune horloge lue, aucun socket, aucune base.
export interface Budget { readonly max: number; readonly fenetreMs: number; }
export interface Verdict { readonly freine: boolean; readonly retryApresS: number; }

export class Frein {
    constructor(entreesMax?: number);
    /// Consulte SANS rien enregistrer. Appelée AVANT tout travail coûteux.
    consulter(cles: readonly [string, Budget][], maintenant: number): Verdict;
    /// Enregistre un échec sur chacune des clés.
    echec(cles: readonly [string, Budget][], maintenant: number): void;
    /// Efface une clé — appelée sur un SUCCÈS, et sur la clé de compte SEULE.
    succes(cle: string): void;
    /// Pour la trace et pour le test du plafond.
    taille(): number;
    evictions(): number;
}

// http/adresse-source.ts — PUR.
export function adresseSource(
    remote: string | undefined,
    enteteXff: string | undefined,
    confiance: ReadonlySet<string>,
): string;

// http/entetes.ts — PUR.
export const ENTETES_SECURITE: Readonly<Record<string, string>>;

// obs/journal.ts — PUR : rend la ligne, ne l'écrit pas.
export function ligne(evenement: string, champs: Record<string, string | number>): string;
```

⚠️ **`consulter` et `echec` sont DEUX fonctions, jamais une.** Une seule
fonction qui consulterait *et* compterait ferait payer un échec à une requête
légitime arrivée pendant la fenêtre, et rendrait le frein auto-entretenu : un
attaquant maintiendrait un compte bloqué indéfiniment sans jamais tenter un
mot de passe.

⚠️ **`succes` n'efface QUE la clé de compte** (D1). Le passer sur la clé
d'adresse blanchirait un attaquant qui possède un compte valide.

---

## Ordre et parallélisme

```
              tâche 0  (préalables — SÉQUENTIELLE, BLOQUANTE)
                 |
   +-------------+-------------+-------------+-------------+
   |             |             |             |             |
 T1 frein    T2 adresse    T3 entêtes    T4 journal    T11 balayage secrets
 (PUR)        (PUR)         (PUR)         (PUR)         (test seul)
   |             |             |
   +------+------+             |
          |                    |
        T5 config  ------------+
          |
   +------+------+------+------+
   |      |      |      |      |
  T6     T7     T8     T9     T10
 maxP.  frein  frein  /sante  entêtes
        auth   agent          câblés
   \______|______|______|______/
                 |
        T12 coturn      T13 compose déploiement     T14 nginx+runbook
        T15 rotation du secret d'enrôlement
        T16 client : l'adresse dérivée de location
                 |
              T17 RECETTE (5 critères, 2 exécutions, toutes les rouges)
                 |
              T18 revue transverse de fin de branche
                 |
              T19 CLAUDE.md          T20 document de résultats
```

**Se parallélisent sans risque** : T1, T2, T3, T4, T11 (cinq modules purs ou un
test isolé, aucun fichier commun).
**Se parallélisent avec vigilance** : T12, T13, T14 (trois fichiers distincts,
mais T13 et T14 se citent l'un l'autre — les faire relire ensemble).
**Strictement séquentiels** : T5 avant T7/T8 ; T6 à T10 avant T17 ; T17 avant
T18 ; T18 avant T19 et T20.

---

## Les tâches

### Task 0 : les préalables — P4 est-il clos, et que valent les références ?

- [ ] Vérifier que **P4 est clos** : `docs/superpowers/plans/2026-08-19-plateforme-p4-resultats.md`
      existe, et `git log --oneline -12` porte ses tâches 13 à 19. **Si P4 n'est
      pas clos, ARRÊTER ICI et le dire** — P5 en dépend (spec §4).
- [ ] Relancer et **consigner** : `plateforme` `test:sqlite`, `test:postgres`,
      `typecheck` ; `client npm test`, `typecheck` ; `proto npm test`,
      `typecheck` ; `grep -cE '^etape ' scripts/verify-all.sh` ;
      la commande des 500 lignes ; `git status --porcelain` ; `git log -1`.
- [ ] **Annoncer** ces chiffres **avant** de lire quoi que ce soit d'autre :
      ce sont les références de non-régression de P5, et elles remplacent les
      relevés ① à ⑦ de ce plan, qui datent d'avant la clôture de P4.
- [ ] Vérifier que l'instance Postgres de test tourne
      (`docker compose -f docker-compose.plateforme.yml ps`), et **relever si
      un conteneur coturn tourne** — s'il en tourne un, une recette voisine est
      peut-être en cours : **ne rien démarrer et le signaler**.

**Test :** aucun. C'est une tâche de relevé, et son produit est le journal
`journaux-plateforme-p5/references-t0.log`.

**Ce qui la rend ROUGE :** rien — et c'est déclaré. Une tâche de relevé qui
prétendrait porter une rouge mentirait.

### Task 1 : `securite/frein.ts` — la fenêtre glissante, deux clés, et le plafond

- [ ] Écrire `securite/frein.test.ts` **d'abord**, et le voir **rouge**.
- [ ] Implémenter `securite/frein.ts` selon l'interface ci-dessus. **PUR** :
      aucune horloge lue, `maintenant` est un paramètre partout.
- [ ] En-tête sur le patron de `signaling/propriete.ts` : ce que l'état en
      mémoire ne survit pas (D1), pourquoi il n'est pas en base, et ce que
      l'éviction coûte (D2).

**Assertions, chacune son `it()` :**

| # | Ce qui est éprouvé | La mutation qui le rend ROUGE | Atteignable parce que… |
| --- | --- | --- | --- |
| a | sous le budget, `consulter` rend `freine: false` | initialiser le compteur à `max` | l'état initial est observable |
| b | au budget, `consulter` rend `freine: true` | comparer `>` au lieu de `>=` | la limite est un entier exact |
| c | après `fenetreMs`, le budget est rendu | ne pas comparer l'horodatage | l'horloge est un paramètre : le test avance de `fenetreMs + 1` |
| d | `succes` efface la clé donnée **et elle seule** | effacer toute la table | deux clés distinctes sont posées, une seule effacée |
| e | `consulter` **n'enregistre rien** | faire `consulter` incrémenter | appeler `consulter` `max + 1` fois, puis vérifier `freine: false` |
| f | 🔴 la table ne dépasse jamais `ENTREES_MAX` | retirer la purge et l'éviction | insérer `ENTREES_MAX + 1` clés distinctes et lire `taille()` |
| g | une éviction est **comptée** | ne pas incrémenter | même montage que (f), lire `evictions()` |
| h | `retryApresS` est le temps restant, arrondi vers le haut | rendre `fenetreMs / 1000` en dur | avancer l'horloge de la moitié de la fenêtre |

⚠️ **(f) est l'assertion la plus importante de la tâche**, et c'est celle qu'un
implémenteur pressé laisserait tomber : sans elle, le frein est un vecteur
d'épuisement mémoire (D2). **`ENTREES_MAX` est un paramètre du constructeur
précisément pour que le test puisse en poser un petit** (par exemple 8) au lieu
d'insérer dix mille clés.

**Commit :** `plateforme(p5): le frein, sa fenetre, et le plafond sans lequel il est l'attaque`

### Task 2 : `http/adresse-source.ts` — l'adresse du client, et l'en-tête qu'on ne croit pas

- [ ] Test d'abord, **vu rouge**.
- [ ] Implémenter la règle de **D3**, avec son en-tête : la confiance vide par
      défaut, le **dernier** élément, la normalisation IPv4-mappée, et la limite
      « exactement un proxy ».

**Assertions :**

| # | Ce qui est éprouvé | La ROUGE | Atteignable parce que… |
| --- | --- | --- | --- |
| a | source non de confiance ⇒ l'en-tête est **ignoré** | lire l'en-tête sans condition | l'en-tête porte une adresse *différente* de `remote` |
| b | 🔴 source de confiance ⇒ **dernier** élément | prendre `parts[0]` | l'en-tête d'essai est `203.0.113.7, 198.51.100.4` : les deux valeurs sont distinctes et plausibles |
| c | en-tête absent ⇒ `remote` | rendre la chaîne vide | — |
| d | en-tête présent mais vide/blanc ⇒ `remote` | ne pas filtrer les éléments vides | l'en-tête d'essai est `" , "` |
| e | `::ffff:203.0.113.7` ⇒ `203.0.113.7` | retirer la normalisation | deux appels, l'un mappé l'autre non, doivent rendre la **même** clé |
| f | `remote` absent ⇒ une valeur stable et non vide | rendre `undefined` | un socket fermé rend `undefined` : la clé du frein ne doit pas devenir `"undefined"` par accident, elle doit être **explicitement** une valeur nommée |

⚠️ **(b) est la faute classique**, et c'est pourquoi elle porte le 🔴 : prendre
le premier élément rend l'adresse **forgeable par le demandeur**, donc rend le
frein par adresse contournable en une ligne d'en-tête.

**Commit :** `plateforme(p5): l'adresse du client derriere un proxy, et le dernier element`

### Task 3 : `http/entetes.ts` — deux en-têtes, inconditionnels

- [ ] Test d'abord, **vu rouge**.
- [ ] `ENTETES_SECURITE` = `{'X-Content-Type-Options': 'nosniff', 'Cache-Control': 'no-store'}`.
- [ ] En-tête : le partage de **D7**, et pourquoi ce module est séparé de
      `cors.ts` (celui-ci rend `undefined`, celui-là jamais).

**Assertions :** (a) les deux clés sont présentes ; (b) 🔴 **l'objet ne porte
aucune valeur `*`** — même règle que `cors.ts`, éprouvée ici pour qu'un ajout
futur ne l'introduise pas.

**La ROUGE :** vider l'objet. **Atteignable parce que** l'assertion porte sur
des clés nommées.

**Commit :** `plateforme(p5): nosniff et no-store, inconditionnels et separes du CORS`

### Task 4 : `obs/journal.ts` — la ligne, pas l'écriture

- [ ] Test d'abord, **vu rouge**.
- [ ] `ligne(evenement, champs)` rend `evenement k=v k=v`. Les valeurs qui
      portent une espace ou un `=` sont **entre guillemets** ; les guillemets
      internes sont échappés.
- [ ] En-tête : **D12** en entier — pourquoi la migration générale n'a pas
      lieu, le compte relevé (15 occurrences, 9 fichiers, 5 fichiers de test
      qui assertent, un couplage nommé dans `index.ts`), et que ce module ne
      sert **que** les événements neufs de P5.

**Assertions :** (a) une ligne sans champ ; (b) une valeur avec espace est
citée ; (c) 🔴 **aucune valeur n'est tronquée** — un frein qui tronquerait une
adresse la rendrait ambiguë ; (d) l'ordre des champs suit celui de l'objet,
pour que deux lignes du même événement se comparent.

**La ROUGE :** rendre `JSON.stringify(champs)`. **Atteignable parce que** le
format attendu est comparé à une chaîne exacte.

**Commit :** `plateforme(p5): la ligne de journal, et le compte de ce qu'on ne migre pas`

### Task 5 : `config.ts` — `PLATEFORME_PROXY_DE_CONFIANCE`

- [ ] Test d'abord dans `config.test.ts`, **vu rouge**.
- [ ] Ajouter le champ `proxyDeConfiance: ReadonlySet<string>` à `Config`.
      Valeur : liste séparée par des virgules ; **absente ou vide ⇒ ensemble
      VIDE**, jamais un défaut permissif.
- [ ] Commentaire sur le patron de `PLATEFORME_ORIGINE_CLIENT` : facultative,
      et **son absence produit un refus de croire, pas une permission**. Nommer
      le mode de défaillance de l'oubli (D3) et renvoyer au runbook.

**Assertions :** (a) absente ⇒ ensemble vide ; (b) **chaîne vide ⇒ ensemble
vide** — `env.X ?? 'defaut'` ne rattrape pas `''`, erreur que P1 a payée à sa
tâche 1 ; (c) `"172.18.0.5, 10.0.0.1"` ⇒ deux entrées, **espaces retirées** ;
(d) une entrée vide entre deux virgules est ignorée.

**La ROUGE :** rendre l'ensemble de toutes les adresses quand la variable est
absente. **Atteignable parce que** (a) compare une taille à zéro.

**Commit :** `plateforme(p5): PLATEFORME_PROXY_DE_CONFIANCE, et le defaut qui ne croit rien`

### Task 6 : `maxPayload` sur les deux serveurs — la trame de 100 Mio

- [ ] Test d'abord dans `http/serveur.test.ts`, **vu rouge sur le binaire
      d'aujourd'hui** : ouvrir un socket sur `/`, envoyer une trame de
      `TRAME_MAX_OCTETS + 1` octets, et attendre la **fermeture** avec le code
      1009.
- [ ] Poser `maxPayload: TRAME_MAX_OCTETS` sur `http/serveur.ts:119` et `:126`.
      **Les deux**, avec un commentaire qui dit que le contrôle de forme court
      avant la garde (`signaling/relais.ts:84-86`).
- [ ] `TRAME_MAX_OCTETS = 256 * 1024`, avec la réserve de **D5** : **non
      calibrée**, plancher raisonné et non mesuré.

**Assertions :** (a) une trame trop grande ferme le socket en 1009 ; (b) une
trame **juste sous** la borne est acceptée et sert normalement — sans (b), un
`maxPayload: 1` passerait (a).

**La ROUGE est GRATUITE et atteignable :** le binaire d'aujourd'hui accepte
100 Mio (relevé ⑮), donc le test (a) échoue avant le correctif. ⚠️ **Ne pas
envoyer réellement 100 Mio dans le test** : `TRAME_MAX_OCTETS + 1` suffit et
tient en mémoire.

**Commit :** `plateforme(p5): maxPayload sur les deux serveurs, et le deni de service en une trame`

### Task 7 : le frein sur `/auth/connexion` et `/auth/rafraichir`

- [ ] Tests d'abord dans `routes-auth.test.ts`, **vus rouges**.
- [ ] Câbler : `deps` gagne `frein` et `proxyDeConfiance`. Sur `/auth/connexion`,
      **l'ordre est** : lire le corps → calculer les deux clés → `frein.consulter`
      → si freiné, **429** avec `Retry-After`, la trace, et **rien d'autre** →
      sinon le chemin actuel → sur échec `frein.echec` → sur succès
      `frein.succes(clé de compte)`.
- [ ] Sur `/auth/rafraichir` : **clé d'adresse seule** (D1).
- [ ] La réponse 429 **porte les en-têtes CORS** comme toutes les autres :
      sans eux, le navigateur ne peut pas lire le refus et l'utilisateur voit
      un échec opaque.

**Assertions, chacune son `it()` :**

| # | Ce qui est éprouvé | La ROUGE | Atteignable parce que… |
| --- | --- | --- | --- |
| a | la `n+1`ᵉ tentative sur le **même compte** rend 429 | retirer l'appel `consulter` | `ECHECS_MAX_COMPTE` est petit et l'horloge est injectée |
| b | la `n+1`ᵉ depuis la **même adresse**, comptes tous distincts, rend 429 | ne poser que la clé de compte | les courriels d'essai sont tous différents : seule la clé d'adresse peut mordre |
| c | 🔴 le refus freiné **ne touche pas la base** | déplacer `consulter` après `lireParEmail` | un pilote **compteur** enveloppe la base et le test lit son compte : il doit valoir **0** |
| d | un succès remet le compteur du compte à zéro | retirer `succes` | après `max - 1` échecs, un succès, puis `max - 1` échecs : la dernière doit passer |
| e | un succès **ne** remet **pas** celui de l'adresse | appeler `succes` sur les deux clés | même montage, mais en épuisant la clé d'adresse |
| f | le 429 porte `Retry-After` et les en-têtes CORS | omettre les en-têtes | l'origine d'essai est celle configurée |
| g | la trace nomme la clé retenue | ne rien journaliser | `vi.spyOn(console, …)`, comme les cinq fichiers qui le font déjà |

⚠️ **(c) est l'assertion qui donne son sens au frein.** Un frein posté après le
hachage `scrypt` compte des échecs qu'il a déjà payés au prix fort ; le
mesurer par le **nombre d'accès à la base** est décidable, là où le mesurer en
temps serait instable. **Ce pilote compteur est un décorateur de trois lignes
autour du pilote existant** — pas un faux, pour ne pas mesurer autre chose que
la production.

**Commit :** `plateforme(p5): le frein avant tout hachage, et le 429 que le navigateur peut lire`

### Task 8 : le frein sur `/agent`

- [ ] Tests d'abord dans `agents/canal.test.ts`, **vus rouges**.
- [ ] Câbler dans `agents/canal.ts` : deux clés (`agent:<vm>` et
      `adr:<adresse>`), `consulter` **avant** `verifierEnrolement` donc avant
      `scrypt`, refus avec le motif **`enrolement`** et rien d'autre (D4), et
      la fermeture existante conservée.
- [ ] L'adresse vient de la requête de montée : `servirLeCanalAgent` reçoit
      désormais l'`IncomingMessage` du `connection`, ce que `ws` fournit déjà.

**Assertions :** (a) la `n+1`ᵉ tentative sur la **même VM** est refusée ;
(b) idem depuis la **même adresse** sur des VMs distinctes ; (c) 🔴 **le refus
freiné rend exactement le même message que le refus d'enrôlement** — sinon
l'oracle d'énumération que `agents/enrolement.ts` ferme sur trois paragraphes
rouvre par la porte du frein ; (d) le journal, lui, **distingue** les deux ;
(e) 🔴 le refus freiné **ne dérive aucun `scrypt`** — mesuré par le même pilote
compteur qu'en tâche 7, `lireParVm` devant valoir **0**.

**La ROUGE de (c) est atteignable** parce que le message de refus est comparé
octet pour octet à celui du refus non freiné, produit dans le même test.

**Commit :** `plateforme(p5): le frein d'enrolement, avant scrypt, et le refus qui n'enumere rien`

### Task 9 : `/sante` — le verdict, en cache borné

- [ ] Tests d'abord dans `http/routes-sante.test.ts`, **vus rouges**.
- [ ] `servirSante(req, rep, deps)` sur le patron des trois routeurs existants,
      chaîné dans `servirTout` **en dernier**.
- [ ] `SELECT 1` par le pilote ; verdict mis en cache `PERIODE_SANTE_MS = 1000`,
      horloge injectée.

**Assertions :** (a) base saine ⇒ 200 et `{"etat":"ok"}` ; (b) base en échec ⇒
**503** et `{"etat":"degrade"}` ; (c) 🔴 **la réponse ne porte rien d'autre** —
comparaison de l'objet **entier**, pas un `toContain`, pour qu'un ajout futur
de version ou de compteur casse le test ; (d) 🔴 **N appels dans la période ne
font qu'UNE requête** — pilote compteur ; (e) après la période, une nouvelle
requête a lieu ; (f) une méthode autre que `GET` rend 405.

**La ROUGE de (d) :** retirer le cache. **Atteignable parce que** le compteur
passe de 1 à N, et N est choisi > 1.

**Commit :** `plateforme(p5): /sante, son cache, et la reponse qui ne divulgue rien`

### Task 10 : les en-têtes de sécurité sur les quatre routeurs

- [ ] Tests d'abord — **un `it()` par routeur**, pas un test global : un test
      unique passerait dès qu'un seul routeur les pose.
- [ ] Étaler `ENTETES_SECURITE` dans les `writeHead` de `routes-auth.ts`,
      `routes-vm.ts`, `routes-session.ts`, `routes-sante.ts`, **y compris sur
      les réponses d'erreur** (401, 405, 413, 429, 500, 503) : une réponse
      d'erreur porte souvent plus d'information qu'une réponse normale.

**Assertions :** quatre `it()` de présence, plus un cinquième : 🔴 **la réponse
200 de `/auth/connexion` porte `Cache-Control: no-store`** — c'est **la seule**
qui porte des jetons, et c'est celle qu'un test générique oublierait.

**La ROUGE :** retirer l'étalement d'un **seul** routeur ; le test de ce
routeur doit tomber et **les trois autres rester verts**. **Atteignable parce
que** les `it()` sont séparés — c'est la leçon de la rouge ①A de P2.

**Commit :** `plateforme(p5): les en-tetes de securite sur les quatre routeurs, un it() chacun`

### Task 11 : le balayage « aucun secret dans un fichier versionné »

- [ ] Écrire `securite/secrets.test.ts`, **vu rouge d'abord** (voir la
      procédure ci-dessous).
- [ ] Le balayage porte sur `git ls-files` **à la racine du dépôt**, et cherche
      des **AFFECTATIONS** — `NOM=valeur` ou `NOM: valeur` — pour une liste de
      noms de secrets : `TURN_SECRET`, `PLATEFORME_SECRET_JETON`,
      `AGENT_SECRET`, `POSTGRES_PASSWORD`, `WINDOWS_PASSWORD`,
      `WINDOWS_ADMIN_PASSWORD`, `RECETTE_MOTDEPASSE`.
- [ ] **Une valeur qui est une interpolation (`${…}`) n'est pas un secret**, et
      une valeur vide non plus.
- [ ] **Liste d'exceptions, chacune avec sa raison écrite**, sur le patron de
      `DRAPEAUX_INTERDITS` d'`enroler-agent.ts` : aujourd'hui
      `docker-compose.plateforme.yml` porte `POSTGRES_PASSWORD: plateforme-test`
      pour son instance **jetable**, ce que son propre en-tête déclare en
      toutes lettres. **Toute entrée neuve exige sa raison** — le test compare
      la liste à un objet dont les valeurs sont des raisons non vides.

⚠️ **Pourquoi les NOMS et pas les VALEURS** : chercher des valeurs est
indécidable (la spec le dit : « chercher les valeurs est impossible ») ;
chercher une **affectation d'un nom connu** est décidable, et c'est le geste
qu'un humain fait par inadvertance.

**Procédure de la ROUGE — et c'est ici que le piège du `git checkout`
s'applique en plein :**

1. `git status --porcelain` avant, **consigné** ;
2. créer `essai-secret.env` **contenant une valeur factice et jamais un vrai
   secret**, et le `git add` (le balayage lit `git ls-files`, donc le fichier
   doit être suivi) ;
3. lancer le test, le **voir rouge**, verser la sortie ;
4. `git rm --cached essai-secret.env` **puis supprimer le fichier NOMMÉMENT** —
   ⛔ **jamais `git clean`** : le chantier concurrent F1 a des fichiers non
   suivis dans cet arbre — **cinq au relevé ⑬, seize une heure plus tard** —,
   et `git clean` les emporterait tous ;
5. `git status --porcelain` après, **identique** à celui d'avant.

**Atteignable parce que** l'étape 2 crée exactement l'état que le balayage
dénonce.

**Commit :** `plateforme(p5): le balayage des secrets versionnes, et ses exceptions raisonnees`

### Task 12 : `docker-compose.coturn.yml` — l'écoute nommée, et le constat TURNS

- [ ] Ajouter `--listening-ip=${TURN_LISTENING_IP:?…}` et
      `--relay-ip=${TURN_RELAY_IP:?…}`, **tous deux obligatoires** (D10).
- [ ] Écrire dans l'en-tête du fichier **le constat de D9** : TURNS n'est pas
      livré, les deux raisons (443 occupé par un tiers sur la machine de
      mesure ; conflit structurel sur une adresse unique), et que la cible
      « réseaux restrictifs » du cadrage **n'est donc pas couverte**.
      `--no-tls --no-dtls` reste, **commenté comme assumé**.
- [ ] Écrire la mise en garde : `docker compose … config` **imprime le secret**.
- [ ] Consigner dans `.env` (non versionné) les deux variables neuves, et
      **faire tourner `TURN_SECRET`** — hygiène, hors critère, voir D10.

**Test :** aucun test unitaire — c'est un fichier de configuration. **Le
contrôle est dans la recette** (critère ②), et il a **deux moitiés** : la
statique (`--env-file /dev/null` ⇒ **sortie 1** avec le motif) et la dynamique
(le journal de coturn).

**Ce qui la rend ROUGE :** poser un défaut (`${TURN_LISTENING_IP:-0.0.0.0}`) —
la commande statique réussirait alors. **Atteignable, et vérifié le 20 août
2026 sur une maquette hors du dépôt** : l'idiome `${VAR:?msg}` rend bien
`sortie 1` et le message.

⛔ **Cette tâche ne DÉMARRE pas coturn.** Le démarrage appartient à la recette,
qui vérifie d'abord qu'aucune recette voisine n'en emploie un.

**Commit :** `plateforme(p5): coturn n'ecoute que sur l'adresse nommee, et le constat TURNS ecrit`

### Task 13 : `docker-compose.plateforme.yml` — le profil `deploiement`

- [ ] Ajouter, **sous `profiles: [deploiement]`** (E9), les trois services de
      D15 : `postgres-deploiement` (même image, identifiants `${…:?}`, volume
      nommé, `127.0.0.1:5434`), `plateforme`, `proxy`.
- [ ] **Aucun littéral de secret** — c'est le critère ④, et la tâche 11 le
      balaie.
- [ ] En-tête : **`docker compose -f docker-compose.plateforme.yml up -d` ne
      change pas de sens** et démarre toujours la seule instance de test.
- [ ] Le service `plateforme` reçoit `PLATEFORME_HOTE` sur le réseau interne
      et **aucun port publié** : seul le proxy l'atteint (D9).
- [ ] 🔴 **Une seule réplique**, avec le commentaire de D14 : deux instances ne
      s'apparient jamais, et rien dans le code ne l'empêche.

**Test :** aucun test unitaire. Contrôle dans la recette : `docker compose …
--profile deploiement --env-file /dev/null config` doit **échouer** en nommant
la première variable manquante ; avec les variables posées, il doit réussir et
**ne publier que `127.0.0.1:5434`**.

**Ce qui la rend ROUGE :** poser un mot de passe littéral — la tâche 11 devient
rouge. **Atteignable** : c'est exactement l'état d'aujourd'hui pour l'instance
de test, que la liste d'exceptions couvre nommément.

**Commit :** `plateforme(p5): le profil deploiement, sans un seul litteral de secret`

### Task 14 : `deploiement/nginx.conf` et `deploiement/README.md`

- [ ] `nginx.conf` : terminaison TLS ; `map $http_upgrade` et l'aiguillage de
      **la racine** entre le fichier statique et la montée WebSocket (D8) ;
      `proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for` ;
      `X-Forwarded-Proto` ; les en-têtes du HTML (CSP, HSTS, `Referrer-Policy`,
      `frame-ancestors`) ; **un seul `upstream`, une seule cible** (D14).
- [ ] La CSP nomme `connect-src` avec `wss:` — sans quoi la page se charge et
      **le média ne s'établit jamais**, panne muette exacte de la classe
      « ce qu'un navigateur exige ». **C'est le critère ⑤ qui la juge**, pas ce
      fichier.
- [ ] `README.md` : le runbook. **Quatre invariants en tête**, chacun avec sa
      raison : (1) **une instance, et une seule** ; (2) `PLATEFORME_HOTE`
      n'est jamais une adresse publique ; (3) `PLATEFORME_PROXY_DE_CONFIANCE`
      **doit** porter l'adresse du proxy, sinon le frein par adresse dégénère
      en frein global (D3) ; (4) le service doit être **relancé avec son
      environnement** — le piège que `CLAUDE.md` documente depuis le chantier
      TURN, et que la spec §6 demande explicitement de reprendre dans le
      runbook de P5.
- [ ] Y écrire aussi **ce que le déploiement ne couvre pas** : TURNS (D9), la
      réplication (D14), le secret d'enrôlement sur la VM (D11).

**Test :** aucun test unitaire. Le contrôle est le critère ⑤.

**Ce qui la rend ROUGE :** retirer `connect-src wss:` de la CSP — le critère ⑤
doit alors relever une violation dans la console du navigateur.
**Atteignable parce que** le navigateur réel est disponible (relevé ⑰) et
qu'une violation CSP est un événement de console observable.

**Commit :** `plateforme(p5): nginx termine TLS, aiguille la racine sur Upgrade, et le runbook`

### Task 15 : `npm run admin:agent -- --roter`

- [ ] Tests d'abord (`depot/agent.test.ts`, `admin/enroler-agent.test.ts`),
      **vus rouges**.
- [ ] `depot/agent.ts` gagne `remplacerEmpreinte(p, vmId, empreinte)` —
      `UPDATE agent_enrole SET empreinte_secret = ? WHERE vm_id = ?`.
- [ ] `admin/enroler-agent.ts` gagne `--roter` : refuse si la VM **n'est pas**
      enrôlée (avec son motif — l'appelant est l'administrateur, D11), tire un
      secret neuf, remplace l'empreinte, l'imprime **une seule fois**.
- [ ] 🔴 **Le préfixe n'est PAS touché** (D11), et le code le dit.
- [ ] Le refus de `--secret` sur l'argv reste, **et couvre aussi `--roter`**.

**Assertions :** (a) après rotation, l'ancien secret est **refusé** et le neuf
**accepté** — de bout en bout par `verifierEnrolement` ; (b) 🔴 **`prefixe_session`
est inchangé** — relu avant et après ; (c) une VM inconnue rend un refus, pas
une exception ; (d) `--roter --secret X` est refusé.

**La ROUGE de (b) :** faire tourner aussi le préfixe. **Atteignable parce que**
le test relit la colonne des deux côtés de l'appel.

**Commit :** `plateforme(p5): admin:agent --roter, et le prefixe qu'on ne touche pas`

### Task 16 : le client — l'adresse dérivée de `location`, pas du protocole en dur

⚠️ **Vérifier d'abord `git log --oneline -5 -- client/src/` et
`git status --porcelain`** : P4 tâches 13-14 et G1 touchent ce répertoire.

- [ ] Test d'abord (`client/src/adresse-plateforme.test.ts`), **vu rouge**.
- [ ] `adressePlateforme(location, parametre)` — **PUR, sans DOM** : le
      paramètre d'URL explicite l'emporte ; sinon, **même origine que la page**,
      `http`/`https` et `ws`/`wss` dérivés de `location.protocol`, **et le port
      de la page**, pas 8080.
- [ ] Brancher les **trois** sites : `client/src/connexion.ts:23`,
      `client/src/main.ts:31-32`, `client/src/shell-page.ts:12`.
      ⚠️ **Ces trois numéros ont été relevés le 20 août 2026 et P4 peut les
      avoir déplacés : les relire avant d'éditer.**

**Assertions :** (a) `https:` ⇒ `wss:` et `https:` ; (b) `http:` ⇒ `ws:` et
`http:` ; (c) le paramètre explicite l'emporte sur les deux ; (d) 🔴 **aucun
`:8080` n'apparaît quand la page est servie sur le port par défaut** — c'est
tout l'objet de la tâche ; (e) le mode d'essai local **reste possible** : sans
paramètre et sur `http://localhost:5173`, l'adresse rendue est celle de la
page — ⚠️ **ce qui CHANGE le comportement d'essai actuel** (qui visait 8080),
et **c'est déclaré** : la voie d'essai devient le paramètre explicite, que
`main.ts` documente déjà (`?session=demo&signaling=…`).

**La ROUGE :** garder `ws://` en dur ; (a) tombe. **Atteignable parce que** le
`location` est un objet **passé en paramètre**, donc le test peut poser
`https:` sans navigateur.

**Commit :** `client(p5): l'adresse suit le protocole de la page, et le contenu mixte disparait`

### Task 17 : la recette — cinq critères, DEUX exécutions chacun, toutes les rouges jouées

⛔ **Préalables, dans cet ordre, et une seule fois** : vérifier qu'aucune
recette voisine ne tourne (`docker ps` ne montre pas de coturn ; F1 n'est pas
en train de mesurer) ; **la VM Windows n'est PAS employée** ; l'arbre est
propre sur `plateforme/` et `client/`.

**Journaux** : tout dans `docs/superpowers/plans/journaux-plateforme-p5/`,
versés dans git, **et jamais un secret en clair** — voir le contrôle final
ci-dessous.

| # | Critère | Deux exécutions | La ROUGE, jouée |
| --- | --- | --- | --- |
| ① | la suite de dépôt est verte sur Postgres **déployé** | deux fois contre `127.0.0.1:5434`, **toutes deux Postgres** (déclaré) | pointer la suite sur l'instance de **test** et montrer que le **discriminant** le dit (D15) |
| ② | coturn n'ouvre que sur l'adresse nommée | deux lancements | **gratuite** : le fichier d'avant la tâche 12, qui ouvre partout |
| ③ | les tentatives sont freinées, sur les **deux** chemins | deux fois (sqlite, postgres) | retirer `consulter` de `routes-auth.ts`, puis de `canal.ts` — **deux rouges distinctes** |
| ④ | aucun secret dans un fichier versionné | deux fois | la procédure de la tâche 11 |
| ⑤ | **le chemin navigateur réel, derrière le proxy** | deux fois | **gratuite** : le client d'avant la tâche 16, qui ouvre `ws://` depuis une page `https://` |

**Le critère ⑤ en détail — c'est le seul qui ferme la classe « ce qu'un
navigateur exige et qu'un test serveur ne voit pas » :**

- [ ] Monter le profil `deploiement`, avec un certificat **auto-signé engendré
      dans `/tmp`** — ⛔ **jamais dans le dépôt** : une clé privée versionnée
      serait le critère ④ pris en flagrant délit par sa propre recette.
- [ ] Piloter `/usr/bin/google-chrome` par CDP, sur le patron de
      `client/recette/harness.mjs`, en lui faisant accepter le certificat de
      la recette (`--ignore-certificate-errors-spki-list`, jamais un
      `--ignore-certificate-errors` global qui rendrait le contrôle vacueux
      sur d'autres points).
- [ ] Relever **quatre choses**, chacune son assertion : (a) la page se charge
      en `https:` ; (b) **aucune violation CSP** dans la console ; (c) la
      connexion `wss://` s'établit — donc plus de contenu mixte ; (d) la
      requête préalable `OPTIONS` est servie et la requête `POST /auth/connexion`
      **est lisible** par la page.
- [ ] ⚠️ **Si le navigateur ne peut pas être piloté, la tâche ÉCHOUE et le
      dit.** Un saut est un échec — y compris ici.

**Le contrôle qui ferme le piège du secret**, à jouer **avant** de committer
les journaux :

```
# rend un COMPTE, jamais la valeur ; doit valoir 0 sur chaque pièce
sed -n 's/^TURN_SECRET=//p' .env > /tmp/p5-motif
grep -c -F -f /tmp/p5-motif docs/superpowers/plans/journaux-plateforme-p5/*.log
rm -f /tmp/p5-motif
```

⚠️ **Et le même contrôle pour `PLATEFORME_SECRET_JETON` et le secret
d'enrôlement imprimé par `admin:agent`.** Une pièce qui échoue n'est pas
corrigée à la main : elle est **refiltrée par liste blanche** puis recontrôlée.

**Journal attendu, un par exécution**, en-tête portant : le moteur, le commit
mesuré, la taille du binaire n/a (service TS), l'heure, et **le discriminant
d'instance pour le critère ①**.

**Commit :** `recette(p5): cinq criteres, deux executions chacun, et les cinq rouges jouees`

### Task 18 : la revue transverse de fin de branche — OBLIGATOIRE

**Elle est obligatoire, et son barème est connu** : D7 **5**, D8 **3**, D9 **6**,
D10 **douze**, D11 **sept**, P1 **huit**, P2 **dix**, S1 **cinq**, E **neuf**,
P3 **douze**, S2 **douze**. **Un rapport qui ne trouve rien est un rapport qui
n'a pas cherché.**

**Sa cible propre, nommée d'avance** — ce sont les défauts qui **franchissent
une frontière de tâche**, corrects des deux côtés pris séparément :

- [ ] **Les affirmations de code devenues fausses dans leur propre branche.**
      Balayer les commentaires qui parlent de « P5 », « à rouvrir en P5 », « le
      frein est P5 ③ », « c'est le sujet de P5 », « aucun frein » : `config.ts`,
      `agents/canal.ts` (les `MOTIFS_FERMANTS`), `agents/enrolement.ts`,
      `admin/enroler-agent.ts` (l'en-tête qui renvoie à P5),
      `signaling/relais.ts`, `signaling/propriete.ts`. **P5 est le sous-bloc
      qui les rend fausses** : elles annoncent toutes un futur qui vient
      d'arriver.
      🔴 **`grep -rn "P5" plateforme/src client/src` AVANT d'écrire la liste**
      — « corrigé à sa place » est une affirmation de **complétude**, et une
      affirmation de complétude s'énumère avant de s'écrire.
- [ ] **Les nombres.** Toute ligne de ce plan, du document de résultats et de
      `CLAUDE.md` qui porte un compte doit être **remesurée**, y compris celles
      que la revue elle-même vient d'éditer : *éditer une ligne de tableau ne
      fait pas relire le nombre qu'elle porte* — leçon payée deux fois en D10.
- [ ] **Les contrôles qui ne peuvent pas échouer.** Reprendre les huit
      assertions de la tâche 1, les six de la tâche 2, et les cinq de la
      tâche 17 : pour chacune, **la rouge a-t-elle été VUE ?**
- [ ] **Les pièces.** Aucune sortie de commande fabriquée ; aucune preuve dans
      un fichier gitignoré ; `git ls-files docs/…/journaux-plateforme-p5 | wc -l`
      consigné.
- [ ] **Les secrets.** Rejouer le contrôle de la tâche 17 sur **toutes** les
      pièces versées, une dernière fois.

**Commit :** `revue(p5): la revue transverse de fin de branche, et ce qu'elle trouve`

### Task 19 : `CLAUDE.md` gagne sa section P5

- [ ] **Tailles relevées PAR LA COMMANDE**, après la dernière édition de la
      ronde — une table relevée en début de ronde serait fausse à la fin de la
      même ronde, erreur que D8 a commise en croyant bien faire.
- [ ] La section porte : le verdict des cinq critères **avec leur nombre
      d'exécutions** ; les décisions D1, D3, D5, D9, D10, D11, D14 ; **ce que
      P5 n'établit PAS** ; les pièges neufs ; et 🔴 **la mise à jour de la
      section « Chantier C volet 2 »**, dont la phrase « coturn écoute sur
      toutes les interfaces de l'hôte, dont l'adresse publique » **cesse d'être
      vraie du fichier** après la tâche 12 — **annoter, pas supprimer** : elle
      reste vraie comme relevé daté du 30 juillet 2026.
- [ ] 🔴 **Écrire noir sur blanc que la cible « réseaux restrictifs » n'est pas
      couverte** (D9). C'est une promesse du cadrage que ⑤ ne tient pas, et un
      index qui la tairait ferait croire le contraire.
- [ ] ⚠️ **Ne pas écrire cette section en même temps que la tâche 18 de P4.**

**Commit :** `docs(p5): la section P5, et coturn qui n'ecoute plus partout`

### Task 20 : clore le document de résultats

- [ ] `docs/superpowers/plans/2026-08-19-plateforme-p5-resultats.md`, sur le
      patron de ceux de P1 à P3 : les critères avec leur nombre d'exécutions,
      les décisions, **ce que P5 n'établit PAS**, les pièges neufs, le contrôle
      explicite qu'aucune preuve ne vit hors de git, le témoin de clôture
      (`./scripts/verify-all.sh`, **relancé APRÈS la dernière édition de code**),
      et **ce que ⑤ lègue après son dernier sous-bloc**.
- [ ] Le témoin de clôture porte son tableau d'étapes et **dit s'il compte les
      dix `etape` ou les dix-sept `==>`**.

**Commit :** `docs(p5): le document de resultats, et ce que ⑤ laisse ouvert`

---

## Ce que ce plan NE prescrit PAS, et pourquoi

- **Aucune migration de schéma.** Le frein est en mémoire (D1), la rotation
  réécrit une colonne existante. Rien à ajouter au lint SQL (E7).
- **Aucune dépendance** (D13), et le contrôle qui le prouve reste inchangé.
- **Aucun test de charge, aucune latence, aucun taux** — spec §8.
- **Aucune sonde extérieure** pour juger l'inaccessibilité du relais depuis
  Internet : elle exigerait une machine hors de ce réseau (spec §4, §8).
- **Aucune migration générale vers la journalisation structurée** (D12), et son
  coût est chiffré pour le chantier qui la fera.
- **Aucune réplication, aucun verrou d'instance unique** (D14) : le remède
  minimal est nommé, non livré.
- **Aucune modification de `agent/`, `proto/`, `scripts/`, du produit
  historique** — et c'est ce qui rend D11 inévitable.
- **Aucun emploi de la VM Windows** : aucun critère de ⑤ n'en a jamais eu
  besoin, et elle est tenue par un chantier concurrent.
- **Aucune calibration.** `FENETRE_MS`, `ECHECS_MAX_COMPTE`,
  `ECHECS_MAX_ADRESSE`, `ENTREES_MAX`, `TRAME_MAX_OCTETS`, `PERIODE_SANTE_MS`
  rejoignent la liste déjà longue de ce dépôt. **Aucun jugement d'usage n'est
  porté sur aucune d'elles.**
