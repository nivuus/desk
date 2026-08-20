# Sous-bloc P3 — l'identité des agents, et le canal plateforme ↔ agent : plan d'implémentation

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fermer la moitié du trou que P2 laisse ouverte — un pair
`{"role":"agent"}` obtient aujourd'hui des identifiants TURN valables 86 400 s
sans présenter la moindre identité — et, du même geste, rendre l'espace de noms
des sessions **global** par un préfixe opaque émis par la plateforme, de sorte
que deux VMs cessent de se disputer le nom `bureau` et le compteur `w-{n}`.

**Architecture:** trois étages, et **aucun d'eux ne touche à la pureté de la
garde de P2**.

1. **Un canal `/agent`**, WebSocket sur le même port, dont le schéma vit dans
   `proto/` avec sa constante de version vérifiée **des deux côtés**. L'agent
   s'y enrôle par un secret haché en base et y reçoit un **préfixe** et un
   **jeton d'agent** ; il y bat le cœur, et le canal lui rend un jeton frais à
   chaque battement.
2. **La garde du relais reste pure et synchrone.** Elle ne lit ni base ni
   horloge réelle : elle vérifie le jeton d'agent exactement comme celui d'un
   humain — même fonction, un *claim* de type de plus — et exige que le sujet
   du jeton **préfixe** le nom de session demandé.
3. **Le préfixe voyage jusqu'aux deux pairs** : l'agent Rust le pose devant
   `bureau` et devant `w-{n}` ; la page-shell le lit au lieu de l'écrire en
   dur.

**Tech Stack:** inchangée — Node/TypeScript, Vitest, `ws`, `node:sqlite`, `pg`,
Rust/`tokio-tungstenite` côté agent. 🔴 **P3 N'AJOUTE AUCUNE DÉPENDANCE de
production**, ni en TypeScript ni en Rust : `randomBytes`, HMAC-SHA256 et
`scrypt` sont dans `node:crypto`, et le canal `/agent` réemploie `ws` côté
serveur et `tokio-tungstenite` côté agent — déjà présents. Le contrôle
`plateforme/src/base/pilote.test.ts:49` (`expect(deps).toEqual(['pg', 'ws'])`)
reste **inchangé** et doit rester **vert** : c'est le témoin de cette propriété.

**Spec :** `docs/superpowers/specs/2026-08-19-plateforme-design.md` (commit
`217a765`), §3.3, §3.4 et §4 « P3 ».
**Sous-blocs précédents :** `docs/superpowers/plans/2026-08-19-plateforme-p1-resultats.md`
et `…-p2-resultats.md`, **qui font autorité sur l'état du code** — la spec, elle,
a vieilli sur plusieurs numéros de ligne (voir E2).

**Ce plan ne couvre QUE P3.** Rien de l'orchestration ni de l'attribution d'une
VM à un utilisateur (P4) : aucune interface `Orchestrateur`, aucun
`InventaireStatique`, aucune route qui rende un identifiant de session à un
navigateur, aucune lecture de `session.utilisateur_id`. Rien du durcissement de
production (P5) : ni TLS, ni frein sur les routes d'authentification, ni
`coturn` restreint, ni `/sante`. **La table `application` est créée et reste
vide** : son chemin d'écriture est ④, pas P3.

---

## Contraintes globales

### Ce qui a été relevé PAR LA COMMANDE avant d'écrire une ligne

Toutes les valeurs ci-dessous ont été obtenues le **19 août 2026**, sur cette
machine, en lançant réellement la commande. Aucune n'est recopiée d'un document
antérieur. **Ce qui n'a pas été mesuré est marqué comme tel partout ailleurs
dans ce plan.**

| # | Commande | Résultat relevé |
| --- | --- | --- |
| ① | `cd plateforme && npm run test:sqlite` | `Test Files 22 passed (22)`, `Tests 137 passed (137)` |
| ② | `cd plateforme && npm run test:postgres` | `Test Files 22 passed (22)`, `Tests 137 passed (137)` |
| ③ | `cd plateforme && npm run typecheck` | sortie **0** |
| ④ | `cd client && npm test` | `Test Files 13 passed (13)`, `Tests 120 passed (120)` |
| ⑤ | `cd client && npm run typecheck` | sortie **0** |
| ⑥ | `cd proto && npm test` | `Test Files 2 passed (2)`, `Tests 37 passed (37)` |
| ⑦ | `cd proto && npm run typecheck` | sortie **0** |
| ⑧ | `node --version` | `v24.9.0` |
| ⑨ | `docker compose -f docker-compose.plateforme.yml ps` | `guacamole-postgres-plateforme-1 … Up 3 hours (healthy) 127.0.0.1:5433->5432/tcp` |
| ⑩ | la commande des 500 lignes de `CLAUDE.md` | **deux** fichiers au-dessus du plafond : `agent/src/encode.rs` **1536**, `agent/src/windows_source.rs` **630** |

⚠️ **Les relevés ① à ⑦ sont les références de non-régression de P3.** Toute
tâche qui les fait baisser a cassé quelque chose ; toute tâche qui les fait
monter doit dire **de combien et pourquoi**, et l'annoncer **avant** de lire le
compte — c'est ainsi que D10 a rattrapé un test supprimé par un `Write`
d'écrasement.

🔴 **`cargo test --workspace` N'A PAS ÉTÉ LANCÉ pour établir cette référence**,
et c'est déclaré plutôt que dissimulé : un chantier concurrent (microphone)
travaille dans `agent/` en ce moment même (voir §« Périmètre concurrent »), et
un compte de tests Rust relevé aujourd'hui ne serait attribuable ni à lui ni à
P3. **La première tâche qui touche `agent/` doit relever cette référence
elle-même, sur un arbre dont elle nomme le commit** — piège hérité de D11 et
repayé par P2 (`temoin-cargo-arbre-propre.log`).

### La règle des 500 lignes : DEUX extractions sont requises, et elles précèdent leurs additions

**Relevé par la commande le 19 août 2026** (`{ git ls-files; git ls-files
--others --exclude-standard; } | grep -vE 'node_modules|package-lock|Cargo.lock|/dist/|testdata/|^docs/|^CLAUDE.md' | xargs wc -l | sort -rn`),
pour les seuls fichiers que P3 modifie ou qui bordent son périmètre :

| Fichier | Lignes | Marge | Ce que P3 y ajoute | Porte |
| --- | --- | --- | --- | --- |
| 🔴 `agent/src/superviseur/table.rs` | **492** | **8** | le préfixe devant les deux `w-{n}` | **EXTRACTION OBLIGATOIRE, tâche 17, AVANT la tâche 19** |
| ⚠️ `proto/src/control.rs` | **470** | 30 | **rien** — P3 crée `proto/src/plateforme.rs`, il ne touche pas celui-ci | aucune, et c'est une décision (E14) |
| ⚠️ `agent/src/demarrage.rs` | **481** | 19 | rien si le câblage passe par `superviseur.rs` (77) et `main.rs` | à **re-mesurer** avant la tâche 19 |
| `agent/src/transport.rs` | **491** | 9 | rien | aucune |
| `client/verify-webrtc.mjs` | **494** | 6 | rien — P3 ne touche aucun pilote CDP | aucune |
| `plateforme/src/signaling/relais.ts` | **310** | 190 | rien, ou une ligne (E10) | aucune |
| `plateforme/src/http/serveur.ts` | **122** | 378 | la seconde branche de montée (~30) | aucune |
| `plateforme/src/identite/garde.ts` | **121** | 379 | la vérification du rôle `agent` (~35) | aucune |
| `plateforme/src/identite/jeton.ts` | **115** | 385 | le *claim* de type (~20) | aucune |
| `plateforme/src/signaling/garde-fil.test.ts` | **215** | 285 | la réécriture du test de la fenêtre E2 (~40) | aucune |
| `plateforme/src/depot/session.ts` | **105** | 395 | un paramètre `vmId` (~5) | aucune |
| `plateforme/src/signaling/trace.ts` | **87** | 413 | la résolution du préfixe (~25) | aucune |
| `agent/src/superviseur/protocole.rs` | **110** | 390 | le préfixe (~20) | aucune |
| `agent/src/signaling.rs` | **204** | 296 | le jeton dans la poignée de main (~10) | aucune |
| `agent/src/superviseur/signalisation.rs` | **81** | 419 | idem (~10) | aucune |
| `agent/src/main.rs` | *(à mesurer)* | | deux variables d'environnement (~15) | à mesurer |
| `client/src/shell-page.ts` | **88** | 412 | le préfixe lu (~10) | aucune |
| `scripts/run-agent.sh` | **126** | 374 | deux lignes | aucune |

