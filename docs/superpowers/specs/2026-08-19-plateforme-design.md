# Sous-projet ⑤ — Plateforme

**Date** : 19 août 2026
**Cadrage parent** : `2026-07-27-refonte-produit-design.md`, §5 ⑤ (« Plateforme »),
§3 (décisions actées), §9.4 (ordre de construction)
**Statut** : conception, plan à écrire
**Portée** : auth, orchestration de VMs, signaling, persistance — le service
serveur du nouveau produit

---

## 1. Objet, et pourquoi ce document découpe

Le cadrage décrit ⑤ en quatre puces (§5 ⑤, l. 217-225). Prises au pied de la
lettre elles tiennent en quinze lignes ; construites, elles font un service
complet — identité, base de données, orchestration d'hyperviseur, relais
WebRTC — qui est **le seul composant du nouveau produit qui n'existe pas
encore**. Le spécifier d'un tenant produirait un document que personne ne peut
exécuter d'un tenant. Ce document le découpe en **cinq sous-blocs P1 à P5**
(§4), dont le premier est une tranche verticale démontrable **sans la VM
Windows**.

Le cadrage place ⑤ en **quatrième** position (§9), après le pont fichiers et la
gestion d'apps. Ce document ne conteste pas cet ordre, mais il relève un fait
que le cadrage ne pouvait pas connaître en juillet : **le produit tel qu'il est
aujourd'hui ne peut servir qu'un seul utilisateur, et c'est une propriété de
son code, pas une limite de déploiement** (§2.3). Tant que ⑤ n'est pas fait,
③ et ④ se construisent sur un socle mono-tenant.

**Ce document ne modifie aucun code.** Il n'a été écrit qu'avec des lectures,
et chaque affirmation sur l'existant porte son `fichier:ligne`.

---

## 2. État des lieux, relevé et non supposé

### 2.1 Ce qui existe : 284 lignes de signaling, et rien d'autre côté serveur

