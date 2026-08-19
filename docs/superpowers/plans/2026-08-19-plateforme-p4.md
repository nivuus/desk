# Sous-bloc P4 — l'orchestration et l'attribution d'une VM : plan d'implémentation

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** donner une VM à un utilisateur, et une seule ; brancher la source du
préfixe que P3 a laissée en paramètre sans source ; donner à
`agents/fraicheur.ts` l'appelant de production qu'il déclare lui-même
attendre ; et refuser **par un type**, jamais par un silence, tout ce que le
backend d'inventaire ne sait pas faire.

**Architecture:** quatre étages, dont **trois sont purs**.

1. **Le vocabulaire du refus** (`orchestration/refus.ts`,
   `orchestration/interface.ts`) — les motifs et les opérations sont des
   **tableaux `as const` dont les types dérivent**, jamais l'inverse : c'est
   ce qui rend impossible d'ajouter un verbe sans lui donner son code HTTP et
   sa place dans la liste blanche des routes. **PUR.**
2. **Le dépôt de la table `vm`** (`depot/vm.ts`) — il n'existe pas
   aujourd'hui : `admin/enroler-agent.ts:105` est le seul `INSERT INTO vm` du
   dépôt, et **aucun code de production ne lit ni n'écrit
   `vm.utilisateur_id`**.
3. **L'orchestrateur** (`orchestration/inventaire-statique.ts`) — il liste, il
   dit l'état d'une VM en appelant `fraicheur.etatDe`, il attribue, et il
   **refuse `demarrer`, `arreter` et `instantane` par un refus typé**.
4. **Deux routes HTTP** (`http/routes-vm.ts`, `http/routes-session.ts`) et un
   extracteur de jeton porteur (`http/porteur.ts`, **PUR**) — plus la commande
   d'administration `npm run admin:attribuer`, et l'écriture du préfixe dans
   le coffre du navigateur.

**Tech Stack:** inchangée — Node/TypeScript, Vitest, `ws`, `node:sqlite`, `pg`.
🔴 **P4 N'AJOUTE AUCUNE DÉPENDANCE de production.** Le contrôle
`plateforme/src/base/pilote.test.ts:49` (`expect(deps).toEqual(['pg', 'ws'])`)
reste **inchangé** et doit rester **vert** : c'est le témoin de cette
propriété.

🔴 **P4 NE TOUCHE PAS `proto/`, NI `agent/`.** Aucune variante de message, aucun
vecteur, aucune constante de version. Voir **D6** : `PLATEFORME_VERSION` ne
monte pas, et c'est le sous-bloc **G1** de la gestion d'apps qui la portera
à 2.

**Spec :** `docs/superpowers/specs/2026-08-19-plateforme-design.md` (commit
`217a765`), §3.6 et §4 « P4 ».
**Sous-blocs précédents :** `docs/superpowers/plans/2026-08-19-plateforme-p1-resultats.md`,
`…-p2-resultats.md`, `…-p3-resultats.md`, **qui font autorité sur l'état du
code** — la spec, elle, a vieilli sur plusieurs points, et **quatre d'entre eux
portent sur P4 lui-même** (E1, E2, E3, E4).

**Ce plan ne couvre QUE P4.** Rien de la production (P5) : ni Postgres déployé,
ni `docker-compose.plateforme.yml` complet, ni TLS/WSS, ni proxy inverse, ni
`coturn` restreint, ni TURNS sur 443, ni frein sur les routes
d'authentification, ni journalisation structurée, ni `/sante`. **La table
`application` reste vide** : son chemin d'écriture est le sous-projet ④.
**Aucun backend d'hyperviseur n'est écrit** : `demarrer`, `arreter` et
`instantane` ne sont **jamais exécutés**, seule leur voie de refus l'est.

---

## Contraintes globales

### Ce qui a été relevé PAR LA COMMANDE avant d'écrire une ligne

Toutes les valeurs ci-dessous ont été obtenues le **20 août 2026**, sur cette
machine, en lançant réellement la commande. Aucune n'est recopiée d'un document
antérieur. **Ce qui n'a pas été mesuré est marqué comme tel partout ailleurs
dans ce plan.**

| # | Commande | Résultat relevé |
| --- | --- | --- |
| ① | `cd plateforme && npm run test:sqlite` | `Test Files 28 passed (28)`, `Tests 196 passed (196)` |
| ② | `cd plateforme && npm run test:postgres` | `Test Files 28 passed (28)`, `Tests 196 passed (196)` |
| ③ | `cd plateforme && npm run typecheck` | sortie **0** |
| ④ | `cd client && npm test` | `Test Files 22 passed (22)`, `Tests 196 passed (196)` |
| ⑤ | `cd client && npm run typecheck` | sortie **0** |
| ⑥ | `cd proto && npm test` | `Test Files 4 passed (4)`, `Tests 79 passed (79)` |
| ⑦ | `cd proto && npm run typecheck` | sortie **0** |
| ⑧ | `node --version` | `v24.9.0` |
| ⑨ | `docker compose -f docker-compose.plateforme.yml ps` | `guacamole-postgres-plateforme-1 … Up 11 hours (healthy) 127.0.0.1:5433->5432/tcp` |
| ⑩ | la commande des 500 lignes de `CLAUDE.md` | **deux** fichiers au-dessus du plafond : `agent/src/encode.rs` **1536**, `agent/src/windows_source.rs` **630** |
| ⑪ | `grep -cE '^etape ' scripts/verify-all.sh` | **10** |
| ⑫ | `git status --porcelain` | une seule ligne, `?? .playwright-mcp/` |

⚠️ **Les relevés ① à ⑦ sont les références de non-régression de P4.** Toute
tâche qui les fait baisser a cassé quelque chose ; toute tâche qui les fait
monter doit dire **de combien et pourquoi**, et l'annoncer **avant** de lire le
compte — c'est ainsi que D10 a rattrapé un test supprimé par un `Write`
d'écrasement.

🔴 **`cargo test --workspace` N'A PAS ÉTÉ LANCÉ pour établir cette référence**,
et c'est déclaré plutôt que dissimulé : un chantier concurrent (**F1**, pont
fichiers / ProjFS) travaille dans `agent/` en ce moment même — il a commité
`3c0d84c` **pendant la rédaction de ce plan** —, et un compte de tests Rust
relevé aujourd'hui ne serait attribuable ni à lui ni à P4. **P4 ne touche
aucun fichier Rust** (voir le §« Périmètre concurrent ») : aucune de ses
tâches n'a donc à relever cette référence, et la tâche de recette ne
revendiquera **aucun** chiffre `cargo`.