🔴 **`agent/src/superviseur/table.rs` est à 492 lignes, marge 8, et P3 doit y
écrire.** C'est la situation exacte que ce dépôt a payée trois fois au
sous-bloc D10 en franchissant le plafond — et la seule réponse qui ait jamais
fonctionné est **l'extraction faite AVANT l'addition** (D9,
`capteur/serveur/instances.rs` : marge rendue de 10 à 65 avant que la tâche
suivante n'y écrive). **La tâche 17 est donc une tâche à part entière, sans
addition d'aucune sorte, et elle précède la tâche 19.** Une compression de
commentaire pour repasser sous la ligne est **interdite** — `CLAUDE.md` le dit
nommément, et D9 l'a payée deux fois dans le même sous-bloc.

⚠️ **Porte chiffrée, à jouer après CHAQUE tâche qui touche un fichier
existant** : relancer `wc -l` sur les fichiers de la table ci-dessus. Si l'un
d'eux dépasse **450**, extraire **avant** de continuer, jamais après. Le seuil
est à 450 et non à 500 pour la raison ci-dessus.

⚠️ **La divergence texte/commande que `CLAUDE.md` signale depuis D10 n'est pas
tranchée ici non plus** : le § « Portée » énumère des répertoires, la commande
ne filtre que `node_modules`, les verrous, `dist/`, `testdata/`, `docs/` et
`CLAUDE.md`. P3 applique la contrainte la plus stricte des deux, ce qui est sûr
dans les deux lectures, et **ne prend pas au passage une décision qui appartient
au propriétaire du dépôt**.

### Les règles de méthode, héritées et non négociables

- **Jamais `git add -A`** : nommer les fichiers, un par un. Un `git add -A` a
  déjà emporté le travail concurrent d'une autre tâche dans un commit qui ne
  compilait pas. 🔴 **Et un autre agent travaille dans le même arbre git en ce
  moment.**
- **Un contrôle qu'on n'a jamais vu rouge n'est pas un contrôle.** Chaque test
  neuf est exécuté **avant** l'implémentation et **vu échouer**. Chaque tâche
  nomme la mutation qui le rend rouge.
  🔴 **ET CHAQUE ASSERTION D'UN CRITÈRE À DEUX ASSERTIONS EXIGE SA PROPRE
  ROUGE.** C'est la leçon la plus fraîche du dépôt : le plan de P2 annonçait
  que sa rouge ①A ferait tomber « les DEUX assertions » du critère ① ; **elle
  n'en faisait tomber qu'une**, `expect` interrompant le test à la première, si
  bien que la seconde — l'absence d'`ice-config`, c'est-à-dire la fuite même
  qu'on voulait fermer — **n'était éprouvée par rien**. Il a fallu écrire une
  huitième rouge (①A-bis). **Ce plan porte des critères à deux et trois
  assertions ; chacune a sa rouge nommée, et le § de recette l'exige.**
- **Ce plan est lui-même une source de contrôles vacueux.** D10 en a attrapé
  quatre, dont **trois écrits par son propre plan**. Le doute porte sur ce
  document.
- **Ne jamais fabriquer une sortie de commande.** D10 a attrapé deux pièces
  fabriquées, dont une inscrite dans `CLAUDE.md` **à l'intérieur d'une
  correction qui dénonçait une affirmation non étayée**. Le mécanisme nommé par
  son auteur : *réutiliser la sortie d'une commande antérieure pour répondre à
  la question d'une AUTRE, sans la relancer.*
- **Un saut est un échec.** `npm run test:postgres` ne se saute pas quand
  l'instance manque : il échoue.
- **Les valeurs d'essai sont RÉALISTES, jamais commodes.** C'est la leçon la
  plus chère de P1 : la double passe n'écrivait que des `1_000`, et déclarait
  portable un schéma que Postgres refusait pour toute écriture réelle. Toute
  migration de P3 s'écrit en `BIGINT` pour les horodatages (`_a`), et le
  harnais applique déjà `INSTANT_MIGRATION = 1_700_000_000_000`
  (`plateforme/src/base/harnais.ts:32`).
- **Aucun taux ne sera revendiqué.** Chaque énoncé de résultat porte son nombre
  d'exécutions. **Deux exécutions par critère de recette, pas une**, et la
  convention de P2 est reconduite : exécution 1 = `sqlite`, exécution 2 =
  `postgres`, déclaré dans l'en-tête de chaque journal.
- **Toute preuve d'une affirmation portée dans `CLAUDE.md` est versée dans
  git.** D10 a établi par la commande que l'espace de travail de D9
  (`.superpowers/sdd/`, gitignoré) **a disparu**, emportant six constats de
  revue définitivement perdus.
- **Relire chaque citation `fichier:ligne` APRÈS l'avoir écrite.** 🔴 **Ce plan
  a trouvé CINQ de ses propres citations fausses en se relisant**, alors même
  qu'elles venaient de relevés méthodiques :

  | Écrit d'abord | Réel |
  | --- | --- |
  | la poignée de main de `client/src/webrtc.ts` l. **218** | l. **229** |
  | `agent/src/demarrage.rs:249-256` (la perte de signaling journalisée) | l. **263-268** |
  | `agent/src/demarrage.rs:244-248` (le raisonnement qui l'assume) | l. **257-262** |
  | l'horloge mutable de `garde-fil.test.ts` l. **30** | l. **28** |
  | `client/src/webrtc.ts:109` — recopié d'un commentaire du dépôt | l. **130-131** ; **la l. 109 est vide** |

  **Le dernier est le plus instructif** : il n'était pas de moi, il était
  **recopié verbatim d'un commentaire de `plateforme/src/signaling/appariement.ts:48-51`**,
  écrit par un sous-bloc antérieur et devenu faux depuis. **Recopier une
  citation d'un commentaire du dépôt ne dispense pas de la relire** — c'est le
  naufrage du « 487 » à sa source.

### Périmètre concurrent — à lire avant de toucher `agent/`, `proto/` ou `client/`

Un agent conduit le chantier **E1 (microphone)** dans le même arbre. Relevé par
la commande le 19 août 2026 :

```
$ git log --oneline -8 -- proto/ agent/
3723b2a ajoute(e1): ready porte mic, sans bump de version, absence valant faux
784f1fc ajoute(e1): la piste micro depose et rien d'autre, avec sa garde d'exclusivite
85ed23a corrige(e1): deux m-lines audio ne se marchent plus dessus, et la retention tombe a 2
…
$ git status --porcelain
 M client/src/webrtc.test.ts
```

🔴 **Il touche `agent/`, `proto/` ET `client/`** — c'est-à-dire les trois
paquets que les familles 0, 5 et 6 de ce plan modifient. **Trois conséquences,
et aucune n'est facultative** :

1. **Avant de commencer la famille 0, la famille 5 ou la famille 6**, relancer
   `git log --oneline -5 -- proto/ agent/ client/src/` et `git status
   --porcelain`, et **ne pas démarrer** si un fichier du périmètre de la tâche
   y figure comme modifié.
2. **Les familles 1, 2, 3, 4 et 7 ne touchent que `plateforme/`** et ne sont
   bloquées par rien. Elles se jouent en premier.
3. **P3 ne se fusionne pas à moitié** — voir E1 : la coupure de compatibilité
   est atomique dans la branche.

⛔ **Interdits absolus pour toute tâche de ce plan** : `src/`, `web/`,
`index.js`, `docker-compose.yml`, `docs/superpowers/plans/2026-08-19-micro*`,
`docs/superpowers/plans/journaux-micro/`, et tout fichier d'`agent/micro*`.

---

## Décisions tranchées — les cinq questions que ce plan devait trancher

### D1 — Comment l'agent obtient son identité : **variable d'environnement**, et une tâche dédiée pour `run-agent.sh`

Trois voies étaient ouvertes par la spec (§3.6 mentionne « empreinte du secret
d'enrôlement » dans un fichier d'inventaire, sans dire où vit le secret **côté
VM**).

| Voie | Sort |
| --- | --- |
| posée par l'orchestrateur | **ÉCARTÉE** — l'orchestrateur n'existe qu'en P4, qui dépend de P3. Retenir cette voie rendrait P3 inexécutable |
| fichier sur la VM | **ÉCARTÉE** — `scripts/run-agent.sh:25-109` écrit déjà `C:\dev\run-agent.ps1` **en clair** sur un partage CIFS lisible depuis l'hôte. Un fichier de plus n'ajouterait aucune protection, et ajouterait un chemin, une lecture et un mode de panne |
| **variable d'environnement** | **RETENUE** — c'est le mécanisme que `run-agent.sh` possède déjà, et le seul que l'agent sache lire (`agent/src/main.rs::config()`, l. 145-209) |

**Deux variables : `AGENT_VM` (le nom de la VM enrôlée) et `AGENT_SECRET` (le
secret d'enrôlement).** Elles s'ajoutent au groupe conditionnel de
`scripts/run-agent.sh` (l. 31-87, forme `${VAR:+\$env:VAR = '$VAR'}`, 57
variables aujourd'hui) et se lisent dans `agent/src/main.rs::config()`.

🔴 **UNE TÂCHE DÉDIÉE (tâche 20) NE FAIT QUE CES DEUX LIGNES.** Ce dépôt a payé
**trois fois** l'oubli de cette ligne — `SUPERVISEUR` en D1, `MULTIFENETRE_REPRISE`
en D2, `AUDIO` en D7 —, et le symptôme est toujours le même : **l'agent démarre
sans la variable et sans rien signaler**, si bien qu'on mesure un binaire qui
n'a jamais reçu ce qu'on croit lui avoir donné. En D7 l'implémenteur **et** le
relecteur avaient vérifié la propriété en traçant le code ; le tracé était
juste, la valeur ne pouvait simplement pas atteindre le processus. La tâche 9 de
D6 et la tâche dédiée de D3 sont les deux fois où le piège a été **évité**, et
toutes deux l'ont été par une tâche séparée.

⚠️ **Le coût est nommé, et il n'est pas nul** : le secret apparaît en clair dans
`C:\dev\run-agent.ps1` sur le partage CIFS, exactement comme les 57 autres
variables. Il **n'est pas** dans `argv` — la leçon de P2 (`ps` expose la ligne
de commande de tout processus) est respectée —, mais il n'est pas protégé pour
autant. **C'est acceptable pour une VM de développement et doit être rouvert en
P5**, avec les cookies et les en-têtes de sécurité.

### D2 — La forme du préfixe, et ce qu'il coûte aux deux pairs qui le lisent

**16 octets tirés de `randomBytes`, encodés en `base64url` — 22 caractères,
128 bits**, le minimum que la spec §3.4 exige. Séparateur `:`, comme la spec
l'écrit : `<préfixe>:bureau`, `<préfixe>:w-1`.

**Ce qu'il coûte au pair AGENT** : `agent/src/superviseur/protocole.rs:26`
cesse d'être une constante (`pub const SESSION_DE_CONTROLE: &str = "bureau";`)
et devient une **fonction** de composition ; les deux sites
`agent/src/superviseur/table.rs:239` et `:397` composent de même. La propriété
acquise et documentée du compteur — il ne recule jamais, `table.rs:163-165` —
**n'est pas touchée** : le préfixe rend l'espace de noms global sans rien
changer au mécanisme qui la porte. C'est exactement l'arbitrage de la spec
§3.4, et c'est la raison pour laquelle un identifiant entièrement émis par la
plateforme a été écarté.

**Ce qu'il coûte au pair NAVIGATEUR** : `client/src/shell-page.ts:11`
(`const SESSION_DE_CONTROLE = 'bureau';`) cesse d'être un littéral. Voir E3 pour
la source du préfixe, qui n'est **pas** celle que la spec §3.4 annonce.

⚠️ **La collision apparente avec le format d'identifiant TURN est levée par
l'alphabet du préfixe, pas par chance.** `plateforme/src/signaling/ice.ts:38`
compose `username = \`${expiration}:${session}\``, format figé par
`ice.test.ts:13` (`expect(username).toBe('4600:ma-session')`) et imposé par
coturn en mode `use-auth-secret`. Avec un préfixe, l'identifiant devient
`4600:<préfixe>:bureau` — **trois segments**. Le découpage de coturn se fait sur
le **premier** `:`, et le préfixe étant en `base64url` (`A-Za-z0-9_-`) il **ne
peut pas contenir de `:`** : la première borne reste non ambiguë. 🔴 **Cette
propriété n'est PAS éprouvée contre un coturn vivant par ce plan** — voir E6 et
le §« Ce que ce plan NE prescrit PAS ».

### D3 — `CONTROL_VERSION` et `PROTOCOL_VERSION` ne montent NI l'une NI l'autre, et voici la preuve

La question se tranche par lecture, pas par prudence.

- **`PROTOCOL_VERSION = 2`** (`proto/src/input.rs:14`, miroir
  `proto/ts/input.ts:4`) versionne le **format binaire d'entrée** —
  `version: u8 | type: u8 | charge` (`input.rs:3`). Aucune de ses six formes
  (les six `kind` de `proto/vectors.json`) ne porte de nom de session.
  **Rien à monter.**
- **`CONTROL_VERSION = 3`** (`proto/src/control.rs:14`, miroir
  `proto/ts/control.ts:8`) versionne le canal de données **agent ↔ navigateur**.
  Sa vérification est branchée **variante par variante**, par
  `#[serde(rename = "v", deserialize_with = "verifie_version")]` — onze
  occurrences relevées entre `control.rs:89` et `control.rs:204` — et côté TS
  par `parseAgentControl` (`control.ts:130-139`). **Aucune de ces variantes ne
  porte de nom de session** : `Resize`, `Ready`, `SessionEnd`, `Pointer`,
  `Rumble`, `Capabilities`, `Link`, `Asleep`, `Fullscreen`, `Visibility`
  (`TYPES_AGENT`, `control.ts:106-108`). **Rien à monter.**
- **Le protocole superviseur ↔ shell** (`agent/src/superviseur/protocole.rs`),
  lui, **porte bien `session: String`** dans ses quatre variantes
  (`FenetreOuverte`, `FenetreFermee` l. 31-34, `Refus` l. 35-36, `Viewport`
  l. 42-43). Mais **il n'a aucun champ de version** (spec §2.6, vérifié : le
  fichier ne contient ni constante, ni champ `v`, ni fonction de vérification),
  et la spec §9 range explicitement son versionnement **hors périmètre** :
  « P3 ne le corrige pas ; il ne le reproduit pas non plus ». Et **c'est la
  VALEUR du champ qui change, jamais sa forme** : `"bureau"` devient
  `"<préfixe>:bureau"`, une chaîne reste une chaîne.

**Conclusion : aucune constante de version existante ne monte, et le protocole
superviseur ↔ shell n'en gagne pas.** Ce que P3 crée, en revanche, est une
constante **neuve** — `PLATEFORME_VERSION = 1` — pour le canal `/agent`, sur le
modèle de `CONTROL_VERSION` et **jamais** sur celui de `protocole.rs` (spec
§3.3 point 3).

⚠️ **Un précédent utile, relevé dans le code même** : `control.rs:441-462`
documente qu'un **champ ajouté et optionnel** ne demande pas de bump — c'est ce
que le chantier E1 vient de faire (`ready porte mic, sans bump de version`) —
mais avertit que `AgentControl` porte `deny_unknown_fields`, donc que
`#[serde(default)]` est **obligatoire** sur le champ ajouté. P3 n'ajoute aucun
champ à `AgentControl` ; la note est reprise ici pour qu'un successeur ne
cherche pas la règle ailleurs.

### D4 — Le canal `/agent` : où vit son schéma, comment il se versionne, ce qui se passe si les versions divergent

**Où** : `proto/src/plateforme.rs` (Rust) et `proto/ts/plateforme.ts`
(TypeScript), plus `pub mod plateforme;` dans `proto/src/lib.rs` — fichier de
**4 lignes**, relu, qui ne déclare aujourd'hui que `control` et `input`. Côté
TypeScript **aucune déclaration n'est nécessaire** : `proto/tsconfig.json:12`
inclut `["ts/**/*.ts"]` par glob, et `proto/package.json` ne déclare que
`vitest run` et `tsc --noEmit`, tous deux couverts par les étapes **5** et **6**
de `scripts/verify-all.sh` (l. 66-70). Côté Rust, `proto` est membre du
workspace (`Cargo.toml:3`), donc l'étape **1** (`cargo test --workspace`,
`verify-all.sh:45-46`) le prend. **`scripts/verify-all.sh` n'est donc PAS
modifié par P3**, et c'est un fait relevé, pas une espérance.

**Comment il se versionne** : `PLATEFORME_VERSION: u8 = 1`, champ `v`
**obligatoire** sur chaque variante, vérifié à la désérialisation par une
fonction `verifie_version` copiée **dans sa forme** sur `control.rs:37-52` —
y compris son avertissement, qui est le piège que ce mécanisme existe pour
éviter :

> pas de `default` sur le champ `v` — un message sans champ `v` doit être
> rejeté (champ obligatoire), pas silencieusement complété avec la version
> courante. `default` court-circuiterait `deserialize_with` quand le champ est
> absent, ce qui romprait la vérification.

**Ce qui se passe si les versions divergent** — et ce n'est **pas** symétrique,
parce que les deux bouts n'ont pas le même pouvoir :

| Sens | Comportement |
| --- | --- |
| l'agent envoie une version que la plateforme ne connaît pas | la plateforme répond `{v, type:'refus', motif:'version'}`, **journalise avec la version reçue**, et **ferme le socket** (code 1008). Elle ne tente aucune négociation à la baisse : il n'y a qu'une version |
| la plateforme envoie une version que l'agent ne connaît pas | `serde_json::from_str` échoue, l'agent journalise en `warn!` avec le texte de l'erreur, **ferme le canal**, et **n'ouvre aucune session de signaling** — sans jeton d'agent, la garde le refuserait de toute façon |

🔴 **La divergence de version ne doit JAMAIS se lire comme une panne
réseau.** L'agent qui reçoit un refus `version` **cesse de réessayer** ; celui
qui perd le socket **réessaie** (E9). Deux comportements, deux traces
distinctes — sans quoi une incompatibilité de version se déguiserait en boucle
de reconnexion infinie, qui est le mode de panne le plus coûteux à
diagnostiquer.

**Vecteurs croisés** : `proto/plateforme-vectors.json`, **un fichier neuf**, et
non un ajout à `proto/vectors.json` — voir E8.

### D5 — La compatibilité : 🔴 OUI, P3 CASSE l'agent déjà déployé, et c'est une décision

Voir **E1** ci-dessous, qui la détaille et la chiffre. En un mot : le relais
refusera un `{"role":"agent"}` sans jeton, donc **tout binaire d'agent
antérieur à P3 cesse d'établir la moindre session**. Il n'y a **pas
d'interrupteur permissif** — P2 a refusé le sien (E6 de P2 : « sans interrupteur
permissif ») et P1 le sien (E1 de P2 : la garde est un paramètre **requis**,
« un défaut *accepter* rendrait un service mal câblé indiscernable du bon »).
Le remède est un rebâtissage : `scripts/build-agent.sh`, puis
`scripts/run-agent.sh` avec les deux variables neuves.

### D6 — Le découpage : P3 reste UN sous-bloc, et voici pourquoi

