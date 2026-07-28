# Spike multi-fenêtres — plan d'implémentation

> **Pour les agents exécutants :** SOUS-COMPÉTENCE REQUISE — utiliser
> `superpowers:subagent-driven-development` (recommandé) ou
> `superpowers:executing-plans` pour dérouler ce plan tâche par tâche. Les étapes
> utilisent la syntaxe à cases à cocher (`- [ ]`) pour le suivi.

**But :** déterminer si une PWA installée peut ouvrir une fenêtre navigateur en
réponse à un message serveur — sans geste utilisateur — et par quel mécanisme, afin
de lever le risque n°1 du chantier D avant d'engager les chantiers A et B.

**Architecture :** un serveur Node autonome (HTTP statique + WebSocket) sert une
page de test exposant quatre variantes d'ouverture de fenêtre. Chaque variante est
déclenchée par un message venu du serveur, jamais par le clic sur son propre
bouton. La fenêtre ouverte signale sa vie par un `fetch` vers le serveur, qui
rediffuse l'information sur le WebSocket ; la page classe alors l'issue en
`blocked` / `opened-but-lost` / `success`. Aucun contact avec l'agent Rust, la VM
Windows ou WebRTC.

**Pile technique :** Node 24, TypeScript + `tsx` (serveur), JavaScript ESM pur
(navigateur, aucun build), `ws`, Vitest, `sharp` (génération d'icônes, déjà
présent à la racine du dépôt).

**Spécification de référence :**
`docs/superpowers/specs/2026-07-28-spike-multifenetres-design.md`

## Contraintes globales

- **Langue** : tout commentaire, message de commit et texte d'interface est en
  **français**, avec les accents corrects. C'est la convention du dépôt.
- **Conventions Node du dépôt** : `"type": "module"`, TypeScript `strict`,
  lancement par `tsx`, tests par `vitest run`. Calquer `signaling/package.json`.
- **Le serveur du spike écoute sur le port 3445** — c'est le port vers lequel
  Pomerium route déjà `https://app.allanic.me` (`allow_websockets: true` déjà
  actif). Aucune modification de `/etc/pomerium/config.yaml`.
- **L'ancien serveur Guacamole occupe ce port** et doit être arrêté avant, puis
  redémarré après (tâche 5). Il est dans le conteneur `guacamole-web-1`
  (`network_mode: host`, `restart: always`).
- **Aucun fichier de l'ancien système n'est modifié** : ni `index.js`, ni `src/`,
  ni `web/`, ni `docker-compose.yml`. La spec produit §11 les déclare intacts
  jusqu'à leur remplacement.
- **Ne jamais `git add` à la racine sans chemin explicite.** `docker-compose.yml`
  et `index.js` ne sont pas suivis et contiennent des mots de passe en clair.
  Chaque commit liste ses fichiers un par un.
- **Seuil d'activation transitoire : 5 000 ms.** Un verdict n'est retenu que si
  l'ouverture a lieu plus de 5 s après le dernier geste utilisateur (sauf
  variante 3, dont le geste est le déclencheur assumé). Chromium accorde une
  activation transitoire d'environ 5 s ; sans cette garde, un « succès » ne
  prouverait rien.
- **Branche de travail** : `spike-multifenetres` (déjà créée, contient la spec).

---

## Structure des fichiers

```
spike-multifenetres/
  package.json              — dépendances et scripts du spike
  tsconfig.json             — TypeScript strict, calqué sur signaling/
  src/
    server.ts               — createSpikeServer(port) : statique + WS + /fire + /alive
    index.ts                — point d'entrée, lit SPIKE_PORT (défaut 3445)
  test/
    server.test.ts          — tests du serveur
    classify.test.js        — tests du classement des issues
    reset-origin.test.js    — tests de l'hygiène de l'origin
  public/
    index.html              — page principale, les 4 variantes
    app.js                  — pilotage des variantes et journal
    lib/classify.js         — fonction pure de classement (testée)
    lib/reset-origin.js     — fonction pure d'hygiène (testée)
    reset.html              — étape 0 du protocole
    opened.html             — page ouverte : signale sa vie
    manifest.json           — PWA installable, scope "/"
    sw.js                   — service worker, variante 4
    icon-192.png            — icône PWA
    icon-512.png            — icône PWA
```

**Responsabilités.** `server.ts` ne connaît rien des variantes : il sert des
fichiers, accepte des WebSockets, et rediffuse deux événements (`fire`, `alive`).
Toute la logique de test vit dans le navigateur. Les deux seuls morceaux de
logique navigateur qui méritent des tests — le classement des issues et l'hygiène
de l'origin — sont isolés dans `public/lib/` en modules ESM purs, importables tels
quels par Vitest **et** par le navigateur, sans build.

---

## Tâche 1 : serveur du spike

**Fichiers :**
- Créer : `spike-multifenetres/package.json`
- Créer : `spike-multifenetres/tsconfig.json`
- Créer : `spike-multifenetres/src/server.ts`
- Créer : `spike-multifenetres/src/index.ts`
- Test : `spike-multifenetres/test/server.test.ts`

**Interfaces :**
- Consomme : rien.
- Produit :
  - `createSpikeServer(port: number): Promise<SpikeServer>` où
    `interface SpikeServer { port: number; close(): Promise<void> }`
  - Routes HTTP : `GET /` et tout fichier de `public/` ; `POST /fire` (corps JSON
    `{ "variant": number }`) ; `GET /alive?variant=N`.
  - Protocole WebSocket sur `/ws` : le serveur diffuse
    `{ "type": "fire", "variant": number, "at": number }` et
    `{ "type": "alive", "variant": number, "at": number }`.

- [ ] **Étape 1 : créer `spike-multifenetres/package.json`**

```json
{
  "name": "@guacamole/spike-multifenetres",
  "version": "0.1.0",
  "private": true,
  "type": "module",
  "scripts": {
    "start": "tsx src/index.ts",
    "test": "vitest run"
  },
  "dependencies": {
    "ws": "^8.18.0"
  },
  "devDependencies": {
    "@types/node": "^22.0.0",
    "@types/ws": "^8.5.12",
    "tsx": "^4.19.0",
    "typescript": "^5.5.0",
    "vitest": "^2.0.0"
  }
}
```

- [ ] **Étape 2 : créer `spike-multifenetres/tsconfig.json`**

```json
{
  "compilerOptions": {
    "target": "ES2022",
    "module": "ESNext",
    "moduleResolution": "bundler",
    "strict": true,
    "esModuleInterop": true,
    "skipLibCheck": true,
    "allowJs": true,
    "noEmit": true
  },
  "include": ["src", "test"]
}
```

- [ ] **Étape 3 : installer les dépendances**

Lancer : `cd spike-multifenetres && npm install`
Attendu : installation sans erreur, `node_modules/` créé (ignoré par le
`.gitignore` racine).

- [ ] **Étape 4 : écrire le test qui échoue**

Créer `spike-multifenetres/test/server.test.ts` :