⚠️ **`verify-all.sh` : « dix » et « dix-sept » sont vrais de deux choses
différentes, et je dis lequel j'ai compté.** Le relevé ⑪ ci-dessus est le
nombre d'**appels de la fonction `etape` dans le script** : **dix**, mesuré par
moi ce jour. Le nombre d'**en-têtes `==>` à l'écran** est **dix-sept** — sept
d'entre eux venant de l'intérieur de l'étape `client : npm run
design:verifier` —, et **je ne l'ai PAS re-relevé** : c'est le chiffre mesuré
par la revue transverse de P3 sur une exécution complète
(`journaux-plateforme-p3/temoin-verify-all-cloture-finale.log`). La tâche de
recette de P4 relancera le script et **dira lequel des deux elle compte**.

🔴 **`plateforme` et `proto` sont bien dans `verify-all.sh`** : `plateforme`
y porte **trois** étapes (`test:sqlite`, `test:postgres`, `typecheck`), `proto`
**deux** (`npm test`, `npm run typecheck`). **P4 n'ajoute aucune étape** — il
n'ajoute ni paquet, ni script npm de vérification. Le seul script npm neuf est
`admin:attribuer`, qui n'est pas une étape de vérification.

### Ce que les sondes ont mesuré, et qui gouverne les critères ② et ③

**Deux sondes ont été écrites et lancées avant d'écrire ce plan**, l'une sur
`node:sqlite` (SQLite **3.50.4**, relevé par la sonde de la spec §3.2), l'autre
sur l'instance de `docker-compose.plateforme.yml` (**PostgreSQL 16.15**, relevé
par `SHOW server_version`). Elles portent sur la table `vm` et l'index
`vm_un_utilisateur` **tels qu'ils existent** (`0001-socle.sql:45-58`). Les
scripts sont éphémères et **ne sont pas versés** ; la tâche 15 les rejouera
sous forme de tests, et **ce sont les tests qui feront foi**.

| Geste | SQLite 3.50.4 | PostgreSQL 16.15 |
| --- | --- | --- |
| `UPDATE vm SET utilisateur_id='bob' WHERE id='v1'` (v1 est à alice) — **UPDATE NU** | **OK, 1 ligne** — la VM d'alice est VOLÉE | **OK, 1 ligne** — idem |
| `UPDATE … WHERE id='v1' AND utilisateur_id IS NULL` (v1 est à alice) | **OK, 0 ligne**, aucune exception | **OK, 0 ligne**, aucune exception |
| `UPDATE … WHERE id='vX' AND utilisateur_id IS NULL` (**VM inconnue**) | **OK, 0 ligne**, aucune exception | **OK, 0 ligne**, aucune exception |
| `UPDATE … WHERE id='v2' AND utilisateur_id IS NULL` alors qu'**alice a déjà v1** | **LÈVE** — `UNIQUE constraint failed: vm.utilisateur_id` | **LÈVE** — `duplicate key value violates unique constraint "vm_un_utilisateur"` |
| deux transactions concurrentes sur la **dernière VM libre** | *(non mesuré : `node:sqlite` est mono-processus en mémoire)* | **B BLOQUE sur le verrou de ligne de A, puis rend 0 ligne après le `COMMIT` de A.** Exactement un gagnant, aucune exception, aucun écrasement |
| `SELECT COUNT(*)` relu **à travers le pilote du service** | *(trivialement `number`)* | **`number`** — le `setTypeParser` de `base/pilote-postgres.ts` couvre l'`int8` d'un `COUNT`, mesuré : `[{"n":1}] typeof = number` |

**Trois conséquences, et elles sont la matière des décisions D4 et D5.**

### La règle des 500 lignes : AUCUNE extraction n'est requise, et voici les chiffres

**Relevé par la commande le 20 août 2026** (`{ git ls-files; git ls-files
--others --exclude-standard; } | grep -vE 'node_modules|package-lock|Cargo.lock|/dist/|testdata/|^docs/|^CLAUDE.md' | xargs wc -l | sort -rn`),
pour les seuls fichiers que P4 modifie ou qui bordent son périmètre :

| Fichier | Lignes | Marge | Ce que P4 y ajoute | Porte |
| --- | --- | --- | --- | --- |
| `plateforme/src/agents/canal.test.ts` | **361** | 139 | rien | aucune |
| `plateforme/src/signaling/relais.ts` | **327** | 173 | rien | aucune |
| `plateforme/src/http/routes-auth.ts` | **241** | 259 | **rien** — P4 écrit deux modules SÉPARÉS (voir D8) | aucune |
| `plateforme/src/identite/garde.ts` | **176** | 324 | rien | aucune |
| `plateforme/src/http/serveur.ts` | **169** | 331 | le chaînage des deux routeurs (~20) | aucune |
| `plateforme/src/http/serveur.test.ts` | **160** | 340 | le test du chaînage (~30) | aucune |
| `plateforme/src/depot/session.ts` | **118** | 382 | `compterOuvertesDe` (~15) | aucune |
| `plateforme/src/depot/agent.ts` | **96** | 404 | rien | aucune |
| `client/src/shell-page.ts` | **96** | 404 | rien | aucune |
| `plateforme/src/agents/fraicheur.ts` | **76** | 424 | rien — P4 lui donne un APPELANT, pas une ligne | aucune |
| `client/src/connexion.ts` | **75** | 425 | l'appel à `POST /session` (~25) | aucune |
| `client/src/prefixe.ts` | **72** | 428 | `poserPrefixe`, `effacerPrefixe` (~30) | aucune |
| `plateforme/src/agents/prefixe.ts` | **68** | 432 | rien | aucune |
| `plateforme/src/http/cors.ts` | **39** | 461 | `GET` dans `Access-Control-Allow-Methods` (~2) | aucune |

🔴 **AUCUNE EXTRACTION N'EST REQUISE PAR P4, et ce n'est pas une omission :
c'est un relevé.** Le plus gros fichier de tout le périmètre est un fichier de
test à **361** lignes ; le plus gros fichier de production que P4 touche est
`serveur.ts` à **169**. Le dépôt n'a jamais eu de sous-bloc où la question ne
se posait pas ; celui-ci en est un, et le dire est plus utile que d'inventer une
extraction pour se conformer à une habitude.

⚠️ **Porte chiffrée, à jouer après CHAQUE tâche qui touche un fichier
existant** : relancer `wc -l` sur les fichiers de la table ci-dessus. Si l'un
d'eux dépasse **450**, extraire **avant** de continuer, jamais après. Une
compression de commentaire pour repasser sous la ligne est **interdite** —
`CLAUDE.md` le dit nommément, et D9 l'a payée deux fois dans le même
sous-bloc.

⚠️ **À REMESURER, et non recopié** : `agent/src/transport.rs` était donné à
**495** (marge 5) par le document de résultats de P3. **Relevé par la commande
le 20 août 2026 : il vaut toujours 495**, et `agent/src/encode/arret.rs`
toujours **500** (marge **0**). Les autres marges serrées du dépôt à cette date,
**relevées** : `client/verify-webrtc.mjs` **494**, `agent/src/capture.rs`
**492**, `agent/src/demarrage.rs` **491**. **P4 ne touche aucun de ces six
fichiers**, et aucune de ses tâches n'a de raison de les ouvrir.

⚠️ **`plateforme/` EST DÉJÀ dans le §« Portée » de `CLAUDE.md`** — relevé par
la commande, la ligne énumère `agent/src/`, `client/src/`, `plateforme/`,
`proto/`, `src/`, `web/`, `scripts/`. La divergence texte/commande que la spec
§5 annonçait à ce sujet **a été fermée par P1** ; il n'y a rien à faire ici.
La divergence qui reste ouverte est celle de `client/verify-webrtc.mjs` (hors
`client/src/`), et **elle appartient au propriétaire du dépôt**, pas à P4.

### Les règles de méthode, héritées et non négociables

- **Jamais `git add -A`** : nommer les fichiers, un par un. Un `git add -A` a
  déjà emporté le travail concurrent d'une autre tâche dans un commit qui ne
  compilait pas. 🔴 **Et un autre agent travaille dans le même arbre git en ce
  moment : `HEAD` a bougé de `b0c364a` à `3c0d84c` PENDANT la rédaction de ce
  plan.**
- 🔴 **Jamais `git commit --amend`**, et jamais de commit sans pathspec
  explicite : `git commit` valide TOUT l'index.
- **Un contrôle qu'on n'a jamais vu rouge n'est pas un contrôle.** Chaque test
  neuf est exécuté **avant** l'implémentation et **vu échouer**. Chaque tâche
  nomme la mutation qui le rend rouge.
  🔴 **ET CHAQUE ASSERTION D'UN CRITÈRE À PLUSIEURS ASSERTIONS EXIGE SA PROPRE
  ROUGE, ET SON PROPRE `it()`.** C'est la leçon que P2 a payée : sa rouge ①A
  devait faire tomber « les DEUX assertions » du critère ① ; **elle n'en
  faisait tomber qu'une**, `expect` interrompant le test à la première, si bien
  que la seconde — l'absence d'`ice-config`, c'est-à-dire la fuite même qu'on
  voulait fermer — **n'était éprouvée par rien**. **Ce plan porte quatre
  critères et dix assertions ; chacune a son `it()` et sa rouge nommée.**
- 🔴 **Ce plan est lui-même une source de contrôles vacueux.** D10 en a attrapé
  quatre, dont **trois écrits par son propre plan**, et le 19 août 2026 un plan
  de ce dépôt a prescrit une rouge **impossible** — le contrôle visé ne
  comparait pas dans le sens supposé. **La décision D4 de ce plan est
  exactement ce cas, attrapé avant dispatch** : la rouge que la spec prescrit
  pour le critère ② (« retirer l'index partiel : la double attribution
  réussit ») **ne rougit pas la propriété que le critère énonce**, et c'est
  mesuré. Le doute porte sur ce document.
- **Ne jamais fabriquer une sortie de commande.** D10 a attrapé deux pièces
  fabriquées, dont une inscrite dans `CLAUDE.md` **à l'intérieur d'une
  correction qui dénonçait une affirmation non étayée**. Le mécanisme nommé par
  son auteur : *réutiliser la sortie d'une commande antérieure pour répondre à
  la question d'une AUTRE, sans la relancer.*
- **Un saut est un échec.** `npm run test:postgres` ne se saute pas quand
  l'instance manque : il échoue. Aucun `skipIf`, aucun `runIf`, aucun
  `it.todo` — relevé : la suite de `plateforme/` n'en contient **aucun**
  aujourd'hui, et P4 n'en introduit pas.
- **Les valeurs d'essai sont RÉALISTES, jamais commodes.** C'est la leçon la
  plus chère de P1 : la double passe n'écrivait que des `1_000`, et déclarait
  portable un schéma que Postgres refusait pour toute écriture réelle. Tout
  horodatage d'essai de P4 est de la magnitude d'une époque en millisecondes —
  `plateforme/src/base/harnais.ts:32` porte `INSTANT_MIGRATION =
  1_700_000_000_000`, et c'est la référence.
- **L'horloge est TOUJOURS un paramètre**, jamais `Date.now()` lu dans un
  module. C'est ce qui rend la transition du critère ④ observable. Précédents :
  `agents/fraicheur.ts`, `identite/jeton.ts`, `signaling/ice.ts`,
  `depot/session.ts`.
- **Aucun taux ne sera revendiqué.** Chaque énoncé de résultat porte son nombre
  d'exécutions. **Deux exécutions par critère de recette, pas une**, et la
  convention de P2/P3 est reconduite : **exécution 1 = `sqlite`, exécution 2 =
  `postgres`**, déclaré dans l'en-tête de chaque journal.
- **Toute preuve d'une affirmation portée dans `CLAUDE.md` est versée dans
  git.** D10 a établi par la commande que l'espace de travail de D9
  (`.superpowers/sdd/`, gitignoré) **a disparu**, emportant six constats de
  revue définitivement perdus — et P4 hérite de ce constat sous une forme
  aggravée : le rapport de la tâche 14 de D9, que `CLAUDE.md` invitait encore à
  « relire », **n'existe plus**.
- **Relire chaque citation `fichier:ligne` APRÈS l'avoir écrite.** ⚠️ **Et la
  relire une seconde fois APRÈS avoir exécuté ce qui la déplace** : c'est la
  leçon neuve de P3, dont une citation était **exacte à l'écriture du plan et
  fausse à la fin de son exécution**, parce que le plan lui-même prescrivait de
  déplacer la ligne citée. **Les vingt-cinq citations de ce plan ont été
  relevées par `sed -n 'Np'` et relues une par une** ; celles que P4 déplacera
  sont signalées dans leur tâche.

### Périmètre concurrent — à lire avant de toucher `plateforme/http/` ou `client/`

Deux chantiers vivent dans le même arbre.

**① F1 — pont fichiers / ProjFS.** Relevé par la commande le 20 août 2026 :

```
$ git log --oneline -3
3c0d84c pont(f1): trois rappels asynchrones, l'ordre de PrjFileNameCompare, et le filtre qu'on n'ignore pas
b0c364a recette(s2): verify-all relance APRES la revue transverse, et son journal verse
7b036b0 docs(s2): la section S2, et la revue transverse de fin de branche
```

Il travaille dans `agent/src/pont/` et **il emploie la VM Windows**. Deux
conséquences : **P4 ne touche aucun fichier de `agent/`** (aucune de ses tâches
n'en a besoin), et **la VM est une ressource EXCLUSIVE** — voir la tâche 16.

**② G1 — gestion d'apps, sous-bloc 1.** Son plan existe
(`docs/superpowers/plans/2026-08-19-gestion-apps-g1.md`) et **n'a pas encore
été exécuté** (aucun commit `(g1)` dans `git log`). 🔴 **Il collisionne avec P4
sur QUATRE points, tous nommés en D6 et D8.** Avant de démarrer la famille 4,
relancer `git log --oneline -5 -- plateforme/src/http/` et `git status
--porcelain`, et **ne pas démarrer** si un fichier du périmètre de la tâche y
figure comme modifié.

⛔ **Interdits de MODIFICATION pour toute tâche de ce plan** : `agent/`,
`proto/`, `scripts/`, `src/`, `web/`, `index.js`, `docker-compose.yml`,
`docs/superpowers/plans/2026-08-19-gestion-apps-*`,
`docs/superpowers/plans/2026-08-19-pont-fichiers-*`, et tout fichier
d'`agent/src/pont*`. ⚠️ **`scripts/` est interdit d'ÉCRITURE, pas de
lecture ni d'exécution** : la tâche 16 lance `scripts/run-agent.sh` (et
`build-agent.sh` si et seulement si le binaire est périmé) sans en changer une
ligne. **Contrairement à P3, P4 n'ajoute AUCUNE variable d'environnement** —
c'est la seule raison pour laquelle il échappe au piège que ce dépôt a payé
trois fois (`SUPERVISEUR` en D1, `MULTIFENETRE_REPRISE` en D2, `AUDIO` en D7) :
il n'y a pas de ligne à oublier.

---

## Décisions tranchées — les dix questions que ce plan devait trancher

### D1 — `InventaireStatique` lit la BASE, pas un fichier de configuration

La spec §3.6 écrit : « Backend v1 : **`InventaireStatique`**, alimenté par un
fichier de configuration déclaratif décrivant des VMs qui existent déjà (nom,
adresse, empreinte du secret d'enrôlement) ». **Cette phrase a été écrite avant
que P3 n'existe**, et P3 a livré exactement ces trois champs — en base, avec
leur commande de création.

| Voie | Sort |
| --- | --- |
| fichier déclaratif versionné | **ÉCARTÉE** — il porterait `nom`, `adresse` et l'**empreinte du secret**, c'est-à-dire les trois colonnes que `admin/enroler-agent.ts:105` et `depot/agent.ts:47` écrivent déjà. Deux sources de vérité pour la même chose divergent en silence, et c'est celle qui n'est lue par personne qui gagne le jour où l'on s'y fie |
| fichier déclaratif **non versionné** | **ÉCARTÉE** pour la même raison, plus une seconde : une empreinte de secret dans un fichier de configuration est un chemin de secret de plus, et P5 en a déjà un à traiter (leg n°6 de P3, le secret en clair dans `C:\dev\run-agent.ps1`) |
| variable d'environnement | **ÉCARTÉE** — `config.ts` lit six variables et **aucune n'est une liste** ; y encoder un inventaire serait un format sans grammaire |
| **la base** (`vm` ⟕ `agent_enrole`) | **RETENUE** |

**Ce que « statique » veut dire, alors** : le backend **ne pilote aucun
hyperviseur**. Il n'allume rien, n'éteint rien, ne photographie rien, et ne
crée aucune machine. Son inventaire est **ce qu'un administrateur a enrôlé**,
et il ne peut agir sur le monde qu'à travers la seule colonne dont il est
propriétaire : `vm.utilisateur_id`. C'est cette lecture qui donne leur sens aux
refus typés du §3.6, et elle est **plus forte** que la lecture « fichier » —
un fichier aurait pu être rechargé, ce qui aurait laissé croire à une forme de
gestion.

**Comment on ajoute une VM** : `npm run admin:agent -- --vm <nom> --adresse
<hôte>`, exactement comme aujourd'hui. P4 n'ajoute **aucun** chemin de création
de VM. Il ajoute `npm run admin:attribuer`, qui n'en crée pas non plus : il
pose `utilisateur_id` sur une VM déjà enrôlée (tâche 12).

### D2 — La forme des refus typés : une union dont la table de codes DÉRIVE

La spec §3.6 propose `Resultat = { refus: 'non supporté par ce backend',
operation, backend }`. **Le champ `refus` y est une phrase française**, alors
que tout le reste du service emploie un **code court** : `{refus:'identifiants'}`
(`routes-auth.ts`), `{refus:'methode'}`, `{refus:'corps-trop-grand'}`,
`{refus:'interne'}` (`serveur.ts:76`), et les quatre motifs de `garde.ts:46`.
Une phrase ne se compare pas, ne se traduit pas, et se réécrit sans que rien ne
casse.

**Décision : un code court, et la liste des codes DÉRIVE d'un tableau `as
const`.**

```ts
export const MOTIFS = [
    'non-supporte',        // le backend ne sait pas faire — 501
    'vm-inconnue',         // inconnue, OU appartenant à un autre : le MÊME refus (D8)
    'vm-deja-attribuee',   // la VM a déjà un propriétaire
    'utilisateur-servi',   // l'utilisateur a déjà une VM (l'index partiel)
    'aucune-vm',           // l'utilisateur n'en a aucune — critère ③
    'agent-injoignable',   // `vu_a` trop vieux, ou nul — critère ④
] as const;
export type Motif = (typeof MOTIFS)[number];

export type Resultat =
    | { ok: true }
    | { ok: false; motif: Motif; operation: Operation; backend: string };

export const CODE_HTTP: Record<Motif, number> = { … };
```

🔴 **`Record<Motif, number>` est le remède structurel, et il est choisi
délibérément contre une liste écrite à la main.** Ce dépôt a payé **quatre
fois** un `match`/`switch` catch-all qui tuait un fil en silence
(`pont_media.rs`, D5/D6/D7/D8), et `proto/ts/control.ts:106` porte encore un
`TYPES_AGENT` **écrit à la main sans être confronté à l'union**. Ici, ajouter
un motif sans lui donner son code HTTP est **une erreur de compilation**, que
`npm run typecheck` — étape 10 de `verify-all.sh` — attrape. Et le sens de la
dérivation est celui-là et pas l'autre : **le tableau produit le type**, si
bien que la liste d'exécution et la liste de types sont **le même objet**, et
non deux objets qu'on espère égaux.

⚠️ **`operation` et `backend` sont CONSERVÉS**, tels que la spec les nomme.
`backend` vaut `'inventaire-statique'` — une constante exportée, pas un
littéral recopié —, et son intérêt est le jour où un second backend existera :
sans lui, un refus ne dirait pas *qui* refuse.

⚠️ **Un `Resultat` n'est jamais un `Promise<void>`, jamais un `boolean`, et
jamais une exception.** C'est le point que la spec §3.6 appelle « la décision de
forme la plus importante », et il porte sa raison : un `Promise<void>` qui ne
fait rien serait une panne muette.

### D3 — Le cycle de vie, verbe par verbe : ce que chacun FAIT et ce qu'il REFUSE

```ts
type EtatVm = 'prete' | 'injoignable';        // voir E2

