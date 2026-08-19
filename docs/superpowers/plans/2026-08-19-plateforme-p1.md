# Sous-bloc P1 — le service naît, absorbe le signaling, et persiste : plan d'implémentation

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Faire naître le paquet `plateforme/`, y absorber les 284 lignes de
`signaling/` **sans changer le protocole du fil**, poser une couche de
persistance SQL portable éprouvée sur deux moteurs, et tracer une session en
base — le tout sans jamais toucher la VM Windows.

**Architecture:** Un seul processus, un seul port. Un serveur HTTP porte la
montée en WebSocket ; le relais de signaling d'aujourd'hui devient l'un de ses
occupants, sur le chemin racine, avec ses messages, ses six types relayés et sa
poignée de main inchangés. À côté, une interface `Pilote` à deux
implémentations (`node:sqlite`, `pg`), un lanceur de migrations, et un dépôt
`session` qui écrit une ligne à l'appariement et la clôt à la déconnexion.

**Tech Stack:** Node/TypeScript (`plateforme/`, paquet autonome comme
`client/`, `proto/` — **il n'y a aucun workspace npm dans ce dépôt**), Vitest,
`ws`, `node:sqlite`, `pg`, Docker Compose pour l'instance Postgres de test.

**Spec :** `docs/superpowers/specs/2026-08-19-plateforme-design.md`, §4 « P1 ».

**Ce plan ne couvre QUE P1.** Rien de l'authentification (P2), rien du canal
plateforme ↔ agent (P3), rien de l'orchestration (P4), rien du durcissement de
production (P5). Les tables `utilisateur` et `vm` sont créées par P1 **pour une
raison structurelle mesurée** (§« Divergences », D3) et **restent vides** ; les
tables `agent_enrole`, `jeton_rafraichissement` et `application` ne sont pas
créées du tout.

---

## Global Constraints

- **Plafond de 500 lignes** par fichier de code source écrit à la main
  (`CLAUDE.md`, §« Conventions de code »). **Relevé par la commande le
  19 août 2026** : le plus gros fichier du périmètre serveur est
  `signaling/src/server.ts` à **208** lignes ; aucun fichier de `signaling/`,
  `client/` ou `proto/` n'approche le plafond. **`server.ts` est le seul
  fichier à surveiller** : c'est lui qui grossira en P2, P3 et P4.
  **L'extraction d'`appariement.ts` est une tâche de P1 (Task 4), placée AVANT
  toute addition** — pas une réaction de P2. C'est le seul geste qui a
  fonctionné dans ce dépôt (D9, `capteur/serveur/instances.rs`) ; les deux
  fichiers traités après coup y ont été **compressés**, geste que `CLAUDE.md`
  interdit nommément, puis extraits quand même.
- **Jamais `git add -A`** : nommer les fichiers. Un `git add -A` a déjà emporté
  le travail concurrent d'une autre tâche dans un commit qui ne compilait pas.
  **Un autre agent travaille dans le même arbre git.**
- **Aucune tâche de ce plan n'emploie la VM Windows.** C'est une décision de la
  spec (§4), pas une commodité : ⑤ est un sous-projet serveur, et faire
  dépendre sa recette d'une ressource exclusive et lente rendrait chaque
  itération coûteuse et chaque échec ambigu. Les pairs `agent` et `client` sont
  **simulés par des sockets scriptés** — c'est déjà ce que fait
  `signaling/src/server.test.ts`.
- **Un contrôle qu'on n'a jamais vu rouge n'est pas un contrôle.** Chaque test
  neuf est exécuté **avant** l'implémentation et **vu échouer**, et le message
  d'échec attendu est écrit dans la tâche. Ce dépôt a payé **trois fois** pour
  un contrôle qui ne pouvait pas échouer (F1 de D7, la sonde P1 de D8, le
  confondeur de D9), et D10 en a attrapé **quatre**, dont **trois écrits par le
  plan lui-même**. Ce plan-ci en est une source comme les autres : le doute
  porte sur lui.
- **Un saut est un échec.** Si Postgres est absent, `npm run test:postgres`
  **échoue** ; il ne se saute pas avec un avertissement. Un test qui disparaît
  quand sa dépendance manque est la même erreur sous une autre forme.
- **Aucun taux ne sera revendiqué.** Chaque énoncé de résultat porte son nombre
  d'exécutions. Règle de recette : **deux exécutions par critère, pas une**
  (héritée de D9 et D10).
- **Références d'entrée, relevées par la commande le 19 août 2026** :

  | Commande | Résultat relevé |
  | --- | --- |
  | `cd signaling && npx vitest run` | `Test Files 3 passed (3)`, `Tests 20 passed (20)` — `ice.test.ts` 6, `resilience.test.ts` 2, `server.test.ts` 12 |
  | `node --version` | `v24.9.0` |
  | `npm --version` | `11.6.0` |
  | `docker compose version` | `Docker Compose version v5.5.0` |
  | `npm view pg version` | `8.23.0` |
  | `command -v psql` | **absent** — tout accès Postgres passe par le pilote `pg`, jamais par `psql` |

- **Le protocole du fil ne change pas** (spec §10.1) : même port, même chemin
  racine, mêmes six types relayés (`server.ts:32-39`), même poignée de main
  `{role, session}`. **L'agent et le client d'aujourd'hui doivent fonctionner
  sans recompilation.** Seule `PLATEFORME_HOTE` doit être posée.
- **Aucune dépendance native.** Principe §4.2 du cadrage, écrit après le
  naufrage de `fuse-native`. `ws` et `pg` sont purement JavaScript ;
  `node:sqlite` est intégré à Node.

---

## Divergences relevées entre la spec et le code réel, AVANT d'écrire une ligne

Ces six points ont été trouvés en lisant le code que P1 déplace. Chacun est
tranché ici, avec son coût, pour qu'aucune tâche ne les redécouvre à
l'exécution.

### D1 — « les 483 lignes de tests restent INCHANGÉES » est INTENABLE, et il faut le dire

La spec l'écrit deux fois : §4 P1 (« déplacé […] **avec ses 483 lignes de
tests** ») et surtout la colonne ROUGE de son critère ① (« le critère est que
les tests soient **inchangés** »).

`signaling/src/resilience.test.ts` **ne peut pas** rester inchangé. Il lance le
véritable point d'entrée comme processus enfant, et trois de ses constantes
sont des **chemins relatifs au paquet** :

```
27  const __dirname = path.dirname(fileURLToPath(import.meta.url));
28  const signalingRoot = path.join(__dirname, '..');
29  const tsxBin = path.join(signalingRoot, 'node_modules', '.bin', 'tsx');
38          const proc = spawn(tsxBin, [path.join(__dirname, 'index.ts')], {
39              cwd: signalingRoot,
40              env: { ...process.env, SIGNALING_PORT: '0' },
46              const match = output.match(/le port (\d+)/);
```

Déplacé en `plateforme/src/signaling/`, `path.join(__dirname, '..')` désigne
`plateforme/src` et non la racine du paquet : `tsxBin` pointerait sur
`plateforme/src/node_modules/.bin/tsx`, **qui n'existe pas**. Le test ne serait
pas « rouge pour une bonne raison » : il échouerait à démarrer.

**Tranché** : ce plan lit le critère ① dans sa formulation la plus stricte qui
soit tenable — **aucune ASSERTION ne change, et le harnais change du minimum
strictement nécessaire, ligne par ligne, chaque ligne déclarée et prouvée par
un `git diff`.** Le déménagement (Task 2) ne change **qu'une seule ligne** — la
28 —, et deux de plus au branchement du serveur HTTP (Task 3). Les deux autres
fichiers de test restent **identiques au bit près**, prouvé par `sha256sum`
et non par une impression.

**Empreintes d'entrée, relevées le 19 août 2026** — c'est contre elles que la
preuve se fait :

```
0b32514d39fd5b0bf4c80884fccf8fd89178e5ee56f2135a2f899bc1cbbbb99c  ice.test.ts
89d1f25454974344131758aef10697830e109a0a77a34f56af91da27d7d75630  resilience.test.ts
2a1304e0f623af7b52f2b76aa7e2b292b24fdcc361ec94bbe9d920bf49977aab  server.test.ts
2a1304e0…  ← server.test.ts importe `./server` (l. 3) : voir D2
```

### D2 — renommer `server.ts` en `relais.ts` touche `server.test.ts`

L'arborescence de la spec (§5) nomme `signaling/relais.ts` (« le `server.ts`
d'aujourd'hui, allégé »). Or `signaling/src/server.test.ts:3` porte
`import { createSignalingServer } from './server';`.

**Tranché** : le déménagement (Task 2) est **verbatim, sans aucun renommage**.
Le renommage `server.ts` → `relais.ts` a lieu en Task 4, **avec** l'extraction
d'`appariement.ts`, où le changement d'une ligne d'import dans `server.test.ts`
est visible, motivé et borné. Séparer les deux gestes est ce qui permet de
prouver le premier.

### D3 — 🔴 `vm.utilisateur_id REFERENCES utilisateur(id)` FORCE P1 à créer `utilisateur`

Le schéma de la spec (§5) fait référencer `utilisateur(id)` par `vm`, et P1 ne
livre que `session` et `vm`. **Mesuré le 19 août 2026 sur le SQLite embarqué
(3.50.4)** :

```
ADD CONSTRAINT : REFUSE -> near "CONSTRAINT": syntax error
```

**SQLite ne sait pas ajouter une contrainte par `ALTER TABLE`.** Une clé
étrangère naît avec sa table ou n'existe jamais. Donc, de deux choses l'une :
ou bien P1 crée `utilisateur` (structure seule, vide, sans aucun comportement),
ou bien `vm.utilisateur_id` n'aura **jamais** de clé étrangère.

**Tranché** : la migration `0001` de P1 crée `utilisateur`, `vm`, `session` et
`schema_migration`. **`utilisateur` reste vide en P1** — P2 lui ajoute le
comportement, pas la table. Le coût est nommé : une table vide dans le schéma
dès P1, contre une contrainte d'intégrité perdue pour toujours.

⚠️ **Ce fait déborde P1 et doit être porté à P2/P4** : toute contrainte que
`utilisateur` ou `vm` recevra plus tard doit naître avec sa table, ou exiger
une reconstruction de table en douze étapes — laquelle est elle-même un danger
de portabilité, puisque Postgres ne s'y prend pas de la même façon.

### D4 — 🔴 la table `session` de la spec n'a AUCUNE colonne pour l'identifiant de session du signaling

Schéma §5 : `session(id, utilisateur_id NOT NULL, vm_id NOT NULL, ouverte_a,
fermee_a, motif)`. Or le critère ② de P1 exige « après appariement puis
déconnexion des deux pairs : une ligne dans `session` ». En P1 il n'y a ni
utilisateur ni VM — les deux colonnes `NOT NULL` n'ont **aucune valeur
honnête** à recevoir —, et **rien n'y range le nom de session de la poignée de
main**, qui est pourtant la seule identité qu'une session possède à ce stade.

**Tranché**, en deux mouvements :

- la migration `0001` crée `session` avec **`nom_session TEXT NOT NULL`** —
  colonne absente de la spec, ajoutée ici parce que sans elle la ligne écrite
  ne désigne rien ;
- `utilisateur_id` et `vm_id` naissent **`NULL`**, avec un commentaire de
  migration disant qu'ils seront renseignés par P2 et P3. **Ils ne seront pas
  resserrés en `NOT NULL` plus tard** — voir D3 : SQLite ne le permet pas sans
  reconstruction. C'est le coût, et il est assumé plutôt que découvert.

### D5 — `SERIAL` traverse les DEUX moteurs sans erreur : le test de portabilité ne suffit pas

Mesuré le 19 août 2026 sur SQLite 3.50.4 :

```
AUTOINCREMENT sqlite : ACCEPTE
SERIAL sqlite : ACCEPTE (type libre)
```

SQLite accepte n'importe quel nom de type par affinité. `SERIAL` y passe donc
sans bruit — et il passe aussi sur Postgres, où il **signifie autre chose**.
Deux passes vertes, deux schémas différents : **le test d'exécution ne peut pas
attraper `SERIAL`.**

**Tranché** : le sous-ensemble portable est gardé par **deux** contrôles, et
non par un — un **lint statique** sur les fichiers `.sql` (Task 8), qui seul
attrape `SERIAL`, et la **double passe d'exécution** (Task 9), qui attrape ce
que le lint ne sait pas voir. Cette redondance n'en est pas une : chacun
attrape ce que l'autre laisse passer, et le fait est mesuré, pas supposé.

### D6 — « quatre étapes » de `verify-all.sh` en désigne trois

La spec §7.3 annonce que `scripts/verify-all.sh` gagne **quatre** étapes, puis
en nomme trois (`npm test`, `npm run test:postgres`, `npm run typecheck`), la
quatrième étant la remarque « le typage n'est plus absent d'aucun paquet ».
Comme `signaling/` **disparaît** en P1, il n'y a aucun quatrième paquet à
typer.

**Tranché** : `verify-all.sh` gagne **trois** étapes —
`plateforme : npm run test:sqlite`, `plateforme : npm run test:postgres`,
`plateforme : npm run typecheck` — et le compte de trois est écrit dans la
tâche plutôt que le compte de quatre.

---

## Structure des fichiers

**Créés :**