Le découpage a été considéré. La ligne de coupe naturelle existe et elle est
nette : **familles 0 à 4 et 6** (serveur, protocole, navigateur) sont
recettables **avec des pairs simulés**, la spec §4 le prescrivant d'ailleurs —
« les pairs `agent` et `client` sont **simulés** par des sockets scriptés » —,
et **les quatre critères de P3 sont tous atteignables sans une ligne de Rust**.
La **famille 5** (l'agent Rust) ne servirait alors que la corroboration sur VM,
que la spec range déjà **hors critère**.

**Le découpage est néanmoins ÉCARTÉ, pour une raison de comportement et non de
commodité** : entre un P3a qui refuse les agents anonymes et un P3b qui donne
une identité à l'agent Rust, **le produit ne fonctionne plus du tout**. La
coupure de compatibilité (E1) doit être **atomique dans la branche**. Faire
autrement obligerait à livrer P3a avec la garde encore ouverte — c'est-à-dire un
interrupteur permissif étalé sur deux sous-blocs, ce que ce dépôt refuse.

**Ce que l'ordre du plan concède quand même au découpage** : toute tâche qui
touche `agent/`, `proto/` ou `client/` est regroupée dans les familles 0, 5 et 6,
placées **après** les familles 1 à 4, de sorte que le travail commence sans
attendre que le chantier microphone libère ces paquets. **La branche ne se
fusionne qu'entière.**

---

## Divergences relevées entre la spec, le code réel de P1/P2, et ce que P3 doit faire

Elles sont tranchées **avant** d'écrire une ligne, sur le modèle de P1 (six) et
de P2 (onze). Chacune porte le relevé qui la fonde.

### E1 — 🔴 P3 casse la compatibilité de l'agent déployé, et la spec §10 ne le dit pas

**La spec §10.3 écrit** : « P3 touche exactement quatre sites nommés, tous
listés avec leur ligne. C'est le seul sous-bloc qui modifie l'agent, et sa
modification est d'une ligne par site : un préfixe. » Et son avertissement
ajoute : « un préfixe absent vaut le préfixe vide, ce qui restitue exactement le
comportement d'aujourd'hui (`bureau`, `w-1`) — le mode d'essai local reste donc
possible, `scripts/run-agent.sh` compris. »

**Ces deux phrases sont vraies du PRÉFIXE et fausses de l'AUTHENTIFICATION.**
Fermer E2 — l'objet central de P3 — exige que
`plateforme/src/identite/garde.ts:72` (`if (role === 'agent') return { ok: true };`)
cesse d'accepter inconditionnellement. Dès lors :

- **un binaire d'agent antérieur à P3 n'établit plus aucune session**, ni
  `bureau` ni fenêtre : ses deux poignées de main (`agent/src/signaling.rs:59`
  et `agent/src/superviseur/signalisation.rs:35`) émettent `{role, session}`
  **sans jeton**, relu et vérifié ;
- **le « mode d'essai local » ne survit pas non plus** dans la forme que la
  spec décrit : il exige désormais d'enrôler une VM dans la base locale
  (tâche 12, `npm run admin:agent`) et de poser les deux variables de D1.

⚠️ **La spec ne se contredit pas pour autant, et il faut le dire** : sa dernière
phrase pose déjà que « sans enrôlement valide, aucune session ne s'établit du
tout ». **C'est sa phrase du milieu qui a vieilli**, pas sa conclusion.

**Décision : coupure franche, déclarée, sans interrupteur permissif.** Le
remède opérationnel est nommé dans la tâche 23 : `scripts/build-agent.sh` puis
`scripts/run-agent.sh` avec `AGENT_VM` et `AGENT_SECRET`.

### E2 — Les numéros de ligne de la spec §3.4 et §10.2 ont DÉRIVÉ, deux sur quatre

Relevés par la commande le 19 août 2026, chacun relu après écriture :

| Site | Spec | Réel | Contenu vérifié |
| --- | --- | --- | --- |
| `agent/src/superviseur/protocole.rs` | `:17` | **`:26`** | `pub const SESSION_DE_CONTROLE: &str = "bureau";` |
| `agent/src/superviseur/table.rs` | `:239` et `:397` | **exacts** | `let session = IdSession(format!("w-{}", self.compteur));` aux deux |
| `client/src/shell-page.ts` | `:10` | **`:11`** | `const SESSION_DE_CONTROLE = 'bureau';` |
| `client/src/webrtc.ts` (spec §10.2) | `:193` | **`:229`** | `socket.send(JSON.stringify({ role: 'client', session: options.sessionId, jeton }));` |

⚠️ **Le dernier a dérivé de 36 lignes, et c'est P2 qui l'a déplacé** en y
ajoutant le jeton (`webrtc.ts:228`) et deux blocs de commentaire. **Aucun
numéro de ligne de ce plan ne doit être recopié sans être relu** : c'est le
naufrage du « 487 », que ce dépôt a payé neuf fois.

### E3 — Le quatrième site ne peut PAS « recevoir le préfixe de la plateforme » en P3

La spec §3.4 annonce, pour `client/src/shell-page.ts`, « reçu de la
plateforme ». Mais **la spec §4 assigne cette route à P4** : « l'enchaînement
« l'utilisateur demande une session → la plateforme vérifie l'attribution et la
fraîcheur de l'agent → elle rend l'identifiant de session, **le préfixe** et la
configuration ICE » ». Livrer cette route en P3 exigerait l'attribution d'une
VM à un utilisateur, c'est-à-dire P4 tout entier.

**Décision : P3 transforme le littéral en PARAMÈTRE, P4 branche la source.**
`client/src/prefixe.ts` — pur, sans DOM, sur le précédent explicite de
`client/src/jeton.ts` (dont l'en-tête l. 4-10 explique pourquoi : `client/` n'a
aucun `vitest.config.*` et l'environnement de test est Node) — lit le préfixe
dans le coffre, puis dans la chaîne de requête, et **rend la chaîne vide à
défaut**. La chaîne vide restitue exactement `bureau`, ce que la spec §10 pose
déjà comme règle.

⚠️ **Le coût est celui que la spec §10 nomme elle-même** : « une plateforme mal
configurée retombe silencieusement dans un espace de noms partagé ». Il est
accepté pour la même raison — la garde refuse l'agent sans enrôlement, donc
aucune session ne s'établit du tout.

### E4 — La garde de P2 est PURE et SYNCHRONE : l'enrôlement ne peut pas y vivre

`plateforme/src/identite/garde.ts:63-67` :
`garde(secret, maintenant: () => number, proprietes)`, et `verifier` rend un
`Verdict` **sans `Promise`**. Vérifier un secret d'enrôlement haché exigerait
une lecture de base — donc un `await` — **sur le chemin de la poignée de main**,
et ferait de la garde un objet asynchrone que `relais.ts:189` ne sait pas
appeler.

**Décision : l'enrôlement vit HORS du relais, sur le canal `/agent`**, où il est
asynchrone sans gêner personne ; il délivre un **jeton d'agent** que la garde
vérifie **exactement comme celui d'un humain**. La garde reste pure et
synchrone, et elle ne gagne pas une ligne d'accès à la base.

### E5 — 🔴 Jeton d'agent et jeton humain sont signés par le MÊME secret : sans *claim* de type, ils sont interchangeables

`plateforme/src/identite/jeton.ts:51-66` signe `{ sub, exp }` avec
`PLATEFORME_SECRET_JETON`. Si le jeton d'agent emploie le même secret et la
même charge, alors :

- un **jeton humain volé** ouvrirait un rôle `agent` — donc un `ice-config` sur
  toute session dont il préfixerait le nom ;
- un **jeton d'agent** ouvrirait un rôle `client`, contournant l'appartenance de
  session que P2 a posée (`signaling/propriete.ts`).

**Décision : `signer` gagne un paramètre `type`.** Valeur `'agent'` pour les
agents ; **l'absence du *claim* vaut `'utilisateur'`**, de sorte qu'aucun jeton
émis par P2 et encore en vol ne soit invalidé. La garde exige `type === 'agent'`
pour le rôle `agent` et `type !== 'agent'` pour le rôle `client`.

🔴 **DEUX ROUGES DISTINCTES, une par sens de confusion** — et pas une seule
avec deux `expect`, précisément à cause de la leçon ①A-bis de P2.

### E6 — Le séparateur `:` du préfixe croise le format d'identifiant TURN

Voir D2 pour le raisonnement complet. **Décision : garder `:`** (la spec §3.4
l'écrit), et **figer par un test** la chaîne exacte que `deriverIdentifiants`
produit pour une session préfixée. ⚠️ **Aucun coturn vivant n'est sollicité par
ce plan** : la propriété « coturn coupe sur le premier `:` » est une lecture de
la convention `use-auth-secret`, **non éprouvée ici**, et elle est portée au
§« Ce que ce plan NE prescrit PAS ».

### E7 — `plateforme/` n'est câblé sur `proto/` par RIEN

Relevé : `plateforme/tsconfig.json:12` vaut `"include": ["src/**/*.ts"]`, quand
`client/tsconfig.json:12` vaut `"include": ["src/**/*.ts", "../proto/ts/**/*.ts"]`.
Et `grep -rn "proto/ts" plateforme/src` ne rend rien.

**Décision : élargir l'`include` de `plateforme/tsconfig.json` sur le modèle
exact de `client/`**, et importer par chemin relatif (`../../proto/ts/plateforme`),
comme les onze imports de `client/src/` le font déjà. Ni paquet npm, ni copie du
schéma — une copie divergerait en silence, ce qui est le défaut que
`client/recette/jeton-recette.mjs` a dû garder par relecture de fichier en P2.

### E8 — `proto/vectors.json` est structuré pour `input` SEUL, et sa version n'est vérifiée que d'un côté

Relevés : le fichier porte `"version": 2` (l. 3) — miroir de `PROTOCOL_VERSION`,
pas de `CONTROL_VERSION` — et 15 cas dont le champ `kind` ne prend que six
valeurs binaires. Le consommateur Rust (`proto/src/input.rs:322-381`,
`include_str!` l. 323) fait un `match case["kind"]`, et **ne vérifie pas
`doc["version"]`**. Le consommateur TS (`proto/ts/input.test.ts:2`, `:43`,
`:47`) est le **seul endroit du dépôt** qui vérifie `vectors.version`.
`control.rs` / `control.ts` n'ont, eux, **aucun vecteur**.

**Décision : un fichier neuf, `proto/plateforme-vectors.json`**, avec sa propre
clé `version` **vérifiée des DEUX côtés**. Deux raisons : ajouter des `kind`
étrangers à `vectors.json` traverserait le `match` de `input.rs:337-370`, dont
ce plan n'a pas relevé la branche par défaut ; et la lacune du côté Rust est
corrigée **pour le fichier neuf**, sans réécrire rétroactivement un test qui
n'appartient pas à P3.

### E9 — 🔴 Aucun des deux clients WebSocket de l'agent n'a de reprise, et le canal `/agent` en exige une

Relevé : `grep -n 'reconnect\|reprise\|retry\|reconnexion'` sur
`agent/src/signaling.rs`, `agent/src/superviseur/signalisation.rs` et
`agent/src/superviseur.rs` ne rend **rien**. La chute est **détectée** —
`watch::channel(false)` en `signaling.rs:69`, levée en `:138` et `:153-156` —
mais seulement **journalisée** : `agent/src/demarrage.rs:263-268` écrit
`connexion de signaling perdue (aucune renégociation possible pour cette
session)`, et `signalisation.rs:70` un `warn!` équivalent. C'est assumé dans le
code (`demarrage.rs:257-262`) parce que le média ne dépend plus du signaling une
fois l'offre échangée.

**Ce raisonnement ne se transpose PAS au canal `/agent`** : ce canal porte le
battement de cœur, donc `vu_a`. Sans reprise, **la première coupure réseau
rendrait la VM `injoignable` définitivement**, et le critère ④ punirait une
coupure de réseau comme une panne d'agent.

**Décision : le client de `/agent` porte une boucle de reprise à repli
exponentiel borné.** C'est du comportement **NEUF**, que ni `signaling.rs` ni
`signalisation.rs` n'ont, et il est déclaré comme tel. Le calcul du délai vit
dans un module **pur**, testable sur l'hôte ; le socket, lui, ne l'est pas.

⚠️ **Exception nommée** : un refus `version` **ne se réessaie pas** (D4).

### E10 — `session.vm_id` : P3 le remplit, mais le relais n'apprend rien de la base

Legs n°3 de P2 : « `session.vm_id` reste entièrement NULL ». P3 le remplit — le
préfixe du nom de session désigne la VM. Mais
`ObservateurDeSession.apparie(nomSession, utilisateurId?)`
(`plateforme/src/signaling/relais.ts:104`) est **synchrone et sans retour**, à
dessein.

**Décision : la résolution se fait dans `signaling/trace.ts`**, qui est déjà
l'implémentation de production de cet observateur et déjà le seul à connaître la
base (`trace.ts:42-45`). Le relais ne gagne **aucune** connaissance de la base,
et `apparie` ne change pas de signature.

⚠️ **`session.vm_id` reste NULLABLE**, et pour la même raison que
`utilisateur_id` : `0001-socle.sql:64-75` explique qu'une session appariée sans
préfixe connu n'a rien d'honnête à inscrire. **`NOT NULL` serait FAUX, pas
seulement coûteux.**

### E11 — La ROUGE gratuite du critère ① ne passe PAS par le message de refus

Le motif « un agent est déjà connecté à la session … » vit dans
`plateforme/src/signaling/appariement.ts:55`, il est **figé mot pour mot** par
`appariement.test.ts:12-13`, et il est **lu par les pairs** — `agent/src/signaling.rs:130`, relu et juste, et
**`client/src/webrtc.ts:130-131`**.

> ❌ **CE « RELU ET JUSTE » A ÉTÉ RENDU FAUX PAR L'EXÉCUTION DE CE PLAN
> LUI-MÊME** (revue transverse de fin de branche, 19 août 2026). Le commit
> `5fbc89b` — la tâche 19, celle qui met le jeton dans les deux poignées de
> main — a poussé cette lecture de la ligne **130 à la ligne 139**. La
> citation était **exacte quand ce plan a été écrit**, et fausse quand il a
> fini de s'exécuter.
>
> ⚠️ **C'est une leçon que ce dépôt n'avait pas encore formulée ainsi** :
> relire une citation `fichier:ligne` AVANT d'écrire ne suffit pas quand le
> plan qui la porte prescrit par ailleurs de déplacer la ligne citée. Il faut
> la relire **après avoir exécuté ce qui la déplace**. Corrigé aux deux places
> du dépôt qui la portaient — `plateforme/src/signaling/appariement.ts` et
> `appariement.test.ts` —, énumérées par `grep -n` avant l'édition.

⚠️ **Le commentaire d'`appariement.ts:48-51` qui porte cette liste cite, lui,
`client/src/webrtc.ts:109`, et CE NUMÉRO A DÉRIVÉ** — la l. 109 est **vide**, la
lecture réelle vit aux l. 130-131 (`message.type === 'error'`, puis
`message.reason`). **Relevé au passage et NON corrigé ici** : `client/` est le
périmètre d'un travail concurrent, et c'est une dette d'une ligne pour la revue
transverse (tâche 24). La propriété énoncée, elle, reste **VRAIE** : un pair lit
bien ce motif.

**P3 ne le change pas.** Deux agents préfixés différemment ne se rencontrent
plus dans la même entrée de la table d'appariement : ils ne produisent donc plus
ce message, **sans qu'une ligne d'`appariement.ts` ait bougé**. La rouge se joue
en donnant aux deux agents simulés le **même** préfixe vide — c'est-à-dire le
comportement du binaire de P2.

### E12 — Un test livré par P2 affirme littéralement l'inverse de ce que P3 livre

`plateforme/src/signaling/garde-fil.test.ts:204-214`, relu :

```ts
it('un pair `agent` SANS jeton est toujours accepté — la fenêtre de E2', async () => {
    // ⚠️ Ce test EXISTE pour rendre visible ce que P2 ne ferme PAS : le
    // rôle `agent` demeure un chemin anonyme vers des identifiants TURN de
    // 24 h. L'exiger casserait le chantier D en cours. Le jour où P3
    // l'inversera, il faudra le réécrire À DESSEIN, pas par surprise.
```

**C'est la seule assertion de P1/P2 que P3 réécrit**, et son auteur l'a prévu.
Comme la tâche 12 de P2, **la tâche qui la réécrit ne se parallélise avec rien,
et son `git diff` est une pièce du document de résultats.**

### E13 — `application` est la seconde table manquante, et elle naît en P3 avec sa contrainte

Relevé : le schéma v1 de la spec §5 compte sept tables ; `agent_enrole` et
`application` **n'existent nulle part** dans `plateforme/`.

**Décision : les deux naissent dans `0003-agents.sql`.** `application` reste
**vide** — son chemin d'écriture est ④ — mais elle naît **avec sa clé étrangère
vers `vm(id)`**, en application directe du legs n°2 de P1 : « toute contrainte
doit naître avec sa table », SQLite ne sachant pas l'ajouter par `ALTER TABLE`
(mesuré : `near "CONSTRAINT": syntax error`).

### E14 — `proto/src/control.rs` est à 470 lignes, marge 30, et P3 n'y touche pas

Relevé par la commande. Ce n'est pas une contrainte que P3 subit — c'est la
raison pour laquelle **le canal `/agent` vit dans un fichier neuf** plutôt que
d'être une variante de plus d'`AgentControl`. La décision était déjà celle de la
spec (§3.3 : « un module `plateforme` avec sa propre constante de version ») ; le
chiffre la corrobore.

### E15 — Le déni de service en une trame reste OUVERT, et le commentaire qui le dit doit être PRÉCISÉ, pas corrigé

Relevé : la garde court sur le **premier message**, donc **après**
`JSON.parse` (`relais.ts:147`) et `isJsonObject` (`relais.ts:158`), et
l'appel de garde est à `relais.ts:189`. P3 **réduit** la surface anonyme —
plus aucun `ice-config` ne part sans jeton, quel que soit le rôle — mais **ne
ferme pas** la possibilité d'ouvrir un socket et d'envoyer une trame. Le frein
est P5 ③.

**Décision : `resilience.test.ts` garde sa raison d'être**, et le commentaire de
`resilience.test.ts:3-4` (« un déni de service en une trame, **sans
authentification requise** ») sera **PRÉCISÉ** par la revue transverse, pas
corrigé — c'est exactement le sort qu'a reçu la place n°3 de la revue
transverse de P2, et pour la même raison : la phrase reste vraie, seule sa
portée change.

### E16 — La création d'un enrôlement n'a ni point d'entrée ni convention

Même lacune qu'E11 de P2 pour les comptes humains. **Décision : le précédent est
repris à la lettre** — `npm run admin:agent -- --vm <nom> --adresse <hôte>`,
**secret tiré au sort par la commande et écrit UNE SEULE FOIS sur stdout**,
jamais relisible ; un `--secret` en argument est **refusé explicitement avec son
motif** (assertion de test), parce que `ps` expose la ligne de commande de tout
processus. La garde de point d'entrée de
`plateforme/src/admin/creer-utilisateur.ts:115-117` est recopiée : sans elle, un
import depuis un test lancerait la commande.

---

## Structure des fichiers

```
proto/
  src/plateforme.rs              NEUF — PLATEFORME_VERSION, les messages, verifie_version
  src/lib.rs                     MODIFIÉ — une ligne : pub mod plateforme;
  ts/plateforme.ts               NEUF — le miroir
  ts/plateforme.test.ts          NEUF
  plateforme-vectors.json        NEUF — version vérifiée des DEUX côtés (E8)

plateforme/
  tsconfig.json                  MODIFIÉ — include gagne ../proto/ts/**/*.ts (E7)
  package.json                   MODIFIÉ — un script : admin:agent
  src/agents/prefixe.ts          NEUF — PUR : génération, découpage, composition
  src/agents/prefixe.test.ts     NEUF
  src/agents/enrolement.ts       NEUF — vérification du secret, refus INDISTINCT
  src/agents/enrolement.test.ts  NEUF
  src/agents/fraicheur.ts        NEUF — PUR : seuil et état, horloge en paramètre
  src/agents/fraicheur.test.ts   NEUF
  src/agents/canal.ts            NEUF — la boucle du canal /agent
  src/agents/canal.test.ts       NEUF
  src/admin/enroler-agent.ts     NEUF
  src/admin/enroler-agent.test.ts NEUF
  src/base/migrations/0003-agents.sql  NEUF — agent_enrole + application
  src/depot/agent.ts             NEUF
  src/depot/agent.test.ts        NEUF
  src/depot/session.ts           MODIFIÉ — ouvrirSession gagne vmId?
  src/identite/jeton.ts          MODIFIÉ — le claim de type (E5)
  src/identite/garde.ts          MODIFIÉ — le rôle agent exige son jeton
  src/http/serveur.ts            MODIFIÉ — la seconde branche de montée
  src/signaling/trace.ts         MODIFIÉ — session.vm_id (E10)
  src/signaling/garde-fil.test.ts MODIFIÉ — la réécriture d'E12

agent/
  src/superviseur/table/…        NEUF — l'extraction de la tâche 17
  src/plateforme.rs              NEUF — le client du canal, avec sa reprise
  src/plateforme/repli.rs        NEUF — PUR : le calcul du délai de reprise
  src/superviseur/protocole.rs   MODIFIÉ — la composition du nom de session
  src/superviseur/table.rs       MODIFIÉ — le préfixe devant w-{n}
  src/superviseur.rs             MODIFIÉ — l'enrôlement précède la signalisation
  src/superviseur/signalisation.rs  MODIFIÉ — le jeton dans la poignée de main
  src/signaling.rs               MODIFIÉ — idem
  src/main.rs                    MODIFIÉ — AGENT_VM, AGENT_SECRET

client/
  src/prefixe.ts                 NEUF — PUR, sans DOM (E3)
  src/prefixe.test.ts            NEUF
  src/shell-page.ts              MODIFIÉ — le préfixe lu, plus de littéral

scripts/run-agent.sh             MODIFIÉ — deux lignes, tâche DÉDIÉE (D1)
```

⚠️ **`scripts/verify-all.sh` N'EST PAS MODIFIÉ** : ses neuf étapes couvrent déjà

<!-- ANNOTATION G1 (20 août 2026) — « ses neuf étapes » EST FAUX : il en compte
DIX. Relevé par la commande, `grep -c '^etape "' scripts/verify-all.sh` → 10,
au 19 août puis au 20 août 2026. La dixième est
`client : npm run design:verifier` (`scripts/verify-all.sh:73`), ajoutée par le
sous-projet ⑥ APRÈS la rédaction de ce plan. La conclusion de la phrase — le
script n'est pas modifié — reste vraie, et G1 ne l'a pas modifié non plus. -->
`proto/` (1, 5, 6), `plateforme/` (7, 8, 9), `client/` (3, 4). Relevé, pas
supposé.

> ❌ **« SES NEUF ÉTAPES » EST FAUX, et la numérotation qui suit l'est avec —
> corrigé à la clôture de P3 (19 août 2026), par la commande.** Le script
> compte **DIX** appels `etape` : `cargo test --workspace` (1),
> `cargo clippy --workspace` (2), `client : npm test` (3),
> `client : npm run typecheck` (4), **`client : npm run design:verifier` (5,
> ajoutée par le sous-projet ⑥ / sous-bloc S1)**, `proto : npm test` (6),
> `proto : npm run typecheck` (7), `plateforme : npm run test:sqlite` (8),
> `plateforme : npm run test:postgres` (9), `plateforme : npm run typecheck`
> (10). **La CONCLUSION du paragraphe survit sans une retouche** : `proto/`
> est bien couvert (6, 7), `plateforme/` aussi (8, 9, 10), `client/` aussi
> (3, 4, 5), et P3 n'a effectivement pas modifié ce script.
>
> ⚠️ **Et « dix » ne suffit pas non plus à décrire ce qu'on lit** : une
> exécution complète affiche **DIX-SEPT** en-têtes `==>` — dix du script, et
> **sept** de l'intérieur de l'étape 5 (un `npm run build`, puis les six
> contrôles du socle ; le septième contrôle, §7.5, est un test unitaire et
> tourne dans l'étape 3). **Deux comptes vrais de deux choses différentes.**

---

## Interfaces partagées

```ts
// proto/ts/plateforme.ts — miroir exact de proto/src/plateforme.rs
export const PLATEFORME_VERSION = 1;

export type VersLaPlateforme =
    | { v: number; type: 'enroler'; vm: string; secret: string }
    | { v: number; type: 'battement' };

export type DepuisLaPlateforme =
    | { v: number; type: 'enrole'; prefixe: string; jeton: string; expire_a: number }
    | { v: number; type: 'battement-recu'; jeton: string; expire_a: number }
    | { v: number; type: 'refus'; motif: MotifCanal };

export type MotifCanal = 'version' | 'forme' | 'enrolement' | 'sequence';
```

```ts
// plateforme/src/agents/prefixe.ts — PUR
export const SEPARATEUR = ':';
export const OCTETS_PREFIXE = 16;              // 128 bits, spec §3.4
export function nouveauPrefixe(): string;      // base64url, 22 caractères
export function composer(prefixe: string, nom: string): string;
export function decouper(session: string): { prefixe: string; nom: string };
```

```ts
// plateforme/src/agents/fraicheur.ts — PUR, horloge en paramètre
export const SEUIL_INJOIGNABLE_MS = 90_000;    // NON CALIBRÉ
export type EtatAgent = 'prete' | 'injoignable';
export function etatDe(vuA: number | null, maintenant: number): EtatAgent;
```

```ts
// plateforme/src/depot/agent.ts
export interface LigneAgent {
    vm_id: string; empreinte_secret: string; prefixe_session: string; vu_a: number | null;
}
export async function enroler(p: Pilote, vmId: string, empreinte: string, prefixe: string): Promise<void>;
export async function lireParVm(p: Pilote, vmId: string): Promise<LigneAgent | undefined>;
export async function lireParPrefixe(p: Pilote, prefixe: string): Promise<LigneAgent | undefined>;
export async function marquerVu(p: Pilote, vmId: string, maintenant: number): Promise<void>;
```

```ts
// plateforme/src/identite/jeton.ts — la SEULE modification de signature
export type TypeSujet = 'utilisateur' | 'agent';
export function signer(
    sujet: string, secret: string, maintenant: number,
    dureeMs?: number, type?: TypeSujet,          // défaut 'utilisateur'
): string;
export type VerdictJeton =
    | { ok: true; sujet: string; type: TypeSujet }   // 'utilisateur' si le claim est absent
    | { ok: false; motif: MotifJeton };
```

```rust
// proto/src/plateforme.rs
pub const PLATEFORME_VERSION: u8 = 1;
#[derive(Debug, Clone, Serialize)] #[serde(tag = "type", rename_all = "kebab-case")]
pub enum VersLaPlateforme { Enroler { v: u8, vm: String, secret: String }, Battement { v: u8 } }
#[derive(Debug, Clone, Deserialize)] #[serde(tag = "type", rename_all = "kebab-case")]
pub enum DepuisLaPlateforme { /* v vérifié par verifie_version sur CHAQUE variante */ }
```

```rust
// agent/src/plateforme/repli.rs — PUR
pub const REPLI_MIN_MS: u64 = 500;
pub const REPLI_MAX_MS: u64 = 30_000;          // NON CALIBRÉS
pub fn delai_de_repli(tentative: u32) -> u64;
```

---

## Ordre et parallélisme

| Famille | Tâches | Dépend de | Parallélisable | Paquets touchés |
| --- | --- | --- | --- | --- |
| 1 — le préfixe, **pur** | 4 | rien | avec tout | `plateforme/` |
| 2 — la persistance | 6, 7, 8 | 6 ← rien ; 7 et 8 ← 6 | 7 et 8 entre elles | `plateforme/` |
| 3 — l'identité de l'agent | 9, 10, 11, 12 | 9 ← rien ; 10 ← 4, 9 ; 11 ← 7 ; 12 ← 7, 11 | 9 et 11 avec la famille 1 | `plateforme/` |
| 4 — le canal `/agent` | 13, 14, 15, 16 | 13 ← rien ; 15 ← rien ; 14 ← 11, 13, 15, F0 ; 16 ← 4, 7 | 13, 15, 16 entre elles | `plateforme/` |
| 0 — le protocole partagé | 1, 2, 3 | 1 et 2 ← rien ; 3 ← 1, 2 | 1 et 2 entre elles | 🔴 `proto/` |
| 6 — le navigateur | 21 | 4 | avec la famille 5 | 🔴 `client/` |
| 5 — l'agent Rust | 17, 18, 19, 20 | **17 ← rien, et PRÉCÈDE 19** ; 18 ← F0 ; 19 ← 17, 18 ; 20 ← 19 | 17 et 18 entre elles | 🔴 `agent/`, `scripts/` |
| 7 — recette et clôture | 22, 23, 24, 25, 26 | 22 ← tout sauf 23 ; 23 ← 22 ; 24, 25, 26 ← 22 | 24 et 25 entre elles | — |

**Chemin critique** : 6 → 7 → 11 → 14 → 22.
**Cinq tâches purement isolées peuvent démarrer ensemble au premier tour** :
4, 6, 9, 13, 15 — **et toutes les cinq sont dans `plateforme/`**, donc aucune
n'attend que le chantier microphone libère quoi que ce soit.

🔴 **La famille 0 est placée APRÈS les familles 1 à 4 dans l'ordre d'exécution
bien qu'elle ne dépende de rien** : c'est elle qui touche `proto/`, et le
chantier concurrent y a commité trois fois. Relancer `git log --oneline -5 --
proto/` avant de la démarrer.

🔴 **La tâche 17 (extraction de `table.rs`) précède la tâche 19 sans exception**,
et n'ajoute **aucune** ligne de comportement.

⚠️ **La tâche 16 est la seule qui touche un test livré par P2** (E12) : elle ne
se parallélise avec rien, et son `git diff` est une pièce du document de
résultats.

⚠️ **IL N'Y A PAS DE TÂCHE 5, et c'est déclaré plutôt que découvert.** Une
première rédaction séparait le préfixe côté navigateur (`client/src/prefixe.ts`)
du préfixe côté plateforme ; ils ont été réunis dans la **tâche 21**, qui touche
`client/` et attend donc que le chantier concurrent libère ce paquet. **Les
numéros ne sont PAS renumérotés** : ils sont cités dans le tableau ci-dessus, dans
la §« Structure des fichiers » et dans quatre tâches, et une renumérotation
tardive est exactement le geste par lequel une référence survit à ce qu'elle
désigne. **Vingt-cinq tâches, numérotées 1 à 26 sans le 5.**

---

# Famille 1 — le préfixe, pur

### Task 4 : le préfixe opaque, sa composition et son découpage

**Objet :** rendre l'espace de noms des sessions global par un préfixe de
128 bits non devinable, et fournir les deux fonctions pures qui le composent et
le découpent.

**Files:**
- Create: `plateforme/src/agents/prefixe.ts`, `plateforme/src/agents/prefixe.test.ts`
- Modify: aucun

**Interfaces:** Produces `SEPARATEUR`, `OCTETS_PREFIXE`, `nouveauPrefixe`,
`composer`, `decouper`.

- [ ] **Step 1 : écrire les tests d'abord, et les voir ROUGES**

| Test | Ce qui le rend ROUGE |
| --- | --- |
| `nouveauPrefixe()` rend 22 caractères de l'alphabet `base64url` | encoder en `base64` ordinaire : `+` et `/` apparaissent, et `/` casserait le découpage d'une URL un jour |
| deux appels **diffèrent** | fixer la graine au lieu de tirer de `randomBytes` |
| 🔴 le préfixe ne contient **jamais** le séparateur | c'est l'assertion sur laquelle repose D2 tout entier : la rendre vraie par construction, puis la muter en autorisant `:` dans l'alphabet, fait tomber le test **et** celui du découpage |
| `composer(p, 'bureau')` rend `` `${p}:bureau` `` | — |
| `decouper('P:bureau')` rend `{ prefixe: 'P', nom: 'bureau' }` | découper sur le **dernier** `:` au lieu du premier : `P:w-1` passe encore, mais un nom qui porterait un `:` casserait |
| `decouper('bureau')` — **sans préfixe** — rend `{ prefixe: '', nom: 'bureau' }` | lever, ou rendre `undefined` : c'est le comportement du mode d'essai local (spec §10), et il doit être **explicite** |
| 🔴 `deriverIdentifiants(secret, composer(p,'bureau'), 3600, 1_000_000)` rend un `username` dont le **premier** segment est `4600` | c'est le contrôle d'E6. La mutation qui le rougit : composer le nom TURN en mettant l'expiration **après** la session |

⚠️ **Le dernier test importe `deriverIdentifiants` de `signaling/ice.ts` sans le
modifier.** Il fige la chaîne exacte, sur le modèle d'`ice.test.ts:13`, qui vaut
`expect(username).toBe('4600:ma-session')`.

- [ ] **Step 2 : implémenter.** `randomBytes(OCTETS_PREFIXE).toString('base64url')`.
- [ ] **Step 3 : voir vert.** Compte de tests attendu : **7**, **annoncé avant
      d'être lu**.

---

# Famille 2 — la persistance

### Task 6 : la migration `0003-agents.sql`, et les DEUX gardes de P1 qui doivent la voir

**Objet :** créer `agent_enrole` et `application`, chacune avec **toutes** ses
contraintes, dans le sous-ensemble portable.

**Files:**
- Create: `plateforme/src/base/migrations/0003-agents.sql`
- Modify: `plateforme/src/base/pilotes.test.ts` (la liste des versions attendues)

- [ ] **Step 1 : écrire la migration, et la faire voir ROUGE par les gardes**

Deux gardes de P1 doivent l'examiner, et **il faut vérifier qu'elles le font**
avant de croire qu'elles la protègent :

1. **le lint statique** — `plateforme/src/base/sous-ensemble.test.ts`, qui lit
   `readdirSync(REPERTOIRE).filter(f => f.endsWith('.sql'))` (l. 61) : le
   fichier neuf y entre **tout seul**, sans modifier le test ;
2. **la double passe** — `pilotes.test.ts:33` fige `toEqual([1, 2])` ; il faut y
   écrire **`[1, 2, 3]`**. ⚠️ **C'est une assertion anti-tautologie voulue**
   (motif l. 28-32) : elle **doit** être mise à jour à la main, sinon la
   migration neuve passerait inaperçue.

🔴 **Trois rouges à jouer, et chacune est un piège que P1 a payé** :

| Mutation | Ce qui doit rougir |
| --- | --- |
| écrire `vu_a INTEGER` au lieu de `BIGINT` | le lint `/\b\w+_a\s+INTEGER\b/i` (`sous-ensemble.test.ts:45-48`). **C'est le défaut qui empêchait le service de démarrer sur Postgres** |
| écrire un `DEFAULT 'x'` | le lint `expect(sql).not.toMatch(/['"]/)` (`:83-88`), **et** `rendreMarqueurs` qui lève |
| nommer un horodatage `derniere_vue` au lieu de `vu_a` | **RIEN NE ROUGIT** — et c'est le point : la portée du lint est bornée par la convention de nommage `_a` (leg n°8 de P1). ⚠️ **Cette rouge est à jouer PRÉCISÉMENT parce qu'elle ne rougit pas** : elle documente la limite, et son résultat attendu est « aucun test ne tombe » |

- [ ] **Step 2 : le schéma**

```sql
CREATE TABLE agent_enrole (
    vm_id            TEXT PRIMARY KEY REFERENCES vm(id),
    empreinte_secret TEXT NOT NULL,
    prefixe_session  TEXT NOT NULL UNIQUE,
    vu_a             BIGINT NULL
);

CREATE TABLE application (
    id     TEXT PRIMARY KEY,
    vm_id  TEXT NOT NULL REFERENCES vm(id),
    nom    TEXT NOT NULL,
    chemin TEXT NOT NULL,
    vue_a  BIGINT NOT NULL
);
```

⚠️ **`application` naît AVEC sa clé étrangère et reste vide** (E13) : legs n°2 de
P1, SQLite ne sachant pas ajouter une contrainte par `ALTER TABLE`. Le
commentaire de la migration doit le **dire**, sur le modèle de
`0001-socle.sql:23-36`, pour qu'un lecteur de ④ ne croie pas à un oubli.

⚠️ **`prefixe_session` est `UNIQUE`** — c'est ce qui rend `lireParPrefixe`
décidable, et c'est le seul index dont le canal dépend.

- [ ] **Step 3 : la double passe, les DEUX moteurs**

```bash
cd plateforme && npm run test:sqlite && npm run test:postgres
```

**Un saut est un échec.** Si l'instance manque :
`docker compose -f docker-compose.plateforme.yml up -d`.

---

### Task 7 : le dépôt `agent_enrole`

**Objet :** enrôler, relire par VM et par préfixe, et marquer `vu_a`.

**Files:**
- Create: `plateforme/src/depot/agent.ts`, `plateforme/src/depot/agent.test.ts`
- Modify: aucun

**Interfaces:** Produces `LigneAgent`, `enroler`, `lireParVm`, `lireParPrefixe`,
`marquerVu`.

- [ ] **Step 1 : les tests d'abord, ROUGES**

| Test | Ce qui le rend ROUGE |
| --- | --- |
| `enroler` puis `lireParVm` rend la ligne | — |
| `lireParPrefixe` rend la même ligne | interroger sur `vm_id` : le préfixe ne retrouve rien |
| `lireParVm` / `lireParPrefixe` rendent `undefined` sur inconnu, **sans lever** | lever : le canal répondrait 500 là où il doit répondre un refus, et l'écart de comportement serait un oracle (précédent : `depot/utilisateur.ts:45-47`) |
| un second `enroler` sur le **même** préfixe est refusé par l'index unique | retirer `UNIQUE` de la migration |
| `marquerVu` écrit une **magnitude d'époque** et la relit | écrire `1_000` : le test passerait sur les deux moteurs sans rien prouver. **Employer `1_787_136_773_742`, comme `pilotes.test.ts:111`** |

⚠️ **`pg` rend les `BIGINT` en `string`** : la relecture fait `Number(...)`,
comme `pilotes.test.ts:100-140`.

- [ ] **Step 2 : implémenter**, en réemployant `baseNeuve` de
      `plateforme/src/base/harnais.ts:41`.
- [ ] **Step 3 : les deux moteurs.** Compte attendu : **5**, annoncé avant lecture.

---

### Task 8 : `ouvrirSession` gagne `vmId`

**Objet :** permettre à la trace d'écrire `session.vm_id`, legs n°3 de P2.

**Files:**
- Modify: `plateforme/src/depot/session.ts`, `plateforme/src/depot/session.test.ts`

- [ ] **Step 1 : le test d'abord, ROUGE.** Une ligne ouverte avec `vmId` le
      relit ; une ligne ouverte **sans** garde `vm_id` à `null`. **Rouge :**
      rendre le paramètre obligatoire — la session `bureau` d'un agent non
      enrôlé n'a rien à y mettre, et le test « sans » tombe.
- [ ] **Step 2 : implémenter.** `vmId?: string`, **après** `utilisateurId?`,
      pour ne pas déplacer les appelants existants.
- [ ] **Step 3 :** les deux moteurs, non-régression sur `137`.

---

# Famille 3 — l'identité de l'agent

### Task 9 : le *claim* de type, et les deux confusions qu'il ferme

**Objet :** distinguer un jeton d'agent d'un jeton humain, **sans invalider les
jetons émis par P2**.

**Files:**
- Modify: `plateforme/src/identite/jeton.ts`, `plateforme/src/identite/jeton.test.ts`

- [ ] **Step 1 : les tests d'abord, ROUGES**

| Test | Ce qui le rend ROUGE |
| --- | --- |
| `signer(s, secret, t)` sans type puis `verifierJeton` rend `type: 'utilisateur'` | rendre `undefined` : **tout jeton de P2 encore en vol deviendrait indécidable**, ce qui est une rupture que rien n'exige |
| `signer(s, secret, t, d, 'agent')` puis vérification rend `type: 'agent'` | — |
| 🔴 un jeton dont le *claim* `typ` a été **forgé** à `'agent'` sans la signature est refusé | ne pas inclure le *claim* dans la charge signée. **Le helper `forger` existe déjà** — `jeton.test.ts:29-32` — et il a été écrit pour la confusion d'algorithme ; il sert ici sans être réécrit |
| un `typ` de valeur inconnue est refusé (`motif: 'forme'`) | le laisser passer, ou le ramener à `'utilisateur'` : un jeton de type inconnu deviendrait un jeton humain |

⚠️ **Le nom du *claim* est `typ` et non `type`** : `typ` est déjà employé par
l'**en-tête** JWT (`jeton.ts:62`, `{ alg: ALGORITHME, typ: 'JWT' }`), et le
mettre dans la **charge** sous le même nom serait une source de confusion pour
un relecteur. **Choisir `sty` (sujet-type) ou `role`, et l'écrire dans le
commentaire de tête** — la décision appartient à l'implémenteur, la contrainte
est qu'elle soit **nommée**.

- [ ] **Step 2 : implémenter.** Le *claim* entre dans la charge signée. La
      durée reste `DUREE_JETON_ACCES_MS` (600 000 ms) — **NON CALIBRÉE**, et le
      commentaire le dit.
- [ ] **Step 3 :** non-régression sur les 137. Compte attendu : **+4**.

---

### Task 10 : la garde exige un jeton d'agent, et que son sujet PRÉFIXE la session

**Objet :** fermer E2 — et fermer, du même geste, la possibilité qu'un agent
authentifié occupe la session d'une autre VM.

**Files:**
- Modify: `plateforme/src/identite/garde.ts`, `plateforme/src/identite/garde.test.ts`

- [ ] **Step 1 : les tests d'abord, ROUGES.** Cinq, et **chacun a sa propre
      rouge, jamais une rouge pour deux assertions** :

| Test | Ce qui le rend ROUGE |
| --- | --- |
| 🔴 `{role:'agent'}` **sans jeton** est refusé, motif `jeton-absent` | c'est `garde.ts:72` d'aujourd'hui : `if (role === 'agent') return { ok: true };`. **La rouge est GRATUITE — le binaire de P2 la porte** |
| 🔴 un `{role:'agent'}` avec jeton valide **mais sans `ice-config`**… *(non : voir la note)* | — |
| `{role:'agent', jeton d'agent de sujet P}` sur `P:bureau` est accepté | — |
| 🔴 le **même** jeton sur `Q:bureau` est refusé, motif `session-refusee` | omettre la comparaison de préfixe : un agent enrôlé occuperait la session de toute autre VM. **C'est le pendant agent du critère ③ de P2** |
| 🔴 un jeton **humain** valide présenté en `{role:'agent'}` est refusé | omettre `type === 'agent'` (E5, sens 1) |
| 🔴 un jeton **d'agent** valide présenté en `{role:'client'}` est refusé | omettre `type !== 'agent'` (E5, sens 2) |

⚠️ **La deuxième ligne est barrée à dessein** : l'absence d'`ice-config` ne se
juge **pas** dans `garde.test.ts`, qui ne connaît pas de socket. Elle se juge
dans `garde-fil.test.ts` (tâche 16). **Écrire ici une assertion sur
`ice-config` produirait un contrôle vacueux** — exactement le patron ①A-bis de
P2, où une assertion de fuite vivait dans un test qui ne pouvait pas la voir.

- [ ] **Step 2 : implémenter.** La garde reste **pure et synchrone** (E4) : elle
      compare `session.startsWith(sujet + SEPARATEUR)`, et rien d'autre. Elle ne
      lit **pas** `agent_enrole` — c'est le canal qui a établi l'identité, la
      garde ne fait que la relire dans le jeton.

🔴 **Le message rendu sur le fil ne distingue pas les causes.** `Verdict`
(`garde.ts:47-49`) porte déjà `message` (sur le fil) et `journal` (local) — deux
textes distincts et délibérés, anti-oracle. **Le respecter** : « accès refusé à
la session demandée » pour les deux refus de préfixe et de type.

- [ ] **Step 3 :** compte attendu **+5**, annoncé avant lecture.

---

### Task 11 : l'enrôlement, et le refus qui n'énumère pas

**Objet :** vérifier un secret d'enrôlement contre son empreinte, et refuser
**sans distinguer** « VM inconnue » de « secret faux » — critère ②.

**Files:**
- Create: `plateforme/src/agents/enrolement.ts`, `plateforme/src/agents/enrolement.test.ts`

- [ ] **Step 1 : les tests d'abord, ROUGES**

| Test | Ce qui le rend ROUGE |
| --- | --- |
| bon couple (vm, secret) → `{ ok: true, prefixe }` | — |
| 🔴 **VM inconnue** et **secret faux** rendent le **même** refus, mot pour mot | rendre `'vm-inconnue'` d'un côté et `'secret-invalide'` de l'autre : **c'est un oracle d'énumération**, et c'est littéralement ce que la spec §4 P3 ② nomme comme la ROUGE de ce critère |
| le refus est **journalisé** avec le nom de VM demandé | ne rien journaliser : le refus devient indiagnosticable |
| une empreinte tronquée en base rend `false` **sans lever** | appeler `timingSafeEqual` sans comparer les longueurs d'abord. **Mesuré en P2** : il lève `Input buffers must have the same byte length`, et l'appelant répondrait 500 là où il doit répondre un refus — l'écart serait à lui seul un oracle |

⚠️ **L'empreinte du secret réemploie `identite/mot-de-passe.ts`** — format
`scrypt$N$r$p$sel$empreinte` (`mot-de-passe.ts:3`), donc `verifier` gère déjà
la comparaison de longueur (E9 de P2) et `doitEtreRehache` existe déjà. **Ne pas
réécrire une seconde dérivation** : deux formats d'empreinte dans le même
service divergeraient.

⚠️ **`verifier` LÈVE sur un algorithme inconnu** (`mot-de-passe.ts:109-113`),
délibérément — un refus muet y serait indiscernable d'un secret faux.
`enrolement.ts` doit donc **laisser passer cette exception** vers le canal, qui
la traduit en refus `enrolement` et la journalise avec sa cause.

- [ ] **Step 2 : implémenter.** Une seule fonction asynchrone qui lit
      `lireParVm`, appelle `verifier`, et rend `{ok, prefixe}` ou le refus
      unique.
- [ ] **Step 3 :** les deux moteurs. Compte attendu : **4**.

---

### Task 12 : `npm run admin:agent`, et le secret qui ne s'écrit qu'une fois

**Objet :** créer un enrôlement en ligne de commande, sur le précédent exact
d'E11 de P2.

**Files:**
- Create: `plateforme/src/admin/enroler-agent.ts`, `plateforme/src/admin/enroler-agent.test.ts`
- Modify: `plateforme/package.json` (un script)

- [ ] **Step 1 : les tests d'abord, ROUGES**

| Test | Ce qui le rend ROUGE |
| --- | --- |
| `analyserArguments(['--vm','w1','--adresse','192.168.3.2'])` rend la paire | — |
| 🔴 `--secret <valeur>` est **refusé explicitement**, code 2, avec son motif | l'accepter : `ps` expose la ligne de commande de tout processus de la machine. **C'est la rouge la plus utile du fichier**, et elle est l'exact jumeau de `DRAPEAUX_INTERDITS` (`creer-utilisateur.ts:32`) |
| le secret est **tiré au sort** et écrit sur stdout **une seule fois** | le dériver du nom de VM : il deviendrait devinable |
| l'empreinte va en base, **jamais le secret** | écrire le clair : le test relit la colonne et le trouve |

- [ ] **Step 2 : implémenter.** `randomBytes(32).toString('base64url')` pour le
      secret ; `hacher` de `mot-de-passe.ts` pour l'empreinte ;
      `nouveauPrefixe()` pour le préfixe. **La garde de point d'entrée de
      `creer-utilisateur.ts:115-117` est recopiée** — sans elle, un import
      depuis un test lancerait la commande.
- [ ] **Step 3 :** `npm run admin:agent -- --vm essai --adresse 127.0.0.1`
      contre une base sqlite jetable, sortie relevée.

---

# Famille 4 — le canal `/agent`

### Task 13 : la seconde branche de montée WebSocket

**Objet :** router `/agent` vers son propre `WebSocketServer`, sans toucher au
relais.

**Files:**
- Modify: `plateforme/src/http/serveur.ts`, `plateforme/src/http/serveur.test.ts`

Le routage existe déjà et a été écrit **pour cela** — `serveur.ts:3-4` dit :
« P3 y ajoutera `/agent` sans toucher au relais ». Le handler
(`serveur.ts:85-100`) extrait le chemin puis refuse tout ce qui n'est pas `/`
par un `404` écrit à la main.

- [ ] **Step 1 : les tests d'abord, ROUGES.** `serveur.test.ts` porte déjà
      `tenter(url, borneMs)` (l. 53) et un test « refus de montée sur chemin
      inconnu » (l. 79).

| Test | Ce qui le rend ROUGE |
| --- | --- |
| `ws://…/agent` **monte** | ne pas ajouter la branche : le `404` d'aujourd'hui la ferme |
| `ws://…/` monte toujours | remplacer la comparaison au lieu de l'étendre |
| 🔴 `ws://…/inconnu` est **toujours** refusé | remplacer le `if (chemin !== '/')` par un `if (chemin === '/inconnu')` : le service deviendrait ouvert à tout chemin. **Le test l. 79 existe déjà et doit rester vert** |

- [ ] **Step 2 : implémenter.** Un second `new WebSocketServer({ noServer: true })`
      et une branche. **Deux chemins, pas une table de routage** : une table
      pour deux entrées serait de l'abstraction non payée.
- [ ] **Step 3 :** non-régression sur les 137.

---

### Task 14 : la boucle du canal — enrôlement, battement, jeton frais

**Objet :** l'unique consommateur du protocole `plateforme`.

**Files:**
- Create: `plateforme/src/agents/canal.ts`, `plateforme/src/agents/canal.test.ts`
- Modify: `plateforme/src/http/serveur.ts` (le câblage)

- [ ] **Step 1 : les tests d'abord, ROUGES.** Style de `garde-fil.test.ts` :
      de **vrais** `WebSocket`, port `0` attribué par le système, horloge
      injectée par une `let maintenant` mutable (`garde-fil.test.ts:28`).

| Test | Ce qui le rend ROUGE |
| --- | --- |
| `enroler` avec le bon secret rend `enrole` avec préfixe et jeton | — |
| 🔴 le jeton rendu est **vérifiable par la garde** et de type `agent` | signer sans le type : la garde de la tâche 10 le refuse. **Ce test traverse deux modules à dessein** — c'est le seul endroit où la chaîne complète est éprouvée |
| 🔴 `enroler` au mauvais secret rend `refus` **et ferme le socket** | ne pas fermer : un pair refusé garderait un socket ouvert et pourrait réessayer sans limite, ce qui est le déni de service que P5 doit freiner |
| 🔴 un message de version **`PLATEFORME_VERSION + 1`** est refusé, motif `version` | omettre la vérification côté TypeScript : le message passe. **C'est la moitié TS du critère ③** |
| `battement` avant `enroler` est refusé, motif `sequence` | l'accepter : un anonyme ferait avancer `vu_a` d'une VM qu'il n'a pas enrôlée |
| `battement` après `enroler` avance `vu_a` **en base** | ne rien écrire : `vu_a` reste `null` et l'agent est éternellement injoignable |
| `battement` rend un jeton **dont l'expiration est postérieure** au précédent | rendre le même jeton : l'agent tomberait à l'expiration du premier, dix minutes après l'enrôlement, sans le voir venir |

⚠️ **Le refus ne se journalise pas avec le secret.** Le balayage du critère ④ de
P2 (« chercher **le champ**, pas la valeur ») s'applique tel quel : le test
balaie les traces à la recherche du **nom** `secret`.

- [ ] **Step 2 : implémenter.** L'écriture de `vu_a` suit la règle de P1 : elle
      est lancée **sans être attendue**, avec un `.catch` qui journalise et
      n'interrompt rien — *« une promesse rejetée dans un gestionnaire
      d'événement `ws` abat tout le process Node »*
      (`CLAUDE.md`, section P1 ⑥). **Le battement est une observation, jamais
      une dépendance.**
- [ ] **Step 3 :** les deux moteurs. Compte attendu : **7**.

---

### Task 15 : la fraîcheur, pure, et la transition que le test DOIT voir

**Objet :** décider `prete` / `injoignable` à partir de `vu_a`, avec une horloge
en paramètre.

**Files:**
- Create: `plateforme/src/agents/fraicheur.ts`, `plateforme/src/agents/fraicheur.test.ts`

- [ ] **Step 1 : les tests d'abord, ROUGES**

| Test | Ce qui le rend ROUGE |
| --- | --- |
| `vu_a` **null** → `injoignable` | rendre `prete` : une VM jamais vue serait annoncée prête |
| `vu_a` récent → `prete` | — |
| 🔴 `vu_a` ancien de `SEUIL_INJOIGNABLE_MS + 1` → `injoignable` | rendre `prete` inconditionnellement |
| 🔴 **la TRANSITION est observée** : un même `vu_a`, deux instants, `prete` puis `injoignable` | 🔴 **C'est la ROUGE littérale du critère ④ de la spec** : « un seuil qui n'est jamais atteint dans le test ne prouve rien : le test **doit** voir la transition ». La mutation qui le rougit est de figer l'horloge — **exactement le piège que le critère ② de P2 a nommé pour les jetons** |

⚠️ **`SEUIL_INJOIGNABLE_MS = 90_000` N'EST PAS CALIBRÉE.** Elle rejoint la liste
déjà longue du dépôt — `BPP_MIN`, `FACTEUR_FOCUS`, `PART_DORMANTE_BPS`,
`HYSTERESIS`, `TAILLE_MAX_SORTIE`, `DUREE_JETON_ACCES_MS`, `DUREE_SECONDES` —
et le commentaire du fichier doit le dire. **Aucun jugement d'usage n'est porté
sur elle par ce plan.**

- [ ] **Step 2 : implémenter.** Pure, aucune base, aucun `Date.now()`.
- [ ] **Step 3 :** compte attendu **4**.

---

### Task 16 : `session.vm_id`, et la réécriture d'E12

**Objet :** remplir la colonne que P2 lègue, et **réécrire à dessein** le test
qui affirme l'inverse de ce que P3 livre.

**Files:**
- Modify: `plateforme/src/signaling/trace.ts`, `plateforme/src/signaling/trace.test.ts`,
  `plateforme/src/signaling/garde-fil.test.ts`

🔴 **Cette tâche ne se parallélise avec rien, et son `git diff` est une pièce du
document de résultats** — même statut que la tâche 12 de P2.

- [ ] **Step 1 : les tests d'abord, ROUGES**

| Test | Ce qui le rend ROUGE |
| --- | --- |
| une session `P:bureau` dont `P` est enrôlé écrit `session.vm_id` | ne pas résoudre : la colonne reste `null` |
| une session `bureau` **sans préfixe** laisse `vm_id` à `null` | lever, ou écrire une chaîne vide : le mode d'essai local casserait |
| une session à préfixe **inconnu** laisse `vm_id` à `null`, **et journalise** | écrire quand même : la colonne mentirait |
| 🔴 **la réécriture d'E12** : un pair `agent` **sans jeton** est refusé **et ne reçoit aucun `ice-config`** | 🔴 **DEUX assertions, donc DEUX rouges**, et l'ordre importe : `expect` interrompt le test à la première. **Écrire les deux assertions dans DEUX tests distincts**, pour que chacune soit éprouvée seule. C'est la leçon ①A-bis de P2, appliquée d'avance plutôt que découverte |

⚠️ **Le test réécrit doit poser `TURN_URL` / `TURN_SECRET`**, comme
`garde-fil.test.ts:33-41` le fait déjà en `beforeAll` : sans elles,
`configurationIce` rend `undefined` et l'assertion « aucun `ice-config` »
**serait incapable d'échouer**. Le commentaire l. 3-9 le dit déjà ; le respecter.

- [ ] **Step 2 : implémenter.** La résolution vit dans `trace.ts`, pas dans le
      relais (E10) : `decouper(nomSession)` puis `lireParPrefixe`.
- [ ] **Step 3 :** les deux moteurs.

---

# Famille 0 — le protocole partagé

⛔ **Relancer `git log --oneline -5 -- proto/` et `git status --porcelain`
avant de démarrer.**

### Task 1 : `proto/src/plateforme.rs`

**Objet :** le schéma du canal, côté Rust, avec sa constante de version vérifiée
à la désérialisation.

**Files:**
- Create: `proto/src/plateforme.rs`
- Modify: `proto/src/lib.rs` (une ligne : `pub mod plateforme;`)

- [ ] **Step 1 : les tests d'abord, ROUGES.** Dans le fichier, sur le modèle de
      `control.rs:310-322` :

| Test | Ce qui le rend ROUGE |
| --- | --- |
| un message bien formé se sérialise en JSON attendu, **chaîne exacte** | changer le `rename_all` : la chaîne diffère |
| 🔴 une version **absente** est rejetée | poser `#[serde(default)]` sur `v` — **et c'est le piège que `control.rs:37-42` documente en toutes lettres** : `default` court-circuiterait `deserialize_with` quand le champ est absent |
| 🔴 une version **`PLATEFORME_VERSION + 1`** est rejetée | omettre `verifie_version` sur **une** variante : le test qui porte sur cette variante-là tombe. **Écrire un test par variante entrante**, jamais un seul |
| un `type` inconnu est rejeté | — |

- [ ] **Step 2 : implémenter.** `verifie_version` copiée **dans sa forme** sur
      `control.rs:37-52`, y compris son commentaire d'avertissement.
- [ ] **Step 3 :** `cargo test -p proto`. **Relever le compte de départ AVANT**
      (le chantier concurrent a modifié `proto/`), et l'annoncer.

---

### Task 2 : `proto/ts/plateforme.ts`

**Objet :** le miroir TypeScript, et sa vérification de version.

**Files:**
- Create: `proto/ts/plateforme.ts`, `proto/ts/plateforme.test.ts`

Aucune déclaration n'est nécessaire : `proto/tsconfig.json:12` inclut
`["ts/**/*.ts"]` par glob, et `proto/package.json` lance `vitest run`.

- [ ] **Step 1 : les tests d'abord, ROUGES.** Modèle : `control.ts:130-139`.

| Test | Ce qui le rend ROUGE |
| --- | --- |
| `PLATEFORME_VERSION` vaut la même valeur que côté Rust | — *(ce n'est pas décidable ici ; c'est la tâche 3 qui le rend décidable)* |
| 🔴 une version `n+1` est rejetée | ne comparer que `parsed.type` : le message passe |
| une version **absente** est rejetée | comparer par `!=` au lieu de `!==` : `undefined != 1` est vrai, donc le test passerait encore — ⚠️ **vérifier que cette mutation rougit réellement avant de la déclarer** |
| un `type` inconnu est rejeté | — |

⚠️ **Le TS de `control.ts` valide à la frontière puis fait un `cast`**
(`return parsed as AgentControl`), pas une validation champ par champ. **Suivre
ce précédent**, et l'écrire, pour qu'un lecteur ne croie pas à une validation
plus forte qu'elle n'est.

- [ ] **Step 2 : implémenter.**
- [ ] **Step 3 :** `cd proto && npm test && npm run typecheck`.

---

### Task 3 : `proto/plateforme-vectors.json`, vérifié des DEUX côtés

**Objet :** le vecteur croisé, avec la lacune d'`input` corrigée pour le fichier
neuf (E8).

**Files:**
- Create: `proto/plateforme-vectors.json`
- Modify: `proto/src/plateforme.rs` (le test de conformité), `proto/ts/plateforme.test.ts` (idem)

- [ ] **Step 1 : les tests d'abord, ROUGES**

| Test | Ce qui le rend ROUGE |
| --- | --- |
| chaque cas se sérialise **et** se désérialise en Rust | modifier un octet du vecteur |
| chaque cas se sérialise en TypeScript | idem |
| 🔴 **le Rust vérifie `doc["version"]` contre `PLATEFORME_VERSION`** | l'omettre — c'est la lacune exacte d'`input.rs`, qui ne vérifie **pas** `doc["version"]` (seul `ts/input.test.ts:47` le fait) |
| 🔴 le TypeScript vérifie la même chose | l'omettre |
| 🔴 `cases` n'est pas vide | un fichier de vecteurs vide ferait passer toute la boucle : **anti-tautologie**, sur le modèle de `input.rs:326` et `sous-ensemble.test.ts:64-70` |

⚠️ **Il n'existe AUCUN générateur de vecteurs dans ce dépôt** (`grep -rn
"vectors"` rend six occurrences, toutes des lecteurs). **Le fichier s'écrit à la
main**, et son commentaire de tête dit — comme `vectors.json:2` — que toute
modification doit être répercutée des deux côtés.

---

# Famille 6 — le navigateur

⛔ **`client/src/webrtc.test.ts` est modifié dans l'arbre par le chantier
concurrent. Relancer `git status --porcelain` avant de démarrer.**

### Task 21 : le préfixe côté navigateur

**Objet :** que `shell-page.ts` cesse d'écrire `'bureau'` en dur.

**Files:**
- Create: `client/src/prefixe.ts`, `client/src/prefixe.test.ts`
- Modify: `client/src/shell-page.ts`

- [ ] **Step 1 : les tests d'abord, ROUGES.** `prefixe.ts` est **pur et sans
      DOM**, sur le précédent explicite de `client/src/jeton.ts` (en-tête
      l. 4-10) : le coffre est un **paramètre**, `globalThis.localStorage` n'est
      touché que dans le défaut d'argument.

| Test | Ce qui le rend ROUGE |
| --- | --- |
| coffre garni → le préfixe du coffre | — |
| coffre vide, `?prefixe=P` → `P` | inverser la priorité : un préfixe de requête écraserait celui du coffre à chaque rechargement |
| ni l'un ni l'autre → **chaîne vide** | lever, ou rendre `undefined` : la page casserait au lieu de retomber sur `bureau` (spec §10) |
| `composer('', 'bureau')` rend `'bureau'` | poser le séparateur inconditionnellement : la session deviendrait `':bureau'`, **qui n'est le nom d'aucune session existante**, et rien ne le signalerait |

🔴 **Le dernier test est le plus important du fichier.** C'est le seul qui
garantisse que le préfixe absent restitue **exactement** le comportement
d'aujourd'hui, et un `':bureau'` silencieux serait une panne muette de la classe
que ce dépôt combat.

⚠️ **N'écrire aucun test avec `Buffer`** : `client/` n'a pas `@types/node`, un
test qui l'emploie passe sous Vitest et **casse `npm run typecheck`**
(`TS2580`, mesuré en P2). Employer `btoa` / `TextEncoder`.

- [ ] **Step 2 : implémenter.** `shell-page.ts:11` cesse d'être un littéral ;
      `shell-page.ts:62` compose. **Ne pas toucher `client/src/main.ts`**, ni
      `webrtc.ts` : la session d'une fenêtre est déjà préfixée **par l'agent**,
      qui la nomme dans `fenetre-ouverte`.
- [ ] **Step 3 :** `cd client && npm test && npm run typecheck`. Compte attendu :
      **120 + 4**.

---

# Famille 5 — l'agent Rust

⛔ **Relancer `git log --oneline -5 -- agent/` et `git status --porcelain` avant
de démarrer.** ⛔ **Aucune de ces tâches n'emploie la VM Windows** — seule la
tâche 23 (corroboration) le fait.

### Task 17 : 🔴 EXTRACTION de `agent/src/superviseur/table.rs`, AVANT toute addition

**Objet :** rendre au fichier la marge que la tâche 19 va consommer. **Aucune
addition de comportement, aucune ligne de préfixe.**

**Files:**
- Create: `agent/src/superviseur/table/<extrait>.rs`
- Modify: `agent/src/superviseur/table.rs`

**Relevé, par la commande, le 19 août 2026 : 492 lignes, marge 8.**

- [ ] **Step 1 : relever le compte de tests Rust de départ**, sur un arbre dont
      on **nomme le commit** — `git worktree add` si l'arbre est sale.
      🔴 **Sans cette précaution, le compte n'est attribuable à personne**
      (piège D11, repayé par P2).
- [ ] **Step 2 : extraire, VERBATIM.** Le candidat naturel est le bloc de tests
      du fichier, sur le précédent de `superviseur/table.rs` lui-même, qui
      déclare déjà `#[path = "table/tests.rs"] mod tests;` et
      `#[path = "table/tests_relance.rs"] mod tests_relance;` — **c'est le même
      mécanisme, déjà employé dans ce fichier**, et la « Convention de module
      enfant » de `CLAUDE.md` range explicitement cet usage **hors de sa
      portée**. ⚠️ **Le choix exact du bloc appartient à l'implémenteur** ; la
      contrainte est que ce soit une **extraction verbatim**, comparée mot pour
      mot, et **jamais une compression de commentaire**.
- [ ] **Step 3 : relever le compte APRÈS**, et vérifier qu'il est **identique**.
      Une extraction qui change un compte de tests n'est pas une extraction.
- [ ] **Step 4 : `wc -l`**, et écrire la marge obtenue dans le message de commit.

---

### Task 18 : le client du canal `/agent`, avec sa reprise

**Objet :** ouvrir `/agent`, s'enrôler, battre le cœur, et **se reprendre** —
comportement neuf (E9).

**Files:**
- Create: `agent/src/plateforme.rs`, `agent/src/plateforme/repli.rs`
- Modify: `agent/src/main.rs` (déclaration de module)

- [ ] **Step 1 : les tests d'abord, ROUGES.** Seul `repli.rs` est testable sur
      l'hôte ; il est **pur**.

| Test | Ce qui le rend ROUGE |
| --- | --- |
| `delai_de_repli(0)` vaut `REPLI_MIN_MS` | — |
| le délai **croît** avec la tentative | rendre une constante |
| 🔴 le délai est **borné** par `REPLI_MAX_MS` | omettre le `min` : au bout de quelques dizaines de tentatives, l'agent attendrait des heures et la VM serait injoignable sans que rien ne le dise |
| `delai_de_repli` ne **déborde** pas sur une grande tentative | employer `1 << tentative` sans saturation : `u64` déborde à 64 |

⚠️ **`REPLI_MIN_MS` et `REPLI_MAX_MS` NE SONT PAS CALIBRÉES**, et le commentaire
le dit.

- [ ] **Step 2 : implémenter le client.** 🔴 **Un refus `version` ne se réessaie
      PAS** (D4) : il journalise en `warn!` avec la version reçue et rend la
      main. Toute autre chute se réessaie.
- [ ] **Step 3 :** `cargo check --target x86_64-pc-windows-gnu` depuis `agent/`
      — c'est l'acquis d'outillage de D3, et il couvre types, emprunts,
      visibilités et durées de vie **sans** l'édition de liens.

---

### Task 19 : le préfixe dans l'agent, et le jeton dans les deux poignées de main

**Objet :** les quatre sites de la spec §3.4 côté agent, plus les deux poignées
de main.

**Files:**
- Modify: `agent/src/superviseur/protocole.rs`, `agent/src/superviseur/table.rs`,
  `agent/src/superviseur.rs`, `agent/src/superviseur/signalisation.rs`,
  `agent/src/signaling.rs`, `agent/src/main.rs`

**Dépend de la tâche 17. Ne pas démarrer si `table.rs` est encore à 492.**

- [ ] **Step 1 : les tests d'abord, ROUGES.** `protocole.rs` et `table.rs` sont
      **hors `#[cfg(windows)]`** (`superviseur.rs:14-15`, relu), donc testables
      sur l'hôte. `signalisation.rs` est `#![cfg(windows)]` (l. 11) et ne l'est
      **pas** — le dire.

| Test | Ce qui le rend ROUGE |
| --- | --- |
| préfixe vide → `"bureau"` et `"w-1"`, **exactement comme aujourd'hui** | poser le séparateur inconditionnellement : `":bureau"` |
| préfixe `P` → `"P:bureau"` et `"P:w-1"` | — |
| 🔴 le compteur **ne recule toujours jamais** entre deux préfixes | remettre le compteur à zéro à la pose du préfixe : *« un identifiant réutilisé apparierait un message tardif du navigateur à la mauvaise fenêtre »* (`table.rs:163-165`, relu). **La propriété est acquise et documentée : P3 ne doit pas la perdre en passant** |

- [ ] **Step 2 : implémenter.** `SESSION_DE_CONTROLE` (`protocole.rs:26`) cesse
      d'être une constante nue ; `table.rs:239` et `:397` composent ;
      `superviseur.rs:29` passe le nom composé ; les deux poignées de main
      (`signaling.rs:59`, `signalisation.rs:35`) gagnent `"jeton"`.
      `main.rs::config()` lit `AGENT_VM` et `AGENT_SECRET`.
- [ ] **Step 3 :** `cargo test -p agent` (compte **annoncé avant lecture**) et
      `cargo check --target x86_64-pc-windows-gnu`.

---

### Task 20 : 🔴 les deux lignes de `scripts/run-agent.sh` — TÂCHE DÉDIÉE

**Objet :** transmettre `AGENT_VM` et `AGENT_SECRET` à l'agent. **Cette tâche ne
fait rien d'autre.**

**Files:**
- Modify: `scripts/run-agent.sh`

**Pourquoi une tâche à elle seule** : ce dépôt a payé **trois fois** l'oubli de
cette ligne — `SUPERVISEUR` (D1), `MULTIFENETRE_REPRISE` (D2), `AUDIO` (D7) — et
**le symptôme est un agent qui démarre sans rien signaler**. En D7,
l'implémenteur *et* le relecteur avaient vérifié la propriété **en traçant le
code** : le tracé était juste, la valeur ne pouvait simplement pas atteindre le
processus.

- [ ] **Step 1 :** relever la forme employée par ses voisines, l. 31-87 :
      `${VAR:+\$env:VAR = '$VAR'}`, **une variable par ligne, sans trou**.
- [ ] **Step 2 :** ajouter les deux lignes dans ce groupe.
- [ ] **Step 3 : le contrôle qui vaut** — et ce n'est **pas** une relecture :

```bash
AGENT_VM=essai AGENT_SECRET=abc bash -n scripts/run-agent.sh   # syntaxe
grep -n 'AGENT_VM\|AGENT_SECRET' scripts/run-agent.sh          # présence
```

⚠️ **La preuve de bout en bout appartient à la tâche 23** (corroboration sur
VM) : c'est là seulement qu'on saura que la valeur atteint le processus.

---

# Famille 7 — recette et clôture

### Task 22 : la recette — quatre critères, DEUX exécutions chacun, toutes les rouges jouées

**Objet :** juger les quatre critères de la spec §4 « P3 », **sans la VM**.

**Files:**
- Create: `docs/superpowers/plans/journaux-plateforme-p3/*.log`

⛔ **Aucune tâche de cette recette n'emploie la VM Windows** — décision de la
spec §4, pas commodité. Les pairs `agent` et `client` sont **simulés par des
sockets scriptés**, comme `plateforme/src/signaling/server.test.ts` le fait déjà.

**Convention des deux exécutions**, reconduite de P2 : **exécution 1 = sqlite,
exécution 2 = postgres**, déclaré dans l'en-tête de chaque journal. Elle est
**plus forte** que deux passes identiques pour les critères qui touchent la
base, et **strictement identique** pour ceux qui sont purs.

| # | Critère | Comment il est jugé | La ROUGE, nommée |
| --- | --- | --- | --- |
| ① | **Deux agents simulés, deux VMs, aucun conflit** — et **chacun ne voit que sa session** | deux enrôlements distincts, deux préfixes distincts ; les deux ouvrent `<P>:bureau` et servent `<P>:w-1` ; **puis** l'agent de `P` se voit refuser `<Q>:bureau` | 🔴 **Rouge GRATUITE, et c'est celle que la spec §7.2 annonce** : donner aux deux agents le **même** préfixe vide reproduit le binaire de P2, et le second reçoit `un agent est déjà connecté à la session bureau` (message figé par `appariement.test.ts:12-13`, **inchangé** — voir E11). ⚠️ **SECONDE ROUGE, DISTINCTE** : retirer la comparaison de préfixe de la garde fait passer l'agent de `P` sur `<Q>:bureau`. **Deux assertions, deux rouges** |
| ② | Un agent au **mauvais secret** est refusé, **sans distinguer** « VM inconnue » de « secret faux » | les deux refus sont comparés **caractère pour caractère** | rendre deux motifs distincts : c'est un oracle d'énumération, et la spec le nomme littéralement comme la rouge de ce critère |
| ③ | Une version de protocole incompatible est refusée **des deux côtés** | un vecteur de version `PLATEFORME_VERSION + 1` est rejeté par le Rust **et** par le TypeScript | 🔴 **DEUX ROUGES, une par côté** : omettre `verifie_version` en Rust, puis omettre la comparaison en TS. **Une seule rouge laisserait un côté non éprouvé** — c'est exactement le patron ①A-bis de P2 |
| ④ | Un agent muet est vu comme tel | `vu_a` cesse d'avancer, l'état **passe** de `prete` à `injoignable`, horloge injectée qui **varie** | figer l'horloge : le test devient inerte. **Le test doit VOIR la transition**, pas seulement lire l'état final |

- [ ] **Step 1 : le témoin.** `./scripts/verify-all.sh` sur un arbre dont on
      **nomme le commit**. ⚠️ **Il peut échouer pour une cause étrangère** —
      P2 l'a vécu, son étape 1 tombant sur un test Rust du chantier voisin. Si
      c'est le cas, **le déclarer**, jouer les sept étapes TypeScript à part, et
      rejouer le témoin complet à la clôture sur un `git worktree` du dernier
      commit de P3.
- [ ] **Step 2 : les quatre critères, deux exécutions chacun**, journaux versés.
- [ ] **Step 3 : les SEPT rouges nommées ci-dessus**, chacune avec son message
      **verbatim**, et **les sources restaurées à l'identique après chacune**.
- [ ] **Step 4 :** un journal supplémentaire, `e2-ferme.log` : la même sonde que
      `journaux-plateforme-p2/e2-role-agent-toujours-anonyme.log` — un pair qui
      déclare `{"role":"agent","session":"x"}` sans rien — **et qui ne reçoit
      plus ni `ice-config` ni acceptation**. 🔴 **C'est la pièce qui clôt le
      legs n°1 de P2**, et elle se lit contre celle de P2, ligne à ligne.

---

### Task 23 : la corroboration sur VM réelle — HORS CRITÈRE

**Objet :** montrer que le bureau et une fenêtre s'établissent comme avant, avec
un superviseur préfixé et enrôlé. **La spec §4 la range explicitement hors
critère.**

⚠️ **Cette tâche EMPLOIE la VM Windows** — la seule du plan. **Ne pas la
démarrer** tant que le chantier microphone la tient. Séquence :

- [ ] `virsh list --all`, démarrer si besoin, attendre WinRM **puis** le montage
      (`until ls /media/vm/dev >/dev/null 2>&1`) — `mountpoint -q` ne suffit
      pas, l'entrée CIFS survit à une VM éteinte.
- [ ] `set -a && source .env && set +a` **avant** `scripts/build-agent.sh` :
      sans cela le script **s'arrête en silence** après « sources
      synchronisées », et le symptôme se lit exactement comme une compilation
      réussie et muette.
- [ ] **Vérifier la TAILLE du binaire** après compilation. Une compilation de
      0,13 s est un aveu.
- [ ] `Get-Process agent` **avant** le lancement : un agent survit à
      l'hibernation de la VM, et `run-agent.sh` ne le tue pas.
- [ ] Enrôler la VM (`npm run admin:agent`), poser `AGENT_VM` et `AGENT_SECRET`,
      lancer, et relever : la ligne `enrôlé`, le préfixe, `<P>:bureau` établie,
      une fenêtre `<P>:w-1`.
- [ ] 🔴 **Relever aussi le cas SANS les variables** : l'agent doit échouer
      **bruyamment**, et le journal doit le dire. C'est la preuve de bout en
      bout de la tâche 20.

**Nombre d'exécutions : à écrire dans le document de résultats, quel qu'il
soit.** Une exécution est acceptable pour une corroboration hors critère — **à
condition de le dire.**

---

### Task 24 : la revue transverse de fin de branche — OBLIGATOIRE

**Objet :** trouver **les affirmations — commentaires, documents, `CLAUDE.md` —
que la branche P3 elle-même a rendues fausses.**

**Elle n'est jamais facultative.** Barème : **5** défauts en D7, **3** en D8,
**6** en D9, **douze** en D10, **sept** en D11, **huit** en P1, **dix** en P2
(réparties sur **vingt-trois places**).

⚠️ **Les deux comptes ne mesurent pas la même chose** : le nombre de **défauts**
est ce qu'un relecteur trouve ; le nombre de **places** est ce qu'il faut
éditer, et c'est le second qui coûte — parce que « corrigé à sa place » est une
affirmation de **COMPLÉTUDE**, et que ce dépôt a payé **neuf fois** pour un
nombre laissé dans une place non balayée.

**Cibles nommées d'avance** — P3 ferme un trou que P1 et P2 ont pris soin de
nommer **partout**, et c'est exactement la classe qui a produit sept des
vingt-trois places de P2 :

| Place | Ce qui devient faux, ou change de portée |
| --- | --- |
| `plateforme/src/identite/garde.ts:18-24` | « un pair `{"role":"agent"}` est ACCEPTÉ SANS JETON […] L'agent Rust n'a aucune identité avant P3 » — **fait** |
| `plateforme/src/identite/garde.ts:72` | le commentaire « la fenêtre `agent` est déclarée, pas oubliée » — **elle est fermée** |
| `plateforme/src/signaling/relais.ts:9-23` | la fenêtre anonyme vers des identifiants TURN de 86 400 s |
| `plateforme/src/signaling/relais.ts` bloc `TYPES_RELAYES` | déjà **corrigé une fois** par P2 — vérifier qu'il ne reste pas de moitié |
| `plateforme/src/signaling/resilience.test.ts:3-4` | « sans authentification requise » — 🔴 **À PRÉCISER, PAS À CORRIGER** (E15) : le déni de service en une trame **reste ouvert** |
| `plateforme/src/config.ts:13-20` | l'argument de `PLATEFORME_HOTE` — **il ne perd rien, mais sa portée change** |
| `plateforme/src/signaling/propriete.ts:14-20` | « le préfixe opaque de session, spec §3.4 » — **il existe** |
| `plateforme/src/signaling/trace.ts:11-14` | « `vu_a` est du ressort de P3 » — **fait** |
| `plateforme/src/base/migrations/0001-socle.sql:64-75` | « `vm_id` reste entièrement vide : c'est P3 » — **fait** |
| `plateforme/src/depot/session.ts:43-48` | « l'agent n'ayant aucune identité avant P3 » |
| `plateforme/src/signaling/relais.ts:97-107` | la doc d'`apparie` : « l'agent, dont l'identité n'existe pas avant P3 » |
| `agent/src/superviseur/protocole.rs:24-25` | la doc de `SESSION_DE_CONTROLE` |
| `agent/src/superviseur/table.rs:374` et `:163-165` | vérifier que le non-recul du compteur est **toujours** énoncé juste |
| `CLAUDE.md` section P1 ⑫ legs 3, 4, 5 | « `session.vm_id` reste NULL », « observer les agents présents », « `relais.ts` accueillera le canal `/agent` » |
| `CLAUDE.md` section P2 ⑪ legs 1, 2, 3 | E2, le préfixe, `session.vm_id` |
| `CLAUDE.md` section P2 ① et ⑥ | « le rôle `agent` reste ANONYME » — **relevés DATÉS : à ANNOTER, jamais à réécrire** |
| `docs/…/specs/2026-08-19-plateforme-design.md` §2.3 ③, §10.3 | déjà annoté une fois par P2 ; §10.3 est réfuté par E1 |

⚠️ **Le sort le plus fréquent n'est PAS « corrigé » mais « annoté »** : un relevé
de P1 ou de P2 reste **vrai comme histoire**, et le barrer le rendrait faux. Ce
sont les **pronostics** qu'il faut reprendre — « c'est P3 », « avant P3 »,
« reste vrai » —, jamais les mesures.

- [ ] **Step 1 :** `grep -rn "P3\|avant P3\|c'est P3" plateforme/src client/src agent/src proto/src docs/superpowers/plans/2026-08-19-plateforme-*.md CLAUDE.md`
      — **énumérer les places AVANT d'éditer**, et **relire `grep -n` place par
      place APRÈS l'édition**. Une substitution qui ne dit pas combien
      d'occurrences elle a touchées est une affirmation de complétude non
      vérifiée.
- [ ] **Step 2 :** corriger ou annoter, en distinguant les deux.
- [ ] **Step 3 :** relever le nombre de **défauts** et le nombre de **places**,
      séparément.

---

### Task 25 : `CLAUDE.md` gagne sa section P3

**Files:**
- Modify: `CLAUDE.md`

- [ ] **Step 1 :** écrire la section sur le modèle des sections P1 et P2 :
      le fait n°1 (E2 fermée, **mesurée**), les variables neuves (`AGENT_VM`,
      `AGENT_SECRET` — 🔴 **les PREMIÈRES du sous-projet ⑤ que
      `scripts/run-agent.sh` transmet**, contrairement à P1 et P2), les quatre
      critères avec leur nombre d'exécutions, ce que P3 n'établit **pas**, les
      pièges neufs, les divergences E1…E16, la revue transverse, et les legs.
- [ ] **Step 2 : les tailles, RELEVÉES PAR LA COMMANDE, APRÈS la dernière
      édition de la ronde** — y compris celles de la revue transverse. **Une
      table mesurée en début de ronde est fausse à la fin de la même ronde**, et
      D8 a commis exactement cette erreur en croyant bien faire.
- [ ] **Step 3 : et il faut dire à quel COMMIT.** L'arbre est partagé.
- [ ] **Step 4 :** 🔴 **`grep -n '<le nombre>' CLAUDE.md` pour CHAQUE chiffre
      corrigé**, avant et après. C'est le seul geste qui ait jamais fermé le
      naufrage du « 487 ».
- [ ] **Step 5 :** annoncer d'avance le compte de fichiers de
      `journaux-plateforme-p3/`, puis le relever par `git ls-files`.

---

### Task 26 : clore le document de résultats

**Files:**
- Create: `docs/superpowers/plans/2026-08-19-plateforme-p3-resultats.md`

- [ ] Le verdict des quatre critères, **avec le nombre d'exécutions dans chaque
      énoncé**. Aucun taux.
- [ ] Chaque ROUGE jouée, **avec son message verbatim** — les sept de la
      tâche 22, plus celles des tâches qui en ont produit d'inattendues.
- [ ] Les divergences trouvées **pendant** l'exécution, en plus des E1…E16
      tranchées d'avance, et le sort de chacune des seize.
- [ ] La corroboration sur VM (tâche 23), **avec son nombre d'exécutions**.
- [ ] Ce que P3 n'établit PAS.
- [ ] Le renvoi vers chaque journal versé.
- [ ] 🔴 Un contrôle explicite qu'**aucune preuve ne vit hors de git** :
      `git ls-files docs/superpowers/plans/journaux-plateforme-p3 | wc -l`.
      D10 a établi par la commande que l'espace de travail de D9 a **disparu**,
      emportant six constats définitivement perdus.

---

## Ce que ce plan NE prescrit PAS, et pourquoi

- **Aucune route HTTP qui rende un préfixe à un navigateur** : c'est P4 (E3).
- **Aucune interface `Orchestrateur`, aucun `InventaireStatique`, aucune
  attribution de VM à un utilisateur** : P4. L'index unique partiel sur
  `vm(utilisateur_id)` **existe déjà depuis P1** (`0001-socle.sql:57-58`) et P3
  n'y touche pas.
- **Aucune écriture dans `application`** : ④ empruntera le canal.
  <!-- ANNOTATION G1 (20 août 2026) : ④ L'A EMPRUNTÉ. `agents/canal.ts` écrit
  `application` à chaque message `catalogue`, via `apps/catalogue.ts::fusionner`
  et `depot/application.ts::appliquer`. Ce legs est CLOS. -->
- **Aucun frein, aucun TLS, aucun cookie, aucun en-tête de sécurité, aucun
  `/sante`, aucun `coturn` restreint** : P5.
- **Aucune revérification d'une session en cours** : la garde ne couvre que la
  poignée de main, et c'est un changement de conception, pas un correctif —
  legs n°9 de P2, reconduit.
- **Aucune calibration.** `SEUIL_INJOIGNABLE_MS`, `REPLI_MIN_MS`,
  `REPLI_MAX_MS`, `OCTETS_PREFIXE`, et les constantes de P1/P2 que P3 ne
  recalibre pas — `DUREE_SECONDES = 86 400`, `DUREE_JETON_ACCES_MS`, `N`/`r`/`p`
  de `scrypt`. **Aucun jugement d'usage n'est porté sur aucune d'elles.**
- **Aucune sonde contre un coturn vivant** (E6) : la propriété « coturn coupe
  sur le premier `:` » est une lecture de la convention `use-auth-secret`,
  **non éprouvée**.
- **Aucun test de charge, aucune latence** : la cible « < 3 s si VM chaude »
  n'est mesurée par aucun critère, et **aucun sous-bloc du chantier D n'a jamais
  mesuré la latence de bout en bout** non plus.
- **Aucun audit par un tiers** : CSRF, fixation de session, attaques
  temporelles — choix raisonnés, non éprouvés (spec §8).
- **Aucune protection contre le vol du secret d'enrôlement sur la VM** (D1) :
  il vit en clair dans `C:\dev\run-agent.ps1`, comme les 57 autres variables.
  **À rouvrir en P5.**
- **Rien du comportement d'un agent qui perd son canal pendant une session
  établie** : le média ne dépend plus du signaling une fois l'offre échangée
  (`agent/src/demarrage.rs:257-262`), mais **ce plan ne l'éprouve pas** pour le
  canal `/agent`.