interface Orchestrateur {
    lister(): Promise<Vm[]>;
    etat(vm: string): Promise<EtatVm>;
    demarrer(vm: string): Promise<Resultat>;
    arreter(vm: string): Promise<Resultat>;
    instantane(vm: string, nom: string): Promise<Resultat>;
    attribuer(vm: string, utilisateur: string): Promise<Resultat>;
}
```

| Verbe | Ce qu'il fait en v1 | Ce qu'il refuse |
| --- | --- | --- |
| `lister()` | lit `vm` ⟕ `agent_enrole` et rend l'inventaire **entier** : `{ id, nom, adresse, utilisateurId, prefixe, vuA }` | rien — un inventaire vide est un inventaire, pas un refus |
| `etat(vm)` | lit `vu_a` et appelle **`fraicheur.etatDe(vuA, maintenant())`**. 🔴 **C'est l'appelant de production que `fraicheur.ts:11-20` déclare attendre**, et il ferme le leg n°2 de P3 | une VM inconnue rend **`injoignable`**, pas une exception : c'est vrai, et c'est ce que `fraicheur.ts:39-42` dit déjà d'une VM jamais vue |
| `attribuer(vm, u)` | `UPDATE vm SET utilisateur_id = ? WHERE id = ? AND utilisateur_id IS NULL`, **dans une transaction**, avec ses trois refus (D4) | `vm-inconnue`, `vm-deja-attribuee`, `utilisateur-servi` |
| `demarrer(vm)` | **rien** | `non-supporte`, **501**, **journalisé** |
| `arreter(vm)` | **rien** | `non-supporte`, **501**, **journalisé** |
| `instantane(vm, nom)` | **rien** | `non-supporte`, **501**, **journalisé** — c'est le critère ① |

⚠️ **Les trois refus partagent une seule fonction**, et il faut le dire :
`refuser(operation)` construit le `Resultat` et écrit la ligne de journal. Un
test unique sur `instantane` éprouverait donc la même ligne que `demarrer`.
**Chacun des trois a néanmoins son propre `it()`** — non pour éprouver trois
lignes différentes, mais parce qu'un verbe qui cesserait un jour de passer par
`refuser` ne se verrait autrement pas. Le coût est de deux tests presque
identiques ; il est payé.

⚠️ **`arreter` n'est pas nommé par le critère ① de la spec, `instantane` seul
l'est.** Il est refusé quand même, pour la raison de §3.6 : *une opération que
le backend ne sait pas faire rend un refus typé*. Le déclarer ici évite qu'un
lecteur croie à un oubli.

🔴 **`demarrer` refusé N'EST PAS UNE COMMODITÉ : c'est le contenu du critère
④.** Le cadrage promet « VM injoignable → le hub l'indique, **propose
redémarrage** ». Avec le backend v1, le hub **indique** et **dit qu'il ne peut
pas redémarrer**. Ce que P4 livre est donc l'aveu, pas la fonction — et la
spec §3.6 le nomme « conséquence produit à assumer ».

### D4 — 🔴 L'index partiel n'établit PAS le critère ② tel qu'il est écrit, et c'est MESURÉ

Le critère ② de la spec dit : « **Deux utilisateurs ne peuvent pas recevoir la
même VM** », et sa colonne ROUGE dit : « retirer l'index partiel : la double
attribution réussit ». `0001-socle.sql:53-56` va dans le même sens : « une
seconde attribution est refusée. […] C'est de lui que dépendra le critère 2 de
P4. »

**Les deux sondes du §« Ce que les sondes ont mesuré » réfutent cette
attribution, sur les deux moteurs.** `CREATE UNIQUE INDEX vm_un_utilisateur ON
vm(utilisateur_id) WHERE utilisateur_id IS NOT NULL` (`0001-socle.sql:57-58`)
rend `utilisateur_id` unique **à travers les lignes** : il interdit qu'**un
utilisateur ait deux VMs**. Il n'interdit **rien** à `UPDATE vm SET
utilisateur_id='bob' WHERE id='v1'` quand `v1` est déjà à alice — une VM n'a
qu'un `utilisateur_id`, et l'écraser ne viole aucune unicité. **Mesuré : le vol
réussit, `1 ligne`, sur SQLite 3.50.4 comme sur PostgreSQL 16.15.**

**Décision : le critère ② se scinde en DEUX propriétés, à DEUX gardes
distinctes, avec DEUX rouges distinctes.**

| Propriété | Garde | Rouge, **mesurée atteignable** |
| --- | --- | --- |
| **②a — une VM n'est attribuée qu'une fois** | la clause `AND utilisateur_id IS NULL` de l'`UPDATE`, plus le contrôle de `lignes === 0` | **retirer la clause** : la sonde montre que le vol passe alors, `1 ligne`, sur les deux moteurs |
| **②b — un utilisateur ne reçoit qu'une VM** | l'**index partiel**, qui LÈVE | **retirer l'index de `0001-socle.sql`** : la sonde montre que la seconde VM passe alors |

⚠️ **La rouge ②b mute une migration livrée par P1.** Elle se joue comme P3 a
joué les siennes : mutation, `sha256` **avant et après restauration**, les deux
empreintes comparées et versées dans le journal.

**Et une troisième propriété, que le critère nomme sans la séparer** :
« violation d'index traduite en refus typé, **jamais en 500** ». Les deux
gardes ne se manifestent pas de la même façon — la première rend `0 ligne` sans
exception, la seconde **lève**, et **le texte de l'exception diffère d'un
moteur à l'autre** (`UNIQUE constraint failed: vm.utilisateur_id` contre
`duplicate key value violates unique constraint "vm_un_utilisateur"`).

🔴 **Le code ne doit donc JAMAIS comparer le message de l'exception.** La
traduction se fait ainsi, et l'ordre est structurel :

1. dans une transaction, **lire d'abord** : la VM existe-t-elle ? a-t-elle déjà
   un propriétaire ? l'utilisateur en a-t-il déjà une ? — trois refus typés,
   trois motifs, **avant toute écriture** ;
2. **puis** tenter l'`UPDATE` conditionnel. `lignes === 0` ⇒ `vm-deja-attribuee`
   (la course a été perdue entre la lecture et l'écriture) ;
3. **et si l'`UPDATE` lève** : **relire** l'état, et ne traduire que ce que la
   relecture explique — l'utilisateur a bien une autre VM ⇒
   `utilisateur-servi`. **Sinon, RELANCER l'exception.**

⚠️ **Le point 3 est le seul endroit du service où une exception est
rattrapée, et il ne doit pas devenir un `catch` muet.** Un `catch` qui
traduirait *toute* exception en `utilisateur-servi` avalerait une base
injoignable et la présenterait comme un refus métier — la panne muette exacte
que la spec §6 interdit (« il ne démarre pas dégradé »). **Son test existe :
un `Pilote` factice dont `executer` lève une erreur étrangère doit faire
REMONTER l'exception**, et la rouge est de retirer le `throw` final.

### D5 — La course sur la dernière VM libre : mesurée, et un seul gagnant

La question est posée par la mission, et elle a été **mesurée** plutôt que
raisonnée. Sur PostgreSQL 16.15, en isolation par défaut (`READ COMMITTED`),
deux transactions faisant le même `UPDATE … WHERE id = ? AND utilisateur_id IS
NULL` sur la **même** VM libre :

- **A** obtient `1 ligne` et garde sa transaction ouverte ;
- **B bloque** — mesuré : la sonde a observé B en attente pendant les 600 ms où
  A n'avait pas encore validé ;
- après le `COMMIT` de A, **B rend `0 ligne`** : Postgres réévalue la clause
  `utilisateur_id IS NULL` sur la ligne mise à jour, qui ne la satisfait plus.

**Exactement un gagnant. Le perdant reçoit `0 ligne`, donc un refus typé
`vm-deja-attribuee` — jamais une exception, jamais un 500, et jamais un
écrasement silencieux.**

⚠️ **Portée exacte, et pas plus** : **une exécution**, sur **une** paire de
transactions, sur **Postgres seul**. Le cas n'a **pas** été mesuré sur
`node:sqlite`, et ne peut pas l'être utilement : la suite l'ouvre en
`:memory:`, dans un processus unique, où il n'y a pas de concurrence à
observer. **Rien n'est établi au-delà de deux concurrents**, et rien de ce qui
arriverait sous une autre isolation. Le test de la tâche 6 reproduira le cas
**séquentiellement** (le perdant joue après le gagnant), ce qui éprouve la
traduction du refus et **non** la sérialisation par le moteur.

🔴 **Ce que cela impose au code** : l'attribution vit **dans une
`Pilote.transaction`**, et la lecture préalable du point 1 de D4 n'est
**pas** une garantie — c'est un moyen de nommer le bon motif. **La garantie est
la clause `IS NULL` de l'`UPDATE`**, parce qu'elle est réévaluée par le moteur
au moment de l'écriture. Un code qui lirait puis écrirait sans clause serait
juste dans les tests et faux en production, et le test séquentiel ne le verrait
pas. **C'est écrit dans le code, à côté de la clause.**

### D6 — `PLATEFORME_VERSION` ne monte PAS, et voici comment P4 cohabite avec G1

**Relevé par la commande** : `proto/src/plateforme.rs:36` porte
`pub const PLATEFORME_VERSION: u8 = 1;` et `proto/ts/plateforme.ts:16`
`export const PLATEFORME_VERSION = 1;`.

**Décision : P4 ne la touche pas, et ne touche AUCUN fichier de `proto/`.**
La raison se lit, elle ne se suppose pas : le canal `/agent` porte cinq
variantes — `enroler`, `battement` (montantes), `enrole`, `battement-recu`,
`refus` (descendantes) —, et **P4 n'a besoin d'aucune**. La fraîcheur qu'il
consomme est déjà écrite en base par le battement existant
(`agents/canal.ts` → `depot/agent.ts:94 marquerVu`), et les trois verbes
d'action sont **refusés sans jamais parler à l'agent**. Il n'y a rien à
ajouter au protocole, donc rien à versionner.

🔴 **ET C'EST UNE DÉCISION DE COHABITATION, PAS UNE COMMODITÉ.** Le sous-bloc
**G1** de la gestion d'apps prévoit explicitement de porter
`PLATEFORME_VERSION` à **2** (`docs/superpowers/plans/2026-08-19-gestion-apps-g1.md`,
sa décision D10 et sa tâche 2), en y ajoutant `Catalogue`, `Lancer` et
`Lancee`. **Deux chantiers qui monteraient tous deux vers « 2 » produiraient
deux protocoles différents portant le même numéro** — c'est-à-dire exactement
la panne que la vérification de version existe pour empêcher, et elle serait
**silencieuse** : les vecteurs de l'un passeraient la vérification de l'autre.

**La règle retenue, et elle est simple : `PLATEFORME_VERSION` a un seul
propriétaire à la fois, et c'est G1.** P4 n'en est pas propriétaire, ne la lit
pas, ne l'écrit pas. Si un jour P5 ou un successeur de P4 doit ajouter une
variante, il **rebumpe après G1**, jamais en parallèle — et le prix est celui
que G1 nomme déjà : rebâtir et redéployer l'agent et le service ensemble.

⚠️ **Corollaire opérationnel, à ne pas manquer** : G1 impose de **rebâtir
l'agent** avant sa recette. **P4 n'impose rien de tel** — aucun binaire d'agent
n'a besoin d'être refait pour P4, et sa corroboration sur VM (tâche 16) peut
donc se jouer sur le binaire de P3. Si G1 a déjà été fusionné quand P4 se
recette, **c'est G1 qui impose le rebâtissage**, pas P4 ; la tâche 16 relèvera
alors la taille du binaire, comme le piège du `cargo build` de 0,13 s l'exige.

### D7 — La route ne rend PAS la configuration ICE, ni le nom de session composé

La spec §4 « P4 » écrit que la plateforme « rend l'identifiant de session, le
préfixe **et la configuration ICE** ». **Elle rend le préfixe, et lui seul.**
Deux raisons, dont la première est décisive.

**① La configuration ICE est PAR SESSION, et une route HTTP n'en connaîtrait
qu'une sur N.** Relevé : `plateforme/src/signaling/ice.ts:39` compose
``const username = `${expiration}:${session}`;`` — l'identifiant TURN **porte
le nom de session**, et son `credential` en est le HMAC. Une VM ouvre
`<préfixe>:bureau` **plus une session par fenêtre** (`<préfixe>:w-1`,
`w-2`, …). La route ne pourrait donc servir que la session de contrôle, et le
relais continuerait de servir toutes les autres. **Un second chemin de
délivrance qui couvre une session sur N n'est pas une simplification : c'est un
second endroit à garder synchrone, dont on n'a pas le droit de se servir.**

**② Le nom de session composé serait une TROISIÈME copie de `bureau`.** La
constante vit déjà en Rust (`agent/src/superviseur/protocole.rs`) et en
TypeScript (`client/src/shell-page.ts:19`, qui fait
`composer(prefixe, NOM_SESSION_DE_CONTROLE)`), et la spec §2.6 nomme déjà cette
duplication comme un défaut connu. En ajouter une troisième, côté service,
pour économiser une concaténation au navigateur, serait aggraver un défaut
qu'on sait nommer.

**Ce que la route rend, alors** : `{ vm, nom, prefixe, etat }`. Le navigateur
**a déjà tout ce qu'il faut** — `client/src/prefixe.ts:69 composer` construit
`<préfixe>:bureau`, et `client/src/webrtc.ts` reçoit son `ice-config` du relais
comme aujourd'hui. **Aucun code client n'est privé de quoi que ce soit par
cette décision** : c'est ce qui la rend gratuite.

⚠️ **Elle NE rend PAS non plus `adresse`.** L'adresse d'une VM est une
information de topologie interne dont le navigateur n'a aucun usage — il parle
au signaling, jamais à la VM. La rendre l'exposerait à tout utilisateur
authentifié sans qu'aucun besoin ne l'exige. `Orchestrateur.lister()` la porte,
parce que l'administration en a besoin ; les deux routes ne la recopient pas.

### D8 — `attribuer` n'est PAS exposée sur HTTP, et le refus HTTP n'énumère pas

**Il n'existe aucun rôle d'administration dans ce service.** Relevé :
`identite/jeton.ts:47` ne connaît que `'utilisateur' | 'agent'`, et
`config.ts` n'a aucune variable d'administrateur. Une route HTTP qui
attribuerait une VM serait donc, au mieux, ouverte à tout utilisateur
authentifié — c'est-à-dire une escalade de privilège offerte.

**Décision : `attribuer` s'expose par la ligne de commande**, sur le précédent
exact de `npm run admin:utilisateur` et `npm run admin:agent` :
`npm run admin:attribuer -- --email <courriel> --vm <nom|id>`, plus
`--detacher` pour rendre la VM au vivier (tâche 12).

**La surface HTTP de P4 est donc de deux routes**, et le contrat de chacune est
celui que `routes-auth.ts:93` a établi : `(req, rep, deps) => Promise<boolean>`,
`true` = servie, `false` = pas mon chemin.

| Route | Succès | Refus |
| --- | --- | --- |
| `GET /vm` | **200** `{ vms: [{ id, nom, etat, prefixe, sessions_ouvertes }] }` | 401 `jeton-*`, 403 `jeton-agent` |
| `POST /vm/:id/:operation` | *(aucun : les trois opérations refusent)* | 401/403 ; **404** `vm-inconnue` ; **501** `{motif:'non-supporte', operation, backend}` |
| `POST /session` | **200** `{ vm, nom, prefixe, etat: 'prete' }` | 401/403 ; **409** `aucune-vm` ; **503** `agent-injoignable` + `redemarrage` |

🔴 **`:operation` n'est pas lue puis validée : la route ne reconnaît QUE les
opérations de la liste blanche, et cette liste DÉRIVE de l'union.** Un chemin
portant une opération inconnue n'est pas « refusé » : il n'est **pas servi**,
`servir` rend `false`, et le 404 générique de `serveur.ts:64-65` s'applique.
C'est une liste blanche, jamais une liste noire — le mot que
`serveur.ts:15-17` emploie déjà pour le routage des montées WebSocket. Et
`attribuer` **n'y figure pas**, ce qu'un test assère nommément : sans lui,
ajouter `attribuer` à `OPERATIONS_HTTP` un jour de fatigue ouvrirait
l'attribution à tout le monde sans qu'aucun test ne bouge.

🔴 **`vm-inconnue` couvre DEUX cas et c'est délibéré** : la VM n'existe pas,
**ou** elle appartient à un autre. Distinguer les deux ferait un **oracle
d'énumération** — un utilisateur apprendrait quelles VMs existent en lisant le
code de retour. C'est la règle du critère ② de P3 (`agents/enrolement.ts`,
deux refus identiques caractère pour caractère) et celle de
`routes-auth.ts:4-8`, appliquées ici pour la troisième fois.

⚠️ **DIVERGENCE AVEC G1, déclarée et non tranchée par moi.** La décision D9 du
plan de G1 retient, pour ses propres routes, `403 { refus: 'vm-etrangere' }`
sur une VM appartenant à autrui — c'est-à-dire **un oracle**, distinct du 404
d'une VM inconnue. Les deux chantiers ne peuvent pas avoir raison en même
temps. **Ce plan retient le refus non énumérant** ; si G1 est fusionné en
premier, la tâche 9 de P4 **signale l'écart dans son rapport et n'aligne
rien** — unifier est une décision qui appartient au propriétaire du dépôt, pas
à la seconde branche arrivée.

⚠️ **Trois autres points de collision avec G1, tous nommés** :
`plateforme/src/http/cors.ts` (les deux ont besoin de `GET` dans
`Access-Control-Allow-Methods` — **modification identique et idempotente**, la
seconde branche la trouvera faite), `plateforme/src/http/serveur.ts` (les deux
chaînent un routeur neuf avant le 404 — **la seconde doit relire le fichier**,
jamais présumer sa forme), et l'extraction du jeton porteur : G1 prévoit de
lire `Authorization: Bearer` **dans ses propres routes**. **P4 l'extrait dans
`http/porteur.ts`, pur et testé** ; si G1 arrive en premier avec sa copie en
ligne, la tâche 7 de P4 **écrit quand même le module** et la tâche 9 l'emploie,
en signalant la duplication — deux copies d'une garde d'authentification sont
une dette, mais réécrire les routes d'un chantier voisin en est une pire.

### D9 — La source du préfixe côté navigateur : le coffre, écrit à la connexion

`client/src/prefixe.ts:11-17` décrit précisément ce qui manque : « **LA SOURCE
DÉFINITIVE DU PRÉFIXE EST LA PLATEFORME, ET ELLE N'EXISTE PAS ENCORE.** […]
P3 transforme donc le littéral en PARAMÈTRE ; **P4 branchera la source, et
n'aura qu'à écrire dans le coffre.** »

**Décision : `client/src/connexion.ts`, une fois le jeton posé, appelle
`POST /session` et écrit le préfixe dans `localStorage` sous la clé existante
`CLE_PREFIXE` (`client/src/prefixe.ts:29`).** `client/src/shell-page.ts:18-19`
le relit sans changer d'une ligne.

**La RÈGLE descend dans le module pur, le CÂBLAGE reste dans `connexion.ts`** —
c'est la convention que `connexion.ts:5-9` s'impose à lui-même (« toute règle
que ce fichier porterait doit descendre dans `jeton.ts` »). `prefixe.ts` gagne
donc deux fonctions **pures et testées** :

- `poserPrefixe(coffre, prefixe)` — **LÈVE sur la chaîne vide.** Un préfixe
  vide écrit dans le coffre ne serait pas neutre : `lirePrefixe` (l. 62-64)
  retomberait sur `?prefixe=` puis sur `''`, et la page rejoindrait
  silencieusement l'espace de noms partagé. **C'est la panne muette exacte que
  la spec §10 nomme**, et une exception est ce qui l'empêche de passer
  inaperçue ;
- `effacerPrefixe(coffre)` — appelée quand l'utilisateur n'a **aucune** VM :
  laisser en place le préfixe d'une VM qu'on n'a plus ferait ouvrir des
  sessions au nom d'une autre machine.

⚠️ **Sur un refus, `connexion.ts` AFFICHE le motif et NE REDIRIGE PAS.**
Rediriger vers une shell qui rejoindrait l'espace de noms partagé serait
précisément le coût que ce sous-bloc existe pour réduire. Le coût de ce choix
est nommé : **un développeur sans VM enrôlée reste sur l'écran de connexion**.
Le mode d'essai local reste ouvert par le chemin existant et **inchangé** —
`?prefixe=` sur l'URL de la shell (`client/src/prefixe.ts:64`), qui fonctionne
toujours puisque le coffre est vide. **Aucun fichier `.html` n'est modifié.**

🔴 **Ce que P4 ne solde PAS, et il faut le dire ici plutôt qu'en note :** le
préfixe reste **par VM**, pas par session, et `signaling/propriete.ts` reste
**en mémoire**. Après un redémarrage du service, deux clients humains de la
même VM retrouvent le même préfixe et `<préfixe>:w-1` redevient revendicable.
**Le legs n°4 de P3 reste RÉDUIT et NON SOLDÉ**, et P4 ne le réduit pas
davantage : il lui donne seulement la source qui manquait.

### D10 — Le lecteur de `session.utilisateur_id` : un compte, dans `GET /vm`

Le legs n°3 de P3 dit : « `session.utilisateur_id` attend toujours son
lecteur ». Relevé : la colonne est écrite par la chaîne
`garde.ts:160` → `relais.ts:247` → `trace.ts:135` → `depot/session.ts:78`, et
**le seul `SELECT` qui la ramène est `depot/session.ts:113 lireParNom`, dont
aucun appelant n'est du code de production**.

**Décision : `depot/session.ts` gagne `compterOuvertesDe(p, utilisateurId)`, et
`GET /vm` rend `sessions_ouvertes` par VM.** C'est un lecteur **réel** — le hub
en a besoin pour dire « vous avez une session ouverte » — et **minimal** : une
fonction de dépôt et un champ.

⚠️ **Ce que ce lecteur N'ÉTABLIT PAS.** Il compte des lignes `session` non
closes ; il ne dit **pas** qu'une session média est vivante. `depot/session.ts`
et `signaling/trace.ts` documentent déjà l'écart : le média survit au
redémarrage du service alors que la ligne est close par le balayage
(`MOTIF_BALAYAGE`), et une ligne ouverte peut correspondre à un pair parti sans
que la déconnexion ait été vue. **Le champ s'appelle donc `sessions_ouvertes`
et non `sessions_actives`** ; le nom porte la réserve.

⚠️ **Ce lecteur ne referme pas le legs pour autant, et le rapport le dira** :
`lireParNom` reste sans appelant de production, et `vm.vue_a` reste une colonne
**orpheline** — P4 lit `agent_enrole.vu_a`, jamais `vm.vue_a`. Le `SELECT` de
`depot/vm.ts` énumère ses colonnes explicitement et **exclut `vue_a`**, avec
son commentaire : c'est la seule garde bon marché contre un successeur qui la
croirait renseignée.

---

## Divergences relevées entre la spec, le code réel, et ce que P4 doit faire

### E1 — La spec §3.6 annonce un fichier de configuration ; P4 lit la base

Voir **D1**. **Motif** : les trois champs du fichier (`nom`, `adresse`,
empreinte du secret) sont déjà écrits en base par `admin/enroler-agent.ts:105`
et `depot/agent.ts:47`, et deux sources de vérité divergent en silence.
**Sort** : le fichier n'existe pas ; « statique » signifie « ne pilote aucun
hyperviseur », lecture plus forte que « rechargeable ».

### E2 — `EtatVm` a quatre états, dont DEUX qu'aucun backend v1 ne peut produire

La spec §3.6 pose `type EtatVm = 'arretee' | 'demarrage' | 'prete' |
'injoignable'`. Or `plateforme/src/agents/fraicheur.ts:43` — écrit **pour P4**
par P3 — pose `type EtatAgent = 'prete' | 'injoignable'`, avec son argument
(l. 39-42) : « Deux états seulement : ni « peut-être », ni « inconnue ». »

🔴 **`arretee` et `demarrage` ne sont productibles par AUCUN code de P4** : ils
supposent un hyperviseur qu'aucun backend ne pilote, et la spec §8 range
explicitement « rien du comportement d'un hyperviseur réel » hors périmètre.
Les écrire produirait deux variantes que rien n'émet — du code mort **dans un
type**, c'est-à-dire l'espèce la plus difficile à retirer.

**Sort : `EtatVm = EtatAgent`, deux états, réexporté depuis
`orchestration/interface.ts` pour que le nom de la spec existe.** Le jour où un
backend d'hyperviseur élargira l'union, **toute exhaustivité qui en dépend
cassera à la compilation** — c'est la bonne panne, bruyante, et c'est la raison
pour laquelle la table de codes de D2 est un `Record<>`.

### E3 — 🔴 La rouge que la spec prescrit pour le critère ② ne rougit pas ce que le critère énonce

Voir **D4**, mesuré sur les deux moteurs. **Sort** : deux propriétés, deux
gardes, deux rouges — plus une troisième assertion pour « jamais un 500 ».
**C'est la divergence la plus lourde de ce plan**, parce qu'elle aurait produit
un critère vert sur un produit qui laisse voler une VM.

### E4 — La spec fait rendre la configuration ICE par la route ; elle est PAR SESSION

Voir **D7**, et `signaling/ice.ts:39`. **Sort** : la route rend le préfixe ; le
relais continue de servir `ice-config` à chaque session, comme aujourd'hui.
**Aucun code client n'est privé de quoi que ce soit.**

### E5 — `depot/vm.ts` n'existe pas, alors que la spec §5 le nomme

L'arborescence de la spec §5 liste `depot/vm.ts`. Relevé : le répertoire
`plateforme/src/depot/` contient `agent.ts`, `jeton.ts`, `session.ts`,
`utilisateur.ts` — **et rien pour la table `vm`**. **Sort** : P4 le crée
(tâche 4). C'est mineur, et c'est dit pour qu'un lecteur de la spec ne le croie
pas livré.

### E6 — Aucune extraction de jeton HTTP n'existe, et la garde du relais N'EST PAS réutilisable

`identite/garde.ts:69` a pour signature
`verifier(poignee: { role: Role; session: string; jeton?: unknown }): Verdict`
— taillée pour une poignée de main WebSocket, et elle porte le registre
d'appartenance. **Elle est branchée exclusivement sur le relais**
(`serveur.ts:97`, `relais.ts:206`). Rien, nulle part, ne lit l'en-tête
`Authorization`.

**Sort** : `http/porteur.ts`, **pur**, qui lit l'en-tête, appelle
`verifierJeton(jeton, secret, maintenant)` (`identite/jeton.ts:130`) et exige
`type === 'utilisateur'`. Le précédent d'un consommateur direct de
`verifierJeton` hors de la garde est `agents/canal.ts`, dont l'en-tête explique
pourquoi il ne partage rien avec elle.

🔴 **Le refus du type `agent` n'est pas décoratif** : `identite/jeton.ts:40-46`
énumère les deux confusions et dit qu'elles sont graves toutes les deux. Un
jeton d'agent qui ouvrirait `GET /vm` verrait l'inventaire d'un humain. **Deux
rouges distinctes**, une par sens, comme la leçon E5 de P3 l'exige.

### E7 — `depot/utilisateur.ts` n'a pas de `lireParId`, et P4 n'en a pas besoin

Relevé : `creerUtilisateur`, `lireParEmail`, `remplacerEmpreinte`, et rien
d'autre. **Sort** : la commande d'administration prend `--email` et emploie
`lireParEmail` ; les routes HTTP emploient le **sujet du jeton**, qui *est*
l'`utilisateur.id` (`routes-auth.ts:232` le signe ainsi). **Aucun
`lireParId` n'est ajouté** — un accesseur sans appelant est exactement ce que
`fraicheur.ts` a dû déclarer orphelin pendant tout un sous-bloc.

⚠️ **Ce que cela suppose, et qui n'est pas vérifié** : un jeton reste valide
jusqu'à son expiration même si l'utilisateur disparaissait de la base. Il
n'existe aujourd'hui **aucun chemin de suppression d'utilisateur**, donc le cas
n'est pas atteignable ; la spec §3.5 range par ailleurs la révocation immédiate
hors périmètre v1. **Déclaré, non corrigé.**

### E8 — `changes = 0` confond TROIS causes, et c'est mesuré

Les sondes montrent que l'`UPDATE` conditionnel rend `0 ligne` aussi bien pour
une **VM inconnue** que pour une **VM déjà prise** que pour une **ré-attribution
au même utilisateur**. Un code qui déciderait du motif sur ce seul nombre
rendrait un refus qui n'informe pas.

**Sort** : la lecture préalable de D4, point 1 — et la scission
administration / utilisateur de D8, parce que distinguer ces trois cas **est
un oracle** sur une route publique et **est nécessaire** sur une commande
d'administration.

### E9 — La reprise du canal `/agent` n'a JAMAIS été exercée, et P4 s'appuie dessus

Legs n°8 de P3, énoncé tel quel : « aucune coupure n'a été provoquée, ni au
banc ni sur la VM. Son calcul de délai est pur et testé ; le comportement du
socket, non. »

🔴 **P4 en dépend directement.** `vu_a` n'avance que par le battement du canal
(`agent/src/plateforme.rs:45` : `PERIODE_BATTEMENT = 30 s` ;
`agents/canal.ts` → `depot/agent.ts:94`). Si le canal tombait et ne se
reprenait pas, `vu_a` cesserait d'avancer, `etatDe` rendrait `injoignable`
au bout de `SEUIL_INJOIGNABLE_MS` (**90 s**,
`plateforme/src/agents/fraicheur.ts:37`), et **P4 afficherait cet état sans
pouvoir rien y faire**, `demarrer` étant refusé.

**Sort : P4 ne l'exerce PAS non plus, et le dit deux fois** — dans le rapport
de résultats et dans `CLAUDE.md`. **Ce n'est pas un défaut de P4** : c'est un
legs qui devient visible parce que P4 est le premier à consommer `vu_a`.

⚠️ **Le rapport `SEUIL_INJOIGNABLE_MS / PERIODE_BATTEMENT` vaut 3**, ce qui
laisse la place à deux battements perdus — et
`agent/src/plateforme.rs:36-45` documente que **les deux constantes vivent dans
des dépôts distincts et se recalibrent ENSEMBLE**. **Aucune des deux n'est
calibrée**, et P4 n'en calibre aucune.

### E10 — `entetesCors` annonce `POST, OPTIONS` ; `GET /vm` a besoin de `GET`

Relevé : `plateforme/src/http/cors.ts:36` pose
`'Access-Control-Allow-Methods': 'POST, OPTIONS'`. **Sort** : la valeur devient
`'GET, POST, OPTIONS'` (tâche 8). ⚠️ **G1 a besoin de la même modification** —
elle est **identique et idempotente**, et la seconde branche arrivée la
trouvera faite. C'est la forme la moins mauvaise d'une collision, et il faut la
nommer pour qu'aucune des deux ne la refasse à l'envers.

### E11 — Le catch-all de `TYPES_RELAYES` ne mord pas ici, et il faut le vérifier plutôt que le supposer

`plateforme/src/signaling/relais.ts:58` porte `TYPES_RELAYES`, une **liste
blanche fermée** : tout type de message non listé est écarté. Le dépôt a payé
quatre fois un catch-all silencieux du même genre.

**Sort : P4 ne fait transiter AUCUN message par le relais**, ni par le canal
`/agent`. Sa surface est HTTP et la ligne de commande. **`TYPES_RELAYES` n'a
pas à changer**, et une tâche de la revue transverse le vérifiera plutôt que de
le supposer — parce que c'est exactement le genre d'affirmation qu'on écrit
sans la relire.

### E12 — Le rapport de la tâche 14 de D9 n'existe plus, et `CLAUDE.md` invite encore à le relire

`CLAUDE.md` (section D9) porte, à propos du legs n°10, la phrase « Le
contredire est le premier travail de qui le relira », puis sa propre
réfutation par D10 : le rapport vivait dans `.superpowers/sdd/`, **gitignoré et
disparu**. **Sort** : hors périmètre de P4 — mais la règle qu'il en tire est
appliquée sans exception ici : **aucune affirmation de ce plan, du document de
résultats ou de `CLAUDE.md` ne s'adosse à un rapport de tâche**. Tout s'adosse
à un fichier de `journaux-plateforme-p4/`, à un commit, ou à une commande
relancée à la clôture.

---

## Structure des fichiers

```
plateforme/
  package.json                          MODIFIÉ — le script `admin:attribuer` (T12)
  src/
    orchestration/refus.ts              NEUF — MOTIFS, Resultat, CODE_HTTP (T1)
    orchestration/refus.test.ts         NEUF (T1)
    orchestration/interface.ts          NEUF — Vm, EtatVm, Operation, Orchestrateur (T2)
    orchestration/interface.test.ts     NEUF (T2)
    orchestration/selection.ts          NEUF — PUR : vmsDe, laVmDe (T3)
    orchestration/selection.test.ts     NEUF (T3)
    orchestration/inventaire-statique.ts       NEUF (T6)
    orchestration/inventaire-statique.test.ts  NEUF (T6)
    depot/vm.ts                         NEUF (T4)
    depot/vm.test.ts                    NEUF (T4)
    depot/session.ts                    MODIFIÉ — compterOuvertesDe (T5)
    depot/session.test.ts               MODIFIÉ (T5)
    http/porteur.ts                     NEUF — PUR (T7)
    http/porteur.test.ts                NEUF (T7)
    http/cors.ts                        MODIFIÉ — GET (T8)
    http/cors.test.ts                   MODIFIÉ (T8)
    http/routes-vm.ts                   NEUF (T9)
    http/routes-vm.test.ts              NEUF (T9)
    http/routes-session.ts              NEUF (T10)
    http/routes-session.test.ts         NEUF (T10)
    http/serveur.ts                     MODIFIÉ — le chaînage (T11)
    http/serveur.test.ts                MODIFIÉ (T11)
    admin/attribuer-vm.ts               NEUF (T12)
    admin/attribuer-vm.test.ts          NEUF (T12)