| Fichier | Responsabilité |
| --- | --- |
| `plateforme/package.json` | paquet autonome, `engines` épinglé, quatre scripts |
| `plateforme/tsconfig.json` | copie conforme de `signaling/tsconfig.json` |
| `plateforme/src/config.ts` | lecture d'environnement, **une seule fois**, typée — PUR |
| `plateforme/src/config.test.ts` | dont la ROUGE de `PLATEFORME_HOTE` |
| `plateforme/src/index.ts` | point d'entrée : config, base, serveur, arrêt propre |
| `plateforme/src/http/serveur.ts` | serveur HTTP + routage de la montée WebSocket |
| `plateforme/src/http/serveur.test.ts` | écoute bornée, chemin racine, chemin inconnu |
| `plateforme/src/signaling/appariement.ts` | la table des sessions — **PUR**, extrait en Task 4 |
| `plateforme/src/signaling/appariement.test.ts` | |
| `plateforme/src/base/pilote.ts` | interface `Pilote` + `rendreMarqueurs` — **PUR** |
| `plateforme/src/base/pilote.test.ts` | dont la ROUGE du `?` littéral |
| `plateforme/src/base/pilote-sqlite.ts` | `node:sqlite` |
| `plateforme/src/base/pilote-postgres.ts` | `pg` |
| `plateforme/src/base/ouvrir.ts` | choisit le pilote selon `PLATEFORME_BASE` |
| `plateforme/src/base/migrations.ts` | lanceur, transactionnel |
| `plateforme/src/base/migrations/0001-socle.sql` | `schema_migration`, `utilisateur`, `vm`, `session` |
| `plateforme/src/base/sous-ensemble.test.ts` | **lint statique** des `.sql` (D5) |
| `plateforme/src/base/pilotes.test.ts` | la suite de dépôt, jouée contre le pilote courant |
| `plateforme/src/depot/session.ts` | ouvrir / clore / balayer |
| `plateforme/src/depot/session.test.ts` | |
| `docker-compose.plateforme.yml` | **versionné et sans secret**, sur le modèle de `docker-compose.coturn.yml` |
| `docs/superpowers/plans/2026-08-19-plateforme-p1-resultats.md` | le document de résultats, **versé dans git** |

**Déplacés** (`git mv`, contenu verbatim sauf mention) :

| De | Vers | Contenu |
| --- | --- | --- |
| `signaling/src/server.ts` | `plateforme/src/signaling/server.ts` puis `…/relais.ts` (Task 4) | verbatim en Task 2 |
| `signaling/src/ice.ts` | `plateforme/src/signaling/ice.ts` | **verbatim, jamais touché** |
| `signaling/src/index.ts` | `plateforme/src/signaling/index.ts` (Task 2), **supprimé** en Task 3 | |
| `signaling/src/server.test.ts` | `plateforme/src/signaling/server.test.ts` | verbatim en Task 2 ; **une ligne d'import** en Task 4 |
| `signaling/src/ice.test.ts` | `plateforme/src/signaling/ice.test.ts` | **identique au bit près, de bout en bout** |
| `signaling/src/resilience.test.ts` | `plateforme/src/signaling/resilience.test.ts` | **1 ligne** en Task 2, **2 lignes** en Task 3 |

**Supprimés :** `signaling/package.json`, `signaling/package-lock.json`,
`signaling/tsconfig.json` — et donc le répertoire `signaling/` entier. **Laisser
deux copies est la façon dont un fork dérive** (spec §4 P1).

**Modifiés :** `scripts/verify-all.sh`, `.gitignore` (l. 10,
`signaling/dist/`), `CLAUDE.md`.

**Jamais touchés par P1 :** `agent/`, `client/`, `proto/`, `src/`, `web/`,
`index.js`, `docker-compose.yml`, `scripts/run-agent.sh`. Le protocole du fil
ne change pas ; il n'y a donc rien à changer chez les pairs.

---

## Interfaces partagées

Ces signatures sont **fixées ici** : les tâches les consomment telles quelles.

```ts
// plateforme/src/config.ts
export interface Config {
    hote: string;          // PLATEFORME_HOTE — AUCUN défaut
    port: number;          // PLATEFORME_PORT, défaut 8080
    base: 'sqlite' | 'postgres';   // PLATEFORME_BASE, défaut 'sqlite'
    urlBase: string;       // PLATEFORME_BASE_URL (chemin de fichier ou URL pg)
}
export function lireConfig(env: Record<string, string | undefined>): Config;

// plateforme/src/base/pilote.ts
export interface Pilote {
    executer(sql: string, params: unknown[]): Promise<{ lignes: number }>;
    interroger<T>(sql: string, params: unknown[]): Promise<T[]>;
    transaction<T>(corps: (p: Pilote) => Promise<T>): Promise<T>;
    fermer(): Promise<void>;
}
/// Convertit les marqueurs `?` en `$1..$n`. LÈVE si le SQL porte une chaîne
/// littérale : la conversion n'est sûre QUE sous la règle « toute valeur passe
/// en paramètre ».
export function rendreMarqueurs(sql: string): string;

// plateforme/src/base/ouvrir.ts
export function ouvrirBase(config: Config): Promise<Pilote>;

// plateforme/src/base/migrations.ts
export function appliquerMigrations(p: Pilote, repertoire: string, maintenant: number): Promise<number>;

// plateforme/src/depot/session.ts
export function ouvrirSession(p: Pilote, nomSession: string, maintenant: number): Promise<string>;
export function clore(p: Pilote, id: string, maintenant: number, motif: string | null): Promise<void>;
export function balayerLesOuvertes(p: Pilote, maintenant: number): Promise<number>;
export function lireParNom(p: Pilote, nomSession: string): Promise<LigneSession[]>;

// plateforme/src/http/serveur.ts
export interface ServicePlateforme { port: number; close(): Promise<void>; }
export function demarrerServeur(config: Config, base: Pilote): Promise<ServicePlateforme>;

// plateforme/src/signaling/appariement.ts   (Task 4, PUR)
export type Role = 'agent' | 'client';
export class Appariement<S> {
    /// `undefined` = accepté ; sinon le motif de refus, mot pour mot celui
    /// que `server.ts:119-125` produit aujourd'hui.
    declarer(session: string, role: Role, socket: S): string | undefined;
    pair(session: string, role: Role): S | undefined;
    retenirOffre(session: string, sdp: string): void;
    prendreOffre(session: string): string | undefined;
    retirer(session: string, role: Role): { vide: boolean };
}
```

⚠️ **`Appariement` est générique en `S`** : c'est ce qui le rend PUR et
testable sans ouvrir un socket. Le relais l'instancie en `Appariement<WebSocket>`.

---

# Famille 0 — le paquet naît

### Task 1 : `plateforme/` existe, et `config.ts` refuse une écoute non nommée

**Objet :** faire naître le paquet autonome et son premier module pur — la
lecture d'environnement, dont `PLATEFORME_HOTE` **sans aucun défaut**.

**Files:**
- Create: `plateforme/package.json`, `plateforme/tsconfig.json`,
  `plateforme/src/config.ts`, `plateforme/src/config.test.ts`
- Modify: aucun

**Interfaces:**
- Consumes: rien.
- Produces: `lireConfig`, `Config` (§« Interfaces partagées »).

- [ ] **Step 1 : le paquet**

`plateforme/package.json` — les quatre scripts, `engines` **épinglé** (spec
§3.2 : « la version majeure de Node est épinglée, et un changement de majeure
impose de rejouer la suite avant tout autre travail »), et `dependencies`
réduites à `ws` (P1 n'a pas encore besoin de `pg` — Task 7 l'ajoute) :

```json
{
  "name": "@guacamole/plateforme",
  "version": "0.1.0",
  "private": true,
  "type": "module",
  "engines": { "node": ">=24.0.0 <25.0.0" },
  "scripts": {
    "start": "tsx src/index.ts",
    "test": "vitest run",
    "test:sqlite": "vitest run",
    "test:postgres": "vitest run",
    "typecheck": "tsc --noEmit"
  },
  "dependencies": { "ws": "^8.18.0" },
  "devDependencies": {
    "@types/ws": "^8.5.12",
    "tsx": "^4.19.0",
    "typescript": "^5.5.0",
    "vitest": "^2.0.0"
  }
}
```

`test:sqlite` et `test:postgres` sont posés **dès maintenant** avec leur forme
finale préfixée en Task 9 ; ils sont identiques à `test` tant qu'aucun pilote
n'existe, et **c'est dit dans la tâche plutôt que découvert**.

`plateforme/tsconfig.json` : **copie conforme** de `signaling/tsconfig.json`
(dont `"include": ["src/**/*.ts"]`, l. 12) — même cible, même `strict`, même
`noEmit`.

- [ ] **Step 2 : installer, et relever ce qui est installé**

```bash
cd plateforme && npm install 2>&1 | tail -3
```

- [ ] **Step 3 : écrire le test AVANT le module, et le voir ROUGE**

`plateforme/src/config.test.ts` :

```ts
import { describe, expect, it } from 'vitest';
import { lireConfig } from './config';

describe('lireConfig', () => {
    it("refuse de démarrer sans PLATEFORME_HOTE — il n'y a pas de défaut", () => {
        // Le défaut DOIT être l'absence de défaut (spec §4, critère ④).
        // Poser '0.0.0.0' par défaut ferait passer un test d'écoute sans rien
        // garantir : c'est exactement la panne muette que ce dépôt combat.
        expect(() => lireConfig({})).toThrow(/PLATEFORME_HOTE/);
    });

    it("n'invente pas d'adresse quand la variable est vide", () => {
        expect(() => lireConfig({ PLATEFORME_HOTE: '' })).toThrow(/PLATEFORME_HOTE/);
    });

    it('lit les quatre champs, avec leurs défauts non permissifs', () => {
        const c = lireConfig({ PLATEFORME_HOTE: '127.0.0.1' });
        expect(c).toEqual({
            hote: '127.0.0.1',
            port: 8080,
            base: 'sqlite',
            urlBase: ':memory:',
        });
    });

    it('refuse un PLATEFORME_BASE inconnu, plutôt que de retomber sur sqlite', () => {
        expect(() => lireConfig({ PLATEFORME_HOTE: '::1', PLATEFORME_BASE: 'mysql' }))
            .toThrow(/PLATEFORME_BASE/);
    });

    it('refuse un port qui n’est pas un entier', () => {
        expect(() => lireConfig({ PLATEFORME_HOTE: '::1', PLATEFORME_PORT: 'huit-mille' }))
            .toThrow(/PLATEFORME_PORT/);
    });
});
```

```bash
cd plateforme && npx vitest run src/config.test.ts 2>&1 | tail -10
```
**ROUGE attendue**, et c'est le message à relever :
`Failed to resolve import "./config"` — le module n'existe pas encore. Ce n'est
pas la rouge de la logique, seulement celle de l'absence ; la rouge **de la
logique** est jouée au Step 5.

- [ ] **Step 4 : écrire `config.ts`**

Une seule lecture d'environnement pour tout le processus. `PLATEFORME_HOTE`
n'a **pas** de branche de défaut — pas même `?? '127.0.0.1'`. Le commentaire de
tête porte la raison, mot pour mot, parce qu'un successeur qui « répare » ce
« bug » ferait exactement la régression contre laquelle le critère ④ existe :

```ts
// PLATEFORME_HOTE n'a AUCUN défaut, et c'est le point de cette fonction.
//
// `signaling/src/server.ts:67` faisait `new WebSocketServer({ port })` sans
// `host` : le service écoutait sur toutes les interfaces, et délivrait des
// identifiants TURN valables 24 h (`ice.ts:16`) à quiconque atteignait le
// port. Poser un défaut ici — même `127.0.0.1` — ferait passer le critère ④
// du sous-bloc P1 sans rien garantir : l'opérateur ne saurait jamais sur quoi
// il écoute. Une rupture bruyante vaut mieux qu'une écoute universelle
// silencieuse.
```

- [ ] **Step 5 : voir la ROUGE de la LOGIQUE, celle qui compte**

Poser temporairement `const hote = env.PLATEFORME_HOTE ?? '0.0.0.0';` et
relancer :

```bash
cd plateforme && npx vitest run src/config.test.ts 2>&1 | grep -E "✓|×|Tests "
```
**Attendu : les deux premiers tests échouent** (`expected [Function] to throw
error matching /PLATEFORME_HOTE/`), les trois autres passent. **Défaire le
défaut**, relancer : `Tests 5 passed (5)`.

⚠️ **Ce step n'est pas décoratif.** Sans lui, un `throw` mal écrit — un message
qui ne contient pas `PLATEFORME_HOTE`, ou un `throw` placé après la lecture des
autres variables — passerait pour un contrôle.

- [ ] **Step 6 : commit**

```bash
git add plateforme/package.json plateforme/tsconfig.json plateforme/package-lock.json \
        plateforme/src/config.ts plateforme/src/config.test.ts
git commit -m "plateforme(p1): le paquet nait, et l'ecoute n'a pas de defaut"
```

---

# Famille ① — le déménagement, prouvé plutôt que constaté

### Task 2 : déplacer `signaling/` verbatim — UNE seule ligne change, et elle est nommée

**Objet :** faire passer les 284 lignes de production et les 483 lignes de test
dans `plateforme/src/signaling/`, **sans en modifier le comportement**, et
supprimer `signaling/` du dépôt.

**Files:**
- Move: les six `.ts` de `signaling/src/` vers `plateforme/src/signaling/`
- Delete: `signaling/package.json`, `signaling/package-lock.json`,
  `signaling/tsconfig.json`