```typescript
import { afterEach, describe, expect, it } from 'vitest';
import { WebSocket } from 'ws';
import { createSpikeServer, type SpikeServer } from '../src/server.js';

let serveur: SpikeServer | undefined;

afterEach(async () => {
    await serveur?.close();
    serveur = undefined;
});

// Attend le premier message JSON reçu sur le socket, ou échoue après 2 s.
function premierMessage(socket: WebSocket): Promise<unknown> {
    return new Promise((resolve, reject) => {
        const minuteur = setTimeout(() => reject(new Error('aucun message reçu')), 2000);
        socket.once('message', (brut) => {
            clearTimeout(minuteur);
            resolve(JSON.parse(brut.toString()));
        });
    });
}

function ouvert(socket: WebSocket): Promise<void> {
    return new Promise((resolve) => socket.once('open', () => resolve()));
}

describe('serveur du spike', () => {
    it('sert la page principale', async () => {
        serveur = await createSpikeServer(0);
        const reponse = await fetch(`http://127.0.0.1:${serveur.port}/`);
        expect(reponse.status).toBe(200);
        expect(reponse.headers.get('content-type')).toContain('text/html');
    });

    it('diffuse un ordre de déclenchement à tous les clients WebSocket', async () => {
        serveur = await createSpikeServer(0);
        const socket = new WebSocket(`ws://127.0.0.1:${serveur.port}/ws`);
        await ouvert(socket);

        const attendu = premierMessage(socket);
        await fetch(`http://127.0.0.1:${serveur.port}/fire`, {
            method: 'POST',
            headers: { 'content-type': 'application/json' },
            body: JSON.stringify({ variant: 2 }),
        });

        expect(await attendu).toMatchObject({ type: 'fire', variant: 2 });
        socket.close();
    });

    it('rediffuse le signal de vie de la fenêtre ouverte', async () => {
        serveur = await createSpikeServer(0);
        const socket = new WebSocket(`ws://127.0.0.1:${serveur.port}/ws`);
        await ouvert(socket);

        const attendu = premierMessage(socket);
        await fetch(`http://127.0.0.1:${serveur.port}/alive?variant=3`);

        expect(await attendu).toMatchObject({ type: 'alive', variant: 3 });
        socket.close();
    });

    it('refuse une variante hors de 1..4 sans planter', async () => {
        serveur = await createSpikeServer(0);
        const reponse = await fetch(`http://127.0.0.1:${serveur.port}/fire`, {
            method: 'POST',
            headers: { 'content-type': 'application/json' },
            body: JSON.stringify({ variant: 99 }),
        });
        expect(reponse.status).toBe(400);
    });

    it('renvoie 404 sur un chemin inconnu', async () => {
        serveur = await createSpikeServer(0);
        const reponse = await fetch(`http://127.0.0.1:${serveur.port}/inexistant`);
        expect(reponse.status).toBe(404);
    });
});
```

- [ ] **Étape 5 : lancer le test pour vérifier qu'il échoue**

Lancer : `cd spike-multifenetres && npm test`
Attendu : ÉCHEC — `Failed to resolve import "../src/server.js"`.

- [ ] **Étape 6 : créer `spike-multifenetres/public/index.html` minimal**

Le test « sert la page principale » a besoin d'un fichier à servir. Contenu
provisoire, remplacé en tâche 3 :

```html
<!doctype html>
<html lang="fr">
<head><meta charset="utf-8"><title>Spike multi-fenêtres</title></head>
<body><p>Page de test — remplacée en tâche 3.</p></body>
</html>
```

- [ ] **Étape 7 : implémenter `spike-multifenetres/src/server.ts`**

```typescript
// Serveur du spike multi-fenêtres. Sert `public/` et relaie deux événements :
// `fire` (ordre d'ouverture, émis par POST /fire) et `alive` (signal de vie de la
// fenêtre ouverte, émis par GET /alive). Aucun état persistant.
//
// Le déclenchement passe délibérément par le réseau et non par un clic dans la
// page : c'est toute la condition testée — un message serveur n'est pas une
// activation utilisateur transitoire.

import { createServer, type IncomingMessage, type ServerResponse } from 'node:http';
import { readFile } from 'node:fs/promises';
import { extname, join, normalize } from 'node:path';
import { fileURLToPath } from 'node:url';
import { WebSocketServer, WebSocket } from 'ws';

const RACINE_PUBLIQUE = fileURLToPath(new URL('../public/', import.meta.url));

const TYPES_MIME: Record<string, string> = {
    '.html': 'text/html; charset=utf-8',
    '.js': 'text/javascript; charset=utf-8',
    '.json': 'application/json; charset=utf-8',
    '.png': 'image/png',
    '.ico': 'image/x-icon',
};

export interface SpikeServer {
    port: number;
    close(): Promise<void>;
}

function estVarianteValide(valeur: unknown): valeur is number {
    return typeof valeur === 'number' && Number.isInteger(valeur) && valeur >= 1 && valeur <= 4;
}

// Lit le corps d'une requête, borné à 4 Kio : le serveur est exposé via Pomerium,
// et une requête sans fin immobiliserait le process.
function lireCorps(requete: IncomingMessage): Promise<string> {
    return new Promise((resolve, reject) => {
        let corps = '';
        requete.on('data', (morceau) => {
            corps += morceau;
            if (corps.length > 4096) {
                reject(new Error('corps trop volumineux'));
                requete.destroy();
            }
        });
        requete.on('end', () => resolve(corps));
        requete.on('error', reject);
    });
}

export async function createSpikeServer(port: number): Promise<SpikeServer> {
    const clients = new Set<WebSocket>();

    function diffuser(charge: Record<string, unknown>): void {
        const texte = JSON.stringify({ ...charge, at: Date.now() });
        for (const client of clients) {
            if (client.readyState === WebSocket.OPEN) client.send(texte);
        }
    }

    async function servirFichier(chemin: string, reponse: ServerResponse): Promise<void> {
        // `normalize` puis vérification du préfixe : sans cela, `/../.env` sortirait
        // de public/. Le serveur est exposé sur internet via Pomerium.
        const absolu = normalize(join(RACINE_PUBLIQUE, chemin));
        if (!absolu.startsWith(RACINE_PUBLIQUE)) {
            reponse.writeHead(403).end('interdit');
            return;
        }
        try {
            const contenu = await readFile(absolu);
            const type = TYPES_MIME[extname(absolu)] ?? 'application/octet-stream';
            // Aucun cache : un spike relancé plusieurs fois avec des pages modifiées
            // donnerait sinon des verdicts obtenus sur du code périmé.
            reponse.writeHead(200, { 'content-type': type, 'cache-control': 'no-store' });
            reponse.end(contenu);
        } catch {
            reponse.writeHead(404).end('introuvable');
        }
    }

    const http = createServer(async (requete, reponse) => {
        const url = new URL(requete.url ?? '/', `http://${requete.headers.host ?? 'localhost'}`);

        if (requete.method === 'POST' && url.pathname === '/fire') {
            let variante: unknown;
            try {
                variante = (JSON.parse(await lireCorps(requete)) as { variant?: unknown }).variant;
            } catch {
                reponse.writeHead(400).end('JSON invalide');
                return;
            }
            if (!estVarianteValide(variante)) {
                reponse.writeHead(400).end('variante invalide');
                return;
            }
            diffuser({ type: 'fire', variant: variante });
            reponse.writeHead(204).end();
            return;
        }

        if (url.pathname === '/alive') {
            const variante = Number(url.searchParams.get('variant'));
            if (!estVarianteValide(variante)) {
                reponse.writeHead(400).end('variante invalide');
                return;
            }
            diffuser({ type: 'alive', variant: variante });
            reponse.writeHead(204).end();
            return;
        }

        await servirFichier(url.pathname === '/' ? 'index.html' : url.pathname, reponse);
    });

    const wss = new WebSocketServer({ server: http, path: '/ws' });
    wss.on('connection', (socket) => {
        clients.add(socket);
        socket.on('close', () => clients.delete(socket));
        // Un socket en erreur qui n'est pas retiré du Set ferait grossir la
        // diffusion indéfiniment.
        socket.on('error', () => clients.delete(socket));
    });

    await new Promise<void>((resolve) => http.listen(port, resolve));
    const adresse = http.address();
    const portEffectif = typeof adresse === 'object' && adresse ? adresse.port : port;

    return {
        port: portEffectif,
        close(): Promise<void> {
            return new Promise((resolve) => {
                for (const client of clients) client.terminate();
                wss.close(() => http.close(() => resolve()));
            });
        },
    };
}
```

- [ ] **Étape 8 : implémenter `spike-multifenetres/src/index.ts`**

```typescript
import { createSpikeServer } from './server.js';