client/
  src/prefixe.ts                        MODIFIÉ — poserPrefixe, effacerPrefixe (T13)
  src/prefixe.test.ts                   MODIFIÉ (T13)
  src/connexion.ts                      MODIFIÉ — l'appel à POST /session (T14)
docs/superpowers/plans/
  2026-08-19-plateforme-p4-resultats.md        NEUF (T19)
  journaux-plateforme-p4/                      NEUF (T15, T16)
CLAUDE.md                                      MODIFIÉ (T18)
```

🔴 **AUCUNE MIGRATION.** `vm`, son index partiel, `agent_enrole` et `session`
existent tous, avec toutes leurs contraintes. C'est le legs n°2 de P1 — « toute
contrainte doit naître avec sa table », parce que SQLite ne sait pas l'ajouter
par `ALTER TABLE` — **appliqué d'avance par P1 pour le compte de P4**, et il
paie ici exactement comme prévu.

⚠️ **Corollaire à écrire dans le rapport plutôt qu'à découvrir plus tard** : P4
ne créant aucune colonne, il n'a pas eu à choisir entre `ADD COLUMN … NOT NULL`
(qui ne passe que sur une table vide) et `ADD COLUMN` nullable. **Un sous-bloc
ultérieur qui voudrait, par exemple, `vm.attribuee_a`, se heurtera à ce mur**
— et il devra la faire naître nullable, **et la nommer en `_a`**, sans quoi le
lint de `plateforme/src/base/sous-ensemble.test.ts:46`
(`/\b\w+_a\s+INTEGER\b/i`) ne la verrait pas et un `Date.now()` déborderait sur
Postgres. **P4 a délibérément choisi de ne pas créer cette colonne d'avance** :
une colonne sans écrivain est du code mort qu'aucun test ne rougit.

---

## Interfaces partagées

```ts
// orchestration/refus.ts
export const BACKEND_STATIQUE = 'inventaire-statique';
export const MOTIFS = [ 'non-supporte', 'vm-inconnue', 'vm-deja-attribuee',
                        'utilisateur-servi', 'aucune-vm', 'agent-injoignable' ] as const;