Mesuré par `wc -l` le 19 août 2026 (valeurs relevées, tableau composé —
ce bloc n'est pas une transcription) :

| Fichier | Lignes | Nature |
| --- | --- | --- |
| `signaling/src/server.ts` | **208** | production |
| `signaling/src/ice.ts` | **66** | production |
| `signaling/src/index.ts` | **10** | production |
| | **284** | **total de production** |
| `signaling/src/server.test.ts` | 255 | test |
| `signaling/src/resilience.test.ts` | 168 | test |
| `signaling/src/ice.test.ts` | 60 | test |

`npx vitest run` depuis `signaling/` rend **3 fichiers, 20 tests, tous verts**
(exécuté le 19 août 2026, `Duration 4.48s`).

Ce que ces 284 lignes livrent réellement :

- **appariement de deux pairs par session** (`server.ts:103-130`), avec
  exactement deux rôles — `type Role = 'agent' | 'client'` (`server.ts:8`) ;
- **relais typé** : six types seulement passent (`server.ts:32-39`), le reste
  est refusé avec un motif ;
- **mémorisation de l'offre** quand l'agent n'est pas encore là
  (`server.ts:22`, `:154-157`, `:167-173`) — sans quoi le renversement d'ordre
  du sous-bloc D1 perdrait l'offre en silence ;
- **identifiants TURN éphémères** dérivés d'un secret qui ne quitte pas le
  processus (`ice.ts:32-42`), délivrés aux **deux** pairs (`server.ts:132-150`) ;
- **robustesse au message malformé** : un `null` ne tue plus le processus
  (`server.ts:57-59`, éprouvé par `resilience.test.ts` sur un vrai processus
  enfant, pas en mémoire).

⚠️ **Correction d'un point du cahier des charges de ce document** : il annonçait
`signaling/src/resilience.ts`. **Ce fichier n'existe pas** — seul
`resilience.test.ts` existe, et il éprouve `index.ts` lancé comme processus
séparé (son en-tête l. 6-19 explique pourquoi : vitest installe son propre
gestionnaire d'exceptions et masquerait la mort du processus). Il n'y a donc
que **trois** modules de production dans `signaling/`.

### 2.2 « Signaling déjà livré » n'est vrai qu'à moitié

Le cadrage (§5 ⑤) présente le signaling et TURN comme une puce parmi quatre.
Le code montre que la **fonction** est livrée et que **l'infrastructure de
service** ne l'est pas :

| Livré | Manquant |
| --- | --- |
| appariement, relais typé, mémorisation d'offre | authentification (§2.3) |
| identifiants TURN éphémères, HMAC-SHA1, expiration | unicité des identifiants de session (§2.3) |
| 20 tests verts, dont un test de survie du processus | persistance (§2.4) |
| — | `npm run typecheck` (absent de `signaling/package.json:6-9`, alors que `signaling/tsconfig.json` existe) |
| — | présence dans `scripts/verify-all.sh` (§2.5) |
| — | version de protocole (§2.6) |

### 2.3 🔴 Trois faits bornent le produit à UN utilisateur, par construction

Ce sont les faits qui gouvernent tout ce document.

**① L'identifiant de la session de contrôle est une constante littérale.**
`agent/src/superviseur/protocole.rs:17` — `pub const SESSION_DE_CONTROLE: &str = "bureau";`,
recopié à la main côté navigateur en `client/src/shell-page.ts:10`. Le
superviseur s'y déclare en `agent` (`agent/src/superviseur.rs:29`,
`agent/src/superviseur/signalisation.rs:33-38`), la page-shell en `client`
(`client/src/shell-page.ts:45`). Or `server.ts:119-123` refuse un **second**
occupant du même rôle sur la même session. **Deux VMs branchées sur le même
signaling : la seconde reçoit `un agent est déjà connecté à la session
bureau`, et son bureau n'existe jamais.**

**② Les identifiants de session de fenêtre sont un compteur local au
processus.** `agent/src/superviseur/table.rs:239` et `:397` —
`IdSession(format!("w-{}", self.compteur))`, champ d'instance
(`table.rs:166`) initialisé à `0` (`table.rs:193`). Deux VMs produisent toutes
deux `w-1`. Le commentaire de `table.rs:163-165` dit pourquoi le compteur ne
recule jamais — « un identifiant réutilisé apparierait un message tardif du
navigateur à la mauvaise fenêtre » —, et c'est exactement le raisonnement qui
s'applique **entre VMs** dès qu'il y en a deux.

**③ Les identifiants TURN sont délivrés sans aucune authentification.**
`server.ts:132-150` appelle `configurationIce(...)` et envoie le résultat
**immédiatement après la déclaration de rôle**, à tout pair, quel qu'il soit.
La validité est de `86_400` secondes (`ice.ts:16`). L'écoute n'est bornée à
aucune interface : `new WebSocketServer({ port })` (`server.ts:67`) ne reçoit
pas de `host`. Et `CLAUDE.md` (§« Chantier C volet 2 ») relève que coturn
écoute sur l'adresse publique de l'hôte — `UDP listener opened on:
90.87.35.18:3478`, **constaté**.

⚠️ **Formulation exacte, et pas plus** : je n'ai pas mesuré la joignabilité du
port de signaling depuis l'extérieur, et je ne l'affirme donc pas. Ce qui est
établi par le code est que **rien, dans ce code, ne l'empêche** : quiconque
atteint le port obtient des identifiants de relais valables 24 h, et peut
squatter n'importe quel identifiant de session, y compris `bureau`. Le
commentaire de `server.ts:25-27` en a déjà conscience — « un relais qui
accepterait n'importe quoi deviendrait un canal de diffusion arbitraire sur un
serveur sans authentification » — et borne les **types** relayés, ce qui ne
borne pas les **pairs**.

### 2.4 Ce qui n'existe pas du tout

Vérifié par `grep -rniE "sqlite|postgres|drizzle|prisma|knex|kysely|better-sqlite3|jsonwebtoken|argon2|bcrypt"`
sur `signaling/`, `client/`, `proto/` (fichiers `package.json` et `*.ts`, hors
`node_modules`) : **aucun résultat**. Il n'y a, dans le nouveau produit :

- **aucune base de données** — l'état vit dans une `Map` (`server.ts:68`),
  supprimée dès que les deux pairs sont partis (`server.ts:190-192`) ;
- **aucun utilisateur, aucune VM, aucun tenant** — aucun identifiant de ce
  genre nulle part ;
- **aucune authentification** — assumée en toutes lettres par `server.ts:2` :
  « Aucun état persistant, aucune authentification (jalon 1, réseau local) » ;
- **aucune orchestration** — le seul lanceur est `scripts/run-agent.sh`, qui
  vise une VM en dur par WinRM ;
- **aucun hub** — `client/vite.config.ts:9-14` déclare exactement deux entrées,
  `index.html` (une session) et `shell.html` (le bureau). `shell.html` reflète
  les fenêtres **déjà ouvertes** ; `DepuisLaShell` n'a qu'une seule variante,
  `Viewport` (`agent/src/superviseur/protocole.rs:32-35`) : **rien ne permet
  de demander le lancement d'une application.**

Les jetons `token=` que l'on trouve dans le dépôt (`web/index.js:167`)
appartiennent au **Guacamole historique** et n'ont aucun rapport.

### 2.5 Une lacune d'outillage, qui vaut d'être corrigée en P1

`scripts/verify-all.sh:31-56` enchaîne six étapes : `cargo test`,
`cargo clippy`, puis `npm test` et `npm run typecheck` pour `client/` **et**
`proto/`. **`signaling/` n'y figure dans aucune étape**, alors qu'il porte
483 lignes de tests. Et il n'a pas de script `typecheck` à y brancher.

Ce n'est pas un détail de confort : l'en-tête du script explique qu'il existe
**parce que** Vitest repose sur esbuild, qui transpile sans vérifier les types,
et que deux revues successives ont approuvé du code dont `tsc --noEmit`
échouait. Le service serveur est aujourd'hui hors de ce filet.

### 2.6 Le protocole superviseur ↔ shell n'a pas de version, contrairement aux autres

`proto/src/control.rs:14` porte `CONTROL_VERSION = 3` et `proto/src/input.rs:14`
`PROTOCOL_VERSION = 2`, tous deux vérifiés à la désérialisation
(`control.rs:41-49`, `proto/ts/control.ts:120-124`) et miroités en TypeScript.
Le protocole de la session de contrôle (`agent/src/superviseur/protocole.rs`)
n'a **aucun champ de version**, et sa constante d'identifiant est **recopiée à
la main** en TypeScript au lieu de vivre dans `proto/`.

Le canal plateforme ↔ agent créé en P3 (§3.3) suivra le précédent versionné,
pas celui-ci.

### 2.7 Le relais TURN, tel qu'il est déployé, ne couvre pas la cible qu'il vise

`docker-compose.coturn.yml` (versionné, sans secret) pose :
`--listening-port=3478`, `--no-tls`, `--no-dtls`, `--min-port=49160`,
`--max-port=49200`, `network_mode: host`.

Deux conséquences, à énoncer parce que le cadrage promet « un serveur TURN pour
les **réseaux restrictifs** » (§5 ⑤) :

- **il n'y a pas de TURNS.** Un relais en clair sur 3478 est précisément ce que
  les réseaux d'entreprise restrictifs filtrent ; la réponse usuelle est
  TURN over TLS sur 443. La configuration livrée ne l'offre pas.
- **la plage de relais compte 41 ports** (49160 à 49200 inclus). Une allocation
  consommant un port de relais, c'est un plafond de concurrence — dont
  **je n'ai mesuré ni la valeur effective ni le comportement au dépassement**.

Enfin, `docker-compose.coturn.yml:9` impose de composer ce fichier avec
`docker-compose.yml`, **qui n'est pas versionné** (il porte les mots de passe
Windows du produit historique) et qui ne déclare **aucun** service du nouveau
produit — un seul service `web`, le Guacamole historique.

---

## 3. Décisions actées

### 3.1 Le service plateforme ABSORBE le signaling — il ne vit pas à côté

**Décision : `signaling/` disparaît ; son code devient
`plateforme/src/signaling/`, dans le même processus que le reste de la
plateforme, sur le même port.**

Quatre raisons, dans l'ordre de poids :

1. **Le signaling doit de toute façon authentifier.** La garde du §2.3 ③ se
   pose sur le premier message WebSocket ; elle a besoin de vérifier un jeton,
   donc du secret de signature, donc de la table des utilisateurs. Un signaling
   séparé devrait ou partager ce secret, ou appeler la plateforme à chaque
   poignée de main. Les deux sont plus coûteux que d'être le même processus.
2. **`ice.ts` distribue déjà un secret de plateforme.** `TURN_SECRET` est un
   secret d'exploitation ; la fonction qui en dérive des identifiants est
   naturellement un service de la plateforme, pas d'un relais anonyme.
3. **Le chemin critique.** Le cadrage vise « < 3 s si VM chaude » (§6). Chaque
   saut inter-services sur l'établissement de session est du budget dépensé
   pour rien à cette échelle.
4. **Le coût du déménagement est mesuré, pas supposé : 284 lignes.** Elles
   partent **avec leurs 483 lignes de tests**, qui doivent rester verts — c'est
   le critère de non-régression de P1.

**Ce que cette décision n'est pas** : une réécriture. P1 déplace le code et
**ne change pas le protocole du fil** ; l'agent et le client d'aujourd'hui
doivent continuer de fonctionner sans être recompilés (§10).