const port = Number(process.env.SPIKE_PORT ?? 3445);
const serveur = await createSpikeServer(port);
console.log(`spike à l'écoute sur le port ${serveur.port}`);
console.log(`déclenchement externe : curl -X POST http://127.0.0.1:${serveur.port}/fire \\`);
console.log(`  -H 'content-type: application/json' -d '{"variant":2}'`);
```

- [ ] **Étape 9 : lancer les tests pour vérifier qu'ils passent**

Lancer : `cd spike-multifenetres && npm test`
Attendu : 5 tests PASSÉS.

- [ ] **Étape 10 : committer**

```bash
git add spike-multifenetres/package.json spike-multifenetres/package-lock.json \
        spike-multifenetres/tsconfig.json spike-multifenetres/src/server.ts \
        spike-multifenetres/src/index.ts spike-multifenetres/test/server.test.ts \
        spike-multifenetres/public/index.html
git commit -m "feat(spike): serveur statique et relais de déclenchement"
```

---

## Tâche 2 : hygiène de l'origin (étape 0 du protocole)

**Contexte.** `web/index.js:418` enregistre un service worker sur cet origin. Le
navigateur de l'utilisateur en a donc probablement un actif, capable
d'intercepter les requêtes du spike et de rendre tous les verdicts
ininterprétables. Cette tâche le neutralise **et rapporte ce qu'elle a trouvé** —
découvrir un service worker de l'ancienne application est en soi un résultat.

**Fichiers :**
- Créer : `spike-multifenetres/public/lib/reset-origin.js`
- Créer : `spike-multifenetres/public/reset.html`
- Test : `spike-multifenetres/test/reset-origin.test.js`

**Interfaces :**
- Consomme : rien.
- Produit : `nettoyerOrigin({ serviceWorker, caches }): Promise<RapportNettoyage>`
  où `RapportNettoyage = { serviceWorkers: string[], caches: string[],
  supporte: boolean }`. Les champs listent ce qui a été **trouvé puis supprimé**.
  `supporte` vaut `false` si l'API n'est pas disponible (contexte non sécurisé).

- [ ] **Étape 1 : écrire le test qui échoue**

Créer `spike-multifenetres/test/reset-origin.test.js` :

```javascript
import { describe, expect, it } from 'vitest';
import { nettoyerOrigin } from '../public/lib/reset-origin.js';

// Faux `navigator.serviceWorker` : deux enregistrements, dont un de l'ancienne app.
function fauxServiceWorker(portees) {
    const desenregistres = [];
    return {
        desenregistres,
        api: {
            getRegistrations: async () =>
                portees.map((portee) => ({
                    scope: portee,
                    unregister: async () => {
                        desenregistres.push(portee);
                        return true;
                    },
                })),
        },
    };
}

function fauxCaches(noms) {
    const supprimes = [];
    return {
        supprimes,
        api: {
            keys: async () => noms,
            delete: async (nom) => {
                supprimes.push(nom);
                return true;
            },
        },
    };
}

describe('nettoyerOrigin', () => {
    it('désenregistre tous les service workers et rapporte leurs portées', async () => {
        const sw = fauxServiceWorker(['https://app.allanic.me/', 'https://app.allanic.me/excel/']);
        const cache = fauxCaches([]);

        const rapport = await nettoyerOrigin({ serviceWorker: sw.api, caches: cache.api });

        expect(rapport.serviceWorkers).toEqual([
            'https://app.allanic.me/',
            'https://app.allanic.me/excel/',
        ]);
        expect(sw.desenregistres).toHaveLength(2);
    });

    it('vide les caches et rapporte leurs noms', async () => {
        const sw = fauxServiceWorker([]);
        const cache = fauxCaches(['guacamole-v1', 'images']);

        const rapport = await nettoyerOrigin({ serviceWorker: sw.api, caches: cache.api });

        expect(rapport.caches).toEqual(['guacamole-v1', 'images']);
        expect(cache.supprimes).toEqual(['guacamole-v1', 'images']);
    });

    it('rapporte un origin déjà propre sans échouer', async () => {
        const rapport = await nettoyerOrigin({
            serviceWorker: fauxServiceWorker([]).api,
            caches: fauxCaches([]).api,
        });

        expect(rapport).toEqual({ serviceWorkers: [], caches: [], supporte: true });
    });

    it("signale l'absence d'API au lieu de lever", async () => {
        const rapport = await nettoyerOrigin({ serviceWorker: undefined, caches: undefined });

        expect(rapport.supporte).toBe(false);
        expect(rapport.serviceWorkers).toEqual([]);
    });
});
```

- [ ] **Étape 2 : lancer le test pour vérifier qu'il échoue**

Lancer : `cd spike-multifenetres && npm test -- reset-origin`
Attendu : ÉCHEC — module `../public/lib/reset-origin.js` introuvable.

- [ ] **Étape 3 : implémenter `spike-multifenetres/public/lib/reset-origin.js`**

```javascript
// Hygiène de l'origin avant le spike. Module ESM pur : importé tel quel par
// reset.html dans le navigateur, et par Vitest sous Node — d'où l'injection des
// API plutôt qu'un accès direct à `navigator`.
//
// Raison d'être : l'ancienne application enregistre un service worker sur ce même
// origin (web/index.js:418). Un service worker actif intercepterait les requêtes
// du spike et rendrait tous les verdicts ininterprétables.

/**
 * @param {{ serviceWorker?: { getRegistrations(): Promise<Array<{scope: string, unregister(): Promise<boolean>}>> },
 *           caches?: { keys(): Promise<string[]>, delete(nom: string): Promise<boolean> } }} api
 * @returns {Promise<{ serviceWorkers: string[], caches: string[], supporte: boolean }>}
 */