export type Motif = (typeof MOTIFS)[number];
export type Resultat =
    | { ok: true }
    | { ok: false; motif: Motif; operation: Operation; backend: string };
export const CODE_HTTP: Record<Motif, number>;      // dérivé, exhaustif par construction
export function refuser(motif: Motif, operation: Operation, backend?: string): Resultat;

// orchestration/interface.ts
export type EtatVm = EtatAgent;                     // réexport de agents/fraicheur.ts — E2
export interface Vm { id: string; nom: string; adresse: string;
                      utilisateurId: string | null; prefixe: string | null;
                      vuA: number | null }
export const OPERATIONS = ['lister','etat','demarrer','arreter','instantane','attribuer'] as const;
export type Operation = (typeof OPERATIONS)[number];
export const OPERATIONS_HTTP = ['demarrer','arreter','instantane'] as const satisfies readonly Operation[];
export const OPERATIONS_HORS_HTTP = ['lister','etat','attribuer'] as const satisfies readonly Operation[];
export interface Orchestrateur { /* voir D3 */ }

// orchestration/selection.ts — PUR
export function vmsDe(inventaire: readonly Vm[], utilisateurId: string): Vm[];
export function laVmDe(inventaire: readonly Vm[], utilisateurId: string): Vm | undefined;

// depot/vm.ts
export interface LigneVm { id; nom; adresse; utilisateur_id: string|null;
                           prefixe_session: string|null; vu_a: number|null }
export async function lister(p: Pilote): Promise<LigneVm[]>;
export async function lireParId(p: Pilote, id: string): Promise<LigneVm|undefined>;
export async function lireParNom(p: Pilote, nom: string): Promise<LigneVm|undefined>;
export async function attribuerSiLibre(p: Pilote, vmId: string, utilisateurId: string): Promise<number>;
export async function detacher(p: Pilote, vmId: string): Promise<number>;

// depot/session.ts (ajout)
export async function compterOuvertesDe(p: Pilote, utilisateurId: string): Promise<number>;

// http/porteur.ts — PUR
export type VerdictPorteur =
    | { ok: true; utilisateurId: string }
    | { ok: false; motif: 'jeton-absent'|'jeton-invalide'|'jeton-expire'|'jeton-agent'; code: 401|403 };
export function lirePorteur(entetes: Record<string, string|string[]|undefined>,
                            secret: string, maintenant: number): VerdictPorteur;

// client/src/prefixe.ts (ajouts)
export interface CoffreEcrivable extends Coffre { setItem(c: string, v: string): void;
                                                  removeItem(c: string): void }