- Modify: `plateforme/src/signaling/resilience.test.ts` (**l. 28, et elle
  seule**), `.gitignore` (l. 10)

**Interfaces:**
- Consumes: rien. **Aucun renommage de fichier, aucun changement d'import.**
- Produces: rien de neuf.

- [ ] **Step 1 : relever l'état d'entrée — c'est la référence de la preuve**

```bash
cd signaling && npx vitest run 2>&1 | grep -E "✓ src|Test Files|Tests  "
cd .. && sha256sum signaling/src/*.ts
```
Attendu, **relevé le 19 août 2026** : `ice.test.ts` 6 tests,
`resilience.test.ts` 2, `server.test.ts` 12 ; `Test Files 3 passed (3)`,
`Tests 20 passed (20)`. Les six empreintes sont celles du §« Divergences », D1.

⚠️ **Les tests doivent être verts AVANT le déplacement.** Déplacer une suite
rouge rend indécidable ce que le déplacement a cassé.

- [ ] **Step 2 : déplacer, sans rien ouvrir**

```bash
mkdir -p plateforme/src/signaling
git mv signaling/src/server.ts        plateforme/src/signaling/server.ts
git mv signaling/src/ice.ts           plateforme/src/signaling/ice.ts
git mv signaling/src/index.ts         plateforme/src/signaling/index.ts
git mv signaling/src/server.test.ts   plateforme/src/signaling/server.test.ts
git mv signaling/src/ice.test.ts      plateforme/src/signaling/ice.test.ts
git mv signaling/src/resilience.test.ts plateforme/src/signaling/resilience.test.ts
git rm signaling/package.json signaling/package-lock.json signaling/tsconfig.json
```

`index.ts` est déplacé **à côté** des autres, et non hissé en
`plateforme/src/index.ts` : c'est ce qui garde inchangée la l. 38 de
`resilience.test.ts` (`path.join(__dirname, 'index.ts')`). Task 3 le hisse et
paie ce changement-là séparément.

- [ ] **Step 3 : LA ligne, et elle seule**

Dans `plateforme/src/signaling/resilience.test.ts`, l. 28 :

```ts
// AVANT
const signalingRoot = path.join(__dirname, '..');
// APRÈS
// La racine du PAQUET, deux crans au-dessus depuis `src/signaling/` — c'est
// là que vit `node_modules/.bin/tsx`. Seule ligne modifiée par le
// déménagement du sous-bloc P1 ; les assertions sont intactes.
const racinePaquet = path.join(__dirname, '..', '..');
```

et les deux usages du nom (l. 29 `tsxBin`, l. 39 `cwd`). **Renommer
`signalingRoot` en `racinePaquet` fait trois lignes touchées et non une** : le
faire ou non est un arbitrage, et ce plan le tranche en **gardant le nom
`signalingRoot`** — une seule ligne change, la preuve est plus courte, et le
renommage cosmétique appartient à Task 3, qui touche déjà ce fichier.

**Forme retenue, l. 28, et rien d'autre :**

```ts
const signalingRoot = path.join(__dirname, '..', '..');
```

- [ ] **Step 4 : PROUVER, pas constater**

```bash
git add plateforme/src/signaling/server.ts plateforme/src/signaling/ice.ts \
        plateforme/src/signaling/index.ts plateforme/src/signaling/server.test.ts \
        plateforme/src/signaling/ice.test.ts plateforme/src/signaling/resilience.test.ts
git add -- signaling/package.json signaling/package-lock.json signaling/tsconfig.json \
        signaling/src/server.ts signaling/src/ice.ts signaling/src/index.ts \
        signaling/src/server.test.ts signaling/src/ice.test.ts signaling/src/resilience.test.ts
git diff --cached -M --stat
```

⚠️ **Jamais `git add -A`, pas même avec un chemin.** C'est la forme exacte
qu'interdit `CLAUDE.md` : « un `git add -A agent/src` a emporté dans un commit
le travail concurrent d'une autre tâche ». Les douze fichiers sont nommés.
Un autre agent travaille dans cet arbre.
**Attendu : cinq renommages à `0 insertions(+), 0 deletions(-)`**, un renommage
à `1 insertion(+), 1 deletion(-)` pour `resilience.test.ts`, et trois
suppressions (`signaling/package.json`, `-lock.json`, `tsconfig.json`).

```bash
sha256sum plateforme/src/signaling/ice.test.ts plateforme/src/signaling/server.test.ts
```
**Attendu, au bit près** :
`0b32514d39fd5b0bf4c80884fccf8fd89178e5ee56f2135a2f899bc1cbbbb99c` et
`2a1304e0f623af7b52f2b76aa7e2b292b24fdcc361ec94bbe9d920bf49977aab`.

```bash
git diff --cached -M -- plateforme/src/signaling/resilience.test.ts
```
**Attendu : exactement une ligne `-` et une ligne `+`**, toutes deux la l. 28.
**Aucune ligne d'assertion dans le diff.**

⚠️ **C'est ici que le critère ① se gagne ou se perd.** Une comparaison
d'empreintes est décidable ; « les tests sont inchangés » ne l'est pas.

- [ ] **Step 5 : la suite est verte au même compte**

```bash
cd plateforme && npx vitest run 2>&1 | grep -E "✓ src|Test Files|Tests  "
```
**Attendu : `Tests 20 passed (20)`, et la même répartition 6 / 2 / 12.** Un
compte différent — même supérieur — signale que vitest ramasse autre chose que
la suite déplacée.

**Comment le rendre ROUGE, pour vérifier que ce contrôle peut échouer** :
laisser la l. 28 sur `'..'`. Attendu :
`process signaling terminé prématurément` ou `spawn … ENOENT`, sur
`resilience.test.ts` seul, les 18 autres restant verts. **Jouer cette rouge
avant de corriger** — c'est la seule façon de savoir que `resilience.test.ts`
exerce vraiment le chemin du processus enfant, et pas un chemin qui aurait
survécu par accident.

- [ ] **Step 6 : `.gitignore`**

`.gitignore:10` porte `signaling/dist/`. Le remplacer par `plateforme/dist/`.
⚠️ **Ne pas se contenter d'ajouter** : laisser la ligne morte, c'est laisser
croire qu'un répertoire existe.

- [ ] **Step 7 : commit**

```bash
git add .gitignore
git status --porcelain -- signaling plateforme/src/signaling
git commit -m "plateforme(p1): le signaling demenage verbatim, une seule ligne change"
```
Le `git status --porcelain` **avant** le commit est le contrôle : il ne doit
montrer que les douze fichiers du Step 4 plus `.gitignore`, tous déjà indexés.
Toute autre ligne appartient à l'agent concurrent et ne doit pas partir.
Porter dans le message : les deux empreintes inchangées, et `20 passed (20)`
avant et après.

---

### Task 3 : un serveur HTTP, la montée WebSocket sur le même port, et l'écoute bornée

**Objet :** remplacer `new WebSocketServer({ port })` par un serveur HTTP qui
écoute sur `PLATEFORME_HOTE` et route la montée WebSocket — le chemin racine
allant au relais de signaling, **inchangé**.

**Files:**
- Create: `plateforme/src/http/serveur.ts`, `plateforme/src/http/serveur.test.ts`,
  `plateforme/src/index.ts`
- Delete: `plateforme/src/signaling/index.ts`
- Modify: `plateforme/src/signaling/server.ts` (la seule ligne 67),
  `plateforme/src/signaling/resilience.test.ts` (**deux lignes**)

**Interfaces:**
- Consumes: `lireConfig` (Task 1), `createSignalingServer` (Task 2).
- Produces: `demarrerServeur`, `ServicePlateforme` (§« Interfaces partagées »).

- [ ] **Step 1 : ce qui doit rester vrai, relevé chez les pairs**

Les trois émetteurs d'aujourd'hui visent tous le **chemin racine**, sans
composant de chemin :

| Pair | Site | URL |
| --- | --- | --- |
| navigateur, page de session | `client/src/main.ts:29-30` | `ws://${window.location.hostname}:8080` |
| navigateur, page-shell | `client/src/shell-page.ts:7` | `ws://${window.location.hostname}:8080` |
| agent | `scripts/run-agent.sh:26` → `agent/src/signaling.rs:54` | `ws://192.168.3.1:8080` |

Aucun ne porte de chemin : `/` est donc le seul chemin à servir, et le seul que
P1 doit garantir. ⚠️ **Le service d'aujourd'hui accepte n'importe quel chemin**
(`server.ts:67` ne pose ni `path` ni routage) : router sur `/` **restreint** ce
qui était accepté. C'est délibéré — le canal `/agent` de P3 a besoin d'un
routage, et l'ajouter maintenant coûte quinze lignes contre une refonte plus
tard. **Aucun pair connu n'en est affecté** ; le tableau ci-dessus est la
preuve, et il est mesuré.

- [ ] **Step 2 : écrire les tests AVANT, et les voir ROUGES**

`plateforme/src/http/serveur.test.ts` :

```ts
// Trois propriétés, dont deux sont des critères de recette de P1.
it("n'écoute QUE sur l'adresse nommée", async () => { /* connexion sur 127.0.0.1 : OK */ });
it("refuse la montée WebSocket sur un chemin inconnu", async () => { /* /inconnu : le socket se ferme */ });
it("sert le relais de signaling sur le chemin racine, poignée de main comprise", async () => {
    // Un pair 'agent' et un pair 'client' sur '/' : l'offre du client
    // parvient à l'agent — exactement ce que `server.test.ts` éprouve déjà,
    // rejoué ici à travers le serveur HTTP pour prouver que le passage par
    // l'upgrade ne change rien.
});
```

```bash
cd plateforme && npx vitest run src/http/serveur.test.ts 2>&1 | tail -8
```
**ROUGE attendue** : `Failed to resolve import "./serveur"`.

⚠️ **Le premier test ne peut PAS prouver l'inaccessibilité depuis une autre
interface** : sur une machine de développement, `127.0.0.1` et l'adresse de
l'interface sont toutes deux locales, et une sonde depuis l'extérieur exigerait
une machine hors du réseau. **Ce test prouve que le service écoute sur
l'adresse nommée, pas qu'il n'écoute nulle part ailleurs.** La garantie réelle
du critère ④ vient de `config.ts` (Task 1) — pas de défaut, donc pas d'écoute
non nommée — et le test qui la porte est celui de Task 1, vu rouge au Step 5 de
cette tâche-là. **Dire l'inverse serait revendiquer une mesure non prise.**

- [ ] **Step 3 : `http/serveur.ts`**

```ts
import { createServer } from 'node:http';
// `noServer` plutôt que `{ server }` : le routage du chemin est explicite, et
// P3 y ajoutera `/agent` sans toucher au relais. Avec `{ server }`, `ws`
// accepterait toute montée sur tout chemin — c'est le comportement
// d'aujourd'hui (`signaling/src/server.ts:67`, ni `host` ni `path`), et il
// n'est pas extensible.
```

Le serveur HTTP répond `404` sur toute route HTTP (P2 et P4 en ajouteront) ;
`server.listen(config.port, config.hote)` — **les deux arguments, toujours**.
`close()` termine les sockets vivants puis ferme, sur le modèle de
`server.ts:201-206`.

- [ ] **Step 4 : `signaling/server.ts`, une seule ligne**

`createSignalingServer(port: number)` devient
`createSignalingServer(wss: WebSocketServer)` : la fabrique du `WebSocketServer`
part vers `http/serveur.ts`, **tout le reste du fichier est intact**.

⚠️ **`server.test.ts:40` fait `server = createSignalingServer(0)`**, et sa
l. 5 en déduit un type (`ReturnType<typeof createSignalingServer>`). Ce
changement de signature le casserait. **Tranché : `createSignalingServer`
garde une surcharge qui accepte un `port`** et construit alors son propre
`WebSocketServer` — c'est ce qui laisse `server.test.ts` **intact au bit près**
une tâche de plus. La surcharge porte son commentaire :

```ts
// Deux formes, à dessein. La forme `port` est celle qu'éprouve
// `server.test.ts` depuis le jalon 1 : la garder intacte est ce qui permet de
// dire que le déménagement du sous-bloc P1 n'a rien changé au relais. La forme
// `wss` est celle qu'emploie le service, où le serveur HTTP possède le port.
```

```bash
cd plateforme && sha256sum src/signaling/server.test.ts
```
**Attendu, toujours** : `2a1304e0f623af7b52f2b76aa7e2b292b24fdcc361ec94bbe9d920bf49977aab`.

- [ ] **Step 5 : `plateforme/src/index.ts`, et la ligne que `resilience.test.ts` lit**

`plateforme/src/signaling/index.ts` disparaît. Le point d'entrée devient
`plateforme/src/index.ts` : `lireConfig(process.env)`, puis `demarrerServeur`,
puis un `SIGINT` qui ferme proprement (le modèle est `signaling/src/index.ts:7-10`).

🔴 **Contrainte d'interface, à ne pas perdre :
`resilience.test.ts:46` fait `output.match(/le port (\d+)/)`.** La ligne
d'annonce du nouveau point d'entrée **doit** contenir `le port <n>`, sinon le
harnais expire au bout de 10 s (l. 60-62) avec
`démarrage du process signaling expiré`. Forme retenue, qui satisfait le motif
et dit davantage :