export async function nettoyerOrigin(api) {
    const rapport = { serviceWorkers: [], caches: [], supporte: false };

    if (!api?.serviceWorker && !api?.caches) return rapport;
    rapport.supporte = true;

    if (api.serviceWorker) {
        const enregistrements = await api.serviceWorker.getRegistrations();
        for (const enregistrement of enregistrements) {
            // La portée est relevée AVANT la suppression : c'est elle qui dira si
            // l'ancienne application était bien là.
            rapport.serviceWorkers.push(enregistrement.scope);
            await enregistrement.unregister();
        }
    }

    if (api.caches) {
        const noms = await api.caches.keys();
        for (const nom of noms) {
            rapport.caches.push(nom);
            await api.caches.delete(nom);
        }
    }

    return rapport;
}
```

- [ ] **Étape 4 : lancer les tests pour vérifier qu'ils passent**

Lancer : `cd spike-multifenetres && npm test -- reset-origin`
Attendu : 4 tests PASSÉS.

- [ ] **Étape 5 : créer `spike-multifenetres/public/reset.html`**

```html
<!doctype html>
<html lang="fr">
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <title>Spike — hygiène de l'origin</title>
    <style>
        body { font: 16px/1.5 system-ui, sans-serif; max-width: 46rem; margin: 3rem auto; padding: 0 1rem; }
        pre { background: #f4f4f5; padding: 1rem; border-radius: .5rem; overflow-x: auto; }
        .vide { color: #71717a; font-style: italic; }
    </style>
</head>
<body>
    <h1>Étape 0 — hygiène de l'origin</h1>
    <p>
        Désenregistre tout service worker et vide tous les caches de
        <code id="origin"></code>, puis affiche ce qui a été trouvé.
    </p>
    <button id="nettoyer">Nettoyer l'origin</button>
    <h2>Trouvé puis supprimé</h2>
    <pre id="rapport" class="vide">Pas encore exécuté.</pre>
    <p>
        Ensuite : désinstaller manuellement l'ancienne PWA si elle est présente
        (menu du navigateur → Applications), puis revenir sur <a href="/">la page
        principale</a>.
    </p>
    <script type="module">
        import { nettoyerOrigin } from './lib/reset-origin.js';

        document.getElementById('origin').textContent = location.origin;

        document.getElementById('nettoyer').addEventListener('click', async () => {
            const rapport = await nettoyerOrigin({
                serviceWorker: navigator.serviceWorker,
                caches: window.caches,
            });
            const sortie = document.getElementById('rapport');
            sortie.classList.remove('vide');
            sortie.textContent = JSON.stringify(rapport, null, 2);
            console.log('[spike] hygiène de l\'origin', rapport);
        });
    </script>
</body>
</html>
```

- [ ] **Étape 6 : committer**

```bash
git add spike-multifenetres/public/lib/reset-origin.js \
        spike-multifenetres/public/reset.html \
        spike-multifenetres/test/reset-origin.test.js
git commit -m "feat(spike): hygiène de l'origin avec rapport de ce qui est trouvé"
```

---

## Tâche 3 : classement des issues et variantes 1 à 3

**Contexte.** C'est le cœur du spike. Trois issues doivent être distinguées, pas
deux : derrière Pomerium, une session expirée redirige vers l'IdP, et une fenêtre
partie sur l'IdP ressemble beaucoup à une fenêtre qui n'a pas pu s'ouvrir.

**Fichiers :**
- Créer : `spike-multifenetres/public/lib/classify.js`
- Créer : `spike-multifenetres/public/opened.html`
- Créer : `spike-multifenetres/public/app.js`
- Modifier : `spike-multifenetres/public/index.html` (remplace le contenu
  provisoire de la tâche 1)
- Test : `spike-multifenetres/test/classify.test.js`

**Interfaces :**
- Consomme : le protocole WebSocket de la tâche 1 (`fire`, `alive`).
- Produit : `classer({ poigneeNulle, vivante, msDepuisGeste }): Verdict` où
  `Verdict = 'bloque' | 'ouverte-mais-perdue' | 'succes' | 'non-concluant'`.
  `poigneeNulle` vaut `true`, `false`, ou `'sans-objet'` (variante 4, où
  `clients.openWindow()` ne rend pas de poignée exploitable).

- [ ] **Étape 1 : écrire le test qui échoue**

Créer `spike-multifenetres/test/classify.test.js` :

```javascript
import { describe, expect, it } from 'vitest';
import { classer, SEUIL_ACTIVATION_MS } from '../public/lib/classify.js';

const HORS_ACTIVATION = SEUIL_ACTIVATION_MS + 1000;

describe('classer', () => {
    it('classe une poignée nulle en bloqué', () => {
        expect(classer({ poigneeNulle: true, vivante: false, msDepuisGeste: HORS_ACTIVATION }))
            .toBe('bloque');
    });

    it('classe une poignée rendue avec signal de vie en succès', () => {
        expect(classer({ poigneeNulle: false, vivante: true, msDepuisGeste: HORS_ACTIVATION }))
            .toBe('succes');
    });

    it('classe une poignée rendue sans signal de vie en ouverte-mais-perdue', () => {
        expect(classer({ poigneeNulle: false, vivante: false, msDepuisGeste: HORS_ACTIVATION }))
            .toBe('ouverte-mais-perdue');
    });

    // La garde qui empêche de conclure à tort : un « succès » obtenu dans les 5 s
    // suivant un clic ne prouve rien, l'activation transitoire pouvait encore courir.
    it('refuse de conclure quand le geste utilisateur est trop récent', () => {
        expect(classer({ poigneeNulle: false, vivante: true, msDepuisGeste: 1200 }))
            .toBe('non-concluant');
    });

    it('conclut malgré un geste récent quand le geste EST le mécanisme testé', () => {
        expect(classer({
            poigneeNulle: false,
            vivante: true,
            msDepuisGeste: 12,
            gesteAttendu: true,
        })).toBe('succes');
    });

    it('classe la variante 4 sur le seul signal de vie', () => {
        expect(classer({ poigneeNulle: 'sans-objet', vivante: true, msDepuisGeste: HORS_ACTIVATION }))
            .toBe('succes');
        expect(classer({ poigneeNulle: 'sans-objet', vivante: false, msDepuisGeste: HORS_ACTIVATION }))
            .toBe('ouverte-mais-perdue');
    });

    it('expose le seuil d\'activation à 5000 ms', () => {
        expect(SEUIL_ACTIVATION_MS).toBe(5000);
    });
});
```

- [ ] **Étape 2 : lancer le test pour vérifier qu'il échoue**

Lancer : `cd spike-multifenetres && npm test -- classify`
Attendu : ÉCHEC — module `../public/lib/classify.js` introuvable.

- [ ] **Étape 3 : implémenter `spike-multifenetres/public/lib/classify.js`**

```javascript
// Classement des issues d'une tentative d'ouverture de fenêtre.
//
// Trois issues, pas deux. Derrière Pomerium, une session expirée renvoie une
// redirection d'authentification : la fenêtre s'ouvre bel et bien, mais part sur
// l'IdP et ne signale jamais sa vie. Confondre ce cas avec un blocage produirait
// un faux négatif très convaincant — exactement le type de conclusion hâtive que
// les rondes de diagnostic du jalon 1 ont dû défaire quatre fois.

// Durée approximative de l'activation utilisateur transitoire dans Chromium.
export const SEUIL_ACTIVATION_MS = 5000;

/**
 * @param {{ poigneeNulle: boolean | 'sans-objet',
 *           vivante: boolean,
 *           msDepuisGeste: number,
 *           gesteAttendu?: boolean }} observation
 * @returns {'bloque' | 'ouverte-mais-perdue' | 'succes' | 'non-concluant'}
 */
export function classer(observation) {
    const { poigneeNulle, vivante, msDepuisGeste, gesteAttendu = false } = observation;

    // La variante 3 s'appuie délibérément sur un clic : pour elle seule, un geste
    // récent est le mécanisme testé et non un biais.
    if (!gesteAttendu && msDepuisGeste <= SEUIL_ACTIVATION_MS) return 'non-concluant';

    if (poigneeNulle === true) return 'bloque';
    return vivante ? 'succes' : 'ouverte-mais-perdue';
}
```

- [ ] **Étape 4 : lancer les tests pour vérifier qu'ils passent**

Lancer : `cd spike-multifenetres && npm test -- classify`
Attendu : 7 tests PASSÉS.

- [ ] **Étape 5 : créer `spike-multifenetres/public/opened.html`**

```html
<!doctype html>
<html lang="fr">
<head>
    <meta charset="utf-8">
    <title>Spike — fenêtre ouverte</title>
    <style>body { font: 16px/1.5 system-ui, sans-serif; padding: 2rem; }</style>
</head>
<body>
    <h1>Fenêtre ouverte ✓</h1>
    <p>Variante <strong id="variante"></strong>. Signal de vie envoyé au serveur.</p>
    <p>Cette fenêtre peut être fermée.</p>
    <script type="module">
        // Le signal de vie passe par un fetch et non par window.opener.postMessage :
        // clients.openWindow() (variante 4) n'établit aucun opener. Un seul canal
        // pour les quatre variantes évite d'avoir à interpréter deux mécanismes
        // différents dans le même tableau de résultats.
        const variante = new URLSearchParams(location.search).get('variant') ?? '0';
        document.getElementById('variante').textContent = variante;
        fetch(`/alive?variant=${encodeURIComponent(variante)}`).catch((erreur) => {
            console.error('[spike] signal de vie non transmis', erreur);
        });
    </script>
</body>
</html>
```

- [ ] **Étape 6 : créer `spike-multifenetres/public/app.js`**

```javascript
// Pilotage des quatre variantes du spike.
//
// Invariant central : une variante n'est JAMAIS déclenchée par le clic sur son
// propre bouton. Le bouton arme la variante ; l'ordre d'ouverture arrive ensuite
// par le WebSocket, soit après le compte à rebours, soit via POST /fire depuis un
// autre poste. C'est toute la condition testée.

import { classer, SEUIL_ACTIVATION_MS } from './lib/classify.js';

const DELAI_ARMEMENT_MS = 15000;   // > SEUIL_ACTIVATION_MS, avec marge confortable
const DELAI_SIGNAL_VIE_MS = 3000;  // au-delà, la fenêtre est réputée perdue

const journal = document.getElementById('journal');
const etat = document.getElementById('etat');

let dernierGeste = 0;
let armementVariante3 = null;
const vies = new Map();

// Tout geste utilisateur est horodaté : c'est ce qui permettra d'affirmer, chiffre
// à l'appui, que l'activation transitoire avait bien expiré au moment du open().
for (const evenement of ['pointerdown', 'keydown']) {
    window.addEventListener(evenement, () => {
        dernierGeste = Date.now();
        if (evenement === 'pointerdown' && armementVariante3) {
            const executer = armementVariante3;
            armementVariante3 = null;
            executer();
        }
    }, true);
}

function tracer(texte, niveau = 'info') {
    const ligne = document.createElement('div');
    ligne.className = `ligne ${niveau}`;
    ligne.textContent = `${new Date().toLocaleTimeString('fr-FR')} — ${texte}`;
    journal.prepend(ligne);
    console.log(`[spike] ${texte}`);
}

// Attend le signal de vie de la variante, ou renonce après DELAI_SIGNAL_VIE_MS.
function attendreVie(variante) {
    return new Promise((resolve) => {
        vies.set(variante, resolve);
        setTimeout(() => {
            if (vies.delete(variante)) resolve(false);
        }, DELAI_SIGNAL_VIE_MS);
    });
}

async function conclure(variante, poigneeNulle, gesteAttendu = false) {
    const msDepuisGeste = Date.now() - dernierGeste;
    const vivante = await attendreVie(variante);
    const verdict = classer({ poigneeNulle, vivante, msDepuisGeste, gesteAttendu });

    const niveau = verdict === 'succes' ? 'succes'
        : verdict === 'non-concluant' ? 'alerte' : 'echec';
    tracer(
        `variante ${variante} → ${verdict.toUpperCase()} ` +
        `(poignée ${poigneeNulle === 'sans-objet' ? 'sans objet' : poigneeNulle ? 'nulle' : 'rendue'}, ` +
        `vie ${vivante ? 'reçue' : 'absente'}, ${msDepuisGeste} ms depuis le dernier geste)`,
        niveau,
    );
    document.querySelector(`#verdict-${variante}`).textContent = verdict;
}

function ouvrir(variante) {
    const poignee = window.open(`/opened.html?variant=${variante}`, `spike-${variante}`);
    return poignee === null;
}

async function executerVariante(variante) {
    switch (variante) {
        case 1:
        case 2:
            // 1 = onglet normal (témoin), 2 = PWA installée. Même code : c'est le
            // contexte d'exécution qui diffère, pas l'appel.
            await conclure(variante, ouvrir(variante));
            break;

        case 3:
            tracer('variante 3 armée — cliquez n\'importe où pour déclencher', 'alerte');
            armementVariante3 = () => conclure(3, ouvrir(3), true);
            break;

        case 4: {
            const enregistrement = await navigator.serviceWorker.getRegistration();
            if (!enregistrement) {
                tracer('variante 4 impossible : aucun service worker enregistré', 'echec');
                return;
            }
            if (Notification.permission !== 'granted') {
                tracer('variante 4 impossible : permission notifications non accordée', 'echec');
                return;
            }
            await enregistrement.showNotification('Le jeu est prêt', {
                body: 'Cliquez pour ouvrir sa fenêtre (variante 4).',
                data: { url: '/opened.html?variant=4' },
                tag: 'spike-4',
            });
            tracer('notification affichée — cliquez dessus, fenêtre en arrière-plan', 'alerte');
            await conclure(4, 'sans-objet');
            break;
        }
    }
}

const socket = new WebSocket(`${location.protocol === 'https:' ? 'wss' : 'ws'}://${location.host}/ws`);

socket.addEventListener('open', () => {
    etat.textContent = 'connecté';
    etat.className = 'connecte';
});

socket.addEventListener('close', () => {
    etat.textContent = 'déconnecté';
    etat.className = 'deconnecte';
});

socket.addEventListener('message', (evenement) => {
    const message = JSON.parse(evenement.data);
    if (message.type === 'fire') {
        tracer(`ordre reçu du serveur pour la variante ${message.variant}`);
        executerVariante(message.variant);
    } else if (message.type === 'alive') {
        const resolveur = vies.get(message.variant);
        if (resolveur) {
            vies.delete(message.variant);
            resolveur(true);
        }
    }
});

// Armement : le bouton ne déclenche rien lui-même, il demande au serveur de
// déclencher plus tard. Le compte à rebours dépasse SEUIL_ACTIVATION_MS.
for (const bouton of document.querySelectorAll('[data-variante]')) {
    bouton.addEventListener('click', () => {
        const variante = Number(bouton.dataset.variante);
        let restant = Math.ceil(DELAI_ARMEMENT_MS / 1000);
        bouton.disabled = true;
        tracer(
            `variante ${variante} armée : déclenchement dans ${restant} s ` +
            `(> ${SEUIL_ACTIVATION_MS / 1000} s d'activation transitoire). Ne touchez à rien.`,
        );

        const compteur = setInterval(() => {
            restant -= 1;
            bouton.textContent = `${bouton.dataset.libelle} — ${restant} s`;
            if (restant <= 0) {
                clearInterval(compteur);
                bouton.disabled = false;
                bouton.textContent = bouton.dataset.libelle;
                fetch('/fire', {
                    method: 'POST',
                    headers: { 'content-type': 'application/json' },
                    body: JSON.stringify({ variant: variante }),
                }).catch((erreur) => tracer(`échec du déclenchement : ${erreur}`, 'echec'));
            }
        }, 1000);
    });
}

// Enregistrement du service worker : requis pour l'installabilité PWA (variantes
// 2 et 3) et pour la variante 4. Le fichier sw.js n'est PAS couvert par les
// exceptions de politique Pomerium (qui ne visent que manifest.json, .ico et
// .png) : son chargement dépend donc du cookie de session. Un échec ici est un
// résultat du spike, pas un incident — il est tracé comme tel.
if ('serviceWorker' in navigator) {
    navigator.serviceWorker.register('/sw.js')
        .then((enregistrement) => tracer(`service worker enregistré (portée ${enregistrement.scope})`))
        .catch((erreur) => tracer(`service worker refusé : ${erreur} — vérifier la politique Pomerium`, 'echec'));
}

tracer(`mode d'affichage : ${matchMedia('(display-mode: standalone)').matches ? 'standalone (PWA installée)' : 'onglet navigateur'}`);
```

- [ ] **Étape 7 : remplacer `spike-multifenetres/public/index.html`**

```html
<!doctype html>
<html lang="fr">
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <title>Spike multi-fenêtres</title>
    <link rel="manifest" href="/manifest.json">
    <meta name="theme-color" content="#111827">
    <link rel="icon" href="/icon-192.png">
    <style>
        body { font: 16px/1.5 system-ui, sans-serif; max-width: 52rem; margin: 2rem auto; padding: 0 1rem; }
        table { border-collapse: collapse; width: 100%; margin: 1rem 0; }
        th, td { text-align: left; padding: .5rem; border-bottom: 1px solid #e4e4e7; vertical-align: top; }
        button { font: inherit; padding: .4rem .8rem; cursor: pointer; }
        button:disabled { opacity: .5; cursor: default; }
        #journal { margin-top: 1rem; font-family: ui-monospace, monospace; font-size: .85rem; }
        .ligne { padding: .2rem 0; border-bottom: 1px solid #f4f4f5; }
        .succes { color: #15803d; }
        .echec { color: #b91c1c; }
        .alerte { color: #a16207; }
        .connecte { color: #15803d; }
        .deconnecte { color: #b91c1c; }
        .verdict { font-weight: 600; }
    </style>
</head>
<body>
    <h1>Spike — ouverture de fenêtre sans geste utilisateur</h1>
    <p>
        WebSocket : <span id="etat" class="deconnecte">déconnecté</span> ·
        <a href="/reset.html">étape 0 — hygiène de l'origin</a>
    </p>
    <p>
        Chaque bouton <strong>arme</strong> sa variante : l'ouverture est ensuite
        commandée par le serveur, 15 s plus tard. Ne touchez ni souris ni clavier
        pendant le compte à rebours (sauf variante 3, dont le clic est le
        mécanisme testé).
    </p>

    <table>
        <thead>
            <tr><th>#</th><th>Variante</th><th>Armer</th><th>Verdict</th></tr>
        </thead>
        <tbody>
            <tr>
                <td>1</td>
                <td>Onglet normal, sans activation — <em>témoin</em></td>
                <td><button data-variante="1" data-libelle="Armer 1">Armer 1</button></td>
                <td class="verdict" id="verdict-1">—</td>
            </tr>
            <tr>
                <td>2</td>
                <td>PWA installée (<code>standalone</code>), sans activation</td>
                <td><button data-variante="2" data-libelle="Armer 2">Armer 2</button></td>
                <td class="verdict" id="verdict-2">—</td>
            </tr>
            <tr>
                <td>3</td>
                <td>Armement sur le prochain clic</td>
                <td><button data-variante="3" data-libelle="Armer 3">Armer 3</button></td>
                <td class="verdict" id="verdict-3">—</td>
            </tr>
            <tr>
                <td>4</td>
                <td>Notification → <code>clients.openWindow()</code>, fenêtre en arrière-plan</td>
                <td>
                    <button id="permission">Autoriser les notifications</button>
                    <button data-variante="4" data-libelle="Armer 4">Armer 4</button>
                </td>
                <td class="verdict" id="verdict-4">—</td>
            </tr>
        </tbody>
    </table>

    <h2>Journal</h2>
    <div id="journal"></div>

    <script type="module" src="/app.js"></script>
    <script type="module">
        document.getElementById('permission').addEventListener('click', async () => {
            const resultat = await Notification.requestPermission();
            console.log('[spike] permission notifications :', resultat);
        });
    </script>
</body>
</html>
```

- [ ] **Étape 8 : lancer toute la suite de tests**

Lancer : `cd spike-multifenetres && npm test`
Attendu : 16 tests PASSÉS (5 serveur + 4 hygiène + 7 classement).

- [ ] **Étape 9 : committer**

```bash
git add spike-multifenetres/public/lib/classify.js spike-multifenetres/public/app.js \
        spike-multifenetres/public/opened.html spike-multifenetres/public/index.html \
        spike-multifenetres/test/classify.test.js
git commit -m "feat(spike): classement des trois issues et variantes 1 à 3"
```

---

## Tâche 4 : PWA installable et variante 4

**Contexte.** Les variantes 2 et 3 exigent une PWA réellement installée ; la
variante 4 exige un service worker et la permission notifications. Pour que
Chromium propose l'installation, il faut un manifest valide avec icônes 192 et
512, un service worker doté d'un gestionnaire `fetch`, et un contexte sécurisé —
fourni ici par Pomerium (HTTPS).

**Fichiers :**
- Créer : `spike-multifenetres/public/manifest.json`
- Créer : `spike-multifenetres/public/sw.js`
- Créer : `spike-multifenetres/public/icon-192.png`
- Créer : `spike-multifenetres/public/icon-512.png`

**Interfaces :**
- Consomme : `app.js` (tâche 3) enregistre `/sw.js` et appelle
  `showNotification` avec `data.url`.
- Produit : un service worker qui, sur `notificationclick`, appelle
  `clients.openWindow(event.notification.data.url)`.

- [ ] **Étape 1 : générer les deux icônes**

Lancer depuis la racine du dépôt (où `sharp` est installé) :

```bash
node -e "
const sharp = require('sharp');
const carre = (taille) => sharp({
  create: { width: taille, height: taille, channels: 4, background: { r: 17, g: 24, b: 39, alpha: 1 } }
}).png().toFile('spike-multifenetres/public/icon-' + taille + '.png');
Promise.all([carre(192), carre(512)]).then(() => console.log('icônes générées'));
"
```

Attendu : `icônes générées`, et deux fichiers PNG présents.

- [ ] **Étape 2 : vérifier les icônes**

Lancer : `file spike-multifenetres/public/icon-192.png spike-multifenetres/public/icon-512.png`
Attendu : `PNG image data, 192 x 192` et `PNG image data, 512 x 512`.

- [ ] **Étape 3 : créer `spike-multifenetres/public/manifest.json`**

```json
{
  "name": "Spike multi-fenêtres",
  "short_name": "Spike",
  "description": "Test d'ouverture de fenêtre sans geste utilisateur",
  "start_url": "/",
  "scope": "/",
  "display": "standalone",
  "background_color": "#111827",
  "theme_color": "#111827",
  "icons": [
    { "src": "/icon-192.png", "sizes": "192x192", "type": "image/png", "purpose": "any" },
    { "src": "/icon-512.png", "sizes": "512x512", "type": "image/png", "purpose": "any" }
  ]
}
```

- [ ] **Étape 4 : créer `spike-multifenetres/public/sw.js`**

```javascript
// Service worker du spike. Deux rôles :
//   1. rendre la PWA installable — Chromium exige un gestionnaire `fetch` ;
//   2. porter la variante 4 : ouvrir une fenêtre depuis notificationclick, qui
//      est une activation utilisateur valide même fenêtre en arrière-plan.
//
// Le fetch est délibérément un simple passe-plat, sans aucun cache : mettre en
// cache les pages du spike ferait tester du code périmé entre deux relances.

self.addEventListener('install', () => self.skipWaiting());

self.addEventListener('activate', (evenement) => {
    evenement.waitUntil(self.clients.claim());
});

self.addEventListener('fetch', (evenement) => {
    evenement.respondWith(fetch(evenement.request));
});

self.addEventListener('notificationclick', (evenement) => {
    evenement.notification.close();
    const url = evenement.notification.data?.url ?? '/opened.html?variant=4';
    // waitUntil est obligatoire : sans lui le service worker peut être arrêté
    // avant que la fenêtre ne s'ouvre, et l'échec serait imputé au navigateur.
    evenement.waitUntil(self.clients.openWindow(url));
});
```

- [ ] **Étape 5 : vérifier que le serveur sert le manifest et le service worker**

Lancer :

```bash
cd spike-multifenetres && SPIKE_PORT=3999 npm start &
sleep 2
curl -s -o /dev/null -w '%{http_code} %{content_type}\n' http://127.0.0.1:3999/manifest.json
curl -s -o /dev/null -w '%{http_code} %{content_type}\n' http://127.0.0.1:3999/sw.js
curl -s -o /dev/null -w '%{http_code} %{content_type}\n' http://127.0.0.1:3999/icon-512.png
kill %1
```

Attendu :
```
200 application/json; charset=utf-8
200 text/javascript; charset=utf-8
200 image/png
```

- [ ] **Étape 6 : committer**

```bash
git add spike-multifenetres/public/manifest.json spike-multifenetres/public/sw.js \
        spike-multifenetres/public/icon-192.png spike-multifenetres/public/icon-512.png
git commit -m "feat(spike): PWA installable et ouverture depuis notificationclick"
```

---

## Tâche 5 : exécution, mesure et rapport

**Contexte.** Les quatre tâches précédentes construisent l'instrument ; celle-ci
produit le résultat, qui est le seul livrable qui compte. Elle comporte des
étapes manuelles — l'installation d'une PWA n'est pas automatisable de façon
honnête (`--app=URL` produit une fenêtre app-like qui *n'est pas* une PWA
installée, et s'en contenter fausserait la variante 2, celle qui porte la
question).

**Fichiers :**
- Créer : `docs/superpowers/plans/2026-07-28-spike-multifenetres-resultats.md`

**Interfaces :**
- Consomme : tout ce qui précède.
- Produit : le verdict sur le modèle multi-fenêtres du cadrage jeu §4.

- [ ] **Étape 1 : libérer le port 3445**

```bash
cd /home/mallanic/Projects/Guacamole && docker compose stop web
ss -tlnp | grep 3445 || echo "port 3445 libre"
```

Attendu : `port 3445 libre`. Le conteneur a `restart: always`, mais `docker
compose stop` le maintient arrêté jusqu'à un `start` explicite.

- [ ] **Étape 2 : lancer le serveur du spike**

```bash
cd /home/mallanic/Projects/Guacamole/spike-multifenetres && npm start
```

Attendu : `spike à l'écoute sur le port 3445`. Laisser tourner.

- [ ] **Étape 3 : vérifier la chaîne complète à travers Pomerium**

Depuis le navigateur de l'utilisateur : ouvrir `https://app.allanic.me/`.
Attendu : la page du spike s'affiche, l'indicateur WebSocket passe à
**connecté**, et le journal indique le mode d'affichage.

Si le WebSocket reste déconnecté, vérifier `allow_websockets: true` sur la route
`app.allanic.me` — il est présent dans la configuration relevée, mais un
rechargement de Pomerium peut être nécessaire.

- [ ] **Étape 4 : exécuter l'étape 0 (hygiène) et consigner ce qu'elle trouve**

Ouvrir `https://app.allanic.me/reset.html`, cliquer sur « Nettoyer l'origin »,
**copier le rapport JSON affiché**. C'est un résultat à part entière : la
présence d'un service worker de l'ancienne application explique pourquoi cette
étape était nécessaire.

Puis désinstaller l'ancienne PWA si le navigateur en signale une (menu →
Applications), et recharger la page principale.

- [ ] **Étape 5 : variante 1 (témoin, onglet normal)**

Dans un **onglet normal**, cliquer « Armer 1 », puis ne plus rien toucher pendant
15 s. Consigner le verdict affiché et la ligne de journal complète.

Attendu : `bloque`. **Si le verdict est `succes`, arrêter et enquêter** — cela
signifierait que le test lui-même ne reproduit pas la condition visée, et aucun
autre verdict ne serait exploitable.

- [ ] **Étape 6 : installer la PWA**

Menu du navigateur → « Installer Spike multi-fenêtres ». Si l'option n'apparaît
pas, ouvrir DevTools → Application → Manifest et consigner l'erreur
d'installabilité rapportée : c'est un résultat, pas un incident de parcours.

- [ ] **Étape 7 : variante 2 (PWA installée, sans activation)**

Dans la **fenêtre PWA installée**, cliquer « Armer 2 » puis ne plus rien toucher
pendant 15 s. Consigner verdict et journal.

C'est la mesure décisive du spike.

- [ ] **Étape 8 : variante 3 (armement sur le prochain clic)**

Toujours dans la PWA, cliquer « Armer 3 », attendre les 15 s sans rien toucher,
puis — une fois le message « variante 3 armée » affiché — cliquer n'importe où.
Consigner verdict et journal.

- [ ] **Étape 9 : variante 4 (notification, fenêtre en arrière-plan)**

Cliquer « Autoriser les notifications » et accepter. Puis cliquer « Armer 4 »,
**passer la fenêtre PWA en arrière-plan** (donner le focus à une autre
application), attendre la notification, cliquer dessus. Consigner verdict et
journal.

- [ ] **Étape 10 : relever la version de Chromium**

Ouvrir `chrome://version` et copier la première ligne. Un verdict sur les
politiques de popup sans la version du navigateur n'est pas reproductible.

- [ ] **Étape 11 : rédiger le rapport**

Créer `docs/superpowers/plans/2026-07-28-spike-multifenetres-resultats.md`
suivant exactement ce squelette, en remplaçant chaque champ entre chevrons par
la valeur relevée :

```markdown
# Spike multi-fenêtres — résultats

**Date d'exécution** : <date>
**Navigateur** : <ligne complète de chrome://version>
**Exposition** : https://app.allanic.me via Pomerium, serveur du spike sur 3445

## État de l'origin avant le test (étape 0)

<rapport JSON de reset.html, tel quel>

<Une phrase : un service worker de l'ancienne application était-il présent ?>

## Verdicts

| # | Variante | Verdict | ms depuis le dernier geste | Observation |
|---|---|---|---|---|
| 1 | Onglet normal, sans activation | <verdict> | <ms> | <ligne de journal> |
| 2 | PWA installée, sans activation | <verdict> | <ms> | <ligne de journal> |
| 3 | Armement sur le prochain clic | <verdict> | <ms> | <ligne de journal> |
| 4 | Notification → openWindow, arrière-plan | <verdict> | <ms> | <ligne de journal> |

## Recommandation

<L'une des deux formes, tranchée :>

**Le modèle §4 tient.** Mécanisme retenu pour le chantier D : <variante>.
Coût UX : <aucun | un clic | un clic sur notification>. Conséquence pour les
chantiers A et B : ils peuvent être bâtis sur l'hypothèse multi-fenêtres.

**Le modèle §4 ne tient pas.** Aucune variante ne permet d'ouvrir une fenêtre
sans <contrainte>. Conséquence : <ce qui change dans le modèle produit —
barre des tâches dans le hub, ou fenêtre unique avec commutateur>.

## Réserves et limites

- Testé sur Chromium desktop uniquement. ChromeOS, plateforme cliente
  privilégiée (cadrage jeu §6.1), n'est pas couvert. <Un succès ici vaut a
  fortiori là-bas | Un échec ici laisse ChromeOS ouvert.>
- Firefox et Safari ne sont pas testés : le multi-fenêtres y est déjà documenté
  comme non supporté.
- <Le service worker a-t-il pu être enregistré derrière la politique Pomerium,
  qui n'exempte que manifest.json, .ico et .png ?>
```

- [ ] **Étape 12 : restaurer l'ancien serveur**

```bash
cd /home/mallanic/Projects/Guacamole && docker compose start web
sleep 3
ss -tlnp | grep 3445
curl -s -o /dev/null -w 'ancien serveur : %{http_code}\n' http://127.0.0.1:3445/
```

Attendu : le port est de nouveau occupé et l'ancien serveur répond. **Cette étape
n'est pas facultative** : l'ancienne application reste le système utilisable
jusqu'à son remplacement (spec produit §11).

- [ ] **Étape 13 : committer le rapport**

```bash
git add docs/superpowers/plans/2026-07-28-spike-multifenetres-resultats.md
git commit -m "docs(spike): résultats du test d'ouverture de fenêtre"
```

- [ ] **Étape 14 : reporter la conclusion dans le cadrage jeu**

Modifier `docs/superpowers/specs/2026-07-28-support-jeux-design.md` §5, chantier
D, ligne « **Risque n°1 — popup blocker** » : y ajouter une phrase renvoyant au
rapport et donnant le verdict, de sorte que le risque ne soit plus décrit comme
ouvert.

```bash
git add docs/superpowers/specs/2026-07-28-support-jeux-design.md
git commit -m "docs: clore le risque popup blocker du chantier D"
```

---

## Auto-revue du plan

**Couverture de la spécification** — chaque section a sa tâche :

| Section de la spec | Tâche |
|---|---|
| §3 Environnement (port 3445, Pomerium) | Contraintes globales + tâche 5 étapes 1-3 |
| §4 Étape 0 — hygiène de l'origin | Tâche 2, tâche 5 étape 4 |
| §4 Variantes 1, 2, 3 | Tâche 3, tâche 5 étapes 5-8 |
| §4 Variante 4 (notification) | Tâche 4, tâche 5 étape 9 |
| §4 Ce qui reste manuel | Tâche 5 étapes 4, 6, 7, 8, 9 |
| §4 `scope: "/"` du manifest | Tâche 4 étape 3 |
| §5 Trois issues, seuil 3 s | Tâche 3 (`classify.js`, `DELAI_SIGNAL_VIE_MS`) |
| §6 Livrable versionné | Tâche 5 étapes 11 et 13 |
| §7 Restauration | Tâche 5 étape 12 |
| §8 Critère d'arrêt | Tâche 5 : les 4 variantes sont exécutées quoi qu'il arrive |
| §9 Hors périmètre | Aucune tâche ne touche l'agent, la VM ni WebRTC |

**Cohérence des noms** — vérifiée entre tâches : `createSpikeServer`,
`SpikeServer`, `nettoyerOrigin`, `classer`, `SEUIL_ACTIVATION_MS`,
`/fire`, `/alive`, `/ws`, `variant` (côté fil, en anglais pour rester proche des
noms d'API) et `variante` (côté code, en français). Le protocole WebSocket
`{ type, variant, at }` est identique dans `server.ts`, `server.test.ts` et
`app.js`.

**Point de fragilité assumé** : la variante 4 conclut sur le seul signal de vie,
`clients.openWindow()` ne rendant pas de poignée exploitable côté page. Un échec
d'ouverture et une fenêtre partie sur l'IdP y sont donc indiscernables. C'est
acceptable : la variante 4 est un repli, et son attendu est le succès.