export function poserPrefixe(coffre: CoffreEcrivable, prefixe: string): void;   // LÈVE sur ''
export function effacerPrefixe(coffre: CoffreEcrivable): void;
```

---

## Ordre et parallélisme

| Famille | Tâches | Dépend de | Parallélisable | Paquets touchés |
| --- | --- | --- | --- | --- |
| 1 — le socle **pur** | 1, 2, 3 | 1 ← 2 (pour `Operation`) ; 3 ← 2 | 2 démarre seule, puis 1 et 3 ensemble | `plateforme/` |
| 2 — la persistance | 4, 5 | rien | entre elles, et avec la famille 1 | `plateforme/` |
| 3 — l'orchestrateur | 6 | 1, 2, 3, 4 | — | `plateforme/` |
| 4 — HTTP | 7, 8, 9, 10, 11 | 7 ← rien ; 8 ← rien ; 9 ← 3, 5, 6, 7, 8 ; 10 ← 6, 7, 8 ; 11 ← 9, 10 | 7 et 8 entre elles et avec tout ; 9 et 10 entre elles | 🔴 `plateforme/src/http/` — **collision G1** |
| 5 — l'administration | 12 | 4 | avec la famille 4 | `plateforme/` |
| 6 — le navigateur | 13, 14 | 13 ← rien ; 14 ← 10, 13 | 13 avec tout | 🔴 `client/` |
| 7 — recette et clôture | 15, 16, 17, 18, 19 | 15 ← tout sauf 16 ; 16 ← 15 ; 17, 18, 19 ← 15 | 17 et 18 entre elles | — |

**Chemin critique** : 2 → 1 → 6 → 9 → 11 → 15.
**Quatre tâches purement isolées peuvent démarrer ensemble au premier tour** :
2, 4, 5, 7 — **et toutes les quatre sont dans `plateforme/`**, donc aucune
n'attend qu'un chantier concurrent libère quoi que ce soit.

🔴 **La famille 4 est la seule qui collisionne avec G1** (`cors.ts`,
`serveur.ts`, et le jeton porteur). Relancer
`git log --oneline -5 -- plateforme/src/http/` et `git status --porcelain`
avant de la démarrer, et **relire `serveur.ts` en entier** avant de le
modifier plutôt que de présumer sa forme.

🔴 **La tâche 16 (corroboration sur VM réelle) est CONDITIONNELLE.** Le
chantier F1 emploie la VM Windows, qui est une ressource **exclusive**. Si elle
est prise, **la tâche est REPORTÉE et déclarée non faite dans le document de
résultats** — jamais simulée, jamais remplacée par un raisonnement. La spec §4
la range explicitement **hors critère** : P4 est reçu ou non sans elle.

⚠️ **Dix-neuf tâches, numérotées 1 à 19, sans trou.**

---

# Famille 1 — le socle pur de l'orchestration

### Task 2 : les types de l'orchestration, et les deux listes blanches DÉRIVÉES

**Objet :** poser `Vm`, `EtatVm`, `Operation`, `Orchestrateur`, et les deux
listes d'opérations, de telle sorte qu'un verbe ajouté un jour ne puisse pas
échapper à la liste blanche des routes.

**Files:**
- Create: `plateforme/src/orchestration/interface.ts`,
  `plateforme/src/orchestration/interface.test.ts`
- Modify: aucun

**Interfaces:** Produces `EtatVm`, `Vm`, `OPERATIONS`, `Operation`,
`OPERATIONS_HTTP`, `OPERATIONS_HORS_HTTP`, `Orchestrateur`.

- [ ] **Step 1 : écrire les tests d'abord, et les voir ROUGES**

| Test (`it()` distinct) | Ce qui le rend ROUGE |
| --- | --- |
| 🔴 l'**union** de `OPERATIONS_HTTP` et `OPERATIONS_HORS_HTTP`, triée, est égale à `OPERATIONS` triée | retirer `arreter` des deux listes : l'union n'est plus égale. **C'est le contrôle qui interdit d'ajouter un verbe en l'oubliant** |
| 🔴 leur **intersection** est vide | mettre `etat` dans les deux : `it()` distinct du précédent, sans quoi `expect` s'arrêterait avant de l'évaluer |
| 🔴 `attribuer` n'est **pas** dans `OPERATIONS_HTTP` | l'y mettre : le test tombe. Sans lui, l'attribution deviendrait un jour atteignable par tout utilisateur authentifié (D8) |
| `EtatVm` accepte `'prete'` et `'injoignable'`, éprouvé par une valeur de chaque via `etatDe` | remplacer le réexport par une union à quatre membres : `etatDe` ne les produit pas et le test de partition d'états tombe |

⚠️ **Le premier test doit être écrit sur les VALEURS d'exécution, pas sur les
types.** Un contrôle de type seul serait vérifié par `tsc` et invisible à
`vitest` ; ici on veut les deux — `satisfies` pour le compilateur, et un test
qui compare des tableaux pour la suite.

- [ ] **Step 2 : implémenter.** `EtatVm` est un **réexport** de
      `EtatAgent` (`agents/fraicheur.ts:43`), avec le commentaire d'E2 : les
      états `arretee` et `demarrage` de la spec §3.6 sont **délibérément
      absents**, et le jour où un backend d'hyperviseur les produira,
      l'élargissement de l'union cassera à la compilation tout ce qui en
      dépend — c'est la bonne panne.
- [ ] **Step 3 : voir vert.** Compte de tests attendu : **4**, **annoncé avant
      d'être lu**.

---

### Task 1 : les motifs de refus, et la table de codes qui ne peut pas être incomplète

**Objet :** rendre impossible d'ajouter un motif de refus sans lui donner son
code HTTP, et rendre impossible qu'un refus soit un silence.

**Files:**
- Create: `plateforme/src/orchestration/refus.ts`,
  `plateforme/src/orchestration/refus.test.ts`
- Modify: aucun

**Interfaces:** Produces `BACKEND_STATIQUE`, `MOTIFS`, `Motif`, `Resultat`,
`CODE_HTTP`, `refuser`.

- [ ] **Step 1 : écrire les tests d'abord, et les voir ROUGES**

| Test (`it()` distinct) | Ce qui le rend ROUGE |
| --- | --- |
| 🔴 `Object.keys(CODE_HTTP)` trié **est égal** à `[...MOTIFS]` trié | retirer une entrée de `CODE_HTTP` : le test tombe **et** `tsc` échoue. **Les deux gardes sont voulues** — le `Record<>` attrape l'oubli à la compilation, le test l'attrape aussi pour qui lirait le vert de `vitest` sans lire celui de `typecheck` |
| `CODE_HTTP['non-supporte'] === 501` | mettre 500 : un `non-supporte` en 500 se lirait comme une panne du service, pas comme un aveu de son backend |
| `refuser('non-supporte','instantane')` rend `{ ok:false, motif, operation, backend: BACKEND_STATIQUE }` | omettre `operation` : un refus qui ne dit pas *quoi* a été refusé n'informe pas |
| 🔴 aucun `Resultat` de refus n'a `ok === true`, et aucun succès ne porte `motif` — éprouvé par un discriminant | rendre `{ ok: true, motif }` : le type l'interdit, et le test le dit à qui lirait le code |

⚠️ **`Object.keys` d'un `Record<Motif, …>` est un contrôle d'EXÉCUTION, et il
n'est pas redondant avec `tsc`** — c'est le pendant exact de la doctrine de
`plateforme/src/base/sous-ensemble.test.ts:3-14` (« chacun couvre l'angle mort
de l'autre ») : `tsc` ne verrait pas une clé **en trop** ajoutée par un
`as any`, le test si.

- [ ] **Step 2 : implémenter.** Les six motifs de D2, la table, et `refuser`
      dont le paramètre `backend` a **`BACKEND_STATIQUE` pour défaut** —
      jamais un littéral recopié.
- [ ] **Step 3 : voir vert.** Compte de tests attendu : **4**.

---

### Task 3 : la sélection, PURE — quelles VMs sont à cet utilisateur

**Objet :** décider, sans base et sans socket, quelles VMs d'un inventaire
appartiennent à un utilisateur — et garder cette décision hors de la couche
HTTP, où un bug fuiterait l'inventaire entier.

**Files:**
- Create: `plateforme/src/orchestration/selection.ts`,
  `plateforme/src/orchestration/selection.test.ts`

**Interfaces:** Produces `vmsDe`, `laVmDe`.

- [ ] **Step 1 : écrire les tests d'abord, et les voir ROUGES**

| Test (`it()` distinct) | Ce qui le rend ROUGE |
| --- | --- |
| 🔴 `vmsDe(inventaire, 'alice')` ne rend **que** les VMs d'alice | **rendre l'inventaire entier** — c'est la mutation exacte que ce module existe pour rendre visible, et elle est indétectable si le filtre vit dans la route |
| une VM à `utilisateurId === null` **n'est rendue à personne** | traiter `null` comme « libre pour tous » : un utilisateur verrait toutes les VMs non attribuées. ⚠️ C'est le comportement que G1 retient délibérément pour SES routes (« servie, et journalisée ») ; **P4 ne le retient pas** — voir D8 |
| `laVmDe` rend `undefined` quand l'utilisateur n'en a aucune | rendre `inventaire[0]` : le critère ③ deviendrait invérifiable |
| 🔴 `laVmDe` rend l'unique VM quand il y en a une, **et lève si l'inventaire en porte deux pour le même utilisateur** | rendre la première en silence : l'index partiel garantit que le cas n'existe pas, et si la base le portait quand même c'est un défaut, pas une préférence à exprimer |

⚠️ **Le dernier test éprouve un état que l'index partiel rend impossible en
base.** Il est écrit quand même : la fonction reçoit un tableau, et rien dans
sa signature ne dit d'où il vient. Un jour où l'index serait retiré (ce que la
rouge ②b fait exprès), c'est cette exception qui dirait où regarder.

- [ ] **Step 2 : implémenter.**
- [ ] **Step 3 : voir vert.** Compte de tests attendu : **4**.

---

# Famille 2 — la persistance

### Task 4 : `depot/vm.ts` — le dépôt qui n'existait pas

**Objet :** donner à la table `vm` le dépôt que les trois autres tables ont, et
poser l'`UPDATE` **conditionnel** dont dépend la propriété ②a.

**Files:**
- Create: `plateforme/src/depot/vm.ts`, `plateforme/src/depot/vm.test.ts`
- Modify: aucun

**Interfaces:** Produces `LigneVm`, `lister`, `lireParId`, `lireParNom`,
`attribuerSiLibre`, `detacher`.

- [ ] **Step 1 : écrire les tests d'abord, et les voir ROUGES.** Le harnais est
      `baseNeuve` (`plateforme/src/base/harnais.ts:41`) ; les horodatages
      d'essai sont de la magnitude d'`INSTANT_MIGRATION`
      (`harnais.ts:32`, `1_700_000_000_000`), **jamais des petits nombres**.

| Test (`it()` distinct) | Ce qui le rend ROUGE |
| --- | --- |
| `lister` rend la VM **avec son préfixe et son `vu_a`**, joints depuis `agent_enrole` | remplacer le `LEFT JOIN` par un `JOIN` : une VM enrôlée sans battement, ou non enrôlée, disparaîtrait de l'inventaire |
| 🔴 `typeof ligne.vu_a === 'number'` après écriture d'un `Date.now()` réaliste | c'est le défaut de **classe** de P3 : `pg` rendait tout `BIGINT` en **chaîne**, `interroger<T>` fait un `as T[]`, et **aucun typage ne pouvait l'attraper**. Le remède est au pilote (`base/pilote-postgres.ts`) ; cette assertion est le témoin **au point d'usage**, et elle rougit sous `test:postgres` si le `setTypeParser` disparaît |
| 🔴 `attribuerSiLibre` sur une VM **libre** rend `1` | — |
| 🔴 `attribuerSiLibre` sur une VM **déjà prise** rend `0` **sans lever**, et **la VM n'a pas changé de propriétaire** | **retirer `AND utilisateur_id IS NULL`** : mesuré sur les deux moteurs, le vol passe et le test tombe sur ses deux assertions. **Ce sont DEUX `it()`** — le compte, et l'état de la ligne |
| 🔴 `attribuerSiLibre` pour un utilisateur qui a **déjà** une VM **LÈVE** | **retirer l'index de `0001-socle.sql`** : mesuré, l'`UPDATE` passe alors. ⚠️ Cette rouge se joue à la tâche 15, avec `sha256` avant/après restauration |
| `attribuerSiLibre` sur une VM **inconnue** rend `0` sans lever | lever : le motif serait indiscernable d'un défaut de base |
| `detacher` rend la VM au vivier, et une seconde attribution redevient possible | ne pas remettre `NULL` : la VM resterait à jamais prise |

⚠️ **`lister` énumère ses colonnes explicitement et EXCLUT `vm.vue_a`**, avec
son commentaire : la colonne est **orpheline** depuis P1 — P3 a créé
`agent_enrole.vu_a` à sa place, et `vm.vue_a` n'est écrite par **aucun** code de
production. La sélectionner ferait croire à un successeur qu'elle est
renseignée.

- [ ] **Step 2 : implémenter.** Aucune valeur littérale dans le SQL
      (`base/pilote.ts:32` lève, **sur le chemin Postgres uniquement**, et
      `sous-ensemble.test.ts:83-88` ne couvre que les `.sql`) : tout passe en
      paramètre. Le `LEFT JOIN` n'a besoin d'aucune littérale.
- [ ] **Step 3 : voir vert sur LES DEUX moteurs.** `npm run test:sqlite` **et**
      `npm run test:postgres`. Compte de tests attendu : **8**.

---

### Task 5 : `compterOuvertesDe` — le lecteur que `session.utilisateur_id` attend depuis P2

**Objet :** donner à `session.utilisateur_id` son premier lecteur de
production, et fermer le legs n°4 de P2 / n°3 de P3.

**Files:**
- Modify: `plateforme/src/depot/session.ts`, `plateforme/src/depot/session.test.ts`

**Interfaces:** Produces `compterOuvertesDe`.

- [ ] **Step 1 : écrire les tests d'abord, et les voir ROUGES**

| Test (`it()` distinct) | Ce qui le rend ROUGE |
| --- | --- |
| 🔴 le compte passe de **0 à 1** quand une session est ouverte pour cet utilisateur | rendre une constante : le test doit **voir la transition**, pas un nombre. C'est la forme du critère ④ de P3, appliquée ici |
| une session **close** n'est pas comptée | omettre `AND fermee_a IS NULL` |
| une session d'un **autre** utilisateur n'est pas comptée | omettre le `WHERE utilisateur_id = ?` : le compte deviendrait global |
| une session à `utilisateur_id NULL` (l'agent seul, cas nominal de `garde.ts:141-147`) n'est comptée pour personne | traiter `NULL` comme appartenant au demandeur |

- [ ] **Step 2 : implémenter.** Un `SELECT COUNT(*)`, avec les deux clauses.
      ⚠️ **Le résultat d'un `COUNT(*)` est un `int8` sur Postgres, et le
      `setTypeParser` de `base/pilote-postgres.ts` le couvre — MESURÉ à
      travers le pilote du service, `[{"n":1}] typeof = number`** (voir la
      table des sondes). L'assertion `typeof … === 'number'` fait néanmoins
      partie du premier test : c'est la même classe de défaut que celle de la
      tâche 4, sur une valeur qui n'appartient à aucune colonne — donc que le
      test de `pilotes.test.ts`, qui balaie colonne par colonne, ne couvre
      pas.
- [ ] **Step 3 : voir vert sur les deux moteurs.** Compte attendu : **4** de
      plus que le compte actuel de `session.test.ts`, **annoncé avant d'être
      lu**.

---

# Famille 3 — l'orchestrateur

### Task 6 : `InventaireStatique` — trois verbes qui agissent, trois qui refusent

**Objet :** l'unique implémentation de `Orchestrateur` de la v1 : elle lit la
base, appelle `fraicheur.etatDe`, attribue sous transaction, et **refuse par un
type** tout ce qu'un inventaire ne sait pas faire.

**Files:**
- Create: `plateforme/src/orchestration/inventaire-statique.ts`,
  `plateforme/src/orchestration/inventaire-statique.test.ts`
- Modify: aucun

**Interfaces:** Consumes `Pilote`, `depot/vm`, `agents/fraicheur`,
`orchestration/refus`, `orchestration/interface`. Produces
`inventaireStatique(base, maintenant)`.

- [ ] **Step 1 : écrire les tests d'abord, et les voir ROUGES**

| Test (`it()` distinct) | Ce qui le rend ROUGE |
| --- | --- |
| 🔴 `instantane` rend `{ok:false, motif:'non-supporte', operation:'instantane', backend:'inventaire-statique'}` | **le remplacer par un `return` silencieux** — c'est littéralement la rouge du critère ① |
| 🔴 `instantane` **journalise** son refus (`vi.spyOn(console,'warn')`) | retirer le `console.warn` : **`it()` distinct de la précédente**, sans quoi `expect` s'arrêterait à la première et la seconde ne serait éprouvée par rien (leçon ①A/①A-bis de P2) |
| `demarrer` refuse de même | — |
| `arreter` refuse de même | — |
| 🔴 `etat` rend `prete` à `vu_a + SEUIL` **exactement**, et `injoignable` à `vu_a + SEUIL + 1` | **figer l'horloge injectée** : la borne ne serait plus assiégée des deux côtés, et un seuil jamais atteint ne prouve rien |
| `etat` d'une VM **jamais vue** (`vu_a` nul) rend `injoignable` | rendre `prete` : une VM enrôlée jamais démarrée serait annoncée prête |
| `etat` d'une VM **inconnue** rend `injoignable` | lever : voir D3 |
| 🔴 `attribuer` sur une VM libre rend `{ok:true}` **et la ligne porte le propriétaire** | deux `it()` : le verdict, et l'état relu |
| 🔴 `attribuer` sur une VM **déjà prise** rend `{ok:false, motif:'vm-deja-attribuee'}` **et ne change pas le propriétaire** | deux `it()`. Rouge : retirer la clause `IS NULL` (mesuré : le vol passe) |
| 🔴 `attribuer` à un utilisateur qui a **déjà** une VM rend `{ok:false, motif:'utilisateur-servi'}` **et ne LÈVE pas** | supprimer la traduction : l'exception remonte, et la couche HTTP rend **500** — c'est la troisième assertion du critère ② |
| `attribuer` sur une VM inconnue rend `vm-inconnue` | — |
| 🔴 une exception **étrangère** à l'unicité est **RELANCÉE**, pas traduite | un `Pilote` factice dont `executer` lève `new Error('base injoignable')` : le test attend un rejet. Rouge : traduire toute exception en `utilisateur-servi` — une base morte serait présentée comme un refus métier |
| **le perdant d'une course séquentielle** (A attribue, puis B tente la même VM) reçoit `vm-deja-attribuee` | ⚠️ **ce test est SÉQUENTIEL et ne mesure pas la sérialisation** — voir D5 |

- [ ] **Step 2 : implémenter.** L'attribution vit dans une
      `Pilote.transaction` ; l'ordre est celui de D4 (lire, puis `UPDATE`
      conditionnel, puis relire en cas d'exception, sinon **relancer**).
      🔴 **La clause `AND utilisateur_id IS NULL` porte son commentaire** :
      c'est elle, et non la lecture préalable, qui donne la garantie, parce
      qu'elle est réévaluée par le moteur au moment de l'écriture (mesuré sur
      Postgres 16.15, D5). 🔴 **Aucun code ne compare le TEXTE d'une
      exception** : les deux moteurs n'écrivent pas le même.
- [ ] **Step 3 : voir vert sur les deux moteurs.** Compte attendu : **15**.

---

# Famille 4 — HTTP

### Task 7 : `http/porteur.ts` — lire `Authorization: Bearer`, et refuser le jeton d'agent

**Objet :** la seule chose qui manque entre un en-tête HTTP et un identifiant
d'utilisateur. **PUR** : ni base, ni socket, ni horloge lue.

**Files:**
- Create: `plateforme/src/http/porteur.ts`, `plateforme/src/http/porteur.test.ts`

**Interfaces:** Consumes `verifierJeton` (`identite/jeton.ts:130`). Produces
`lirePorteur`.

- [ ] **Step 1 : écrire les tests d'abord, et les voir ROUGES**

| Test (`it()` distinct) | Ce qui le rend ROUGE |
| --- | --- |
| en-tête absent → `{ok:false, motif:'jeton-absent', code:401}` | rendre `ok:true` avec un sujet vide |
| `Bearer <jeton valide d'utilisateur>` → `{ok:true, utilisateurId: sujet}` | — |
| 🔴 `Bearer <jeton d'AGENT>` → `{ok:false, motif:'jeton-agent', code:403}` | **accepter le type `agent`** : un agent verrait l'inventaire d'un humain. **Rouge n°1 des deux sens de confusion d'E5 de P3** |
| jeton **expiré** → `jeton-expire`, avec une horloge en paramètre qui **varie** | figer l'horloge : le test devient inerte |
| schéma autre que `Bearer` (`Basic …`, `bearer …` en minuscules) → `jeton-invalide` | accepter n'importe quel schéma, ou comparer sans tenir compte de la casse **sans le dire** : le contrôle doit être explicite dans un sens ou dans l'autre |
| en-tête **répété** (`string[]`, ce que Node permet) → `jeton-invalide` | prendre `entetes.authorization[0]` en silence : deux en-têtes d'autorisation est une requête ambiguë, pas une requête à interpréter |

⚠️ **La seconde confusion — un jeton d'utilisateur ouvrant un rôle `agent` —
est déjà fermée par `identite/garde.ts` et éprouvée par P3.** Elle n'est pas
rejouée ici ; **elle est nommée** pour qu'un lecteur ne croie pas qu'un seul
sens est gardé.

- [ ] **Step 2 : implémenter.**
- [ ] **Step 3 : voir vert.** Compte attendu : **6**.

---

### Task 8 : `cors.ts` accepte `GET`

**Objet :** deux caractères, et une collision déclarée.

**Files:**
- Modify: `plateforme/src/http/cors.ts`, `plateforme/src/http/cors.test.ts`

- [ ] **Step 1 : test d'abord.** L'assertion existante sur
      `'Access-Control-Allow-Methods'` devient `'GET, POST, OPTIONS'` et est
      **vue rouge** avant la modification.
- [ ] **Step 2 : implémenter.** `cors.ts:36`.
      ⚠️ **Ne toucher à RIEN d'autre** : le défaut est le refus, la valeur `*`
      n'est produite sous aucune condition, et la comparaison d'origine est une
      égalité — trois propriétés que ce fichier documente et qui ne sont pas
      l'objet de cette tâche.
- [ ] **Step 3 : vert.** ⚠️ **Si G1 a déjà posé `GET`**, cette tâche n'a rien à
      faire : elle le **constate**, le dit dans son rapport, et ne réécrit pas
      la ligne.

---

### Task 9 : `routes-vm.ts` — `GET /vm` et `POST /vm/:id/:operation`

**Objet :** la surface HTTP de l'inventaire, et le 501 du critère ①.

**Files:**
- Create: `plateforme/src/http/routes-vm.ts`, `plateforme/src/http/routes-vm.test.ts`

**Interfaces:** Consumes `lirePorteur`, `inventaireStatique`, `vmsDe`,
`compterOuvertesDe`, `entetesCors`, `CODE_HTTP`, `OPERATIONS_HTTP`. Produces
`servirVm(req, rep, deps): Promise<boolean>`.

- [ ] **Step 1 : écrire les tests d'abord, et les voir ROUGES.** Les tests
      parlent en `fetch` Node contre un serveur réel sur un port attribué par
      le système, dans le style de `routes-auth.test.ts`.