⚠️ **Ce que l'absorption ne procure PAS.** Le cadrage écrit « Scalabilité
horizontale possible (plus d'état en mémoire) » (§5 ⑤). Une base de données
retire de la mémoire l'état **durable** ; elle ne retire pas l'état de
**routage**, parce qu'un WebSocket vit dans un processus et un seul. Faire
tourner deux instances de plateforme exigerait un routage collant par session
ou un bus entre instances. **Hors périmètre v1, et nommé comme tel** (§9) —
pas résolu par la persistance, contrairement à ce que la phrase du cadrage
laisse croire.

### 3.2 Persistance : SQL direct, sous-ensemble portable, deux pilotes

**Décision : pas d'ORM, pas de constructeur de requêtes. Une interface
`Pilote` minimale à deux implémentations — `node:sqlite` (dev et tests) et
`pg` (production) — et un sous-ensemble SQL portable écrit une seule fois.**

**Pourquoi pas un ORM ou un constructeur de requêtes.** La contrainte du
cadrage est « SQLite en dev, Postgres en prod ». Aucun outil ne la résout
gratuitement : Drizzle demande un schéma **par dialecte** (`sqliteTable` contre
`pgTable`), Prisma ajoute une étape de génération et un moteur, Kysely demande
un dialecte par pilote. Tous ajoutent une dépendance structurante à un projet
dont la doctrine, en Rust comme en TypeScript, est d'écrire des modules purs et
testés plutôt que d'adopter des cadres.

**Ce qui procure réellement la portabilité, c'est une discipline sur le
sous-ensemble SQL employé** — et cette discipline se garde par un test, pas par
une bibliothèque. Le sous-ensemble :

| Règle | Raison |
| --- | --- |
| identifiants : `TEXT`, UUID v4 par `node:crypto.randomUUID()` | disponible partout, aucune dépendance ; l'ordre chronologique vient de `cree_a`, pas de l'identifiant |
| horodatages : `INTEGER`, millisecondes epoch, **toujours écrites par l'application** | `CURRENT_TIMESTAMP` et `now()` n'ont pas la même sémantique dans les deux moteurs, et une horloge lue par la base n'est pas testable |
| booléens : `INTEGER` 0/1 | SQLite n'a pas de type booléen ; Postgres accepte `INTEGER` |
| interdits : `SERIAL`, `AUTOINCREMENT`, `datetime()`, `now()`, types spécifiques (`uuid`, `timestamptz`, `jsonb`) | divergent |
| autorisés et employés : `INSERT … RETURNING`, `ON CONFLICT … DO UPDATE`, index uniques **partiels** (`WHERE … IS NOT NULL`), clés étrangères, transactions | **éprouvés** sur le SQLite embarqué (relevé ci-dessous) ; documentés comme supportés par Postgres, **non éprouvés** |
| **toute valeur passe en paramètre** — aucune littérale dans le SQL | c'est ce qui interdit l'injection, **et** ce qui rend sûre la conversion de marqueurs ci-dessous |

**Ce sous-ensemble n'est pas supposé : il a été éprouvé sur le SQLite
réellement embarqué**, le 19 août 2026, avant d'être écrit ici. Sortie
verbatim (avertissement `ExperimentalWarning` filtré) :

```
version SQLite = 3.50.4
index partiel      : OK
RETURNING          : {"id":"a"}
deux NULL tolérés  : OK
doublon refusé     : UNIQUE constraint failed: u.u_id
ON CONFLICT        : {"u_id":"z"}
```

Le quatrième et le cinquième point sont ceux dont dépend le critère ② de P4
(deux utilisateurs ne peuvent pas recevoir la même VM) : l'index unique
**partiel** tolère plusieurs VM non attribuées **et** refuse la seconde
attribution. ⚠️ **Le pendant Postgres de ce relevé n'a PAS été pris** — ces
constructions y sont documentées comme supportées, et c'est précisément ce que
le test de portabilité de §7.1 a pour rôle d'établir plutôt que de croire.

L'interface :

```ts
interface Pilote {
    executer(sql: string, params: unknown[]): Promise<{ lignes: number }>;
    interroger<T>(sql: string, params: unknown[]): Promise<T[]>;
    transaction<T>(corps: (p: Pilote) => Promise<T>): Promise<T>;
    fermer(): Promise<void>;
}
```

Le SQL est écrit avec le marqueur `?`. Le pilote Postgres le convertit en
`$1..$n` par une **fonction pure et testée** ; la conversion est sûre **parce
que** la règle ci-dessus interdit les littérales, donc aucun `?` ne peut se
trouver dans une chaîne SQL. Le cas d'un `?` littéral est la **ROUGE** de son
test.

**Pourquoi `node:sqlite` et pas `better-sqlite3`.** Vérifié par la commande le
19 août 2026 sur le Node de cette machine (`v24.9.0`) :

```
$ node -e "const {DatabaseSync}=require('node:sqlite'); const d=new DatabaseSync(':memory:'); \
    d.exec('CREATE TABLE t(id TEXT PRIMARY KEY, n INTEGER)'); \
    d.prepare('INSERT INTO t VALUES(?,?)').run('a',1); \
    console.log(JSON.stringify(d.prepare('SELECT * FROM t').all()));" 2>&1 | tail -5
[{"id":"a","n":1}]
(node:3780439) ExperimentalWarning: SQLite is an experimental feature and might change at any time
(Use `node --trace-warnings ...` to show where the warning was created)
```

Le module fonctionne, **et il est marqué expérimental** — je ne le cache pas,
c'est le coût de la décision. Il est accepté parce que : SQLite ne sert **qu'en
développement et en test** (la production est Postgres) ; l'échec d'une
évolution d'API est **bruyant et immédiat**, sur l'hôte, avant tout
déploiement ; et il évite un module natif, ce qui honore directement le
principe §4.2 du cadrage — « zéro dépendance native non maintenue », principe
écrit après le naufrage de `fuse-native`. `pg` est un pilote purement
JavaScript : **l'ensemble de la plateforme reste sans dépendance native.**

**La condition attachée** : la version majeure de Node est **épinglée** dans
`plateforme/package.json` (`engines`), et un changement de majeure impose de
rejouer la suite avant tout autre travail.

**Migrations** : fichiers `.sql` numérotés, appliqués en ordre, dans une
transaction, avec une table `schema_migration(version, applique_a)`. Le
lanceur est un module d'une centaine de lignes, testé. Les migrations
obéissent au même sous-ensemble portable — et c'est là que le test de
portabilité (§7.1) mord en premier.

### 3.3 Canal plateforme ↔ agent : WebSocket, schéma partagé dans `proto/`

Le cadrage laisse le choix ouvert : « canal de contrôle (gRPC ou WS) » (§4).
**Décision : WebSocket.**

1. **L'agent a déjà un client WebSocket éprouvé** — `agent/src/signaling.rs`
   (204 lignes, `tokio-tungstenite`), avec sa détection de chute
   (`signaling.rs:26-33`) et sa reprise. gRPC imposerait `tonic`, `prost` et
   `protoc` dans une chaîne de compilation qui traverse déjà une VM Windows par
   partage CIFS — un coût réel, contre zéro gain à cette échelle.
2. **L'agent compose vers l'extérieur.** Il est dans une VM ; la plateforme ne
   peut pas l'appeler. Le canal doit être ouvert par l'agent et rester ouvert.
   Un WebSocket fait cela nativement.
3. **Le schéma partagé existe déjà et il est versionné.** `proto/` porte du
   Rust et du TypeScript miroirs, avec vecteurs de conformité croisée
   (`proto/vectors.json`) et vérification de version à la désérialisation. Le
   nouveau canal y ajoute un module `plateforme` avec sa propre constante de
   version, **sur le modèle de `CONTROL_VERSION`** — et jamais sur celui de
   `superviseur/protocole.rs`, qui n'en a pas (§2.6).
4. **Un seul type de transport à exploiter** : un port, un proxy inverse, une
   terminaison TLS, un jeu de traces.

**Sérialisation : JSON**, comme la session de contrôle actuelle. Le canal porte
des événements de cycle de vie (quelques-uns par minute), pas du média : le
gain d'un format binaire n'y a aucun référent. `proto/ts/input.ts` reste le
précédent binaire pour ce qui est chaud ; ce canal ne l'est pas.

### 3.4 L'identité d'une session est émise par la plateforme, jamais inventée par l'agent

C'est le remède aux faits ① et ② du §2.3, et il est **délibérément minimal**.

**Décision : la plateforme délivre à l'agent, à l'enrôlement, un préfixe opaque
de session.** Le superviseur préfixe les deux identifiants qu'il produit
aujourd'hui :

| Aujourd'hui | Demain | Site à changer |
| --- | --- | --- |
| `"bureau"` | `<préfixe>:bureau` | `agent/src/superviseur/protocole.rs:17` |
| `w-{n}` | `<préfixe>:w-{n}` | `agent/src/superviseur/table.rs:239` et `:397` |
| `'bureau'` codé en dur | reçu de la plateforme | `client/src/shell-page.ts:10` |

**Pourquoi un préfixe plutôt qu'un identifiant entièrement émis par la
plateforme** : le compteur du superviseur porte une propriété acquise et
documentée — il ne recule jamais, pour qu'un message tardif n'apparie pas la
mauvaise fenêtre (`table.rs:163-165`). Un aller-retour vers la plateforme à
chaque ouverture de fenêtre remplacerait cette propriété locale par une
dépendance réseau sur le chemin d'ouverture. Le préfixe rend l'espace de noms
global **sans toucher au mécanisme qui marche**.

**Le préfixe est opaque et non devinable** (aléatoire, ≥ 128 bits, en base
url-safe) : il n'est pas un secret, mais il ne doit pas permettre d'énumérer
les sessions d'autrui avant que la garde d'autorisation ne soit posée.

### 3.5 Auth : mot de passe par `scrypt`, jeton d'accès court, rafraîchissement en base

Le cadrage dit « email + mot de passe (OIDC prévu), sessions JWT » (§5 ⑤) et ne
nomme aucune fonction de dérivation.

**Décision : `scrypt` de `node:crypto`**, et non Argon2id.

Argon2id est préférable dans l'abstrait ; ses liaisons Node sont **natives**.
`scrypt` est intégré à Node, à mémoire dure, et figure dans les recommandations
OWASP. La colonne stocke une chaîne **préfixée par son algorithme et ses
paramètres** (`scrypt$N$r$p$sel$empreinte`), de sorte qu'un passage ultérieur à
Argon2id se fasse par re-hachage à la connexion suivante, sans migration de
données. **Le coût de la décision est nommé** : on renonce au meilleur choix
théorique pour préserver la propriété « aucune dépendance native ».

**Jetons** : JWT HS256 signé par un secret de plateforme, **durée courte**
(quelques minutes), portant l'identifiant d'utilisateur et rien d'autre. Le
rafraîchissement est un jeton opaque **stocké haché** en base, révocable,
rotatif à chaque emploi.

⚠️ **Propriété assumée, à écrire dans le code et non à découvrir** : un jeton
d'accès court **n'est pas révocable avant son expiration**. Ce qui est
révocable est la chaîne de rafraîchissement. Une révocation immédiate exigerait
une liste de révocation consultée à chaque message — hors périmètre v1 (§9).

**La durée de validité TURN n'est pas recalibrée.** `DUREE_SECONDES = 86_400`
(`ice.ts:16`) est justifiée dans son propre commentaire par le rafraîchissement
d'allocation en session longue. P2 change **qui** en reçoit (seulement un pair
authentifié, pour sa propre session), pas **combien de temps**. La recalibrer
sans mesure serait exactement le geste que ce dépôt reproche à ses constantes
non calibrées.