```ts
console.log(`plateforme à l'écoute sur ${config.hote}, le port ${service.port}`);
```

⚠️ **C'est un couplage entre un `console.log` et une expression régulière de
test.** Il est nommé ici plutôt que subi ; le commentaire du point d'entrée le
dit, et le test le vérifie de lui-même en échouant.

- [ ] **Step 6 : les DEUX lignes de `resilience.test.ts`**

```ts
// l. 38 : le point d'entrée a été hissé à la racine de `src/`
const proc = spawn(tsxBin, [path.join(signalingRoot, 'src', 'index.ts')], {
// l. 40 : PLATEFORME_HOTE n'a pas de défaut (critère ④) — sans elle, l'enfant
// refuse de démarrer, et le test mesurerait ce refus au lieu de la résilience.
    env: { ...process.env, PLATEFORME_HOTE: '127.0.0.1', PLATEFORME_PORT: '0' },
```

**Aucune assertion touchée.** Les deux `it(...)`, leurs `expect`, leurs
messages et leurs sessions témoins sont ceux des l. 102 à 167, inchangés.

```bash
cd plateforme && git diff -- src/signaling/resilience.test.ts | grep -c '^[+-][^+-]'
```
**Attendu : `4`** — deux `-`, deux `+`. Tout autre nombre veut dire qu'une
assertion a bougé.

- [ ] **Step 7 : jouer la ROUGE du couplage — celle qu'on n'aurait pas vue**

Changer l'annonce en `` `plateforme à l'écoute sur ${config.hote}:${service.port}` ``
et relancer :

```bash
cd plateforme && npx vitest run src/signaling/resilience.test.ts 2>&1 | tail -8
```
**Attendu : `démarrage du process signaling expiré`, après 10 s, sur les deux
tests.** Rétablir la forme du Step 5. **Sans cette rouge, on ne sait pas si le
test lit vraiment la sortie de l'enfant** — il pourrait passer parce qu'un
processus résiduel occupe le port.

- [ ] **Step 8 : la suite entière**

```bash
cd plateforme && npx vitest run 2>&1 | grep -E "Test Files|Tests  "
cd plateforme && npm run typecheck
```
**Attendu : `Tests 23 passed (23)`** — les 20 déplacés plus les 3 de
`http/serveur.test.ts` ; `typecheck` sortie 0.

⚠️ **Écrire le compte attendu AVANT de le mesurer.** D10 a récupéré un test
supprimé par erreur uniquement parce que le compte est sorti à 452 au lieu des
453 annoncés d'avance.

- [ ] **Step 9 : commit**

```bash
git add plateforme/src/http/serveur.ts plateforme/src/http/serveur.test.ts \
        plateforme/src/index.ts plateforme/src/signaling/server.ts \
        plateforme/src/signaling/resilience.test.ts
git add -- plateforme/src/signaling/index.ts
git commit -m "plateforme(p1): un port, un serveur HTTP, et l'ecoute nommee"
```

---

### Task 4 : extraire `appariement.ts` AVANT toute addition, et renommer en `relais.ts`

**Objet :** sortir la table des sessions de `server.ts` dans un module **pur**,
avant que P2, P3 et P4 n'y ajoutent la garde d'authentification, la liaison
agent ↔ VM et l'appartenance de session.

**Files:**
- Create: `plateforme/src/signaling/appariement.ts`,
  `plateforme/src/signaling/appariement.test.ts`
- Move: `plateforme/src/signaling/server.ts` → `plateforme/src/signaling/relais.ts`
- Modify: `plateforme/src/signaling/server.test.ts` (**la ligne 3, et elle
  seule**), `plateforme/src/http/serveur.ts` (l'import)

**Interfaces:**
- Consumes: rien.
- Produces: `Appariement<S>`, `Role` (§« Interfaces partagées »).

⚠️ **Cette tâche est le seul geste de P1 qui existe pour un sous-bloc futur.**
`relais.ts` fait 208 lignes ; le plafond est 500 ; il n'y a aucune urgence de
compte. **L'urgence est de doctrine** : ce dépôt a établi en D9 que l'extraction
faite AVANT l'addition rend sa marge (`capteur/serveur.rs` : 10 → 65), et que
celle faite après se paie d'une compression que `CLAUDE.md` interdit. P2 est
l'addition ; P1 est l'avant.

- [ ] **Step 1 : écrire `appariement.test.ts` AVANT le module**

Le module est **pur** : générique en `S`, aucun socket, aucun `ws`, aucune
horloge. Les tests emploient des chaînes en guise de sockets.

```ts
it("refuse un second occupant du même rôle, avec le motif d'aujourd'hui", () => {
    const a = new Appariement<string>();
    expect(a.declarer('s', 'agent', 'sock-1')).toBeUndefined();
    // Le motif est repris MOT POUR MOT de `server.ts:119-125` : le changer
    // casserait un pair qui le lit. `agent/src/signaling.rs:130` journalise
    // `reason` ; le navigateur le remonte dans une Error (`webrtc.ts:109`).
    expect(a.declarer('s', 'agent', 'sock-2'))
        .toBe('un agent est déjà connecté à la session s');
});
it('isole les sessions entre elles', () => { /* … */ });
it("retient la dernière offre et l'oublie quand elle est prise", () => { /* … */ });
it('la dernière offre écrase les précédentes', () => { /* … */ });
it('oublie la session quand ses deux pairs sont partis', () => { /* … */ });
```

```bash
cd plateforme && npx vitest run src/signaling/appariement.test.ts 2>&1 | tail -6
```
**ROUGE attendue** : `Failed to resolve import "./appariement"`.

- [ ] **Step 2 : extraire, verbatim autant que possible**

Déplacer dans `appariement.ts` : `type Role` (`server.ts:8`),
`interface Session` (l. 10-23) **avec son commentaire de neuf lignes sur
`offreEnAttente`** (l. 13-21) — ce commentaire explique le renversement d'ordre du sous-bloc D1 et
**part avec la donnée qu'il justifie**, comme `CLAUDE.md` l'exige du
commentaire de `TAMPON` en D9 —, `isRole` (l. 43-45), la `Map` (l. 68), la
logique de déclaration (l. 118-130), la mémorisation d'offre (l. 154-157,
167-173) et le retrait (l. 181-193).

**Restent dans `relais.ts`** : tout ce qui touche un socket — `send`,
`isJsonObject` et son commentaire de dix lignes sur `null` (l. 47-56), `TYPES_RELAYES`,
la délivrance ICE (l. 132-150), le corps du gestionnaire `message`.

⚠️ **`isJsonObject` NE part PAS.** Il garde un message de fil, pas une table ;
et son commentaire (l. 47-59) désigne nommément `index.ts` — le déplacer sans
son contexte de socket le rendrait incompréhensible.

- [ ] **Step 3 : renommer, et la seule ligne de `server.test.ts`**

```bash
cd plateforme && git mv src/signaling/server.ts src/signaling/relais.ts
```
`server.test.ts:3` : `from './server'` → `from './relais'`.

⚠️ **Le fichier de test ne change pas de nom.** `server.test.ts` porte les
mêmes douze tests sur la même fonction ; le renommer ajouterait un renommage à
prouver pour un gain nul. C'est un arbitrage, il est écrit, et il peut être
défait par qui préférera la symétrie de noms.

```bash
cd plateforme && git diff -- src/signaling/server.test.ts | grep -c '^[+-][^+-]'
```
**Attendu : `2`.**

- [ ] **Step 4 : les comptes, et la ROUGE de l'extraction**

```bash
cd plateforme && npx vitest run 2>&1 | grep -E "Test Files|Tests  "
cd plateforme && wc -l src/signaling/relais.ts src/signaling/appariement.ts
```
**Attendu : `Tests 28 passed (28)`** (23 + 5) ; `relais.ts` autour de **130**,
`appariement.ts` autour de **90**. Reporter les deux chiffres **relevés**, pas
ceux-ci.

**Comment le rendre ROUGE** : intervertir les deux branches du motif de refus
(écrire `client` là où le rôle déclaré est `agent`). Attendu :
`expected 'un client est déjà connecté à la session s' to be 'un agent est déjà
connecté à la session s'`, **et** l'échec d'un des douze tests de
`server.test.ts`. **Jouer cette rouge** : c'est ce qui établit que le module
extrait est bien celui que le relais emploie, et pas un jumeau mort.

- [ ] **Step 5 : commit**

```bash
git add plateforme/src/signaling/appariement.ts plateforme/src/signaling/appariement.test.ts \
        plateforme/src/signaling/relais.ts plateforme/src/signaling/server.test.ts \
        plateforme/src/http/serveur.ts
git add -- plateforme/src/signaling/server.ts
git commit -m "plateforme(p1): appariement extrait AVANT l'addition, server devient relais"
```

---

# Famille ② — la persistance, et la discipline qui remplace la bibliothèque

⚠️ **Ce que cette famille échange, écrit une fois pour toutes** (spec §3.2 et
§7.1) : une bibliothèque contre une discipline. **Aucun compilateur ne vérifie
une chaîne SQL écrite à la main.** Les deux gardes qui remplacent le
compilateur sont le **lint statique** (Task 8) et la **double passe
d'exécution** (Task 9), et le §« Divergences » D5 établit **par la mesure** que
ni l'un ni l'autre ne suffit seul.

### Task 5 : `base/pilote.ts` — l'interface, et le rendu des marqueurs qui LÈVE

**Objet :** poser l'interface `Pilote` et la seule fonction **pure** de la
couche : la conversion `?` → `$1..$n`, qui refuse un SQL portant une chaîne
littérale.

**Files:**
- Create: `plateforme/src/base/pilote.ts`, `plateforme/src/base/pilote.test.ts`

**Interfaces:**
- Consumes: rien.
- Produces: `Pilote`, `rendreMarqueurs` (§« Interfaces partagées »).

- [ ] **Step 1 : le fait mesuré qui commande cette tâche**

La spec écrit que la conversion « est sûre **parce que** la règle interdit les
littérales, donc aucun `?` ne peut se trouver dans une chaîne SQL ». **Ce n'est
pas une propriété du langage** — mesuré le 19 août 2026 sur SQLite 3.50.4 :

```
litteral avec ? : SQL valide
```

`SELECT '?' AS x` est du SQL parfaitement valide. Une conversion naïve le
transformerait en `SELECT '$1' AS x` et changerait silencieusement le sens de
la requête. **La discipline n'est donc pas garantie par le moteur : elle doit
être garantie par la fonction elle-même.**

**Décision prise au niveau de ce plan, et non transcrite de la spec :
`rendreMarqueurs` LÈVE si le SQL contient une apostrophe.** La spec dit « le
cas d'un `?` littéral est la ROUGE de son test » sans dire ce que la fonction
en fait ; ce plan tranche pour le refus, parce qu'un refus rend la discipline
**mécanique au lieu de documentaire**.

**Le coût est nommé** : aucune migration, aucune requête de dépôt ne pourra
porter de chaîne littérale — pas même une valeur par défaut. Le schéma v1
(spec §5) n'en contient aucune ; **vérifié en Task 8, Step 4**. Le jour où l'une
devient nécessaire, c'est cette décision qu'il faudra rouvrir, et non la
contourner.

- [ ] **Step 2 : les tests, AVANT le module**

```ts
it('numérote les marqueurs dans l’ordre', () => {
    expect(rendreMarqueurs('INSERT INTO t(a,b) VALUES(?,?)'))
        .toBe('INSERT INTO t(a,b) VALUES($1,$2)');
});
it('laisse intact un SQL sans marqueur', () => {
    expect(rendreMarqueurs('SELECT 1')).toBe('SELECT 1');
});
it('REFUSE un SQL portant une chaîne littérale', () => {
    // `SELECT '?' AS x` est du SQL VALIDE (mesuré sur SQLite 3.50.4) : une
    // conversion naïve en changerait le sens sans rien dire. Le refus est ce
    // qui rend mécanique la règle « toute valeur passe en paramètre ».
    expect(() => rendreMarqueurs("SELECT '?' AS x")).toThrow(/littérale/);
    expect(() => rendreMarqueurs("SELECT * FROM t WHERE a = 'x'")).toThrow(/littérale/);
});
it('refuse aussi le guillemet double, qui cache un identifiant délimité', () => {
    expect(() => rendreMarqueurs('SELECT "a b" FROM t')).toThrow(/littérale/);
});
```

```bash
cd plateforme && npx vitest run src/base/pilote.test.ts 2>&1 | tail -6
```
**ROUGE attendue** : `Failed to resolve import "./pilote"`.

- [ ] **Step 3 : écrire `pilote.ts`**

L'interface telle qu'elle est fixée. `rendreMarqueurs` : refuse `'` et `"`,
puis remplace chaque `?` par `$n`.

- [ ] **Step 4 : jouer la ROUGE de la LOGIQUE**

Retirer le refus (garder la seule substitution) et relancer :

```bash
cd plateforme && npx vitest run src/base/pilote.test.ts 2>&1 | grep -E "✓|×|Tests "
```
**Attendu : les deux derniers tests échouent** (`expected [Function] to throw
error matching /littérale/`), les deux premiers passent. Rétablir.

- [ ] **Step 5 : commit**

```bash
git add plateforme/src/base/pilote.ts plateforme/src/base/pilote.test.ts
git commit -m "plateforme(p1): l'interface Pilote, et le rendu des marqueurs qui refuse un litteral"
```

---

### Task 6 : `base/pilote-sqlite.ts` — `node:sqlite`, et l'avertissement qu'on ne masque pas

**Objet :** la première implémentation, celle du développement et des tests.

**Files:**
- Create: `plateforme/src/base/pilote-sqlite.ts`, `plateforme/src/base/ouvrir.ts`
- Modify: `plateforme/package.json` (aucune dépendance ajoutée — `node:sqlite`
  est intégré)

**Interfaces:**
- Consumes: `Pilote` (Task 5), `Config` (Task 1).
- Produces: `ouvrirBase`.

- [ ] **Step 1 : l'avertissement, mesuré**

```bash
node -e "const {DatabaseSync}=require('node:sqlite'); const d=new DatabaseSync(':memory:'); console.log('ok', d.prepare('select sqlite_version() v').get().v);"
```
**Relevé le 19 août 2026, verbatim** :

```
ok 3.50.4
(node:3839737) ExperimentalWarning: SQLite is an experimental feature and might change at any time
(Use `node --trace-warnings ...` to show where the warning was created)
```

🔴 **L'avertissement N'EST PAS MASQUÉ.** Ni `NODE_NO_WARNINGS`, ni
`--no-warnings`, ni `--disable-warning=ExperimentalWarning`, nulle part —
package.json, scripts npm, configuration vitest, `verify-all.sh`. **La spec
§3.2 accepte le module *parce que* « l'échec d'une évolution d'API est bruyant
et immédiat » ; masquer l'avertissement retirerait exactement le bruit qui
justifie la décision.**

Ce qui rend la sortie lisible n'est pas la suppression, c'est la **rareté** :
`node:sqlite` n'est importé **que par `pilote-sqlite.ts`**, donc
l'avertissement paraît une fois par processus, sur deux lignes.

- [ ] **Step 2 : le contrôle qui interdit de le masquer plus tard**

Ce contrôle vit dans `plateforme/src/base/pilote.test.ts` (déjà créé) :

```ts
it("ne masque nulle part l'avertissement expérimental de node:sqlite", () => {
    const pkg = readFileSync(new URL('../../package.json', import.meta.url), 'utf8');
    expect(pkg).not.toMatch(/NODE_NO_WARNINGS|--no-warnings|--disable-warning/);
});
it("n'ajoute aucune dépendance de production hors l'allow-list", () => {
    // Principe §4.2 du cadrage — « zéro dépendance native non maintenue »,
    // écrit après le naufrage de `fuse-native`. `ws` et `pg` sont purement
    // JavaScript ; `node:sqlite` est intégré à Node.
    //
    // ⚠️ Ce contrôle porte sur les `dependencies` DÉCLARÉES, jamais sur un
    // balayage de `node_modules` : mesuré le 19 août 2026, `rollup` — une
    // dépendance TRANSITIVE de vitest, donc de développement — installe déjà
    // `@rollup/rollup-linux-x64-gnu/rollup.linux-x64-gnu.node`. Un
    // `find node_modules -name '*.node'` serait rouge d'emblée, pour une
    // mauvaise raison.
    const deps = Object.keys(JSON.parse(pkg).dependencies).sort();
    expect(deps).toEqual(['pg', 'ws']);   // 'ws' seul avant la Task 7
});
```

**Comment le rendre ROUGE** : ajouter `"test": "node --no-warnings …"` au
`package.json`, ou y déclarer `better-sqlite3`. **Jouer les deux**, séparément,
et relever les deux messages d'échec.

- [ ] **Step 3 : `pilote-sqlite.ts`**

`DatabaseSync` est synchrone ; l'interface est asynchrone. L'adaptation est
triviale et **ne doit pas mentir** : les méthodes rendent des promesses déjà
résolues, et le commentaire de tête le dit — sinon un successeur croira à une
concurrence qui n'existe pas. `transaction` emploie `BEGIN`/`COMMIT`/`ROLLBACK`.
`PRAGMA foreign_keys=ON` est posé à l'ouverture : **sans lui, SQLite n'applique
pas les clés étrangères**, et la portabilité du schéma serait éprouvée d'un
seul côté.

`ouvrir.ts` choisit selon `config.base` et **lève sur une valeur inconnue** —
jamais de repli silencieux sur sqlite.

- [ ] **Step 4 : commit**

```bash
git add plateforme/src/base/pilote-sqlite.ts plateforme/src/base/ouvrir.ts \
        plateforme/src/base/pilote.test.ts
git commit -m "plateforme(p1): le pilote node:sqlite, et l'avertissement qu'on laisse crier"
```

---

### Task 7 : `base/pilote-postgres.ts` et l'instance de test, versionnée et sans secret

**Objet :** la seconde implémentation, et le Postgres contre lequel la Task 9
jouera sa passe.

**Files:**
- Create: `plateforme/src/base/pilote-postgres.ts`, `docker-compose.plateforme.yml`
- Modify: `plateforme/package.json` (ajoute `pg`), `plateforme/src/base/ouvrir.ts`

- [ ] **Step 1 : la dépendance**

```bash
cd plateforme && npm install pg @types/pg && npm ls pg 2>&1 | tail -3
```
`npm view pg version` rendait **`8.23.0`** le 19 août 2026. `pg` est **purement
JavaScript** : la propriété « aucune dépendance native » tient.

Mettre à jour l'allow-list de Task 6 Step 2 : `['pg', 'ws']`.

- [ ] **Step 2 : `docker-compose.plateforme.yml`, versionné et sans secret**

Sur le modèle de `docker-compose.coturn.yml` (versionné, sans secret) :
image `postgres:16-alpine`, port publié sur **`127.0.0.1` uniquement**,
identifiants **de test** posés en clair dans le fichier et déclarés comme tels
en commentaire — ce ne sont pas des secrets, et écrire qu'ils n'en sont pas
évite qu'un successeur les prenne pour tels.

⚠️ **Ne pas le composer avec `docker-compose.yml`**, qui n'est **pas versionné**
et qui ne déclare aucun service du nouveau produit (spec §2.7). Ce fichier se
lance seul :

```bash
docker compose -f docker-compose.plateforme.yml up -d
docker compose -f docker-compose.plateforme.yml ps
```
**Relevé d'entrée : `docker compose version` rend `v5.5.0` sur cette machine.**
⚠️ **`psql` est ABSENT** : tout accès passe par le pilote `pg`. Aucune étape de
ce plan n'invoque `psql`.

- [ ] **Step 3 : `pilote-postgres.ts`**

Un `Pool` de `pg`. Chaque `executer`/`interroger` passe le SQL par
`rendreMarqueurs` (Task 5) **avant** de l'envoyer. `transaction` prend un client
du pool et le garde pour toute la durée du corps — **c'est le piège de `pg` : un
`BEGIN` émis sur le pool et un `COMMIT` émis sur un autre client donneraient
deux transactions différentes, silencieusement.**

- [ ] **Step 4 : commit**

```bash
git add plateforme/src/base/pilote-postgres.ts plateforme/src/base/ouvrir.ts \
        plateforme/package.json plateforme/package-lock.json docker-compose.plateforme.yml
git commit -m "plateforme(p1): le pilote pg, et un Postgres de test versionne sans secret"
```

---

### Task 8 : les migrations, le socle, et le lint du sous-ensemble portable

**Objet :** le lanceur transactionnel, la migration `0001`, et le **contrôle
statique** qui seul attrape ce que l'exécution laisse passer.

**Files:**
- Create: `plateforme/src/base/migrations.ts`,
  `plateforme/src/base/migrations/0001-socle.sql`,
  `plateforme/src/base/sous-ensemble.test.ts`

- [ ] **Step 1 : le lint AVANT la migration, et sa ROUGE**

`sous-ensemble.test.ts` lit **tous** les `.sql` de `base/migrations/` et refuse
les jetons interdits par la spec §3.2 :

```ts
const INTERDITS = [
    /\bSERIAL\b/i, /\bAUTOINCREMENT\b/i, /\bdatetime\s*\(/i, /\bnow\s*\(/i,
    /\bCURRENT_TIMESTAMP\b/i, /\bUUID\b/i, /\bTIMESTAMPTZ\b/i, /\bJSONB\b/i,
    /\bBOOLEAN\b/i,
];
```

🔴 **Ce lint n'est pas redondant avec la double passe, et le fait est MESURÉ.**
Le 19 août 2026, sur SQLite 3.50.4 :

```
AUTOINCREMENT sqlite : ACCEPTE
SERIAL sqlite : ACCEPTE (type libre)
```

`SERIAL` traverse SQLite **par affinité de type** et traverse Postgres **en
signifiant tout autre chose**. Les deux passes seraient vertes sur deux schémas
différents. **Seul le lint l'attrape.** Inversement, le lint ne peut rien
contre une construction syntaxiquement licite des deux côtés mais de sémantique
divergente — c'est le rôle de la double passe. Chacun couvre l'angle mort de
l'autre.

```bash
cd plateforme && npx vitest run src/base/sous-ensemble.test.ts 2>&1 | tail -6
```
**ROUGE attendue** : `ENOENT … base/migrations` — le répertoire n'existe pas.
⚠️ **Ce n'est pas une rouge suffisante** : un lint qui ne lit aucun fichier
passe trivialement. Le test **doit** donc aussi asserter qu'il a bien lu au
moins un fichier (`expect(fichiers.length).toBeGreaterThan(0)`), sans quoi il
serait vert le jour où le répertoire serait renommé.

- [ ] **Step 2 : `0001-socle.sql`**

Quatre tables. Les trois décisions du §« Divergences » y sont **inscrites en
commentaire SQL**, pas seulement dans ce plan :

```sql
-- Sous-ensemble portable (spec §3.2) : identifiants TEXT/UUID v4, horodatages
-- INTEGER en millisecondes TOUJOURS écrites par l'application, booléens
-- INTEGER 0/1, aucune valeur littérale.

CREATE TABLE schema_migration (
    version    INTEGER PRIMARY KEY,
    applique_a INTEGER NOT NULL
);

-- `utilisateur` est créée par P1 et RESTE VIDE : P2 lui donne son
-- comportement, pas sa table.
--
-- Pourquoi elle ne peut pas attendre : `vm.utilisateur_id` la référence, et
-- SQLite ne sait pas ajouter une contrainte par ALTER TABLE — mesuré le
-- 19 août 2026 sur SQLite 3.50.4 : « near "CONSTRAINT": syntax error ». Une
-- clé étrangère naît avec sa table ou n'existe jamais.
CREATE TABLE utilisateur (
    id            TEXT PRIMARY KEY,
    email         TEXT NOT NULL UNIQUE,
    empreinte_mdp TEXT NOT NULL,
    cree_a        INTEGER NOT NULL
);

CREATE TABLE vm (
    id             TEXT PRIMARY KEY,
    nom            TEXT NOT NULL,
    adresse        TEXT NOT NULL,
    utilisateur_id TEXT NULL REFERENCES utilisateur(id),
    vue_a          INTEGER NULL
);
-- Index unique PARTIEL : plusieurs VM non attribuées coexistent, une seconde
-- attribution est refusée. Éprouvé le 19 août 2026 sur SQLite 3.50.4 —
-- « deux NULL toleres : OK », « double attribution : REFUSEE -> UNIQUE
-- constraint failed: vm.utilisateur_id ». C'est de lui que dépendra le
-- critère ② de P4.
CREATE UNIQUE INDEX vm_un_utilisateur ON vm(utilisateur_id)
    WHERE utilisateur_id IS NOT NULL;

-- `nom_session` est ABSENTE du schéma de la spec (§5), et ajoutée ici : sans
-- elle, la ligne écrite à l'appariement ne désigne rien — c'est le seul
-- identifiant qu'une session possède avant P2 et P3.
--
-- `utilisateur_id` et `vm_id` naissent NULL alors que la spec les veut
-- NOT NULL : en P1 il n'y a ni utilisateur ni VM, et il n'existe aucune valeur
-- honnête. Ils ne seront PAS resserrés plus tard — voir le commentaire de
-- `utilisateur` : SQLite exige une reconstruction de table, que Postgres ne
-- fait pas de la même façon.
CREATE TABLE session (
    id             TEXT PRIMARY KEY,
    nom_session    TEXT NOT NULL,
    utilisateur_id TEXT NULL,
    vm_id          TEXT NULL,
    ouverte_a      INTEGER NOT NULL,
    fermee_a       INTEGER NULL,
    motif          TEXT NULL
);
CREATE INDEX session_ouvertes ON session(fermee_a) WHERE fermee_a IS NULL;
```

⚠️ **Aucune apostrophe dans ce fichier** — c'est ce qu'exige `rendreMarqueurs`
(Task 5). Le lint du Step 1 gagne donc une assertion de plus : aucun `'` ni `"`
dans un `.sql`. **Sa ROUGE** : ajouter `DEFAULT 'x'` à une colonne.

- [ ] **Step 3 : `migrations.ts`**

Cent lignes environ, testées. Lit le répertoire, trie **numériquement** (pas
lexicographiquement : `0010` avant `0002` serait une panne muette à la dixième
migration), et pour chaque version non encore appliquée : `transaction` →
exécuter les instructions → insérer dans `schema_migration`. **Une migration en
échec annule sa transaction, arrête le service, laisse la version inchangée** —
jamais de schéma à moitié appliqué (spec §6).

- [ ] **Step 4 : le contrôle de la contrainte de Task 5, joué pour de vrai**

```bash
cd plateforme && node -e "
const {readFileSync,readdirSync}=require('node:fs');
for (const f of readdirSync('src/base/migrations')) {
  const t = readFileSync('src/base/migrations/'+f,'utf8');
  console.log(f, /['\"]/.test(t.replace(/--.*$/gm,'')) ? 'PORTE UN LITTERAL' : 'sans littéral');
}"
```
**Attendu : `0001-socle.sql sans littéral`.** (Les commentaires `--` sont
retirés avant le test : ils portent des apostrophes de français, que le SQL ne
voit jamais. ⚠️ **Le lint de Step 1 doit faire le même retrait**, sinon il est
rouge sur sa propre documentation — piège trouvé en écrivant ce plan, pas à
l'exécution.)

- [ ] **Step 5 : commit**

```bash
git add plateforme/src/base/migrations.ts plateforme/src/base/migrations/0001-socle.sql \
        plateforme/src/base/sous-ensemble.test.ts
git commit -m "plateforme(p1): migrations transactionnelles, socle, et le lint que l'execution ne remplace pas"
```

---

### Task 9 : LA MÊME suite contre les DEUX pilotes — et les TROIS rouges à jouer

**Objet :** faire jouer une seule suite de dépôt contre `node:sqlite` **et**
contre Postgres, par deux scripts distincts, et **jouer** les états qui doivent
la faire échouer.

**Files:**
- Create: `plateforme/src/base/pilotes.test.ts`
- Modify: `plateforme/package.json` (les deux scripts prennent leur forme finale)

- [ ] **Step 1 : les scripts**

```json
"test": "vitest run",
"test:sqlite": "PLATEFORME_BASE=sqlite vitest run",
"test:postgres": "PLATEFORME_BASE=postgres vitest run",
```

`PLATEFORME_BASE` est lu par le harnais de `pilotes.test.ts`, qui ouvre le
pilote correspondant, applique les migrations sur une base neuve, et joue
**les mêmes** assertions.

🔴 **Un saut est un échec** (spec §7.1). Si `PLATEFORME_BASE=postgres` et que
l'instance est injoignable, le test **échoue** avec la cause. Il n'y a ni
`it.skipIf`, ni `describe.skip`, ni `if (!disponible) return`. **Ce dépôt a
payé trois fois pour un contrôle qui ne pouvait pas échouer** ; un test qui
disparaît quand sa dépendance manque est la même erreur.

**Comment vérifier que ce refus fonctionne** :

```bash
docker compose -f docker-compose.plateforme.yml down
cd plateforme && npm run test:postgres 2>&1 | tail -8
```
**Attendu : ÉCHEC**, avec une cause de connexion (`ECONNREFUSED`) — pas un
`skipped`, pas un `0 passed`. Relancer l'instance ensuite.

- [ ] **Step 2 : les assertions, écrites une fois**

Migrations appliquées et idempotentes (rejouer n'écrit rien de plus) ; index
partiel (deux `NULL` tolérés, second non-`NULL` refusé) ; `INSERT … RETURNING` ;
`ON CONFLICT … DO UPDATE` ; transaction annulée par une exception ; clé
étrangère refusée.

⚠️ **Ce que ces assertions vérifient sur SQLite est MESURÉ ; ce qu'elles
vérifieront sur Postgres ne l'est PAS.** Relevé le 19 août 2026 sur SQLite
3.50.4 : index partiel OK, deux `NULL` tolérés, double attribution refusée,
`RETURNING` OK. **Aucun pendant Postgres n'a été pris**, ni par la spec (qui le
dit en toutes lettres : « le pendant Postgres de ce relevé n'a PAS été pris »),
ni par ce plan. **C'est précisément ce que cette tâche a pour rôle
d'établir plutôt que de croire.**

- [ ] **Step 3 : jouer la ROUGE prescrite par la spec — `datetime('now')`**

Ajouter à `0001-socle.sql` une colonne `DEFAULT (datetime('now'))`, puis :

```bash
cd plateforme && npm run test:sqlite 2>&1 | tail -6
```
**Attendu : ÉCHEC, et pas là où la spec l'annonce.** `datetime('now')` porte
une apostrophe : le **lint** de Task 8 le refuse d'abord, et `rendreMarqueurs`
le refuserait ensuite. **C'est un résultat plus fort que celui prévu — la
faute est arrêtée avant tout moteur — mais c'est un résultat DIFFÉRENT : il ne
prouve rien de la passe Postgres.** Relever le message exact et le porter au
document de résultats.

- [ ] **Step 4 : jouer la ROUGE qui atteint VRAIMENT Postgres — `AUTOINCREMENT`**

Remplacer la clé de `schema_migration` par
`version INTEGER PRIMARY KEY AUTOINCREMENT`, puis :

```bash
cd plateforme && npm run test:sqlite 2>&1 | tail -4
cd plateforme && npm run test:postgres 2>&1 | tail -6
```
**Attendu : SQLite VERT** — mesuré le 19 août 2026, `AUTOINCREMENT sqlite :
ACCEPTE` — **et Postgres ROUGE**, sur une erreur de syntaxe.

⚠️ **Le second attendu n'est PAS mesuré par ce plan** : `AUTOINCREMENT` est
documenté comme spécifique à SQLite, et je n'ai pas d'instance Postgres sous la
main pour l'établir. **C'est l'exécutant de cette tâche qui le mesure**, et
c'est exactement l'asymétrie que le critère ③ existe pour lever. Si Postgres
l'acceptait, c'est **le sous-ensemble portable** qu'il faudrait rouvrir, pas le
test.

Puis **rétablir** `0001-socle.sql` et vérifier les deux passes vertes.

- [ ] **Step 5 : le lint, lui aussi, est vu ROUGE**

Ajouter `nom SERIAL` à une table, relancer `npm run test:sqlite`.
**Attendu : le lint échoue** — alors que, mesuré, SQLite l'accepterait
(`SERIAL sqlite : ACCEPTE (type libre)`). **C'est la démonstration que le lint
attrape ce que l'exécution laisse passer**, et elle doit être jouée, pas
supposée. Rétablir.

- [ ] **Step 6 : commit**

```bash
git add plateforme/src/base/pilotes.test.ts plateforme/package.json
git commit -m "plateforme(p1): une suite, deux moteurs, et les trois rouges jouees"
```
Porter au message : les deux comptes de tests, et le fait que
`test:postgres` **échoue** quand l'instance est absente.

---

### Task 10 : `depot/session.ts` — ouvrir, clore, balayer

**Objet :** les quatre fonctions de dépôt dont le critère ② a besoin.

**Files:**
- Create: `plateforme/src/depot/session.ts`, `plateforme/src/depot/session.test.ts`

**Interfaces:**
- Consumes: `Pilote` (Task 5).
- Produces: `ouvrirSession`, `clore`, `balayerLesOuvertes`, `lireParNom`.

- [ ] **Step 1 : l'horloge est un PARAMÈTRE, jamais lue**

`maintenant: number` est passé partout — c'est la règle du sous-ensemble
(spec §3.2 : « toujours écrites par l'application ») **et** le précédent du
dépôt : `signaling/src/ice.ts:32-42` prend déjà `maintenant` en paramètre pour
la même raison, et `server.ts:137-139` explique pourquoi le `Date.now()` est lu
chez l'appelant.

**Ce qui rend ce choix vérifiable** : un test pose `maintenant = 1_000_000`,
clôt à `1_000_500`, et asserte les **valeurs exactes**. Un `Date.now()` caché
dans le dépôt le ferait échouer.

**ROUGE à jouer** : remplacer `maintenant` par `Date.now()` dans `ouvrirSession`.
Attendu : `expected 17... to be 1000000`.

- [ ] **Step 2 : les tests, joués contre les DEUX pilotes**

`session.test.ts` emploie le même harnais que `pilotes.test.ts` (Task 9) : il
tourne donc sous `test:sqlite` **et** sous `test:postgres`, sans être écrit
deux fois.

```
it('ouvre une ligne avec ouverte_a posé et fermee_a nul')
it('clôt la ligne : fermee_a posé, motif enregistré')
it('ne clôt pas deux fois : la seconde clôture ne change pas fermee_a')
it('balaie au démarrage les lignes restées ouvertes, et rend leur compte')
it('deux sessions du même nom successives sont deux lignes distinctes')
```

Le dernier mérite son mot : le nom de session n'est **pas** unique dans le
temps — `bureau` revient à chaque démarrage d'agent
(`agent/src/superviseur/protocole.rs`, constante `SESSION_DE_CONTROLE`). La
clé primaire est un UUID, jamais le nom.

- [ ] **Step 3 : commit**

```bash
git add plateforme/src/depot/session.ts plateforme/src/depot/session.test.ts
git commit -m "plateforme(p1): le depot session, horloge injectee"
```

---

# Famille ③ — le service devient un tout

### Task 11 : la base au démarrage — migrations appliquées, et refus de démarrer dégradé

**Objet :** brancher la base sur le point d'entrée, appliquer les migrations, et
**refuser de démarrer** si elle est injoignable.

**Files:**
- Modify: `plateforme/src/index.ts`, `plateforme/src/http/serveur.ts`
- Create: `plateforme/src/index.test.ts`

- [ ] **Step 1 : la règle, et sa raison**

Spec §6, premier cas : « le service **refuse de démarrer**, avec la cause. Il ne
démarre pas dégradé : **un signaling qui apparie sans rien enregistrer serait
indiscernable du bon fonctionnement** ». C'est la classe de défaut contre
laquelle tout ce dépôt est écrit.

Ordre au démarrage, non négociable : `lireConfig` → `ouvrirBase` →
`appliquerMigrations` → `balayerLesOuvertes` → `demarrerServeur`. **Le port ne
s'ouvre qu'en dernier** : un pair ne doit jamais atteindre un service dont la
base n'est pas prête.

- [ ] **Step 2 : le test, et sa ROUGE**

```ts
it("refuse de démarrer quand la base est injoignable, et n'ouvre aucun port", async () => {
    // Postgres sur un port mort. Deux assertions distinctes : le démarrage
    // rejette AVEC la cause, et rien n'écoute.
    await expect(demarrer({ ...config, base: 'postgres', urlBase: 'postgres://…:1/x' }))
        .rejects.toThrow(/base/i);
    await expect(connecterA(config.port)).rejects.toThrow(/ECONNREFUSED/);
});
it('clôt au démarrage les sessions restées ouvertes, et dit combien', async () => { /* … */ });
```

**Comment le rendre ROUGE** : intervertir l'ordre — ouvrir le serveur **avant**
la base. Attendu : la seconde assertion échoue (le port répond), la première
peut encore passer. **C'est le point** : sans la seconde assertion, un service
qui ouvre son port puis meurt passerait pour correct.

- [ ] **Step 3 : le balayage**

`balayerLesOuvertes` clôt les lignes dont `fermee_a` est nul, avec un motif
nommé (`plateforme redémarrée`). Spec §6 : « les sessions vivantes survivent […]
leur ligne en base garde `fermee_a` nul et sera close par un balayage au
démarrage ».

⚠️ **Ce balayage MENT sur les sessions qui ont réellement survécu.** Le média
WebRTC ne dépend plus du signaling une fois l'offre et la réponse échangées
(`agent/src/demarrage.rs:244-252` le journalise explicitement) : une session
peut être **vivante** alors que sa ligne vient d'être close. **La ligne
`session` est donc une trace de l'appariement, pas un état de vérité du média**
— c'est écrit dans le commentaire de `depot/session.ts`, et c'est une limite de
P1, pas un défaut à corriger ici.

- [ ] **Step 4 : commit**

```bash
git add plateforme/src/index.ts plateforme/src/http/serveur.ts plateforme/src/index.test.ts
git commit -m "plateforme(p1): la base d'abord, le port ensuite, et jamais de demarrage degrade"
```

---

### Task 12 : une session laisse une trace — le critère ②

**Objet :** écrire une ligne `session` à l'appariement, la clore à la
déconnexion.

**Files:**
- Modify: `plateforme/src/signaling/relais.ts`, `plateforme/src/http/serveur.ts`
- Create: `plateforme/src/signaling/trace.test.ts`

- [ ] **Step 1 : QUAND, exactement — et ce n'est pas évident**

Le critère ② dit « après appariement **puis déconnexion des deux pairs** ». Le
relais connaît deux instants :

- `relais.ts` (ex-`server.ts:118-130`) — un pair se déclare. **Un seul pair
  n'est pas un appariement** : le superviseur se déclare `agent` sur `bureau`
  au démarrage de la VM et peut y rester seul des heures.
- `appariement.ts` — le second rôle rejoint. **C'est l'appariement.**

**Tranché : la ligne s'ouvre quand les DEUX rôles sont présents**, et se clôt
quand `retirer` rend `{ vide: true }` — l'instant exact où
`server.ts:190-192` supprime la session de la `Map` aujourd'hui. Ce choix rend
le critère ② littéralement lisible : une ligne, `ouverte_a` non nul,
`fermee_a` non nul.

⚠️ **Conséquence à écrire dans le code** : un agent qui se déclare et repart
sans jamais rencontrer de client **ne laisse aucune trace**. C'est une décision,
pas un oubli ; elle se rouvrira le jour où l'on voudra observer les agents
présents — ce qui est le sujet de P3 (`vu_a`), pas de P1.

- [ ] **Step 2 : l'écriture ne doit pas pouvoir tuer une session**

`ouvrirSession` est asynchrone ; le gestionnaire `message` du relais est
synchrone. Une promesse rejetée sans `catch` dans un gestionnaire d'événement
`ws` **abat le processus** — c'est exactement le mode de défaillance que
`isJsonObject` (`relais.ts`, ex-`server.ts:57-59`) a été écrit pour fermer, et
son commentaire (ex-`server.ts:47-56`) le dit en toutes lettres : « une
TypeError non interceptée y est fatale : elle abat tout le process Node ».

**Tranché** : l'écriture est déclenchée sans être attendue, avec un `.catch`
**qui journalise et n'interrompt rien**. La trace en base est une **observation**
du signaling ; elle ne doit jamais en devenir une **dépendance**.

⚠️ **Coût nommé** : une écriture perdue ne se voit qu'au journal, et le critère
② ne l'attraperait pas s'il ne mesurait que « la ligne finit par exister ». Le
test **doit** donc attendre la ligne avec une borne de temps et échouer sur
expiration, pas boucler indéfiniment.

- [ ] **Step 3 : le test, et sa ROUGE — celle que la spec prescrit**

```ts
it('écrit une ligne à l’appariement, et la clôt à la déconnexion des deux pairs', async () => {
    const agent  = await connecter('agent',  'trace-1');
    const client = await connecter('client', 'trace-1');
    const [ligne] = await attendreLigne(base, 'trace-1', 2000);
    expect(ligne.ouverte_a).toBeGreaterThan(0);
    expect(ligne.fermee_a).toBeNull();
    await fermer(agent); await fermer(client);
    const [close] = await attendreClose(base, 'trace-1', 2000);
    expect(close.fermee_a).toBeGreaterThanOrEqual(close.ouverte_a);
});
it("n'écrit rien quand un seul pair s'est déclaré", async () => { /* … */ });
```

**ROUGE prescrite par la spec (§4, critère ②) : « supprimer l'écriture : le
compte reste à 0. À exercer. »** Retirer l'appel à `ouvrirSession` et relancer :

```bash
cd plateforme && npx vitest run src/signaling/trace.test.ts 2>&1 | tail -8
```
**Attendu : expiration au bout de 2 000 ms**, message
`aucune ligne session pour trace-1`. Rétablir.

⚠️ **Jouer AUSSI la rouge du second test** — faire écrire la ligne dès la
première déclaration. Attendu : `n'écrit rien quand un seul pair s'est déclaré`
échoue. **Sans elle, le premier test serait vert avec une écriture posée trop
tôt**, et le critère ② mesurerait autre chose que l'appariement.

- [ ] **Step 4 : le protocole du fil n'a pas bougé**

```bash
cd plateforme && npx vitest run 2>&1 | grep -E "Test Files|Tests  "
cd plateforme && sha256sum src/signaling/ice.test.ts
```
**Attendu** : `ice.test.ts` toujours à
`0b32514d39fd5b0bf4c80884fccf8fd89178e5ee56f2135a2f899bc1cbbbb99c`, et les
**12** tests de `server.test.ts` toujours verts sans qu'aucune assertion n'ait
bougé — c'est le critère ①, revérifié après l'addition qui aurait pu le casser.

- [ ] **Step 5 : commit**

```bash
git add plateforme/src/signaling/relais.ts plateforme/src/http/serveur.ts \
        plateforme/src/signaling/trace.test.ts
git commit -m "plateforme(p1): une session apparee laisse une ligne, et la clot en partant"
```

---

### Task 13 : `verify-all.sh` cesse d'ignorer le service serveur

**Objet :** corriger la lacune d'outillage du §2.5 de la spec — 483 lignes de
tests hors du filet, et aucun `typecheck`.

**Files:**
- Modify: `scripts/verify-all.sh`, `CLAUDE.md` (le seul § « Portée »)

- [ ] **Step 1 : le constat, relevé**

`scripts/verify-all.sh:46-56` enchaîne quatre étapes TypeScript — `client` puis
`proto`, `npm test` et `npm run typecheck` chacun. **`signaling/` n'y figurait
dans aucune** ; et `signaling/package.json:6-9` ne déclarait que `start` et
`test`, **sans `typecheck`**, alors que `signaling/tsconfig.json` existait.

L'en-tête du script (l. 7-14) dit **pourquoi** cela compte : Vitest repose sur
esbuild, qui transpile sans vérifier les types, et **deux revues successives
ont approuvé du code dont `tsc --noEmit` échouait**. Le service serveur était
hors de ce filet.

- [ ] **Step 2 : trois étapes, et non quatre**

```bash
etape "plateforme : npm run test:sqlite"
(cd plateforme && npm run test:sqlite) || echec "plateforme : npm run test:sqlite"

etape "plateforme : npm run test:postgres"
(cd plateforme && npm run test:postgres) || echec "plateforme : npm run test:postgres"

etape "plateforme : npm run typecheck"
(cd plateforme && npm run typecheck) || echec "plateforme : npm run typecheck"
```

⚠️ **La spec §7.3 annonce « quatre étapes » et en nomme trois** (voir
§« Divergences », D6) : la quatrième visait le typage d'un paquet resté sans
`typecheck`, et ce paquet — `signaling/` — **n'existe plus**. Trois est le
compte juste, et le dire vaut mieux que d'inventer une quatrième étape pour
honorer un nombre.

⚠️ **`test:postgres` rend `verify-all.sh` dépendant d'une instance Postgres.**
C'est **voulu** et c'est le prix du §7.1 : un saut est un échec. L'en-tête du
script gagne la ligne qui le dit, avec la commande pour lancer l'instance.

- [ ] **Step 3 : `CLAUDE.md`, § « Portée »**

`CLAUDE.md:20` liste `agent/src/`, `client/src/`, `signaling/`, `proto/`,
`src/`, `web/`, `scripts/`. **Remplacer `signaling/` par `plateforme/`.**

⚠️ **La commande de vérification, elle, attrapait déjà `signaling/`
mécaniquement** — son filtre n'exclut que `node_modules`, les verrous, `dist/`,
`testdata/`, `docs/` et `CLAUDE.md`. C'est la même divergence texte/commande
que `CLAUDE.md` signale déjà, **non tranchée**, pour `client/verify-webrtc.mjs`
(497 lignes, marge 3). **P1 ne tranche pas celle-là** — elle appartient au
propriétaire du dépôt —, il se contente de ne pas en créer une seconde.

- [ ] **Step 4 : la vérification d'ensemble, jouée entièrement**

```bash
docker compose -f docker-compose.plateforme.yml up -d
./scripts/verify-all.sh 2>&1 | tail -20
```
**Attendu : `Toutes les vérifications sont passées.`** Relever les comptes de
chaque étape.

**Comment le rendre ROUGE** : arrêter l'instance Postgres et relancer.
**Attendu : `ÉCHEC : plateforme : npm run test:postgres`**, et le script
s'arrête là — pas un avertissement, pas un saut.

- [ ] **Step 5 : commit**

```bash
git add scripts/verify-all.sh CLAUDE.md
git commit -m "plateforme(p1): verify-all cesse d'ignorer le service serveur"
```

---

# Famille ④ — la recette, et la mémoire

### Task 14 : recette P1 — les quatre critères, deux exécutions chacun

**Objet :** juger P1 sur ses quatre critères, **deux fois chacun**, et verser
les pièces dans git.

**Files:**
- Create: `docs/superpowers/plans/2026-08-19-plateforme-p1-resultats.md`,
  `docs/superpowers/plans/journaux-plateforme-p1/` (les sorties brutes)

🔴 **Toute preuve d'une affirmation portée dans `CLAUDE.md` doit être VERSÉE
DANS GIT.** D9 a perdu **six** constats de revue parce que leur preuve vivait
dans `.superpowers/sdd/`, gitignoré et jamais commité : la tâche 18 de D10 a
établi **par la commande** que le répertoire a disparu, et les six constats
sont **définitivement perdus**. Les journaux de cette tâche vont sous
`docs/superpowers/plans/journaux-plateforme-p1/`, suivis par git.

- [ ] **Step 1 : le tableau des quatre critères, à remplir par la MESURE**

| # | Critère | Comment il est jugé | La ROUGE, et où elle a été jouée |
| --- | --- | --- | --- |
| ① | Le service apparie deux pairs simulés et le média négocie comme avant | `Tests 20 passed` sur la suite déplacée, `sha256sum` identique pour `ice.test.ts` et `server.test.ts`, `git diff` de 1 puis 2 lignes sur `resilience.test.ts` — **aucune assertion touchée** | Task 2 Step 5 (l. 28 laissée sur `'..'`) et Task 3 Step 7 (l'annonce du port) |
| ② | Une session laisse une trace en base | une ligne dans `session` avec `ouverte_a` et `fermee_a` non nuls | Task 12 Step 3 — **les deux** rouges : écriture retirée, puis écriture posée trop tôt |
| ③ | La même suite passe sur `node:sqlite` **et** sur Postgres | `npm run test:sqlite` et `npm run test:postgres` verts | Task 9 Steps 3, 4, 5 — trois rouges, dont **une seule** atteint réellement Postgres |
| ④ | L'écoute est bornée | le service refuse de démarrer sans `PLATEFORME_HOTE` et écoute sur cette seule adresse | Task 1 Step 5 (le défaut `'0.0.0.0'` posé, deux tests rouges) |

- [ ] **Step 2 : DEUX exécutions de chaque, journalisées**

```bash
mkdir -p docs/superpowers/plans/journaux-plateforme-p1
docker compose -f docker-compose.plateforme.yml up -d
for i in 1 2; do
  (cd plateforme && npm run test:sqlite)   > docs/superpowers/plans/journaux-plateforme-p1/sqlite-$i.log 2>&1
  (cd plateforme && npm run test:postgres) > docs/superpowers/plans/journaux-plateforme-p1/postgres-$i.log 2>&1
  (cd plateforme && npm run typecheck)     > docs/superpowers/plans/journaux-plateforme-p1/typecheck-$i.log 2>&1
done
./scripts/verify-all.sh > docs/superpowers/plans/journaux-plateforme-p1/verify-all.log 2>&1
grep -h "Tests  " docs/superpowers/plans/journaux-plateforme-p1/*.log
```

⚠️ **Les journaux porteront l'`ExperimentalWarning` de `node:sqlite`, et c'est
voulu.** Ne pas le filtrer à l'écriture ; il est la trace visible de la
décision §3.2, et le contrôle de Task 6 Step 2 interdit de l'éteindre.

- [ ] **Step 3 : le critère ④ dit exactement ce qu'il établit, et rien de plus**

Ce qui est **établi** : sans `PLATEFORME_HOTE`, le service ne démarre pas ;
avec, il écoute sur cette adresse.

Ce qui n'est **pas** établi : que le service soit inaccessible depuis une autre
interface. Sur une machine de développement, une telle sonde exigerait une
machine hors du réseau ; **elle n'est pas prescrite, donc P1 n'établit pas
l'inaccessibilité effective.** C'est la même réserve que la spec porte sur le
critère ② de P5, et pour la même raison.

- [ ] **Step 4 : jouer la ROUGE de compatibilité, celle qui n'est dans aucun test**

Le seul engagement de P1 qu'aucun test unitaire ne couvre est celui du §10.1 :
« l'agent et le client d'aujourd'hui fonctionnent **sans recompilation** ».
Sans VM, il se juge par un **pair scripté qui imite exactement les octets du
pair réel** :

```bash
cd plateforme && node -e "
const {WebSocket}=require('ws');
// Ce que \`agent/src/signaling.rs:59-60\` envoie, verbatim, avec le nom de
// session que \`agent/src/superviseur/protocole.rs:17\` impose.
const w=new WebSocket('ws://127.0.0.1:8080');
w.on('open',()=>w.send(JSON.stringify({role:'agent',session:'bureau'})));
w.on('message',m=>console.log('recu:',m.toString().slice(0,80)));
" 2>&1 | head -5
```
**Attendu : aucune erreur, et le service accepte la déclaration.** Faire de
même pour `{role:'client', session:'bureau'}`, qui est ce
qu'envoie `client/src/shell-page.ts:45`.

**Comment le rendre ROUGE** : router la montée WebSocket sur `/signaling` au
lieu de `/`. **Attendu : la connexion se ferme.** Cette rouge est la seule
preuve que le chemin racine est réellement servi — et elle est **gratuite à
jouer**.

- [ ] **Step 5 : écrire le document de résultats**

`docs/superpowers/plans/2026-08-19-plateforme-p1-resultats.md`, sur le modèle
des documents de résultats du chantier D. Il porte, **au minimum** :

1. le verdict de chaque critère **avec son nombre d'exécutions** ;
2. **chaque ROUGE jouée, avec son message d'échec verbatim** — c'est ce qui
   distingue un contrôle d'une formule ;
3. les six divergences spec/code du § d'entrée de ce plan, avec leur sort ;
4. ce que P1 **n'établit pas** (§ ci-dessous) ;
5. les tailles de fichiers **relevées par la commande** ;
6. un renvoi nommé vers chaque journal versé.

**§ « Ce que P1 n'établit PAS », à écrire sans le raboter :**

- **Aucun taux.** Deux exécutions par critère, jamais une campagne.
- **Aucune latence, aucune charge.** La cible « < 3 s si VM chaude » n'est
  mesurée par aucun critère ; le nombre de sessions simultanées soutenues est
  inconnu.
- **Aucune exécution avec l'agent ou le navigateur réels.** Les pairs sont
  simulés. La corroboration sur VM est prévue en fin de P3, hors critère.
- **L'inaccessibilité du service depuis une autre interface** (Step 3).
- **Le comportement de Postgres sous charge, en concurrence, ou après
  redémarrage** : la passe `test:postgres` éprouve un dialecte, pas un
  déploiement — c'est le critère ① de P5.
- **Aucune authentification.** `server.ts:2` reste vrai : « aucune
  authentification ». Le port, s'il est atteint, délivre toujours des
  identifiants TURN valables 86 400 s (`ice.ts:16`) à quiconque. **C'est P2**,
  et le critère ④ de P1 est ce qui rend cette fenêtre tolérable — raison pour
  laquelle il est en P1 et non en P5.
- **La scalabilité horizontale** : la persistance ne la procure pas (spec
  §3.1). Un WebSocket vit dans un processus et un seul.
- **Aucune constante calibrée** : ni `DUREE_SECONDES = 86_400`, ni le port par
  défaut, ni les bornes de temps des tests. Elles rejoignent la liste déjà
  longue de ce dépôt — `BPP_MIN`, `FACTEUR_FOCUS`, `PART_DORMANTE_BPS`,
  `HYSTERESIS`, `TAILLE_MAX_SORTIE`.
- **La sémantique Postgres du sous-ensemble n'était pas mesurée à l'écriture du
  plan** : elle l'est par cette recette, et par elle seule. Ce qui **était**
  mesuré, le 19 août 2026, l'était sur SQLite 3.50.4 uniquement.

- [ ] **Step 6 : commit**

```bash
git add docs/superpowers/plans/2026-08-19-plateforme-p1-resultats.md \
        docs/superpowers/plans/journaux-plateforme-p1
git commit -m "recette(p1): les quatre criteres, deux executions chacun, pieces versees"
```

---

### Task 15 : revue transverse de fin de branche, et `CLAUDE.md`

**Objet :** chercher les affirmations — commentaires, documents, README —
devenues **FAUSSES dans la branche elle-même**, puis écrire la section P1 de
`CLAUDE.md`.

**Files:**
- Modify: `CLAUDE.md`, et tout fichier dont un commentaire est devenu faux

⚠️ **Cette tâche n'est jamais facultative.** La revue transverse a trouvé
**cinq** défauts en D7, **trois** Critiques en D8, **six** en D9 et **douze**
en D10. Ils ont tous la même forme : **corrects des deux côtés pris
séparément**, faux ensemble. Une revue par tâche ne peut structurellement pas
les voir.

- [ ] **Step 1 : la cible propre de cette revue — le déménagement produit sa
      propre classe d'énoncés faux**

P1 déplace du code dont les commentaires nomment leur ancien emplacement. Les
candidats sont **énumérables** :

```bash
grep -rn "signaling/" --include='*.ts' --include='*.rs' --include='*.sh' \
     --include='*.yml' --include='*.md' . | grep -v node_modules | grep -v '^./docs/'
grep -rn "SIGNALING_PORT\|signaling à l'écoute\|jalon 1" plateforme/src scripts CLAUDE.md
```

Chaque occurrence est relue **contre le dépôt d'après P1**. Cinq sont connues
d'avance, et il faut chercher au-delà :

| Où | Ce qui devient faux |
| --- | --- |
| `plateforme/src/signaling/relais.ts:2` (ex-`server.ts:2`) | « Aucun état persistant » — **P1 en pose un**. La phrase devient fausse dans la branche même qui la déplace. ⚠️ « Aucune authentification » reste **vrai** : ne pas corriger les deux d'un même geste |
| `relais.ts`, commentaire d'`isJsonObject` (ex-`server.ts:47-56`) | il nomme `index.ts` — « aucun `uncaughtException` n'est installé dans index.ts » — et `index.ts` a **changé de fichier** (Task 3). Vérifier que la propriété tient encore sur le nouveau point d'entrée, et corriger le chemin |
| `resilience.test.ts:6-19` | son en-tête parle de « `src/index.ts` » du paquet signaling |
| `.gitignore:10` | `signaling/dist/` ne désigne plus rien (Task 2 Step 6) |
| `CLAUDE.md:20`, `:1913`, `:1929`, `:1937-1939` | `signaling/src/ice.ts` a déménagé ; le piège « le signaling doit être relancé AVEC l'environnement » reste **entier** et doit être **conservé**, pas supprimé |

- [ ] **Step 2 : balayer par le SENS, pas par la formule**

Leçon payée deux fois par ce dépôt : un balayage sur « non diagnostiqué » a
laissé survivre « pas diagnostiqué ». Chercher la **chose niée** en énumérant
les tournures — « aucun / sans / pas de / jamais / il n'y a ».

- [ ] **Step 3 : relever les tailles PAR LA COMMANDE, APRÈS les dernières éditions**

```bash
{ git ls-files; git ls-files --others --exclude-standard; } \
  | grep -vE 'node_modules|package-lock|Cargo.lock|/dist/|testdata/|^docs/|^CLAUDE.md' \
  | xargs wc -l 2>/dev/null | sort -rn | awk '$1>500'
cd plateforme && wc -l src/**/*.ts | sort -rn | head -12
```
**Attendu pour la première** : les deux seules lignes de la dette gelée —
`agent/src/encode.rs` **1536** et `agent/src/windows_source.rs` **630**
(relevé le 19 août 2026, et **à remesurer**, pas à recopier).

⚠️ **APRÈS la dernière édition de la ronde, jamais avant** : une table relevée
en début de ronde est fausse à la fin de la même ronde — erreur que D8 a
commise en croyant bien faire.

- [ ] **Step 4 : « corrigé à sa place » est une affirmation de COMPLÉTUDE**

Pour chaque nombre corrigé dans `CLAUDE.md`, **énumérer ses places avant
d'écrire**, puis **relire chaque place après l'édition** :

```bash
grep -n '<le nombre>' CLAUDE.md
```

Le naufrage du « 487 » s'est rejoué **neuf fois** dans ce dépôt, dont une fois
dans la vague même qui le corrigeait ailleurs, et deux fois sur une ligne que
la main qui écrivait était **en train d'éditer**. La leçon complète, écrite en
D10 : *éditer une ligne de tableau ne fait pas relire le nombre qu'elle porte*,
et *une substitution qui ne dit pas combien d'occurrences elle a touchées est
une affirmation de complétude non vérifiée*.

- [ ] **Step 5 : les deux pièces que D10 a vues FABRIQUÉES**

🔴 **Ne jamais recopier la sortie d'une commande antérieure pour répondre à la
question d'une AUTRE.** D10 a relevé deux pièces **fabriquées et présentées
comme des relevés** — une transcription `cargo` assemblée à la main, et une
sortie de commande **inventée, inscrite dans `CLAUDE.md`**, présentée comme
« vérifiée par la commande », **à l'intérieur d'une correction qui dénonçait
une affirmation non étayée**. Le relecteur a relancé la commande : elle rendait
zéro.

**Les deux fois le fait rapporté était vrai ; les deux fois la preuve ne
l'était pas.** C'est le mode de défaillance n°1 de ce dépôt, et il ne produit
pas une conclusion fausse — il produit une conclusion **vraie sans preuve**,
donc invérifiable par le suivant.

- [ ] **Step 6 : écrire la section « Sous-projet ⑤ Plateforme — sous-bloc P1 » de `CLAUDE.md`**

Sur le modèle des sections de sous-blocs existantes (D9, D10). Elle porte :

1. les renvois : spec, ce plan, le document de résultats, le répertoire de
   journaux — **et l'état d'encodage de ces journaux** (UTF-8, séquences ANSI
   présentes ou non), comme chaque section de sous-bloc le fait ;
2. **le fait n°1** : `signaling/` n'existe plus, son code vit dans
   `plateforme/src/signaling/`, le protocole du fil est inchangé, `PLATEFORME_HOTE`
   est **obligatoire** et casse le lancement naïf — **volontairement** ;
3. **le fait n°2** : le sous-ensemble SQL portable, ses deux gardes, et
   **pourquoi ni l'un ni l'autre ne suffit** (le relevé `SERIAL` / `AUTOINCREMENT`
   du §« Divergences », D5) ;
4. **les six divergences** spec/code, et notamment que la table `utilisateur`
   naît en P1 **parce que SQLite refuse `ALTER TABLE … ADD CONSTRAINT`** ;
5. le verdict de chaque critère **avec son nombre d'exécutions**, et le § « ce
   que P1 n'établit pas » ;
6. les pièges neufs, dont au minimum : `SERIAL` traverse les deux moteurs sans
   bruit ; `SELECT '?'` est du SQL valide ; un balayage de `node_modules` à la
   recherche d'un `.node` est rouge d'emblée à cause de `rollup` ; l'annonce du
   port du point d'entrée est **couplée** à une expression régulière de test ;
7. la mise à jour du § « Portée » (Task 13 Step 3) et le relevé de tailles du
   Step 3 ci-dessus ;
8. **les variables d'environnement neuves** — `PLATEFORME_HOTE`,
   `PLATEFORME_PORT`, `PLATEFORME_BASE`, `PLATEFORME_BASE_URL` — avec leur
   convention. `PLATEFORME_HOTE` **n'a pas de défaut** : c'est l'inverse de la
   convention `=0 désarme` des variables de banc, et il faut le dire.

⚠️ **`scripts/run-agent.sh` n'est PAS modifié par P1** — l'agent ne lit aucune
de ces quatre variables, elles sont du côté serveur. Le piège maison « toute
variable neuve doit être ajoutée à `run-agent.sh` », payé en D1, D2 et D7, **ne
s'applique pas ici**, et le dire évite qu'un successeur cherche une ligne
manquante.

- [ ] **Step 7 : vérifications finales**

```bash
./scripts/verify-all.sh 2>&1 | tail -6
cd plateforme && npx vitest run 2>&1 | grep -E "Test Files|Tests  "
```
Reporter les nombres **relevés** dans le message de commit. **Les relever, pas
les prédire.**

- [ ] **Step 8 : commit**

```bash
git add CLAUDE.md plateforme/src docs/superpowers/plans/
git status --porcelain
git commit -m "docs(p1): resultats du sous-bloc, revue transverse, et les chiffres releves par la commande"
```
⚠️ `git status --porcelain` **avant** le commit : un autre agent travaille dans
cet arbre, et `git add docs/superpowers/plans/` est déjà plus large qu'il ne
faudrait. Si des fichiers étrangers apparaissent, **les nommer un par un**.

---

## Ordre et dépendances

```
1  (le paquet, config)
└── 2  (déménagement verbatim — exige 1 pour le tsconfig et node_modules)
    └── 3  (HTTP + upgrade + écoute bornée)
        └── 4  (extraction appariement.ts + renommage relais.ts)

1  ─── 5  (pilote.ts, PUR)
        ├── 6  (pilote-sqlite)   ┐
        └── 7  (pilote-postgres) ┘ parallélisables entre elles
             └── 8  (migrations + socle + lint)
                  └── 9  (double passe + les trois rouges)
                       └── 10 (depot/session)

3, 10 ──► 11 (base au démarrage)
4, 11 ──► 12 (la trace de session — critère ②)
2     ──► 13 (verify-all + § Portée)   [peut courir dès que signaling/ a disparu]

12, 13 ──► 14 (recette : les quatre critères, deux exécutions)
       └──► 15 (revue transverse + CLAUDE.md — TOUJOURS la dernière)
```

**Ce qui se parallélise réellement :**

- la **chaîne 5 → 6/7 → 8 → 9 → 10** (persistance) et la **chaîne 2 → 3 → 4**
  (déménagement) sont **indépendantes** après la Task 1. Deux agents peuvent
  les mener de front — elles ne partagent aucun fichier.
- **6 et 7** sont indépendantes l'une de l'autre (deux pilotes, deux fichiers),
  à ceci près que **7 modifie `package.json` et `ouvrir.ts`**, que 6 crée : les
  faire de front demande de séquencer ces deux fichiers-là, ou d'accepter un
  conflit trivial. **Recommandation : 6 puis 7.**
- **13** ne dépend que de la disparition de `signaling/` (Task 2). Elle peut
  courir tôt — mais ses deux étapes `plateforme : npm run test:*` n'existent
  qu'après la Task 9. **Recommandation : après 9.**

**Ce qui ne se parallélise PAS :**

- ⚠️ **Les tâches 2, 3, 4 et 12 touchent toutes `plateforme/src/signaling/`**,
  et trois d'entre elles touchent `resilience.test.ts` ou `relais.ts`. Elles
  sont **strictement séquentielles** : la preuve de la Task 2 (des empreintes
  et un `git diff` de deux lignes) est **détruite** par toute édition
  concurrente du même fichier.
- ⚠️ **L'instance Postgres est un état partagé.** Les tâches 9, 12, 13 et 14 la
  sollicitent ; deux exécutions concurrentes de `test:postgres` sur la même
  base se marcheraient dessus, et l'échec se lirait comme un défaut de
  portabilité. **Sérialisées entre elles**, quelle que soit leur indépendance
  logique — c'est la même règle que « la VM est un état partagé » du chantier D,
  sous une autre forme.
- ⚠️ **La Task 15 est toujours la dernière**, et sa Step 3 se relève **après**
  sa propre Step 6.

**Aucune tâche de ce plan n'emploie la VM Windows.** Elle reste disponible pour
le travail concurrent.