| Test (`it()` distinct) | Ce qui le rend ROUGE |
| --- | --- |
| `GET /vm` sans en-tête → **401** `jeton-absent` | servir sans jeton |
| `GET /vm` avec un jeton d'**agent** → **403** `jeton-agent` | accepter le type `agent` |
| 🔴 `GET /vm` ne rend **que** les VMs du demandeur | rendre l'inventaire entier — et **ce test est distinct de celui de `vmsDe`** : celui-là éprouve la fonction pure, celui-ci éprouve qu'elle est bien **appelée** |
| `GET /vm` rend `etat`, `prefixe` et `sessions_ouvertes` | omettre un champ |
| 🔴 `GET /vm` ne rend **PAS** `adresse` | l'inclure : c'est de la topologie interne, dont le navigateur n'a aucun usage (D7) |
| 🔴 `POST /vm/<id>/instantane` → **501**, corps `{motif:'non-supporte', operation:'instantane', backend:'inventaire-statique'}` | rendre 200. **C'est le critère ①** |
| `POST /vm/<id>/demarrer` → 501 | — |
| `POST /vm/<id>/arreter` → 501 | — |
| 🔴 `POST /vm/<id>/attribuer` → **404 générique**, jamais 501 ni 200 | ajouter `attribuer` à `OPERATIONS_HTTP` : l'attribution deviendrait publique |
| `POST /vm/<id>/<verbe inventé>` → **404 générique** (la route ne le reconnaît pas, `servir` rend `false`) | valider après avoir servi : un verbe inconnu produirait un 501 qui mentirait sur l'existence de l'opération |
| 🔴 `POST /vm/<id d'une VM d'AUTRUI>/instantane` → **404 `vm-inconnue`**, exactement le même corps que pour une VM inexistante | distinguer les deux : c'est un oracle d'énumération. **Deux `it()`, et le second compare les deux corps caractère pour caractère** — la forme exacte du critère ② de P3 |
| les en-têtes CORS sont posés quand `origineClient` est configurée | les omettre : le navigateur refuserait de lire la réponse **sans qu'aucun test Node ne le voie** (`cors.ts:4-9`) |

- [ ] **Step 2 : implémenter.** Le contrat est celui de `routes-auth.ts:93` :
      `Promise<boolean>`. Le chemin est découpé une fois, comparé **exactement**
      pour `/vm` et par segments pour `/vm/:id/:operation` ; **jamais un
      `startsWith`** (`serveur.ts:127-129`). 🔴 **La liste des opérations
      servies est `OPERATIONS_HTTP`, importée, jamais recopiée.**
- [ ] **Step 3 : voir vert sur les deux moteurs.** Compte attendu : **13**.

---

### Task 10 : `routes-session.ts` — `POST /session`, et les deux refus des critères ③ et ④

**Objet :** l'enchaînement de la spec §4 : l'utilisateur demande une session,
la plateforme vérifie l'attribution **et** la fraîcheur, et rend le préfixe.

**Files:**
- Create: `plateforme/src/http/routes-session.ts`,
  `plateforme/src/http/routes-session.test.ts`

**Interfaces:** Consumes `lirePorteur`, `inventaireStatique`, `laVmDe`,
`entetesCors`. Produces `servirSession(req, rep, deps): Promise<boolean>`.

- [ ] **Step 1 : écrire les tests d'abord, et les voir ROUGES**

| Test (`it()` distinct) | Ce qui le rend ROUGE |
| --- | --- |
| `POST /session` sans jeton → 401 ; avec un jeton d'agent → 403 | — |
| 🔴 succès → **200** `{ vm, nom, prefixe, etat:'prete' }` | omettre `prefixe` : la source que P3 attend resterait absente |
| 🔴 succès → le corps **ne porte ni `ice`, ni `adresse`, ni nom de session composé** | les ajouter (D7) |
| 🔴 **③a** — utilisateur sans VM → **409** `{motif:'aucune-vm'}` | rendre 200 avec une liste vide : un listing vide n'est pas un refus |
| 🔴 **③b** — la réponse arrive **sous la borne** | **insérer `await new Promise(r => setTimeout(r, 2000))`** dans la route : mesuré atteignable, le test tombe. ⚠️ **La borne est MESURÉE avant d'être fixée** — voir ci-dessous |
| 🔴 **④a** — VM dont `vu_a` est trop vieux → **503**, corps portant `etat: 'injoignable'` | masquer derrière un `{motif:'reessayez'}` générique |
| 🔴 **④b** — le même corps porte `redemarrage: { possible:false, motif:'non-supporte', backend:'inventaire-statique' }` | **retirer le champ.** `it()` distinct de ④a : c'est le « et dit qu'elle ne sait pas la redémarrer » du critère, et `expect` s'arrêterait à la première assertion |
| 🔴 **④c** — la **transition est vue** : à `vu_a + SEUIL` la route rend 200, à `vu_a + SEUIL + 1` elle rend 503 | figer l'horloge injectée |

⚠️ **La borne de temps du test ③b se MESURE avant d'être écrite** : lancer le
test une première fois avec une borne absurdement large, **relever la durée
réelle**, l'annoncer dans le rapport de tâche, et fixer la borne à une valeur
qui laisse un ordre de grandeur de marge sans jamais atteindre les 2 000 ms de
la mutation. ⚠️ **Une requête de chauffe précède la mesure** : la première
requête d'un test porte l'établissement de connexion, qui n'est pas ce qu'on
mesure. 🔴 **Une borne posée sans avoir été mesurée serait un contrôle dont on
ignore s'il peut échouer** — le patron que ce dépôt a payé quatre fois.

- [ ] **Step 2 : implémenter.** L'horloge vient des `deps`, jamais de
      `Date.now()` lu dans le module. Le corps de la requête n'est **pas lu**
      (il n'y a rien à y mettre : l'index partiel garantit zéro ou une VM par
      utilisateur), ce qui dispense de la borne de 4 Kio de
      `routes-auth.ts:44` — **et ce raisonnement est écrit dans le fichier**,
      parce que le jour où un corps deviendra nécessaire, la borne le devra
      aussi.
- [ ] **Step 3 : voir vert sur les deux moteurs.** Compte attendu : **9**.

---

### Task 11 : `serveur.ts` chaîne les deux routeurs, sans toucher au 404 ni au `catch`

**Objet :** brancher, et **ne rien casser** de ce que P1 et P2 ont figé.

**Files:**
- Modify: `plateforme/src/http/serveur.ts`, `plateforme/src/http/serveur.test.ts`

- [ ] **Step 1 : écrire les tests d'abord, et les voir ROUGES**

| Test (`it()` distinct) | Ce qui le rend ROUGE |
| --- | --- |
| 🔴 `/auth/connexion` répond toujours, `/vm` répond, `/session` répond, `/inconnu` rend le **404 mot pour mot** (`introuvable\n`) | retirer un maillon de la chaîne : sa route rend 404 |
| 🔴 une route qui **rejette** rend toujours **500 `{refus:'interne'}`**, et le processus **survit** | retirer le `.catch` : une promesse rejetée dans un gestionnaire d'évènement Node **abat tout le processus** — le mode de défaillance que `serveur.ts:67-71` documente déjà |
| les deux montées WebSocket (`/` et `/agent`) sont **inchangées** | modifier le routage de `serveur.ts:130` : hors sujet de cette tâche, et le test le fige |

- [ ] **Step 2 : implémenter.** 🔴 **La forme
      `void … .then(servie => { if (!servie) 404 }).catch(…)` est CONSERVÉE
      telle quelle** : `serveur.ts:52-54` dit en toutes lettres « Ne pas
      changer son corps ». Le chaînage se fait **à l'intérieur**, par une
      fonction locale `async function servirTout(req, rep): Promise<boolean>`
      qui `await` les trois routeurs dans l'ordre et rend `false` si aucun n'a
      servi. Le diff sur le corps du `createServer` est ainsi **d'une ligne**.
- [ ] **Step 3 : voir vert sur les deux moteurs.** ⚠️ **Relire `serveur.ts` en
      entier avant de l'éditer** : G1 prévoit d'y chaîner sa propre route, et
      présumer la forme du fichier est la façon dont deux branches se
      détruisent mutuellement.

---

# Famille 5 — l'administration

### Task 12 : `npm run admin:attribuer`, et `--detacher`

**Objet :** le seul chemin par lequel une VM reçoit un propriétaire.

**Files:**
- Create: `plateforme/src/admin/attribuer-vm.ts`,
  `plateforme/src/admin/attribuer-vm.test.ts`
- Modify: `plateforme/package.json` (le script `admin:attribuer`)

- [ ] **Step 1 : écrire les tests d'abord, et les voir ROUGES**

| Test (`it()` distinct) | Ce qui le rend ROUGE |
| --- | --- |
| `analyserArguments` est **PURE** et exige `--email` **et** `--vm`, sans défaut | donner un défaut à l'un des deux |
| 🔴 `--detacher` est exclusif de rien mais **rend la VM au vivier**, et une nouvelle attribution redevient possible | ignorer le drapeau : la VM resterait prise à vie |
| courriel inconnu → code de sortie **2**, message nommant la cause | ⚠️ **ici on NOMME la cause** : c'est une commande d'administration, pas une route publique — l'énumération n'est pas un risque, et cacher la cause à l'administrateur le ferait chercher ailleurs (D8) |
| VM inconnue → code **2** ; VM déjà prise → code **2**, message nommant le propriétaire | — |
| 🔴 utilisateur qui a déjà une VM → code **2**, message le disant — **et non une trace de pile** | laisser l'exception d'unicité remonter : l'administrateur verrait `duplicate key value violates unique constraint`, qui ne lui dit pas quoi faire |
| succès → code **0**, et la ligne relue porte le propriétaire | — |

- [ ] **Step 2 : implémenter**, sur le patron exact de `admin/enroler-agent.ts`
      et `admin/creer-utilisateur.ts` : `analyserArguments` pure, `executer`
      qui lit la configuration, ouvre la base, **applique les migrations**
      (la commande peut être le premier geste sur une base neuve), agit, puis
      écrit sur stdout. `--vm` accepte un **nom** ou un **identifiant** —
      `lireParNom` puis `lireParId`, dans cet ordre, l'administrateur
      connaissant le nom qu'il a donné.
      ⚠️ **Aucun drapeau de secret n'est en jeu ici**, mais la liste des
      drapeaux refusés des deux autres commandes est reprise **telle quelle** :
      une commande d'administration qui accepterait `--mot-de-passe` sans
      s'en servir laisserait quand même la chaîne dans `ps`.
- [ ] **Step 3 : voir vert sur les deux moteurs.** Compte attendu : **7**.

---

# Famille 6 — le navigateur

### Task 13 : `poserPrefixe` et `effacerPrefixe`, purs

**Objet :** la règle de ce qu'on écrit dans le coffre, hors du DOM.

**Files:**
- Modify: `client/src/prefixe.ts`, `client/src/prefixe.test.ts`

- [ ] **Step 1 : écrire les tests d'abord, et les voir ROUGES**

| Test (`it()` distinct) | Ce qui le rend ROUGE |
| --- | --- |
| `poserPrefixe(coffre, 'AB')` puis `lirePrefixe(coffre)` rend `'AB'` | — |
| 🔴 `poserPrefixe(coffre, '')` **LÈVE** | l'accepter : `lirePrefixe` (l. 62-64) retomberait sur `?prefixe=` puis sur `''`, et la page rejoindrait **silencieusement** l'espace de noms partagé — la panne muette que la spec §10 nomme |
| `effacerPrefixe(coffre)` puis `lirePrefixe(coffre, '?prefixe=Q')` rend `'Q'` | ne pas appeler `removeItem` : l'ancien préfixe survivrait et le mode d'essai local cesserait de fonctionner |
| 🔴 le coffre reste un **PARAMÈTRE** : le module ne touche `globalThis` que dans le défaut d'un argument | lire `localStorage` au chargement du module : `client/` n'a **aucun `vitest.config.*`**, l'environnement de test est le Node par défaut, et il n'y a ni `window` ni `localStorage` (`prefixe.ts:4-9`) |

- [ ] **Step 2 : implémenter.**
- [ ] **Step 3 : vert.** `cd client && npm test`. Compte attendu : **4** de plus
      que le compte actuel de `prefixe.test.ts`, **annoncé avant d'être lu**.

---

### Task 14 : `connexion.ts` demande sa session, et écrit le préfixe

**Objet :** brancher la source. **C'est le legs n°1 de P3.**

**Files:**
- Modify: `client/src/connexion.ts`
- Modify: aucun `.html`

- [ ] **Step 1 : il n'y a pas de test unitaire, et c'est DÉCLARÉ.**
      `connexion.ts:5-9` pose la convention et sa clause : **toute règle que ce
      fichier porterait doit descendre dans un module pur**. Les règles de
      cette tâche vivent dans `prefixe.ts` (tâche 13) et dans
      `routes-session.ts` (tâche 10), toutes deux testées. **Ce qui reste ici
      est du câblage** : un `fetch`, deux branches, un message.
      🔴 **Si une CONDITION apparaît dans ce fichier, c'est qu'elle est au
      mauvais endroit** — et le relecteur de cette tâche doit la refuser.
- [ ] **Step 2 : implémenter.** Après `poser(window.localStorage, …)` :
      `POST ${plateformeUrl}/session` avec `Authorization: Bearer ${corps.acces}`.
      - **200** → `poserPrefixe(window.localStorage, corps.prefixe)`, puis
        `window.location.href = suite`, comme aujourd'hui ;
      - **`aucune-vm`** → `effacerPrefixe(window.localStorage)`, message,
        **pas de redirection** ;
      - **`agent-injoignable`** → `poserPrefixe` (le préfixe est connu et
        juste), message portant l'état **et** le fait que la plateforme ne sait
        pas redémarrer la VM, **pas de redirection** ;
      - **échec réseau** → le `catch` existant, **inchangé**.
- [ ] **Step 3 : `cd client && npm run typecheck` (sortie 0) et `npm test`
      (compte inchangé par rapport à la tâche 13).** ⚠️ **Le coût de « pas de
      redirection » est écrit dans le fichier** : un développeur sans VM
      enrôlée reste sur l'écran de connexion, et le mode d'essai local passe
      par `?prefixe=` sur l'URL de la shell, **qui fonctionne toujours** parce
      que le coffre est vide.

---

# Famille 7 — recette et clôture

### Task 15 : la recette — quatre critères, DEUX exécutions chacun, toutes les rouges jouées

**Objet :** établir les quatre critères de la spec §4 « P4 », sur pièces
versées, et voir rouge chaque assertion.

**Files:**
- Create: `docs/superpowers/plans/journaux-plateforme-p4/` et son contenu
- Modify: aucun fichier de code, **hors mutations de rouge restaurées**

- [ ] **Step 1 : le témoin d'entrée.** Relever, **par la commande**, les
      références ① à ⑦ du §« Ce qui a été relevé » **sur l'arbre de clôture**,
      et les verser. **Annoncer les comptes attendus avant de les lire.**
- [ ] **Step 2 : les quatre critères, DEUX exécutions chacun** — exécution 1 =
      `sqlite`, exécution 2 = `postgres`, **déclaré dans l'en-tête de chaque
      journal**.