### 3.6 Orchestration : une interface, un backend d'inventaire, et des refus TYPÉS

```ts
type EtatVm = 'arretee' | 'demarrage' | 'prete' | 'injoignable';

interface Orchestrateur {
    lister(): Promise<Vm[]>;
    etat(vm: string): Promise<EtatVm>;
    demarrer(vm: string): Promise<Resultat>;
    arreter(vm: string): Promise<Resultat>;
    instantane(vm: string, nom: string): Promise<Resultat>;
    attribuer(vm: string, utilisateur: string): Promise<Resultat>;
}
```

Backend v1 : **`InventaireStatique`**, alimenté par un fichier de configuration
déclaratif décrivant des VMs qui existent déjà (nom, adresse, empreinte du
secret d'enrôlement) — c'est littéralement ce que dit le cadrage.

⚠️ **Décision de forme, et c'est la plus importante de ce paragraphe : une
opération que le backend ne sait pas faire rend un REFUS TYPÉ, jamais un
succès silencieux.** `InventaireStatique::instantane` et `::demarrer` ne
peuvent rien faire ; ils rendent
`Resultat = { refus: 'non supporté par ce backend', operation, backend }`, que
l'API expose en `501` et que l'interface affiche. Un `Promise<void>` qui ne
fait rien serait une panne muette, la classe de défaut contre laquelle tout ce
dépôt est écrit.

**Conséquence produit à assumer** : le cadrage promet « VM injoignable → le hub
l'indique, **propose redémarrage** via l'orchestrateur » (§7). Avec le backend
v1, le hub **indique** et **dit qu'il ne peut pas redémarrer**. La promesse
tient le jour où un backend d'hyperviseur existe ; l'interface est là pour lui.

---

## 4. Découpage en sous-blocs

| # | Objet | Dépend de |
| --- | --- | --- |
| **P1** | **Le service naît, absorbe le signaling, et persiste** | — |
| **P2** | L'identité des humains, et la garde du signaling | P1 |
| **P3** | L'identité des agents, et le canal plateforme ↔ agent | P2 |
| **P4** | L'orchestration et l'attribution d'une VM | P3 |
| **P5** | La production : Postgres déployé, et le durcissement | P4 |

**Deux exécutions par critère, pas une** — règle héritée des sous-blocs D9 et
D10. **Aucun taux ne sera revendiqué.**

⚠️ **Aucun critère de P1 à P4 n'exige la VM Windows.** C'est une décision, pas
une commodité : ⑤ est un sous-projet serveur, et faire dépendre sa recette
d'une ressource exclusive et lente rendrait chaque itération coûteuse et
chaque échec ambigu. Les pairs `agent` et `client` sont **simulés** par des
sockets scriptés — c'est déjà ce que fait `signaling/src/server.test.ts`. Une
confirmation sur VM réelle est prévue en fin de P3 et de P4, **hors critère**,
comme corroboration.

### P1 — Le service naît, absorbe le signaling, et persiste

**Livre** :

- le paquet `plateforme/` (Node/TypeScript, son propre `package.json`,
  `tsconfig.json` et `package-lock.json` — il n'y a **aucun workspace npm** dans
  ce dépôt, chaque sous-projet est autonome : `client/`, `signaling/`, `proto/`
  le sont) ;
- **un** serveur HTTP, avec la montée en WebSocket sur le **même port**, et
  **le chemin racine conservé** pour que l'agent et le client d'aujourd'hui
  fonctionnent sans recompilation ;
- le code de `signaling/` déplacé en `plateforme/src/signaling/`, **avec ses
  483 lignes de tests**, et `signaling/` supprimé du dépôt — laisser deux
  copies est la façon dont un fork dérive ;
- **l'extraction de `appariement.ts` AVANT toute addition** (§5) ;
- la couche `Pilote` + le lanceur de migrations + les tables `session` et `vm` ;
- une ligne `session` écrite à l'appariement, close à la déconnexion ;
- `PLATEFORME_HOTE` **explicite**, sans défaut permissif ;
- `plateforme` ajouté à `scripts/verify-all.sh` (test **et** typage), et
  `typecheck` ajouté à son `package.json`.

**Critères** :

| # | Critère | Comment il est jugé | Ce qui le rend ROUGE |
| --- | --- | --- | --- |
| ①  | Le service apparie deux pairs simulés et le média négocie comme avant | la suite déplacée de `signaling/` est verte **sans modification de ses assertions** : 20 tests | modifier une assertion pour la faire passer, c'est perdre la non-régression — le critère est que les tests soient **inchangés** |
| ② | Une session laisse une trace en base | après appariement puis déconnexion des deux pairs : une ligne dans `session` avec `ouverte_a` non nul et `fermee_a` non nul | supprimer l'écriture : le compte reste à 0. **À exercer** |
| ③ | La même suite de dépôt passe sur `node:sqlite` **et** sur Postgres | `npm run test:sqlite` et `npm run test:postgres` verts | introduire `datetime('now')` dans une migration : la passe Postgres échoue. **À exercer** |
| ④ | L'écoute est bornée | le service refuse de démarrer si `PLATEFORME_HOTE` n'est pas posée, et écoute sur cette seule adresse | poser `0.0.0.0` par défaut ferait passer le critère sans rien garantir : le défaut **doit** être l'absence de défaut |

⚠️ **Le critère ④ casse le lancement naïf**, et c'est voulu. `scripts/run-agent.sh`
vise `ws://192.168.3.1:8080` : l'opérateur devra poser l'adresse de son
interface. **Une rupture bruyante vaut mieux qu'une écoute universelle
silencieuse.**

⚠️ **Le critère ③ exige un Postgres** ; il est fourni par un
`docker-compose.plateforme.yml` **versionné et sans secret**, sur le modèle de
`docker-compose.coturn.yml`. **Un saut de la passe Postgres est un ÉCHEC, pas
un saut** : un test qui se saute quand sa dépendance manque est un test qui ne
protège rien.

### P2 — L'identité des humains, et la garde du signaling

**Livre** : table `utilisateur` ; création de compte **par ligne de commande
d'administration** (pas d'inscription publique en v1) ; connexion ; `scrypt` ;
jeton d'accès JWT court + rafraîchissement opaque haché et rotatif ; la garde
sur la poignée de main `{role:'client', session, jeton}` ; la délivrance
d'`ice-config` **conditionnée à l'authentification** ; un écran de connexion
fonctionnel — sans direction visuelle, qui appartient à ⑥.

**Critères** :

| # | Critère | Comment il est jugé | Ce qui le rend ROUGE |
| --- | --- | --- | --- |
| ① | Un pair `client` non authentifié est refusé **et ne reçoit aucun `ice-config`** | deux assertions distinctes : le refus, **et** l'absence du message | le binaire de P1 les échoue toutes deux — la ROUGE est **déjà disponible**, il suffit de la jouer |
| ② | Un jeton expiré est refusé | horloge injectée, comme `ice.ts:32-42` le fait déjà pour `maintenant` | figer l'horloge dans le test la rend inerte : elle doit varier |
| ③ | Un utilisateur ne peut pas rejoindre la session d'un autre | refus typé, journalisé, avec l'identifiant demandé | tant que P3 n'a pas posé le préfixe, ce critère porte sur l'**appartenance enregistrée** de la session, pas sur son nom |
| ④ | Un mot de passe n'est jamais journalisé ni renvoyé | balayage des traces de la suite | une assertion qui ne cherche que la chaîne exacte du mot de passe manquerait un journal du corps de requête entier : chercher **le champ**, pas la valeur |

⚠️ **Fenêtre d'exposition déclarée** : entre P2 et P3, le pair `agent` reste
**non authentifié** — c'est P3 qui lui donne une identité. La garde de P2
ferme la moitié navigateur. Le critère ④ de P1 (écoute bornée) est ce qui rend
cette fenêtre tolérable, et c'est pourquoi il est en P1 et non en P5.

### P3 — L'identité des agents, et le canal plateforme ↔ agent