| # | Critère | Assertions (un `it()` chacune) | Journaux |
| --- | --- | --- | --- |
| ① | `instantane` sur le backend statique **refuse explicitement**, en **501**, **journalisé** | ①a le code et le corps typé ; ①b la ligne de journal | `critere-1-{1,2}.log` |
| ② | Deux utilisateurs ne peuvent pas recevoir la même VM, un utilisateur ne reçoit pas deux VMs, **et jamais un 500** | ②a le vol est refusé et la ligne est inchangée ; ②b la seconde VM est refusée ; ②c le refus est typé, code ≠ 500 | `critere-2-{1,2}.log` |
| ③ | Un utilisateur sans VM reçoit un refus **immédiat** | ③a le refus typé (409 `aucune-vm`) ; ③b la durée sous la borne **mesurée** | `critere-3-{1,2}.log` |
| ④ | Une VM dont l'agent n'a pas été vu récemment est **annoncée injoignable**, et l'API **dit qu'elle ne sait pas la redémarrer** | ④a `etat: 'injoignable'` ; ④b `redemarrage.possible === false` avec son motif ; ④c la **transition** est vue, borne assiégée des deux côtés | `critere-4-{1,2}.log` |

- [ ] **Step 3 : les DIX rouges, toutes JOUÉES, jamais supposées.** Une par
      assertion des quatre critères — le tableau ci-dessous en compte **dix**,
      autant que la §« Les règles de méthode » en annonce. Chaque
      mutation porte son **`sha256` avant et après restauration**, les deux
      empreintes comparées dans le journal.

| Rouge | Mutation | Atteignabilité |
| --- | --- | --- |
| ①a | `instantane` rend `{ok:true}` | triviale |
| ①b | retirer le `console.warn` de `refuser` | triviale |
| 🔴 ②a | retirer `AND utilisateur_id IS NULL` de `attribuerSiLibre` | **MESURÉE** : le vol passe, `1 ligne`, sur SQLite 3.50.4 **et** PostgreSQL 16.15 |
| 🔴 ②b | retirer `CREATE UNIQUE INDEX vm_un_utilisateur` de `0001-socle.sql:57-58` | **MESURÉE** : la seconde VM du même utilisateur passe alors. ⚠️ mute une migration de P1 |
| ②c | laisser l'exception d'unicité remonter | la couche HTTP rend alors **500 `{refus:'interne'}`** (`serveur.ts:74-77`) |
| ③a | la route rend 200 sur un utilisateur sans VM | triviale |
| ③b | `await new Promise(r => setTimeout(r, 2000))` dans la route | triviale, et **au-delà de la borne mesurée d'un ordre de grandeur** |
| ④a | remplacer le corps par `{motif:'reessayez'}` | triviale |
| ④b | retirer le champ `redemarrage` | triviale — **et distincte de ④a**, sans quoi `expect` s'arrêterait avant |
| ④c | figer l'horloge injectée de l'orchestrateur | triviale |

🔴 **Chaque rouge est vérifiée ATTEIGNABLE avant d'être prescrite, et deux
d'entre elles l'ont été par une SONDE plutôt que par un raisonnement** — parce
que la rouge que la spec prescrivait pour le critère ② **ne rougissait pas la
propriété que le critère énonce** (E3). C'est le défaut qu'un plan de ce dépôt
a commis le 19 août 2026, attrapé ici avant dispatch.

- [ ] **Step 4 : le témoin de clôture.** `./scripts/verify-all.sh`, **relancé
      APRÈS la dernière édition de la ronde**, journal versé, sortie **0**.
      ⚠️ **Dire lequel des deux comptes on rapporte** : les **dix** appels
      `etape` du script, ou les **dix-sept** en-têtes `==>` à l'écran. Les deux
      sont vrais de choses différentes, et P3 a payé une correction pour ne pas
      l'avoir dit.
      ⚠️ **`cargo test --workspace` sera dans ce journal ; son compte n'est PAS
      attribuable à P4** — un chantier concurrent travaille dans `agent/`. Le
      rapporter **sans** le revendiquer.

---

### Task 16 : la corroboration sur VM réelle — HORS CRITÈRE, et CONDITIONNELLE

**Objet :** montrer, sur le produit réel, qu'un utilisateur à qui une VM est
attribuée obtient son préfixe et ouvre sa shell.

🔴 **CETTE TÂCHE EST CONDITIONNELLE.** Le chantier F1 emploie la VM Windows,
ressource **exclusive**. **Vérifier `virsh list --all` et l'absence de
`Get-Process agent` étranger avant de commencer** ; si la VM est prise, **la
tâche est REPORTÉE et déclarée non faite dans le document de résultats** —
jamais simulée, jamais remplacée par un raisonnement. La spec §4 la range
explicitement hors critère : **P4 est reçu ou non sans elle.**

- [ ] **Step 1 :** enrôler une VM (`npm run admin:agent`), créer un utilisateur
      (`npm run admin:utilisateur`), **attribuer** (`npm run admin:attribuer`).
- [ ] **Step 2 :** lancer l'agent (`scripts/build-agent.sh` **si et seulement
      si** le binaire est périmé — et alors **relever la TAILLE du binaire**,
      une compilation de 0,13 s étant un aveu), puis `scripts/run-agent.sh`.
      ⚠️ **P4 n'exige AUCUN rebâtissage** : il ne touche ni `agent/` ni
      `proto/`. Si G1 a été fusionné entre-temps, **c'est G1 qui l'exige**, pas
      P4.
- [ ] **Step 3 :** se connecter par l'écran de connexion, vérifier que le
      préfixe arrive dans le coffre, que la shell ouvre `<préfixe>:bureau`, et
      que `vu_a` avance. **Deux exécutions**, journaux versés.
      ⚠️ **Les journaux d'agent portent des séquences ANSI et des CRLF** :
      verser le brut **et** son jumeau `-plat`
      (`sed 's/\x1b\[[0-9;]*m//g'`), comme P3.
- [ ] **Step 4 : dire ce que cette corroboration N'EST PAS.** Elle n'établit ni
      latence, ni cadence, ni durée ; elle n'exerce **pas** la reprise du canal
      `/agent` (E9) ; et elle ne dit rien d'un second utilisateur.

---

### Task 17 : la revue transverse de fin de branche — OBLIGATOIRE

**Objet :** trouver ce qu'aucune revue par tâche ne peut voir — les
affirmations devenues fausses **dans leur propre branche**, et les défauts qui
franchissent une frontière de tâche.

**Barème, pour que le relecteur sache ce qu'on attend de lui** : **5** défauts
en D7, **3** en D8, **6** en D9, **douze** en D10, **sept** en D11, **huit** en
P1, **dix** en P2, **cinq** en S1, **neuf** en E, **douze** en P3, **douze** en
S2. **Une revue qui n'en trouve aucun n'a pas cherché.**

- [ ] **Step 1 : balayer les affirmations que P4 rend fausses**, en
      **énumérant les places par `grep -n` AVANT d'écrire**, et en relisant
      **place par place APRÈS** l'édition. Une substitution qui ne dit pas
      combien d'occurrences elle a touchées est une affirmation de complétude
      non vérifiée.

| Place | Affirmation que P4 rend fausse |
| --- | --- |
| `plateforme/src/agents/fraicheur.ts:11-20` | « IL N'A AUCUN APPELANT DE PRODUCTION […] ce qui est le sujet de P4 » — **P4 le lui donne** |
| `client/src/prefixe.ts:11-17` | « LA SOURCE DÉFINITIVE DU PRÉFIXE […] N'EXISTE PAS ENCORE » — **elle existe** |
| `plateforme/src/signaling/propriete.ts:36-39` | « `session.utilisateur_id` […] c'est ce dont P4 aura besoin » — **il le lit** |
| `plateforme/src/base/migrations/0001-socle.sql:53-56` | 🔴 « une seconde attribution est refusée […] C'est de lui que dépendra le critère 2 de P4 » — **RÉFUTÉ par la mesure (E3)** : l'index refuse la seconde VM du **même utilisateur**, pas le vol d'une VM par un autre |
| spec §3.6 | le fichier de configuration déclaratif (E1), les quatre états de `EtatVm` (E2) |
| spec §4 « P4 » | la configuration ICE rendue par la route (E4), et la rouge du critère ② (E3) |
| spec §3.2 | « l'index unique **partiel** […] refuse la seconde attribution » — même réfutation qu'E3 |
| `CLAUDE.md` P3 ⑬ legs 1, 2, 3 | la route du préfixe, l'orphelin `fraicheur.ts`, le lecteur de `session.utilisateur_id` |
| `CLAUDE.md` P2 ⑪ leg 4 | « elle attend son lecteur » |

- [ ] **Step 2 : chercher par le SENS, pas par la formule.** Une négation se
      dit de plusieurs façons — « pas / non / jamais / reste ouvert / n'est
      établi par rien / attend / à venir / c'est le sujet de P4 » — et **c'est
      celle qu'on n'a pas listée qui survit**.
- [ ] **Step 3 : vérifier plutôt que supposer** trois affirmations de ce plan
      qui sont exactement du genre qu'on écrit sans relire :
      `TYPES_RELAYES` n'a pas changé (E11) ; `PLATEFORME_VERSION` vaut toujours
      **1** des deux côtés ; `plateforme/src/base/pilote.test.ts:49` vaut
      toujours `expect(deps).toEqual(['pg', 'ws'])`.
- [ ] **Step 4 : annoter, jamais réécrire, ce qui est DATÉ.** Un énoncé
      historique qui dit vrai de son époque reste vrai ; le barrer le rendrait
      faux.

---

### Task 18 : `CLAUDE.md` gagne sa section P4

**Files:**
- Modify: `CLAUDE.md`

- [ ] **Step 1 :** poser la section après celle de P3, sur le patron des
      sections P1/P2/P3 : les journaux et leur **famille de lecture**, le fait
      n°1, le verdict des quatre critères **avec leur nombre d'exécutions**, ce
      que le code livre, **ce que P4 n'établit PAS**, les pièges neufs, et ce
      que P4 lègue.
- [ ] **Step 2 : les tailles sont RELEVÉES PAR LA COMMANDE, APRÈS la dernière
      édition de la ronde** — y compris celles de la revue transverse. Une
      table mesurée en début de ronde serait fausse à la fin de la même ronde ;
      c'est l'erreur que D8 a commise **en croyant bien faire**.
      🔴 **Toucher une ligne d'un tableau de comptes oblige à remesurer son
      compte**, même quand ce n'est pas l'objet de l'édition — les deux
      dernières occurrences du naufrage du « 487 » sont exactement cela.
- [ ] **Step 3 : les legs.** Reprendre les onze legs de P3 **un par un** et
      dire ce que P4 en a fait, y compris pour ceux qu'il n'a pas touchés. En
      ajouter les siens, dont **au minimum** : la reprise du canal `/agent`
      toujours jamais exercée (E9) ; le préfixe toujours par VM et
      `propriete.ts` toujours en mémoire (D9) ; `vm.vue_a` toujours orpheline ;
      `lireParNom` toujours sans appelant de production ; la divergence de
      refus avec G1 (D8) ; et **aucune constante calibrée** —
      `SEUIL_INJOIGNABLE_MS`, `PERIODE_BATTEMENT`, `DUREE_JETON_ACCES_MS`,
      `OCTETS_PREFIXE`, `DUREE_SECONDES`, et celles de P1/P2 que P4 ne
      recalibre pas.

---

### Task 19 : clore le document de résultats

**Files:**
- Create: `docs/superpowers/plans/2026-08-19-plateforme-p4-resultats.md`

- [ ] **Step 1 :** le document porte, dans cet ordre : comment lire les
      journaux ; le fait n°1 ; le verdict des quatre critères **avec leur
      nombre d'exécutions** ; les neuf rouges avec leur **message verbatim** ;
      la mesure des sondes d'index ; le sort des douze divergences E1…E12 ; le
      relevé de tailles **par la commande** ; **ce que P4 n'établit PAS** ; et
      le contrôle explicite qu'aucune preuve ne vit hors de git.
- [ ] **Step 2 : `git ls-files docs/superpowers/plans/journaux-plateforme-p4 | wc -l`**,
      et le nombre inscrit dans le document. 🔴 **Aucun rapport de tâche n'est
      cité comme preuve** : l'espace de travail de D9 a disparu avec six
      constats de revue, et le rapport de sa tâche 14 n'existe plus alors que
      `CLAUDE.md` invitait encore à le relire.
- [ ] **Step 3 : le témoin de clôture**, relancé **après** la dernière édition
      de code de la branche, et **dire à quel commit** il est joué.

---

## Ce que ce plan NE prescrit PAS, et pourquoi

- **Aucun backend d'hyperviseur.** `demarrer`, `arreter` et `instantane` ne
  sont jamais exécutés ; seule leur voie de refus l'est. La spec §8 le range
  hors périmètre, et le cadrage l'assume : « le hub **indique** et **dit qu'il
  ne peut pas redémarrer** ».
- **Aucune migration, aucune colonne neuve.** Voir la §« Structure des
  fichiers » : le corollaire est nommé pour le sous-bloc qui en aura besoin.
- **Aucune modification de `proto/`, d'`agent/`, de `scripts/`**, et donc
  **aucune montée de `PLATEFORME_VERSION`** (D6). Elle appartient à G1.
- **Aucune configuration ICE rendue par HTTP** (D7, E4).
- **Aucune route d'administration HTTP** (D8) : il n'existe pas de rôle
  d'administrateur, et en inventer un serait une décision de conception que P4
  n'a pas à prendre.
- **Aucun test de charge, aucune latence, aucun taux.** Deux exécutions par
  critère. Le nombre de VMs qu'un inventaire soutient n'est pas connu, et le
  plafond de 41 ports de relais de coturn (spec §2.7) n'est pas davantage
  mesuré ici.
- **Aucune mesure de la concurrence réelle au-delà de DEUX transactions**
  (D5) : une exécution, sur Postgres seul, séquence bloquée puis débloquée.
  **Rien n'est établi à N concurrents.**
- **Aucun exercice de la reprise du canal `/agent`** (E9) : P4 en dépend et ne
  l'éprouve pas.
- **Aucune calibration.** `SEUIL_INJOIGNABLE_MS = 90_000` et
  `PERIODE_BATTEMENT = 30 s` se recalibrent **ensemble** et vivent dans deux
  dépôts distincts ; aucun jugement d'usage n'est porté sur elles, ni sur
  aucune autre constante — la lacune que ce dépôt traîne depuis `BPP_MIN`.
- **Aucune fermeture du legs n°4 de P3** : le préfixe reste **par VM**,
  `signaling/propriete.ts` reste **en mémoire**, et deux clients humains de la
  même VM se disputent toujours `<préfixe>:w-1` après un redémarrage du
  service. **P4 branche la source du préfixe ; il ne change pas sa portée.**
- **Aucune revérification d'une session en cours** : la garde ne couvre que la
  poignée de main (legs n°9 de P2, reconduit par P3, reconduit ici).
- **Aucun durcissement de production** : ni TLS, ni cookies, ni en-têtes de
  sécurité, ni frein sur `/auth/connexion`, `/agent`, `/vm` ou `/session`, ni
  `coturn` restreint, ni `/sante`. **Les deux routes neuves sont ouvertes à la
  force brute exactement comme `/auth/connexion` l'est**, et c'est P5.
- **Aucun audit par un tiers** : CSRF, fixation de session, attaques
  temporelles — choix raisonnés, non éprouvés (spec §8).