**Livre** : table `agent_enrole` (secret d'enrôlement **haché**, par VM) ; la
poignée de main authentifiée du rôle `agent` ; le **préfixe de session** (§3.4)
et les quatre sites à changer ; le canal `/agent` en WebSocket, dont le schéma
vit dans `proto/` avec sa **constante de version** vérifiée à la
désérialisation, en Rust **et** en TypeScript, avec vecteurs croisés comme
`proto/vectors.json` ; battement de cœur et `vu_a`.

**Critères** :

| # | Critère | Comment il est jugé | Ce qui le rend ROUGE |
| --- | --- | --- | --- |
| ① | **Deux agents simulés, deux VMs, aucun conflit** | les deux ouvrent leur session de contrôle et servent chacun leurs fenêtres | sur le binaire de P2, le second reçoit `un agent est déjà connecté à la session bureau`. **C'est la ROUGE la plus nette du sous-projet, et elle est gratuite à jouer** |
| ② | Un agent au mauvais secret est refusé | refus typé, sans distinguer « VM inconnue » de « secret faux » dans le message rendu | un message qui distingue les deux est un oracle d'énumération : la ROUGE est de le distinguer |
| ③ | Une version de protocole incompatible est refusée des deux côtés | un vecteur de version `n+1` est rejeté par le Rust **et** par le TypeScript | omettre la vérification d'un des deux côtés : le vecteur passe d'un côté |
| ④ | Un agent muet est vu comme tel | `vu_a` cesse d'avancer, l'état passe `injoignable` après un seuil nommé | un seuil qui n'est jamais atteint dans le test ne prouve rien : le test **doit** voir la transition |

**Corroboration hors critère** : une exécution sur la VM réelle, avec le
superviseur préfixé, montrant que le bureau et une fenêtre s'établissent comme
avant.

### P4 — L'orchestration et l'attribution d'une VM

**Livre** : `Orchestrateur` + `InventaireStatique` ; l'attribution d'une VM à un
utilisateur (index unique **partiel** sur `vm(utilisateur_id) WHERE
utilisateur_id IS NOT NULL`) ; l'enchaînement « l'utilisateur demande une
session → la plateforme vérifie l'attribution et la fraîcheur de l'agent →
elle rend l'identifiant de session, le préfixe et la configuration ICE » ; les
refus typés.

**Critères** :

| # | Critère | Comment il est jugé | Ce qui le rend ROUGE |
| --- | --- | --- | --- |
| ① | `instantane` sur le backend statique **refuse explicitement** | refus typé, exposé en `501`, journalisé | le remplacer par un `return` silencieux : le test doit alors échouer |
| ② | Deux utilisateurs ne peuvent pas recevoir la même VM | violation d'index traduite en refus typé, jamais en 500 | retirer l'index partiel : la double attribution réussit |
| ③ | Un utilisateur sans VM reçoit un refus **immédiat** | pas d'attente, pas de délai d'expiration | une implémentation qui attend puis expire passerait un test qui ne mesure que l'issue : le test **borne le temps** |
| ④ | Une VM dont l'agent n'a pas été vu récemment est annoncée injoignable | l'API le dit, et dit qu'elle ne sait pas la redémarrer | masquer l'état derrière un « réessayez » générique |

### P5 — La production : Postgres déployé, et le durcissement

**Livre** : la configuration Postgres de déploiement ; le
`docker-compose.plateforme.yml` complet ; TLS/WSS **délégué à un proxy inverse**
et documenté (pas de terminaison dans le processus) ; **coturn restreint** par
`--listening-ip`, réponse directe à l'avertissement de `CLAUDE.md` §« Chantier C
volet 2 » ; **TURNS sur 443** ou, à défaut, la constatation écrite que la cible
« réseaux restrictifs » n'est pas couverte (§2.7) ; limitation de débit sur les
routes d'authentification ; journalisation structurée ; `/sante`.

**Critères** :

| # | Critère | Comment il est jugé | Ce qui le rend ROUGE |
| --- | --- | --- | --- |
| ① | La suite de dépôt est verte sur Postgres dans la configuration **déployée** | la même suite qu'en P1, contre l'instance de déploiement | une configuration de test qui diffère de celle de déploiement ne prouve rien : le critère porte sur la seconde |
| ② | coturn n'écoute que sur l'adresse nommée | `docker compose … logs coturn` ne montre `listener opened` que sur cette adresse | la configuration actuelle en montre plusieurs, dont l'adresse publique — la ROUGE est le fichier d'aujourd'hui |
| ③ | Les tentatives d'authentification sont freinées | après *n* échecs, la *n+1*ᵉ est refusée par le frein, avec sa trace | retirer le frein : la *n+1*ᵉ passe |
| ④ | Aucun secret dans un fichier versionné | balayage du dépôt sur les noms de variables de secret | chercher les **valeurs** est impossible ; chercher les **noms** dans un fichier versionné est décidable |

⚠️ **Le critère ② se juge sur le journal de coturn, pas sur une sonde réseau
externe.** Une sonde depuis l'extérieur exigerait une machine hors du réseau
local ; **je ne la prescris pas, et je dis donc que ce sous-projet n'établit
pas l'inaccessibilité effective du relais depuis Internet** (§8).

---

## 5. Arborescence, et le plafond de 500 lignes budgété d'avance

**Relevé par la commande le 19 août 2026**, dans la portée de
`CLAUDE.md` :

```
$ { git ls-files; git ls-files --others --exclude-standard; } \
  | grep -vE 'node_modules|package-lock|Cargo.lock|/dist/|testdata/|^docs/|^CLAUDE.md' \
  | xargs wc -l | sort -rn | awk '$1>500'
  51860 total
   1536 agent/src/encode.rs
    630 agent/src/windows_source.rs
```

Aucun fichier de `signaling/`, `client/` ou `proto/` n'approche le plafond. Le
plus gros du périmètre serveur est `signaling/src/server.ts` à **208** lignes.

⚠️ **Le seul fichier à surveiller est celui qui va grossir.** `server.ts` (208)
reçoit en P2 la garde d'authentification, en P3 la liaison agent ↔ VM, et en P4
l'appartenance de session. **L'extraction précède l'addition, sans exception** —
c'est le seul geste qui a fonctionné dans ce dépôt (D9, `serveur/instances.rs` :
marge rendue de 10 à 65 avant l'addition ; les deux fichiers traités après coup
y ont été **compressés**, geste que `CLAUDE.md` interdit nommément, puis
extraits quand même). **L'extraction d'`appariement.ts` est donc une tâche de
P1, pas une réaction de P2.**

```
plateforme/
  package.json  tsconfig.json  package-lock.json
  src/
    index.ts                        point d'entrée, arrêt propre
    config.ts                       lecture d'environnement, une fois, typée
    http/serveur.ts                 HTTP + montée WebSocket
    http/routes-auth.ts             P2
    http/routes-session.ts          P4
    http/routes-vm.ts               P4
    signaling/relais.ts             le `server.ts` d'aujourd'hui, allégé
    signaling/appariement.ts        la table des sessions — PUR, extrait en P1
    signaling/ice.ts                déplacé tel quel (66 l., déjà pur et testé)
    base/pilote.ts                  interface + rendu des marqueurs — PUR
    base/pilote-sqlite.ts           node:sqlite
    base/pilote-postgres.ts         pg
    base/migrations.ts              lanceur
    base/migrations/0001-….sql …
    depot/utilisateur.ts  depot/vm.ts  depot/session.ts
    depot/jeton.ts  depot/application.ts
    identite/mot-de-passe.ts        scrypt — PUR hors appel crypto
    identite/jeton.ts               JWT — PUR, horloge injectée
    identite/garde.ts
    agents/canal.ts                 P3
    agents/enrolement.ts            P3
    orchestration/interface.ts      P4
    orchestration/inventaire-statique.ts
proto/
  src/plateforme.rs   ts/plateforme.ts   (P3, avec sa constante de version)
```

⚠️ **Deux points de convention, tranchés ici pour qu'on n'y revienne pas** :

- **le §« Portée » de `CLAUDE.md` doit gagner `plateforme/`** — la commande de
  vérification, elle, l'attrapera **mécaniquement** (son filtre n'exclut que
  `node_modules`, les verrous, `dist/`, `testdata/`, `docs/` et `CLAUDE.md`).
  C'est exactement la divergence texte/commande que `CLAUDE.md` signale déjà,
  non tranchée, pour `client/verify-webrtc.mjs` (497 lignes, marge 3) ;
- **la « Convention de module enfant » de `CLAUDE.md` ne s'applique pas ici** :
  elle est écrite pour les modules Rust extraits d'un parent `#[cfg(windows)]`.
  Elle est nommée pour dire qu'elle a été considérée, et écartée pour cause de
  portée.

### Le schéma v1

```sql
utilisateur(id TEXT PK, email TEXT NOT NULL UNIQUE, empreinte_mdp TEXT NOT NULL,
            cree_a INTEGER NOT NULL)
vm(id TEXT PK, nom TEXT NOT NULL, adresse TEXT NOT NULL,
   utilisateur_id TEXT NULL REFERENCES utilisateur(id), vue_a INTEGER NULL)
   -- index unique PARTIEL sur utilisateur_id : une VM par utilisateur
agent_enrole(vm_id TEXT PK REFERENCES vm(id), empreinte_secret TEXT NOT NULL,
             prefixe_session TEXT NOT NULL UNIQUE, vu_a INTEGER NULL)
session(id TEXT PK, utilisateur_id TEXT NOT NULL, vm_id TEXT NOT NULL,
        ouverte_a INTEGER NOT NULL, fermee_a INTEGER NULL, motif TEXT NULL)
jeton_rafraichissement(id TEXT PK, utilisateur_id TEXT NOT NULL,
                       empreinte TEXT NOT NULL, expire_a INTEGER NOT NULL,
                       revoque_a INTEGER NULL)
application(id TEXT PK, vm_id TEXT NOT NULL, nom TEXT NOT NULL,
            chemin TEXT NOT NULL, vue_a INTEGER NOT NULL)
schema_migration(version INTEGER PK, applique_a INTEGER NOT NULL)
```

⚠️ **`application` est une table vide en v1.** Le cadrage range « apps » dans la
persistance de ⑤ (§5 ⑤), mais la **découverte** appartient à ④. La plateforme
la stocke et la relit ; **rien, en v1, ne l'écrit** — le chemin d'écriture est
le canal de P3, que ④ empruntera. Dit ici pour qu'un lecteur n'attende pas un
catalogue peuplé.

---

## 6. Gestion des erreurs

| Panne | Comportement |
| --- | --- |
| Base injoignable au démarrage | le service **refuse de démarrer**, avec la cause. Il ne démarre pas dégradé : un signaling qui apparie sans rien enregistrer serait indiscernable du bon fonctionnement |
| Base injoignable en cours de route | les sessions **déjà établies ne sont pas coupées** — le média WebRTC ne dépend plus du signaling une fois l'offre et la réponse échangées (`agent/src/signaling.rs:40-41`). Les **nouvelles** sessions sont refusées, avec un motif |
| Migration en échec | transaction annulée, service arrêté, version inchangée. Jamais de schéma à moitié appliqué |
| Jeton expiré ou invalide | refus typé sur la poignée de main, connexion **fermée** — contrairement au message malformé, que `server.ts:89-100` laisse retenter à dessein |
| Agent enrôlé qui disparaît | `vu_a` cesse d'avancer, la VM devient `injoignable`, le hub le dit **et dit qu'il ne peut pas la redémarrer** avec le backend statique |
| Opération non supportée par le backend | refus **typé** (§3.6), `501`, journalisé. Jamais un succès silencieux |
| Plateforme redémarrée | les sessions vivantes **survivent** (même raison que ci-dessus) ; leur ligne en base garde `fermee_a` nul et sera close par un balayage au démarrage. ⚠️ **La renégociation d'une session qui aurait perdu son signaling pendant le redémarrage n'est pas couverte en v1** (§8) |
| `TURN_URL`/`TURN_SECRET` absents | comportement **inchangé** : aucun relais, et une trace explicite (`server.ts:147-149`). Le piège documenté dans `CLAUDE.md` — « le signaling doit être relancé AVEC l'environnement » — reste entier et doit être repris dans le runbook de P5 |

---

## 7. Stratégie de test

**Tout ce qui peut être pur l'est**, et se teste sans base, sans socket et sans
VM : le rendu des marqueurs de paramètres, la table d'appariement, la
dérivation et la vérification des jetons (**horloge injectée**, comme
`ice.ts:32-42` le fait déjà pour `maintenant` — c'est le précédent du dépôt),
la règle d'attribution, la sélection d'une VM.

Le reste se teste contre une base réelle et des sockets réels, dans le style
que `signaling/src/server.test.ts` a établi : de vrais `WebSocket`, un port
attribué par le système, et — pour tout ce qui met en jeu la survie du
processus — un **processus enfant**, comme `resilience.test.ts:6-19` le
justifie.

### 7.1 Le test de portabilité de dialecte est OBLIGATOIRE, et il est LE garde-fou

La décision §3.2 échange une bibliothèque contre une discipline. **Une
discipline sans test n'est pas une discipline** — et le coût de cette décision,
nommé sans détour, est qu'**aucun compilateur ne vérifie une chaîne SQL écrite
à la main**.

Le remède est structurel : **une seule et même suite de tests de dépôt
s'exécute contre les deux pilotes**, par deux scripts distincts. Elle couvre
les migrations, chaque fonction de chaque dépôt, et les transactions.

⚠️ **Un saut est un échec.** Si Postgres est absent, `npm run test:postgres`
**échoue** — il ne se saute pas avec un avertissement. Ce dépôt a payé trois
fois pour un contrôle qui ne pouvait pas échouer (F1 de D7, la sonde P1 de D8,
le confondeur de D9) ; un test qui disparaît quand sa dépendance manque est la
même erreur sous une autre forme.

### 7.2 « Vu rouge » est une exigence, pas une formule

Pour **chaque** critère du §4, la colonne « Ce qui le rend ROUGE » nomme l'état
à provoquer délibérément, et la recette doit vérifier que le contrôle le
dénonce. Trois d'entre eux ont une ROUGE **gratuite**, parce que le binaire du
sous-bloc précédent la porte déjà :

- P2 ① : le service de P1 délivre `ice-config` à quiconque ;
- P3 ① : le service de P2 refuse le second agent sur `bureau` ;
- P5 ② : `docker-compose.coturn.yml` d'aujourd'hui écoute partout.

**Ce sont des ROUGES à JOUER, pas à supposer.** Un contrôle qu'on n'a jamais vu
rouge n'est pas un contrôle.

### 7.3 Vérification d'ensemble

`scripts/verify-all.sh` gagne **quatre** étapes : `plateforme : npm test`,
`plateforme : npm run test:postgres`, `plateforme : npm run typecheck`, et —
correction de la lacune du §2.5 — le typage n'est plus absent d'aucun paquet
TypeScript du dépôt.

---

## 8. Ce que le sous-projet ⑤ n'établira PAS

- **Aucun taux, nulle part.** Deux exécutions par critère, jamais une campagne.
- **Aucune latence.** La cible « < 3 s si VM chaude » (cadrage §6) n'est
  mesurée par aucun critère de ce document — pas plus que la latence de bout en
  bout, qu'**aucun sous-bloc du chantier D n'a jamais mesurée** non plus.
- **Aucune charge.** Le cadrage prévoit « tests de charge sur le signaling »
  (§8) : **hors v1**. Le nombre de sessions simultanées que la plateforme
  soutient n'est pas connu, et les **41 ports de relais** de coturn (§2.7) sont
  un plafond dont ni la valeur effective ni le comportement au dépassement ne
  seront mesurés.
- **Aucun audit de sécurité.** Le modèle de menace v1 est borné à : un pair
  anonyme ne doit ni obtenir d'identifiants de relais, ni rejoindre la session
  d'autrui. Tout le reste — fixation de session, CSRF sur les routes HTTP,
  attaques temporelles sur la comparaison d'empreintes — est traité par des
  choix raisonnés, **non éprouvés par un tiers**.
- **Rien du comportement d'un hyperviseur réel** : le backend v1 n'en pilote
  aucun. `demarrer`, `arreter` et `instantane` ne sont **jamais exécutés** ;
  seule leur voie de refus l'est.
- **Rien de la scalabilité horizontale** (§3.1) : elle n'est pas obtenue par la
  persistance, contrairement à ce que le cadrage laisse entendre.
- **La révocation immédiate d'un jeton d'accès** est impossible par
  construction (§3.5) ; seule la chaîne de rafraîchissement est révocable.
- **La renégociation d'une session après redémarrage de la plateforme**
  (cadrage §7) n'est pas couverte : ce qui est établi est que le média
  **survit** au redémarrage, pas qu'une session en cours de négociation s'en
  remet.
- **L'inaccessibilité effective du relais depuis Internet** (§4, critère P5 ②) :
  jugée sur le journal de coturn, jamais sur une sonde externe.
- **Aucune constante n'est calibrée** : ni la durée du jeton d'accès, ni les
  paramètres de `scrypt`, ni le seuil de `vu_a`, ni le frein de P5, ni les
  86 400 s de TURN (§3.5). Elles rejoignent la liste déjà longue de ce dépôt —
  `BPP_MIN`, `FACTEUR_FOCUS`, `PART_DORMANTE_BPS`, `HYSTERESIS`,
  `TAILLE_MAX_SORTIE`.
- **La couverture TURNS de la cible « réseaux restrictifs »** (§2.7) : si P5 ne
  livre pas TURN over TLS sur 443, le constat sera **écrit**, pas contourné.

---

## 9. Hors périmètre v1 (explicitement)

- **OIDC, SSO, 2FA, réinitialisation de mot de passe par courriel** (exige un
  SMTP), **inscription publique** — les comptes sont créés en ligne de commande.
- **Provisionnement de VM** (créer une machine), **exécution réelle** des
  instantanés, migration à chaud : l'interface les nomme, aucun backend ne les
  fait.
- **Scalabilité horizontale de la plateforme** : routage collant ou bus
  inter-instances.
- **Upload d'installeurs** (④), **pont fichiers** (③), **découverte
  d'applications et icônes** (④). Ce document leur ouvre le canal (P3) et leur
  réserve une table (§5) ; il ne les livre pas.
- **Le hub visuel** : P2 livre un écran de connexion fonctionnel ; la direction
  visuelle appartient à ⑥.
- **Facturation, quotas, journal d'audit, interface d'administration.**
- **Terminaison TLS dans le processus** : déléguée à un proxy inverse,
  documentée en P5.
- **Toute modification du produit historique** (`src/`, `web/`, `index.js`,
  `docker-compose.yml`) — cadrage §11 : il reste fonctionnel jusqu'à son
  remplacement.
- **Migration de données** : aucune, il n'y en a pas (cadrage §11).
- **Versionner le protocole superviseur ↔ shell** (§2.6) : le manque est relevé,
  le remède appartient au chantier qui touchera ce protocole. P3 ne le corrige
  pas ; il ne le reproduit pas non plus.

---

## 10. Compatibilité : le chantier D ne s'arrête pas

Le chantier D est vivant, et ⑤ ne doit pas le suspendre. Trois engagements :

1. **P1 ne change pas le protocole du fil.** Même port, même chemin racine,
   mêmes six types relayés, même poignée de main `{role, session}`. L'agent
   et le client d'aujourd'hui fonctionnent **sans recompilation**. Seule la
   variable `PLATEFORME_HOTE` doit être posée (§4, P1 ④).
2. **P2 ajoute un champ, il n'en retire aucun.** La poignée de main devient
   `{role, session, jeton}` ; c'est un ajout compatible côté forme, et un refus
   nouveau côté comportement. Le client est modifié au même commit
   (`client/src/webrtc.ts:193` et `client/src/shell-page.ts:45`, les deux seuls
   sites d'émission côté navigateur).
3. **P3 touche exactement quatre sites nommés** (§3.4), tous listés avec leur
   ligne. C'est le seul sous-bloc qui modifie l'agent, et sa modification est
   d'une ligne par site : un préfixe.

⚠️ **Ce que P3 impose au chantier D, et qu'il faut dire** : après P3, un
superviseur lancé **sans** plateforme n'a pas de préfixe. La règle retenue est
qu'**un préfixe absent vaut le préfixe vide**, ce qui restitue exactement le
comportement d'aujourd'hui (`bureau`, `w-1`) — le mode d'essai local reste donc
possible, `scripts/run-agent.sh` compris. **C'est un choix, et son coût est
qu'une plateforme mal configurée retombe silencieusement dans un espace de noms
partagé.** Il est accepté parce que le refus d'enrôlement (P3 ②) le rend
inatteignable dès que la plateforme est en jeu : sans enrôlement valide, aucune
session ne s'établit du tout.
