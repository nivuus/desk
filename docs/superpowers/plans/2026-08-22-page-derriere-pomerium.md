# La page derrière Pomerium — plan d'implémentation

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Doter la plateforme d'un servant de fichiers statiques et fermer le
contournement d'authentification de `/auth/moi`, pour qu'un navigateur puisse
atteindre le produit derrière Pomerium.

**Architecture:** Un routeur de page chaîné **en dernier**, avant le `404`,
piloté par une variable facultative dont l'absence rend le comportement
d'aujourd'hui à l'octet près. Une règle de résolution **pure** (aucun `fs`),
des en-têtes séparés par nature de réponse, et une garde d'adresse source sur
la seule route d'authentification du mode `pomerium`.

**Tech Stack:** TypeScript 5.5, Node (`node:http`, `node:fs`), Vitest 2,
`tsc --noEmit`.

**Spec:** [`docs/superpowers/specs/2026-08-21-page-derriere-pomerium-design.md`](../specs/2026-08-21-page-derriere-pomerium-design.md)
**Cadrage :** [`docs/superpowers/specs/2026-08-21-finalisation-cadrage.md`](../specs/2026-08-21-finalisation-cadrage.md)

## Global Constraints

- **Français partout** : noms de symboles, commentaires, messages d'erreur,
  messages de commit. Les identifiants techniques (`Content-Security-Policy`,
  `no-store`) gardent leur forme.
- **500 lignes maximum** par fichier de code source. `plateforme/` est dans la
  portée. 🔴 **Relever la taille par la commande, jamais la recopier :**
  `wc -l <fichier>`. Relevé du 21 août 2026 : `serveur.ts` **475**,
  `config.ts` **281**, `routes-identite.ts` **168**, `entetes.ts` **45**.
- **Extraire, jamais comprimer** — et dans une tâche **dédiée, avant** celle
  qui ajoute. C'est la tâche 1.
- **Deux commandes de test, jamais une** :
  `cd plateforme && npm run test:sqlite` **et** `npm run test:postgres`.
  `npm run typecheck` est une **étape distincte** : Vitest transpile sans
  vérifier les types.
- 🔴 **Un contrôle qu'on n'a jamais vu rouge n'est pas un contrôle.** Chaque
  tâche exécute son test **avant** l'implémentation et **lit le message
  d'échec**, pas seulement le code de sortie.
- 🔴 **Jamais `git add -A`.** Nommer les fichiers. ⚠️ Un pathspec de répertoire
  ne prend pas le module homonyme (`src/http/page` laisse `src/http/page.ts`).
- Le service ne démarre pas sans `PLATEFORME_HOTE` et `PLATEFORME_SECRET_JETON`
  (≥ 32 caractères). Les tests construisent un `Config` directement et
  contournent `lireConfig` : les deux chemins doivent être éprouvés séparément.

---

### Task 1: Extraire la chaîne de routeurs hors de `serveur.ts`

**Pourquoi d'abord :** `serveur.ts` est à 475/500. La tâche 5 y ajoute un
import, une clé de dépendances et un chaînage, dans un fichier dont la densité
de commentaire dépasse la ligne de code. `CLAUDE.md` prescrit l'extraction en
tâche dédiée **avant** celle qui ajoute, et interdit nommément la compression
après coup. **Aucun comportement ne change dans cette tâche.**

**Files:**
- Create: `plateforme/src/http/chaine.ts`
- Modify: `plateforme/src/http/serveur.ts` (retirer `servirTout` et ses imports de routeurs devenus inutiles ; appeler `servirTout(requete, reponse, deps)`)
- Test: aucun test neuf — la suite existante **est** le garde

**Interfaces:**
- Consumes: les neuf `Dependances*` exportées par les `routes-*.ts`
- Produces: `export type DependancesRoutage`, `export async function servirTout(requete: IncomingMessage, reponse: ServerResponse, deps: DependancesRoutage): Promise<boolean>`

- [ ] **Step 1: Relever la taille AVANT, pour pouvoir la comparer**

```bash
wc -l plateforme/src/http/serveur.ts
```

Consigner le nombre. Attendu : **475** — s'il diffère, quelqu'un a écrit dans le
fichier depuis le relevé, et c'est le nombre du jour qui fait foi.

- [ ] **Step 2: Établir le vert de départ**

```bash
cd /home/mallanic/Projects/Guacamole/plateforme && npm run test:sqlite && npm run typecheck
```

Attendu : PASS. 🔴 **Si c'est déjà rouge, s'arrêter et le dire** — une
extraction jugée sur une suite rouge ne prouve rien.

- [ ] **Step 3: Créer `chaine.ts`**

```ts
// La chaîne des routeurs HTTP, extraite de `serveur.ts` le 22 août 2026.
//
// 🔴 POURQUOI CE FICHIER EXISTE : `serveur.ts` était à 475 lignes sur 500 au
// moment où le lot « page derrière Pomerium » devait y ajouter un onzième
// routeur. `CLAUDE.md` prescrit l'extraction en tâche DÉDIÉE, AVANT celle qui
// ajoute — « extraire, jamais comprimer » — parce que la marge regagnée par
// une extraction se reperd si on la traite comme acquise (payé six fois).
//
// ⚠️ CETTE EXTRACTION NE CHANGE AUCUN COMPORTEMENT. L'ordre des routeurs, les
// commentaires qui l'expliquent et le `false` final sont repris VERBATIM. Le
// seul changement est que `deps` arrive en paramètre au lieu d'être capturé
// par fermeture.

import type { IncomingMessage, ServerResponse } from 'node:http';
import { servirAuth, type DependancesAuth } from './routes-auth';
import { servirIdentite, type DependancesIdentite } from './routes-identite';
import { servirVm, type DependancesVm } from './routes-vm';
import { servirSession, type DependancesSession } from './routes-session';
import { servirApplications, type DependancesApplications } from './routes-applications';
import { servirIcone, type DependancesIcone } from './routes-icone';
import { servirTeleversement, type DependancesTeleversement } from './routes-televersement';
import { servirInstallation, type DependancesInstallation } from './routes-installation';
import { servirSante, type DependancesSante } from './routes-sante';

/// Tout ce que la chaîne consomme, réuni.
///
/// ⚠️ UNE INTERSECTION, PAS UNE INTERFACE QUI ÉTEND : deux `Dependances*` qui
/// déclareraient la même clé avec des types incompatibles feraient ÉCHOUER un
/// `extends` à la déclaration, alors que l'intersection laisse le conflit se
/// révéler à l'APPEL, sur l'objet réel — c'est-à-dire là où il se corrige.
export type DependancesRoutage = DependancesIdentite &
    DependancesAuth &
    DependancesVm &
    DependancesApplications &
    DependancesIcone &
    DependancesTeleversement &
    DependancesInstallation &
    DependancesSession &
    DependancesSante;

/// Essaie les routeurs dans l'ordre, et rend `false` si aucun n'a servi.
export async function servirTout(
    requete: IncomingMessage,
    reponse: ServerResponse,
    deps: DependancesRoutage,
): Promise<boolean> {
    // Corps et commentaires repris VERBATIM de `serveur.ts:294-331` (relevé du
    // 21 août 2026 : `async function servirTout(` en 294, l'accolade fermante
    // en 331). ⚠️ RELEVER LES NUMÉROS PAR `grep -n 'async function servirTout'`
    // AVANT de couper : la tâche est censée être la première du lot, mais un
    // numéro de ligne est faux dès qu'on écrit au-dessus.
}
```

🔴 **Le corps et TOUS les commentaires de `servirTout` sont déplacés
VERBATIM** — y compris les blocs sur l'ordre de `servirIdentite`, sur les deux
routeurs de G3, et sur `/sante` chaînée en dernier. ⚠️ Ne pas les résumer :
« une extraction n'est jamais rigoureusement verbatim » est un piège recensé, et
la parade est de le viser explicitement.

- [ ] **Step 4: Câbler `serveur.ts`**

Remplacer la définition locale de `servirTout` par
`import { servirTout } from './chaine';`, et l'appel `servirTout(requete, reponse)`
par `servirTout(requete, reponse, deps)`.

⚠️ **Retirer les imports de routeurs devenus inutilisés dans `serveur.ts`** —
un import orphelin est un `TS6133`, c'est-à-dire un **échec** de `tsc`, d'une
famille autre que `dead_code`. `CacheSante` reste importé (il est construit
là) ; `servirSante` ne l'est plus.

- [ ] **Step 5: Vérifier que rien n'a changé**

```bash
cd /home/mallanic/Projects/Guacamole/plateforme && npm run typecheck && npm run test:sqlite && npm run test:postgres
```

Attendu : PASS partout, **avec le même nombre de tests qu'à l'étape 2**.

- [ ] **Step 6: Relever la taille APRÈS**

```bash
cd /home/mallanic/Projects/Guacamole && wc -l plateforme/src/http/serveur.ts plateforme/src/http/chaine.ts
```

Attendu : `serveur.ts` **nettement sous 475**, `chaine.ts` sous 500. Consigner
les deux nombres — la tâche 5 en aura besoin.

- [ ] **Step 7: Commit**

```bash
cd /home/mallanic/Projects/Guacamole
git add plateforme/src/http/chaine.ts plateforme/src/http/serveur.ts
git commit -m 'extraction(http) : la chaine des routeurs quitte serveur.ts, avant la tache qui ajoute'
```

---

### Task 2: La règle de résolution — pure, sans `fs`

**Files:**
- Create: `plateforme/src/http/page/resolution.ts`
- Test: `plateforme/src/http/page/resolution.test.ts`

**Interfaces:**
- Produces: `export type Resolution`, `export function resoudre(cheminUrl: string): Resolution`, `export const TYPES_MIME: ReadonlyMap<string, string>`

- [ ] **Step 1: Écrire le test qui échoue**

`plateforme/src/http/page/resolution.test.ts` :

```ts
import { describe, expect, it } from 'vitest';
import { resoudre } from './resolution';

describe('resoudre', () => {
    it('rend index.html pour la racine, et le marque DOCUMENT', () => {
        expect(resoudre('/')).toEqual({
            ok: true,
            fichier: 'index.html',
            mime: 'text/html; charset=utf-8',
            document: true,
        });
    });

    it('rend un fichier nommé', () => {
        expect(resoudre('/hub.html')).toEqual({
            ok: true,
            fichier: 'hub.html',
            mime: 'text/html; charset=utf-8',
            document: true,
        });
    });

    // Une RESSOURCE, pas un document : c'est ce booléen qui décide plus tard
    // entre `no-store` et `immutable`, et se tromper ici tuerait le cache du
    // navigateur sur tous les fichiers empreintés par Vite.
    it("marque un actif comme RESSOURCE, jamais comme document", () => {
        expect(resoudre('/assets/index-a1b2c3.js')).toEqual({
            ok: true,
            fichier: 'assets/index-a1b2c3.js',
            mime: 'text/javascript; charset=utf-8',
            document: false,
        });
    });

    // Le `try_files $uri $uri/ /index.html` de nginx, reproduit : un chemin
    // sans extension retombe sur la page, jamais sur un 404.
    it('replie un chemin sans extension sur index.html', () => {
        expect(resoudre('/hub')).toMatchObject({ ok: true, fichier: 'index.html' });
    });

    // 🔴 LA TRAVERSÉE SE JUGE SUR LE CHEMIN RÉSOLU. Un filtre par sous-chaîne
    // `..` laisserait passer la forme encodée ci-dessous.
    it('refuse une traversée ENCODÉE', () => {
        expect(resoudre('/%2e%2e%2fetc%2fpasswd')).toEqual({ ok: false, motif: 'traversee' });
    });

    it('refuse une traversée en clair', () => {
        expect(resoudre('/../etc/passwd')).toEqual({ ok: false, motif: 'traversee' });
    });

    it('refuse une traversée qui remonte APRÈS être descendue', () => {
        expect(resoudre('/assets/../../etc/passwd')).toEqual({ ok: false, motif: 'traversee' });
    });

    it('refuse un octet NUL', () => {
        expect(resoudre('/index.html%00.txt')).toEqual({ ok: false, motif: 'octet-nul' });
    });

    it('refuse un encodage malformé, plutôt que de lever', () => {
        expect(resoudre('/%zz')).toEqual({ ok: false, motif: 'chemin-invalide' });
    });

    // 🔴 LA LISTE MIME EST CLOSE. Retomber sur `application/octet-stream`
    // publierait tout fichier laissé dans la racine — un `.env`, une clé, un
    // `.map` — avec une invite de téléchargement.
    it("refuse une extension hors de la liste, jamais ne retombe sur octet-stream", () => {
        expect(resoudre('/.env')).toEqual({ ok: false, motif: 'extension-inconnue' });
        expect(resoudre('/index.js.map')).toEqual({ ok: false, motif: 'extension-inconnue' });
    });

    it('sert le manifeste et les icônes que le hub nomme', () => {
        expect(resoudre('/hub.webmanifest')).toMatchObject({
            ok: true,
            mime: 'application/manifest+json',
            document: false,
        });
        expect(resoudre('/favicon.ico')).toMatchObject({ ok: true, document: false });
    });
});
```

- [ ] **Step 2: Exécuter le test et vérifier qu'il échoue**

```bash
cd /home/mallanic/Projects/Guacamole/plateforme && npx vitest run src/http/page/resolution.test.ts
```

Attendu : FAIL — `Failed to resolve import "./resolution"`. **Lire le message**,
pas seulement le code de sortie.

- [ ] **Step 3: Écrire l'implémentation**

`plateforme/src/http/page/resolution.ts` :

```ts
// La RÈGLE de résolution d'un chemin d'URL vers un fichier de la page bâtie.
// PURE : aucun `fs`, aucun `http`, aucune variable d'environnement. C'est ce
// qui la rend éprouvable sur l'hôte, et c'est là que vit toute la sécurité du
// servant — le module qui lit le disque ne fait qu'appliquer ce verdict.

/// 🔴 LISTE CLOSE. Une extension absente d'ici REFUSE.
export const TYPES_MIME: ReadonlyMap<string, string> = new Map([
    ['html', 'text/html; charset=utf-8'],
    ['js', 'text/javascript; charset=utf-8'],
    ['css', 'text/css; charset=utf-8'],
    ['webmanifest', 'application/manifest+json'],
    ['json', 'application/json; charset=utf-8'],
    ['ico', 'image/x-icon'],
    ['png', 'image/png'],
    ['svg', 'image/svg+xml'],
    ['woff2', 'font/woff2'],
]);

const PAGE = 'index.html';

export type Resolution =
    | { readonly ok: true; readonly fichier: string; readonly mime: string; readonly document: boolean }
    | {
          readonly ok: false;
          readonly motif: 'chemin-invalide' | 'octet-nul' | 'traversee' | 'extension-inconnue';
      };

/// Normalise un chemin en segments, en refusant toute remontée qui SORT.
///
/// ⚠️ LE COMPTE SE FAIT SUR LE RÉSULTAT, JAMAIS SUR LA PRÉSENCE DE `..` : c'est
/// ce qui rend `/assets/../index.html` légitime et `/assets/../../x` refusé,
/// là où un filtre par sous-chaîne refuserait les deux ou accepterait les deux.
function normaliser(brut: string): string[] | undefined {
    const sortie: string[] = [];
    for (const segment of brut.split('/')) {
        if (segment === '' || segment === '.') continue;
        // Un antislash n'est pas un séparateur sous Linux, mais un chemin qui
        // en porte un ne vient d'aucune page bâtie par Vite : le refuser coûte
        // zéro et ferme la variante Windows de la traversée.
        if (segment === '..' || segment.includes('\\')) {
            if (segment === '..' && sortie.length > 0) {
                sortie.pop();
                continue;
            }
            return undefined;
        }
        sortie.push(segment);
    }
    return sortie;
}

export function resoudre(cheminUrl: string): Resolution {
    let decode: string;
    try {
        decode = decodeURIComponent(cheminUrl);
    } catch {
        // `decodeURIComponent` LÈVE sur un `%` mal formé. Un servant qui
        // laisserait passer cette exception rendrait un 500 là où un refus
        // suffit — et le `catch` du serveur journaliserait une « route en
        // échec » pour une requête simplement mal écrite.
        return { ok: false, motif: 'chemin-invalide' };
    }
    if (decode.includes('\0')) return { ok: false, motif: 'octet-nul' };

    const segments = normaliser(decode);
    if (segments === undefined) return { ok: false, motif: 'traversee' };

    const dernier = segments[segments.length - 1];
    // Racine, ou chemin sans extension : le `try_files … /index.html` de nginx.
    const fichier = dernier === undefined || !dernier.includes('.') ? PAGE : segments.join('/');

    const point = fichier.lastIndexOf('.');
    const extension = fichier.slice(point + 1).toLowerCase();
    const mime = TYPES_MIME.get(extension);
    if (mime === undefined) return { ok: false, motif: 'extension-inconnue' };

    return { ok: true, fichier, mime, document: extension === 'html' };
}
```

- [ ] **Step 4: Exécuter le test et vérifier qu'il passe**

```bash
cd /home/mallanic/Projects/Guacamole/plateforme && npx vitest run src/http/page/resolution.test.ts && npm run typecheck
```

Attendu : PASS, 11 tests.

- [ ] **Step 5: Jouer une rouge sur la garde de traversée**

Remplacer temporairement le corps de `normaliser` par `return brut.split('/').filter((s) => s !== '' && s !== '.')` (c'est-à-dire retirer la garde), relancer, **et lire quelles assertions rougissent**. Attendu : les trois tests de traversée, **et eux seuls**. Restaurer ensuite **depuis une copie nommée**, jamais par `git checkout --` (qui restaure HEAD, pas l'état d'avant la mutation).

- [ ] **Step 6: Commit**

```bash
cd /home/mallanic/Projects/Guacamole
git add plateforme/src/http/page/resolution.ts plateforme/src/http/page/resolution.test.ts
git commit -m 'page(regle) : la resolution est PURE, et la traversee se juge sur le chemin RESOLU'
```

---

### Task 3: `PLATEFORME_PAGE` dans la configuration

**Files:**
- Modify: `plateforme/src/config.ts`
- Test: `plateforme/src/config.test.ts`

**Interfaces:**
- Produces: `Config.racinePage?: string`

- [ ] **Step 1: Écrire les trois tests qui échouent**

Ajouter à `plateforme/src/config.test.ts` :

```ts
describe('PLATEFORME_PAGE', () => {
    // 🔴 AUCUN DÉFAUT, à la différence de PLATEFORME_ICONES : un défaut comme
    // `client/dist` ferait servir un répertoire au hasard du répertoire
    // courant, et ferait passer le montage nginx — où la plateforme ne doit
    // RIEN servir — d'un 404 franc à un 200 sur des fichiers non voulus.
    it("est ABSENTE par défaut, et le service ne sert alors aucun fichier", () => {
        expect(lireConfig(envValide()).racinePage).toBeUndefined();
    });

    // ⚠️ Le test de la chaîne VIDE est DISTINCT de celui de l'absence :
    // `env.X ?? 'defaut'` ne rattrape pas `''`. P1 a payé cette erreur exacte.
    it('traite la chaîne VIDE comme une absence', () => {
        expect(lireConfig({ ...envValide(), PLATEFORME_PAGE: '' }).racinePage).toBeUndefined();
    });

    it('retient le chemin posé', () => {
        expect(lireConfig({ ...envValide(), PLATEFORME_PAGE: '/srv/page' }).racinePage).toBe(
            '/srv/page',
        );
    });
});
```

🔴 **`envValide()` N'EXISTE PAS : `config.test.ts` emploie un `const BASE` et
des littéraux en ligne.** Lire le fichier et reprendre ce qui s'y trouve — en
créer un second fabricant ferait diverger les deux.

🔴 **UN TEST EXISTANT VA ROUGIR, ET C'EST VOULU.** Le test « lit les champs,
avec leurs défauts non permissifs » compare l'objet **ENTIER** par `toEqual`, et
son commentaire le dit : « un champ ajouté à `Config` sans être ajouté ici le
rendrait rouge, et c'est voulu ». **Y ajouter `racinePage: undefined,`**, avec la
raison en commentaire.

⚠️ **NE JAMAIS LE DESSERRER EN `toMatchObject`** pour le faire passer : cette
assertion est le seul garde du dépôt contre un champ de `Config` ajouté sans
qu'on décide de son défaut.

- [ ] **Step 2: Exécuter et vérifier l'échec**

```bash
cd /home/mallanic/Projects/Guacamole/plateforme && npx vitest run src/config.test.ts
```

Attendu : FAIL sur les trois — `racinePage` n'existe pas.

- [ ] **Step 3: Implémenter**

Dans l'interface `Config`, à côté de `repertoireIcones` :

```ts
    /// PLATEFORME_PAGE — FACULTATIVE, et **AUCUN DÉFAUT**, à la différence de
    /// `PLATEFORME_ICONES` et `PLATEFORME_TELEVERSEMENTS` juste en dessous.
    ///
    /// 🔴 ABSENTE OU VIDE ⇒ LE SERVICE NE SERT AUCUN FICHIER, et son
    /// comportement est celui d'avant le lot À L'OCTET PRÈS : `GET /` rend
    /// `404 introuvable`. C'est ce qui rend l'ajout strictement additif — et
    /// c'est ce qui rend le témoin négatif de la recette jouable.
    ///
    /// ⚠️ UN DÉFAUT SERAIT UN DÉFAUT DE SÉCURITÉ, pas une commodité : dans le
    /// montage nginx, la plateforme ne doit RIEN servir, et un défaut la
    /// ferait publier ce que son répertoire courant contient.
    racinePage?: string;
```

Dans le corps, à côté de `brutIcones` :

```ts
    // Même garde de la chaîne VIDE qu'au-dessus, mais SANS repli : ici, vide
    // et absente valent toutes deux « aucun servant ».
    const brutPage = env.PLATEFORME_PAGE;
    const racinePage = brutPage === undefined || brutPage === '' ? undefined : brutPage;
```

Et `racinePage,` dans l'objet rendu.

- [ ] **Step 4: Exécuter et vérifier le passage**

```bash
cd /home/mallanic/Projects/Guacamole/plateforme && npx vitest run src/config.test.ts && npm run typecheck
```

Attendu : PASS.

- [ ] **Step 5: Commit**

```bash
cd /home/mallanic/Projects/Guacamole
git add plateforme/src/config.ts plateforme/src/config.test.ts
git commit -m 'config(page) : PLATEFORME_PAGE facultative, et son absence est le 404 d hier'
```

---

### Task 4: Les en-têtes de document, et la garde anti-dérive de la CSP

**Files:**
- Create: `plateforme/src/http/page/entetes-page.ts`
- Create: `plateforme/src/http/page/entetes-page.test.ts`
- Modify: `plateforme/src/http/entetes.ts` (corriger le commentaire devenu faux)

**Interfaces:**
- Produces: `export const CSP: string`, `export const ENTETES_DOCUMENT`, `export const ENTETES_RESSOURCE`

- [ ] **Step 1: Écrire le test qui échoue**

`plateforme/src/http/page/entetes-page.test.ts` :

```ts
import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { CSP, ENTETES_DOCUMENT, ENTETES_RESSOURCE } from './entetes-page';

describe('les en-têtes de document', () => {
    it('porte la CSP, Referrer-Policy et X-Frame-Options', () => {
        expect(ENTETES_DOCUMENT['Content-Security-Policy']).toBe(CSP);
        expect(ENTETES_DOCUMENT['Referrer-Policy']).toBe('no-referrer');
        expect(ENTETES_DOCUMENT['X-Frame-Options']).toBe('DENY');
    });

    // 🔴 HSTS RESTE AU TERMINATEUR TLS. La plateforme est joignable en clair
    // sur 192.168.3.1:8080 ; y affirmer que l'origine est HTTPS pour un an
    // serait une affirmation qu'elle n'est pas en position de faire.
    it("n'émet PAS Strict-Transport-Security", () => {
        expect(ENTETES_DOCUMENT).not.toHaveProperty('Strict-Transport-Security');
    });

    it('garde no-store sur le document, qui peut porter un jeton', () => {
        expect(ENTETES_DOCUMENT['Cache-Control']).toBe('no-store');
    });
});

describe('les en-têtes de ressource', () => {
    // 🔴 LA MOITIÉ QUI COMPTE. `no-store` ici tuerait le cache du navigateur
    // sur des noms que Vite empreinte déjà — la panne silencieuse type.
    it('est immutable, JAMAIS no-store', () => {
        expect(ENTETES_RESSOURCE['Cache-Control']).toBe('public, max-age=31536000, immutable');
    });

    it('ne porte NI CSP NI X-Frame-Options : ce sont des en-têtes de document', () => {
        expect(ENTETES_RESSOURCE).not.toHaveProperty('Content-Security-Policy');
        expect(ENTETES_RESSOURCE).not.toHaveProperty('X-Frame-Options');
    });
});

// 🔴 DEUX COPIES D'UNE MÊME POLITIQUE DÉRIVENT. Ce test est la seule chose qui
// l'empêche : un durcissement appliqué d'un seul côté livrerait deux montages
// aux sécurités différentes sans qu'aucune suite ne bronche.
describe('la CSP ne dérive pas de celle de nginx', () => {
    it('est identique à celle de deploiement/nginx.conf', () => {
        // ⚠️ AUCUN REPLI : `readFileSync` LÈVE si le fichier manque, et c'est
        // voulu. « Un `||` de repli transforme *fichier absent* en *contrôle
        // vert*. »
        const nginx = readFileSync(
            new URL('../../../../deploiement/nginx.conf', import.meta.url),
            'utf8',
        );
        const trouvees = [...nginx.matchAll(/add_header Content-Security-Policy "([^"]+)"/g)];
        // Si nginx en déclarait deux, comparer « la première » choisirait en
        // silence. On exige l'unicité plutôt que de trancher.
        expect(trouvees).toHaveLength(1);
        expect(trouvees[0][1]).toBe(CSP);
    });
});
```

⚠️ **Vérifier la profondeur du `new URL`** : depuis
`plateforme/src/http/page/`, la racine du dépôt est **quatre** niveaux plus
haut. Si le test échoue sur `ENOENT`, c'est le compte qui est faux, pas le
fichier qui manque.

- [ ] **Step 2: Exécuter et vérifier l'échec**

```bash
cd /home/mallanic/Projects/Guacamole/plateforme && npx vitest run src/http/page/entetes-page.test.ts
```

Attendu : FAIL — module introuvable.

- [ ] **Step 3: Implémenter**

`plateforme/src/http/page/entetes-page.ts` :

```ts
// Les en-têtes que la plateforme pose sur ce qu'elle SERT comme page.
//
// 🔴 CE MODULE EXISTE PARCE QUE `../entetes.ts` NE POUVAIT PAS SERVIR ICI, et
// la raison est une inversion, pas un manque : son `Cache-Control: no-store`
// est INCONDITIONNEL, et il est là pour les réponses de `/auth/*`, qui portent
// des jetons en clair. Les ressources ont le besoin EXACTEMENT OPPOSÉ — des
// noms empreintés par Vite, cachables un an. Réutiliser `ENTETES_SECURITE` tel
// quel est le geste naturel, et c'est le défaut.
//
// 🔴 HSTS N'EST PAS ICI, ET C'EST DÉLIBÉRÉ. La ligne de partage avec le proxy
// devient : ce qui dépend du DOCUMENT suit le document ; ce qui dépend de TLS
// reste chez qui termine TLS. La plateforme est joignable en clair.

/// La politique de sécurité du contenu.
///
/// 🔴 ELLE EST RECOPIÉE DE `deploiement/nginx.conf`, ET `entetes-page.test.ts`
/// LIT LES DEUX ET LES COMPARE. Sans ce test, un durcissement appliqué d'un
/// seul côté livrerait deux montages aux sécurités différentes.
export const CSP =
    "default-src 'self'; connect-src 'self' wss: https:; img-src 'self' data: blob:; " +
    "media-src 'self' blob:; script-src 'self'; style-src 'self' 'unsafe-inline'; " +
    "font-src 'self'; frame-ancestors 'none'; base-uri 'self'; form-action 'self'";

export const ENTETES_DOCUMENT: Readonly<Record<string, string>> = Object.freeze({
    'X-Content-Type-Options': 'nosniff',
    'Cache-Control': 'no-store',
    'Content-Security-Policy': CSP,
    'Referrer-Policy': 'no-referrer',
    'X-Frame-Options': 'DENY',
});

export const ENTETES_RESSOURCE: Readonly<Record<string, string>> = Object.freeze({
    'X-Content-Type-Options': 'nosniff',
    'Cache-Control': 'public, max-age=31536000, immutable',
});
```

- [ ] **Step 4: Exécuter et vérifier le passage**

```bash
cd /home/mallanic/Projects/Guacamole/plateforme && npx vitest run src/http/page/entetes-page.test.ts
```

Attendu : PASS, 6 tests.

- [ ] **Step 5: Jouer la rouge de la garde anti-dérive**

Modifier **la CSP de `deploiement/nginx.conf`** (ajouter ` ; worker-src 'none'`),
relancer le test, **vérifier qu'il rougit sur la comparaison de chaînes** et
non sur `toHaveLength`. Restaurer depuis une copie nommée.

- [ ] **Step 6: Corriger le commentaire devenu FAUX dans `entetes.ts`**

`plateforme/src/http/entetes.ts` affirme que la CSP appartient au proxy « parce
qu'elle porte sur le DOCUMENT, **que la plateforme ne sert pas** ». Cette
phrase est désormais fausse. La corriger, en nommant ce qui l'a rendue fausse :

```
//   proxy OU plateforme (le HTML)   | Content-Security-Policy | porte sur le
//                                   |                        | DOCUMENT — et
//                                   |                        | depuis le lot
//                                   |                        | « page derrière
//                                   |                        | Pomerium » (22
//                                   |                        | août 2026), la
//                                   |                        | plateforme PEUT
//                                   |                        | le servir : voir
//                                   |                        | page/entetes-page.ts
```

⚠️ **Chercher les AUTRES endroits qui affirment la même chose**, par le SENS et
non par la formule :

```bash
cd /home/mallanic/Projects/Guacamole
grep -rn "ne sert pas\|aucun fichier statique\|sert la page" plateforme/src deploiement client/src --include='*.ts' --include='*.md' --include='*.conf'
```

Corriger chacune, et **les relire une par une après**.

- [ ] **Step 7: Commit**

```bash
cd /home/mallanic/Projects/Guacamole
git add plateforme/src/http/page/entetes-page.ts plateforme/src/http/page/entetes-page.test.ts plateforme/src/http/entetes.ts
git commit -m 'page(entetes) : trois jeux par nature de reponse, et un test qui interdit a la CSP de deriver'
```

---

### Task 5: Le routeur de page, chaîné en dernier

**Files:**
- Create: `plateforme/src/http/page/routes-page.ts`
- Create: `plateforme/src/http/page/routes-page.test.ts`
- Modify: `plateforme/src/http/chaine.ts` (chaîner **après** `servirSante`)
- Modify: `plateforme/src/http/serveur.ts` (passer `racinePage` dans `deps`)

**Interfaces:**
- Consumes: `resoudre` (tâche 2), `ENTETES_DOCUMENT` / `ENTETES_RESSOURCE` (tâche 4), `Config.racinePage` (tâche 3), `DependancesRoutage` (tâche 1)
- Produces: `export interface DependancesPage { racinePage?: string }`, `export async function servirPage(req: IncomingMessage, rep: ServerResponse, deps: DependancesPage): Promise<boolean>`

- [ ] **Step 1: Écrire le test qui échoue**

`plateforme/src/http/page/routes-page.test.ts` — il monte un vrai serveur, comme
`routes-sante.test.ts`. Reprendre le harnais de ce fichier (lecture préalable
obligatoire) — il fournit `CONFIG`, `base` et le cycle `afterEach`. Ajouter les
imports que le harnais neuf réclame :

```ts
import { mkdirSync, mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
```

puis :

```ts
// Une racine temporaire, bâtie à la main : le test ne dépend pas de
// `client/dist`, qui peut ne pas être bâti.
function racineJetable(): string {
    const racine = mkdtempSync(join(tmpdir(), 'page-'));
    mkdirSync(join(racine, 'assets'));
    writeFileSync(join(racine, 'index.html'), '<!doctype html><title>page</title>');
    writeFileSync(join(racine, 'assets', 'index-a1b2c3.js'), 'export const x = 1;\n');
    // Le PIÈGE que le critère ⑥ éprouve : un fichier homonyme d'une route.
    writeFileSync(join(racine, 'sante'), 'ceci ne doit JAMAIS etre servi');
    writeFileSync(join(racine, 'secret.env'), 'MOT_DE_PASSE=x');
    return racine;
}

// 🔴 LE TÉMOIN NÉGATIF. Sans lui, le 200 du test suivant ne prouve pas que
// PLATEFORME_PAGE a servi à quelque chose.
it("sans PLATEFORME_PAGE, GET / rend le 404 d'hier, mot pour mot", async () => {
    const service = await demarrerServeur({ ...CONFIG, racinePage: undefined }, base);
    const r = await fetch(`http://127.0.0.1:${service.port}/`);
    expect(r.status).toBe(404);
    expect(await r.text()).toBe('introuvable\n');
    await service.arreter();
});

it('avec PLATEFORME_PAGE, GET / rend index.html avec ses en-têtes de document', async () => {
    const service = await demarrerServeur({ ...CONFIG, racinePage: racineJetable() }, base);
    const r = await fetch(`http://127.0.0.1:${service.port}/`);
    expect(r.status).toBe(200);
    // Comparer les OCTETS, jamais le seul code 200.
    expect(await r.text()).toBe('<!doctype html><title>page</title>');
    expect(r.headers.get('content-security-policy')).toContain("default-src 'self'");
    expect(r.headers.get('cache-control')).toBe('no-store');
    await service.arreter();
});

it("un actif porte immutable, JAMAIS no-store", async () => {
    const service = await demarrerServeur({ ...CONFIG, racinePage: racineJetable() }, base);
    const r = await fetch(`http://127.0.0.1:${service.port}/assets/index-a1b2c3.js`);
    expect(r.headers.get('cache-control')).toBe('public, max-age=31536000, immutable');
    await service.arreter();
});

// 🔴 LA ROUGE DE L'ORDRE DE CHAÎNAGE. Un fichier nommé `sante` déposé dans la
// racine ne doit JAMAIS supplanter le routeur de santé — la panne la plus
// discrète possible, le service répondant 200 avec un corps plausible.
it("/sante reste servi par SON routeur, malgré un fichier homonyme", async () => {
    const service = await demarrerServeur({ ...CONFIG, racinePage: racineJetable() }, base);
    const r = await fetch(`http://127.0.0.1:${service.port}/sante`);
    expect(r.headers.get('content-type')).toContain('application/json');
    expect(await r.text()).not.toContain('JAMAIS');
    await service.arreter();
});

it("refuse un fichier hors de la liste MIME", async () => {
    const service = await demarrerServeur({ ...CONFIG, racinePage: racineJetable() }, base);
    const r = await fetch(`http://127.0.0.1:${service.port}/secret.env`);
    expect(r.status).toBe(404);
    await service.arreter();
});

// 🔴 HORS GET/HEAD, LE COMPORTEMENT EST CELUI D'HIER À L'OCTET PRÈS. Rendre
// 405 ferait qu'un POST sur un chemin mal orthographié — le repli SPA résout
// n'importe quoi — obtiendrait « méthode » au lieu du 404 qui le désigne.
it('un POST sur un chemin inconnu rend toujours 404, jamais 405', async () => {
    const service = await demarrerServeur({ ...CONFIG, racinePage: racineJetable() }, base);
    const r = await fetch(`http://127.0.0.1:${service.port}/aplication/x`, { method: 'POST' });
    expect(r.status).toBe(404);
    await service.arreter();
});

it('sert un HEAD sans corps', async () => {
    const service = await demarrerServeur({ ...CONFIG, racinePage: racineJetable() }, base);
    const r = await fetch(`http://127.0.0.1:${service.port}/`, { method: 'HEAD' });
    expect(r.status).toBe(200);
    expect(await r.text()).toBe('');
    await service.arreter();
});
```

- [ ] **Step 2: Exécuter et vérifier l'échec**

```bash
cd /home/mallanic/Projects/Guacamole/plateforme && npx vitest run src/http/page/routes-page.test.ts
```

Attendu : FAIL sur tout sauf le témoin négatif — **et le témoin négatif doit
passer dès maintenant**, puisqu'il décrit le comportement actuel. S'il échoue,
c'est le harnais qui est faux, pas le produit.

- [ ] **Step 3: Implémenter le routeur**

`plateforme/src/http/page/routes-page.ts` :

```ts
// Le servant de la page bâtie. Il n'applique que le verdict de `resolution.ts`
// et ne décide rien lui-même — toute la sécurité vit dans la règle pure.

import { createReadStream } from 'node:fs';
import { stat } from 'node:fs/promises';
import type { IncomingMessage, ServerResponse } from 'node:http';
import { resolve, sep } from 'node:path';
import { ENTETES_DOCUMENT, ENTETES_RESSOURCE } from './entetes-page';
import { resoudre } from './resolution';

export interface DependancesPage {
    /// Absente ⇒ le servant se retire, et le 404 générique reprend la main.
    racinePage?: string;
}

export async function servirPage(
    req: IncomingMessage,
    rep: ServerResponse,
    deps: DependancesPage,
): Promise<boolean> {
    if (deps.racinePage === undefined) return false;
    // 🔴 HORS GET/HEAD, ON SE RETIRE — jamais un 405. Voir le § 4.3 de la spec :
    // le repli SPA résout n'importe quel chemin, donc un 405 masquerait la
    // faute de frappe d'un appel d'API au lieu de la nommer.
    if (req.method !== 'GET' && req.method !== 'HEAD') return false;

    const chemin = new URL(req.url ?? '/', 'http://placeholder').pathname;
    const verdict = resoudre(chemin);
    // Un refus rend `false` : la chaîne se termine sur le 404 générique, plutôt
    // que d'inventer une SECONDE forme de 404 que rien ne testerait.
    if (!verdict.ok) return false;

    const racine = resolve(deps.racinePage);
    const fichier = resolve(racine, verdict.fichier);
    // Ceinture. La garantie est la règle pure ; ceci la redouble sur le
    // chemin RÉSOLU du système de fichiers, et ne coûte rien.
    if (fichier !== racine && !fichier.startsWith(racine + sep)) return false;

    try {
        if (!(await stat(fichier)).isFile()) return false;
    } catch {
        return false;
    }

    rep.writeHead(200, {
        'content-type': verdict.mime,
        ...(verdict.document ? ENTETES_DOCUMENT : ENTETES_RESSOURCE),
    });
    if (req.method === 'HEAD') {
        rep.end();
        return true;
    }
    createReadStream(fichier).pipe(rep);
    return true;
}
```

- [ ] **Step 4: Chaîner, EN DERNIER**

Dans `plateforme/src/http/chaine.ts` : ajouter
`import { servirPage, type DependancesPage } from './page/routes-page';`,
ajouter `& DependancesPage` à `DependancesRoutage`, et **remplacer** la ligne
finale `return servirSante(requete, reponse, deps);` par :

```ts
    if (await servirSante(requete, reponse, deps)) return true;
    // 🔴 LE SERVANT DE PAGE EST CHAÎNÉ EN DERNIER, ET C'EST LA GARANTIE, PAS
    // UNE COMMODITÉ. Chaîné en tête, un fichier nommé `sante` ou `vm` déposé
    // dans la racine volerait le chemin d'un routeur d'API, et le service
    // répondrait 200 avec un corps plausible. Chaîné ici, un routeur d'API a
    // déjà rendu `true` : il ne peut pas être supplanté.
    // `routes-page.test.ts` tient cette ligne par un test dédié.
    return servirPage(requete, reponse, deps);
```

Dans `plateforme/src/http/serveur.ts`, ajouter au littéral `deps` :

```ts
        // ⚠️ SEUL `servirPage` LE LIT. Absent ⇒ le servant se retire et le 404
        // générique reprend la main — le comportement d'avant le lot.
        racinePage: config.racinePage,
```

- [ ] **Step 5: Exécuter et vérifier le passage**

```bash
cd /home/mallanic/Projects/Guacamole/plateforme && npm run typecheck && npx vitest run src/http/page/ && npm run test:sqlite && npm run test:postgres
```

Attendu : PASS partout. 🔴 **Si un test préexistant rougit, ne pas l'ajuster
sans avoir lu POURQUOI** — le servant est censé n'en toucher aucun.

- [ ] **Step 6: Jouer la rouge de l'ordre de chaînage**

Déplacer temporairement `servirPage` **en tête** de `servirTout`, relancer.
Attendu : le test `/sante reste servi par SON routeur` rougit, **et lui**.
Restaurer depuis une copie nommée.

- [ ] **Step 7: Relever les tailles APRÈS**

```bash
cd /home/mallanic/Projects/Guacamole && wc -l plateforme/src/http/serveur.ts plateforme/src/http/chaine.ts plateforme/src/http/page/*.ts
```

Aucun ne doit dépasser 500. Consigner.

- [ ] **Step 8: Commit**

```bash
cd /home/mallanic/Projects/Guacamole
git add plateforme/src/http/page/routes-page.ts plateforme/src/http/page/routes-page.test.ts plateforme/src/http/chaine.ts plateforme/src/http/serveur.ts
git commit -m 'page(servant) : la plateforme sert la page batie, et le servant est chaine EN DERNIER'
```

---

### Task 6: La garde d'identité, et le refus de démarrer

**Files:**
- Modify: `plateforme/src/http/adresse-source.ts` (exporter `pairDeConfiance`)
- Modify: `plateforme/src/http/adresse-source.test.ts`
- Modify: `plateforme/src/http/routes-identite.ts`
- Modify: `plateforme/src/http/routes-identite.test.ts`
- Modify: `plateforme/src/config.ts` (le refus de démarrer)
- Modify: `plateforme/src/config.test.ts`
- Modify: `plateforme/src/http/entetes-routeurs.test.ts` (il appelle `/auth/moi`)

**Interfaces:**
- Produces: `export function pairDeConfiance(remote: string | undefined, confiance: ReadonlySet<string>): boolean`
- `DependancesIdentite` gagne `proxyDeConfiance: ReadonlySet<string>` — ⚠️ **`serveur.ts` le passe DÉJÀ** dans `deps` (pour `routes-auth`) : aucun câblage à ajouter.

- [ ] **Step 1: Écrire les tests qui échouent — le prédicat pur**

Dans `plateforme/src/http/adresse-source.test.ts` :

```ts
describe('pairDeConfiance', () => {
    it('accepte un pair déclaré', () => {
        expect(pairDeConfiance('10.0.0.1', new Set(['10.0.0.1']))).toBe(true);
    });

    // Le préfixe des adresses IPv4 mappées, comme `adresseSource` le fait déjà.
    it('normalise le préfixe ::ffff:', () => {
        expect(pairDeConfiance('::ffff:10.0.0.1', new Set(['10.0.0.1']))).toBe(true);
    });

    it('refuse un pair non déclaré', () => {
        expect(pairDeConfiance('10.0.0.2', new Set(['10.0.0.1']))).toBe(false);
    });

    // 🔴 UNE LISTE VIDE NE FAIT CONFIANCE À PERSONNE. Le contraire ferait de
    // l'absence de configuration une ouverture — l'inverse exact du défaut sûr.
    it('refuse tout le monde quand la liste est vide', () => {
        expect(pairDeConfiance('10.0.0.1', new Set())).toBe(false);
    });

    it('refuse une adresse absente', () => {
        expect(pairDeConfiance(undefined, new Set(['10.0.0.1']))).toBe(false);
    });
});
```

- [ ] **Step 2: Écrire les tests qui échouent — les deux bras de la garde**

Dans `plateforme/src/http/routes-identite.test.ts` : passer le `CONFIG` du
fichier à `proxyDeConfiance: new Set(['127.0.0.1'])` (les tests se connectent
en boucle locale), **et** ajouter le bras rouge :

```ts
// 🔴 LES DEUX BRAS, SINON LE ZÉRO N'EST PAS INTERPRÉTABLE : un 401 seul
// serait rendu par une route entièrement en panne.
it("REFUSE l'en-tête d'identité venu d'un pair non déclaré", async () => {
    const service = await demarrerServeur({ ...CONFIG, proxyDeConfiance: new Set(['10.9.9.9']) }, base);
    const r = await fetch(`http://127.0.0.1:${service.port}/auth/moi`, {
        headers: { [ENTETE_IDENTITE]: 'a@b.c' },
    });
    expect(r.status).toBe(401);
    expect((await r.json()).refus).toBe('pair-non-de-confiance');
    await service.arreter();
});
```

⚠️ **`entetes-routeurs.test.ts` appelle aussi `/auth/moi`** : y poser le même
`proxyDeConfiance`. Les recenser avant d'agir :
`grep -rln 'auth/moi' plateforme/src | grep test`.

- [ ] **Step 3: Écrire le test qui échoue — le refus de démarrer**

Dans `plateforme/src/config.test.ts` :

```ts
// 🔴 UN REFUS DE DÉMARRER SE LIT AVANT D'AGIR. Un 401 silencieux pour tout le
// monde se lirait APRÈS, sur un service qui répond, écoute et sert les dix
// autres routeurs — la panne la plus discrète possible.
it("LÈVE en mode pomerium sans PLATEFORME_PROXY_DE_CONFIANCE", () => {
    const env = { ...envValide(), PLATEFORME_AUTH: 'pomerium' };
    delete env.PLATEFORME_PROXY_DE_CONFIANCE;
    expect(() => lireConfig(env)).toThrow(/PLATEFORME_PROXY_DE_CONFIANCE/);
});

// ⚠️ LA GARDE EST LIÉE AU MODE, comme celle de PLATEFORME_HOTE : en
// `motdepasse`, le service s'authentifie lui-même et l'en-tête n'est lu par
// personne.
it("ne lève PAS en mode motdepasse", () => {
    const env = { ...envValide(), PLATEFORME_AUTH: 'motdepasse' };
    delete env.PLATEFORME_PROXY_DE_CONFIANCE;
    expect(() => lireConfig(env)).not.toThrow();
});
```

- [ ] **Step 4: Exécuter les trois et vérifier l'échec**

```bash
cd /home/mallanic/Projects/Guacamole/plateforme && npx vitest run src/http/adresse-source.test.ts src/http/routes-identite.test.ts src/config.test.ts
```

Attendu : FAIL. **Lire quelles assertions rougissent.**

- [ ] **Step 5: Implémenter le prédicat**

Dans `plateforme/src/http/adresse-source.ts` :

```ts
/// Le pair est-il l'un des proxys déclarés ?
///
/// 🔴 IL NE REGARDE QUE `remoteAddress`, JAMAIS `X-Forwarded-For` — à la
/// différence d'`adresseSource` juste au-dessus, et la différence est le point.
/// Honorer un en-tête fourni par l'attaquant pour décider si l'on croit
/// l'attaquant est circulaire. `adresseSource` a raison de le faire, elle :
/// elle attribue une requête à un client une fois le pair déjà jugé.
export function pairDeConfiance(
    remote: string | undefined,
    confiance: ReadonlySet<string>,
): boolean {
    if (remote === undefined || remote === '') return false;
    const pair = normaliser(remote);
    for (const declare of confiance) {
        if (normaliser(declare) === pair) return true;
    }
    return false;
}
```

- [ ] **Step 6: Implémenter la garde**

Dans `plateforme/src/http/routes-identite.ts` : ajouter
`proxyDeConfiance: ReadonlySet<string>;` à `DependancesIdentite`, importer
`pairDeConfiance`, et insérer **avant** `lireIdentitePomerium` :

```ts
    // 🔴 LA GARDE QUI FERME LE CONTOURNEMENT. Sans elle, `/auth/moi` rend un
    // jeton interne valide pour N'IMPORTE QUEL courriel posé dans un en-tête
    // qu'AUCUNE SIGNATURE NE VÉRIFIE : quiconque atteint le port — donc la VM
    // Windows, que le § 7.1 de la spec `auth-pomerium` place nommément dans ce
    // périmètre — s'authentifie sous l'identité de son choix.
    //
    // ⚠️ ELLE EST PLACÉE AVANT LA LECTURE DE L'EN-TÊTE, PAS APRÈS. Après, elle
    // serait correcte aussi — mais le service aurait déjà lu une identité qu'il
    // refuse, et un successeur pourrait déplacer la lecture sans voir que la
    // garde en dépendait.
    //
    // ⚠️ CE QU'ELLE NE PROMET PAS : que seul Pomerium porte cette adresse. Cela
    // reste à la charge de l'exploitant, comme la garde d'écoute de
    // `PLATEFORME_HOTE` le dit déjà d'elle-même.
    if (!pairDeConfiance(req.socket.remoteAddress, deps.proxyDeConfiance)) {
        repondre(rep, 401, { refus: 'pair-non-de-confiance' }, cors);
        return true;
    }
```

- [ ] **Step 7: Implémenter le refus de démarrer**

Dans `plateforme/src/config.ts`, **après** le calcul de `proxyDeConfiance` et d'`auth`,, avant l'objet rendu :

```ts
    // 🔴 TROISIÈME GARDE LIÉE AU MODE, après celle de `PLATEFORME_HOTE`. Même
    // raison : en `pomerium`, l'identité arrive dans un en-tête EN CLAIR
    // qu'aucune signature ne vérifie, et sans la liste des adresses autorisées
    // à le poser, l'en-tête est croyable par n'importe qui.
    //
    // ⚠️ POURQUOI UN REFUS DE DÉMARRER ET NON UN 401 : un refus se lit AVANT
    // d'agir et nomme la variable. Un 401 pour tout le monde se lirait APRÈS,
    // sur un service qui répond et sert les dix autres routeurs.
    if (auth === 'pomerium' && proxyDeConfiance.size === 0) {
        throw new Error(
            'PLATEFORME_PROXY_DE_CONFIANCE est obligatoire en mode pomerium : ' +
                "l'identité arrive dans un en-tête en clair qu'aucune signature ne vérifie, " +
                "et sans la liste des adresses autorisées à le poser, quiconque atteint le port obtient un jeton pour l'identité de son choix. " +
                "Poser l'adresse du proxy, ou PLATEFORME_AUTH=motdepasse.",
        );
    }
```

- [ ] **Step 7bis: Réparer les tests que la garde rend inconstructibles**

🔴 **LA GARDE REND UN ÉTAT INATTEIGNABLE, ET UNE DIZAINE DE TESTS LE
CONSTRUISENT.** « Mode `pomerium` + ensemble de confiance VIDE » n'existe plus,
or `config.test.ts` bâtit exactement cet état pour documenter ses défauts sûrs.
**C'est la conséquence recherchée de la garde, pas un dommage** — mais elle doit
être réparée en disant ce qu'elle signifie, jamais en desserrant une assertion.

Recenser d'abord, ne rien supposer :

```bash
cd /home/mallanic/Projects/Guacamole/plateforme && npx vitest run src/config.test.ts
```

**La règle de réparation, et elle dépend du SUJET du test :**

| Le test porte sur… | Réparation |
| --- | --- |
| le mode `pomerium` lui-même (« vaut pomerium par défaut », « retombe sur le défaut quand VIDE », les deux « LAISSE PASSER » de la garde d'écoute) | ajouter `PLATEFORME_PROXY_DE_CONFIANCE: '172.18.0.5'` à son environnement |
| autre chose que l'authentification (« lit les champs », les deux `PLATEFORME_ICONES`, `PLATEFORME_ORIGINE_CLIENT`) | idem — ajouter la variable, **et compléter le `toEqual` d'objet entier** avec `proxyDeConfiance: new Set(['172.18.0.5'])` |
| **le défaut VIDE de `proxyDeConfiance` lui-même** — les tests `(a)` et `(b)` | 🔴 poser `PLATEFORME_AUTH: 'motdepasse'`, **et l'écrire en commentaire** : *depuis la garde, l'ensemble vide n'est atteignable qu'en `motdepasse` — et c'est précisément ce que la garde signifie* |

🔴 **NE JAMAIS DESSERRER UNE ASSERTION POUR FAIRE PASSER UN TEST.** Le `toEqual`
d'objet entier est le seul garde du dépôt contre un champ de `Config` ajouté sans
qu'on décide de son défaut ; le transformer en `toMatchObject` le tuerait, et le
tuerait en silence.

⚠️ **Chaque commentaire de test qui explique un défaut désormais inatteignable
doit être corrigé, pas seulement son code** — sinon le test dit une chose et le
produit une autre.

- [ ] **Step 8: Exécuter la suite entière**

```bash
cd /home/mallanic/Projects/Guacamole/plateforme && npm run typecheck && npm run test:sqlite && npm run test:postgres
```

Attendu : PASS. ⚠️ **`index.test.ts`, `serveur.test.ts`, `canal.test.ts`,
`trace.test.ts` et `routes-sante.test.ts` posent `auth: 'pomerium'`** : ils
construisent un `Config` directement, donc le refus de démarrer ne les touche
pas ; mais s'ils appellent `/auth/moi`, la garde les touche. **Ne corriger que
ceux qui rougissent, après avoir lu pourquoi.**

- [ ] **Step 9: Jouer la rouge de la garde**

Neutraliser la garde (remplacer la condition par `if (false)`), relancer.
Attendu : le test `REFUSE l'en-tête d'identité venu d'un pair non déclaré`
rougit, **et lui**. Restaurer depuis une copie nommée.

- [ ] **Step 10: Commit**

```bash
cd /home/mallanic/Projects/Guacamole
git add plateforme/src/http/adresse-source.ts plateforme/src/http/adresse-source.test.ts plateforme/src/http/routes-identite.ts plateforme/src/http/routes-identite.test.ts plateforme/src/http/entetes-routeurs.test.ts plateforme/src/config.ts plateforme/src/config.test.ts
git commit -m 'securite(identite) : la porte de /auth/moi se ferme — l en-tete n est cru que du proxy, et le service refuse de demarrer sans lui'
```

---

### Task 7: La documentation, et les cinq affirmations que le lot rend fausses

**Pourquoi une tâche et non un supplément :** chacun de ces points est une
affirmation **aujourd'hui vraie** que le lot rend fausse. Les laisser serait le
« naufrage du 487 ».

**Files:**
- Modify: `CLAUDE.md` (tableau des variables serveur ; § Legs ouverts)
- Modify: `deploiement/README.md` (le montage Pomerium ; les deux variables)
- Modify: `deploiement/plateforme.env.exemple`
- Modify: `docs/superpowers/specs/2026-08-21-auth-pomerium-design.md` (§ 7, lever l'annotation)

- [ ] **Step 1: Énumérer les places AVANT d'écrire**

```bash
cd /home/mallanic/Projects/Guacamole
grep -n "PLATEFORME_PROXY_DE_CONFIANCE" CLAUDE.md deploiement/README.md deploiement/plateforme.env.exemple
grep -n "auth/moi" CLAUDE.md
grep -n "il reste à CONCEVOIR\|reste à concevoir" CLAUDE.md docs/superpowers/specs/2026-08-21-auth-pomerium-design.md
```

🔴 **Consigner la liste.** « Corrigé à sa place » est une affirmation de
complétude : énumérer par `grep -n` **avant**, et relire une par une **après**.

- [ ] **Step 2: Ajouter `PLATEFORME_PAGE` au tableau des variables de `CLAUDE.md`**

Une ligne, dans le tableau « Variables du SERVEUR », disant : facultative,
**aucun défaut**, absente ou vide ⇒ aucun servant et `GET /` rend `404`, et
**pourquoi un défaut serait un défaut de sécurité**.

- [ ] **Step 3: Requalifier la ligne `PLATEFORME_PROXY_DE_CONFIANCE`**

Elle dit « FACULTATIVE » sans condition. Elle devient : facultative en
`motdepasse`, **obligatoire en `pomerium`, où le service refuse de démarrer
sans elle** — et elle porte désormais **deux** rôles (le crédit de
`X-Forwarded-For`, et l'autorisation de poser l'en-tête d'identité).

- [ ] **Step 4: Requalifier le legs `/auth/moi` dans le § « Legs ouverts »**

Il dit « aucun frein sur `/auth/moi` » et annonce une table non bornée. Le
remplacer par le constat exact — **c'était un contournement d'authentification,
et un frein ne l'aurait pas fermé** — et le marquer **fermé** par ce lot, en
nommant la garde. ⚠️ **Barrer plutôt qu'effacer**, comme ce dépôt le fait
partout.

- [ ] **Step 5: Marquer le blocage ① du critère ⑦ comme LEVÉ**

Dans `CLAUDE.md` § « Ce que le chantier `auth-pomerium` laisse dû » **et** au
§ 7 de la spec `auth-pomerium`. 🔴 **Le blocage ② demeure et doit le dire
explicitement** : le flux OAuth exige un humain, et le critère ⑦ reste **non
joué**. Écrire « critère ⑦ levé » serait faux.

- [ ] **Step 6: Le runbook**

Dans `deploiement/README.md` : une section « Le montage Pomerium », qui pose
`PLATEFORME_AUTH=pomerium`, `PLATEFORME_HOTE=192.168.3.1`, `PLATEFORME_PAGE`
vers le `client/dist` bâti, et `PLATEFORME_PROXY_DE_CONFIANCE`.

🔴 **Y écrire la commande qui MESURE l'adresse source de Pomerium, plutôt qu'une
valeur** — Pomerium est en `network_mode: host` et c'est le noyau qui choisit :

```bash
# Sur une connexion RÉELLE de Pomerium, pas par déduction :
ss -tn state established '( dport = :8080 or sport = :8080 )'
```

⚠️ **Ne pas toucher à l'invariant ⑤** : le montage nginx reste ce qu'il est.
Ajouter que les deux montages sont **exclusifs**, et pourquoi.

- [ ] **Step 7: `plateforme.env.exemple`**

Ajouter `PLATEFORME_PAGE` (commentée, avec son avertissement d'absence de
défaut) et rendre `PLATEFORME_PROXY_DE_CONFIANCE` obligatoire dans le bloc
`pomerium`.

- [ ] **Step 8: Relire les places une par une**

Relancer les `grep` de l'étape 1 et **lire chaque occurrence**, pas seulement
compter. ⚠️ **Chercher aussi par le SENS** : une négation se dit de plusieurs
façons, et c'est celle qu'on n'a pas listée qui survit.

- [ ] **Step 9: Commit**

```bash
cd /home/mallanic/Projects/Guacamole
git add CLAUDE.md deploiement/README.md deploiement/plateforme.env.exemple docs/superpowers/specs/2026-08-21-auth-pomerium-design.md
git commit -m 'doc(page) : cinq affirmations que le lot rend fausses, et le legs /auth/moi REQUALIFIE avant d etre ferme'
```

---

### Task 8: La recette — jouer les huit critères, chaque rouge comprise

**Files:**
- Create: `docs/superpowers/plans/2026-08-22-page-derriere-pomerium-resultats.md`
- Create: `docs/superpowers/plans/journaux-page-pomerium/` (les journaux bruts, **suivis par git**)

🔴 **Une preuve ne doit jamais vivre dans un rapport gitignoré** — six constats
de revue ont disparu ainsi.

- [ ] **Step 1: La suite entière, depuis un shell propre**

```bash
cd /home/mallanic/Projects/Guacamole
env -u TURN_URL -u TURN_SECRET ./scripts/verify-all.sh 2>&1 | tee docs/superpowers/plans/journaux-page-pomerium/verify-all.log
```

⚠️ **`verify-all.sh` n'est PAS hermétique** : avec `TURN_URL`/`TURN_SECRET` dans
l'environnement, six tests de signaling échouent. ⚠️ Le script compte **dix**
étapes et l'exécution affiche **dix-huit** en-têtes `==>` : dire lequel on
annonce.

- [ ] **Step 2: Bâtir la page et démarrer le service en mode `pomerium`**

```bash
cd /home/mallanic/Projects/Guacamole/client && npm ci && npm run build
cd /home/mallanic/Projects/Guacamole/plateforme
PLATEFORME_HOTE=127.0.0.1 \
PLATEFORME_SECRET_JETON="$(openssl rand -hex 24)" \
PLATEFORME_AUTH=pomerium \
PLATEFORME_PROXY_DE_CONFIANCE=127.0.0.1 \
PLATEFORME_PAGE=../client/dist \
npm start 2>&1 | tee ../docs/superpowers/plans/journaux-page-pomerium/service-arme.log
```

🔴 **LE SECRET SE TIRE AU SORT, IL NE S'ÉCRIT PAS.** La première rédaction de ce
plan y posait un littéral, et **le garde de secrets du dépôt
(`plateforme/src/securite/secrets.test.ts`) a dénoncé le PLAN lui-même** — il
balaie `git ls-files` à la racine, `docs/` compris, précisément parce qu'un
journal de recette est ce qu'on verse le plus vite et ce qu'on relit le moins.
Ce dépôt avait déjà payé ce patron une fois, sur le plan d'`auth-pomerium`.

- [ ] **Step 3: Jouer les huit critères, et consigner chaque sortie**

| # | Commande | Attendu |
| --- | --- | --- |
| ① | le même démarrage **sans** `PLATEFORME_PAGE`, puis `curl -si localhost:8080/` | `404`, corps `introuvable` — **le témoin négatif** |
| ② | `curl -s localhost:8080/ \| diff - ../client/dist/index.html` | aucune différence — **les octets, pas le code 200** |
| ③ | `curl -sI localhost:8080/` | `content-security-policy`, `referrer-policy`, `x-frame-options` présents ; **`strict-transport-security` ABSENT** |
| ④ | `curl -sI localhost:8080/assets/<le fichier réel>` | `cache-control: public, max-age=31536000, immutable` |
| ⑤ | `curl -si 'localhost:8080/%2e%2e%2f%2e%2e%2fetc%2fpasswd'` | `404` |
| ⑥ | `touch ../client/dist/sante` puis `curl -si localhost:8080/sante` | du JSON de santé, **jamais le fichier** |
| ⑦ | `curl -si -H 'X-Pomerium-Claim-Email: a@b.c' localhost:8080/auth/moi` depuis `127.0.0.1`, puis **avec `PLATEFORME_PROXY_DE_CONFIANCE=10.9.9.9`** | `200` + jeton, puis `401 pair-non-de-confiance` — **les deux bras** |
| ⑧ | démarrer en `pomerium` **sans** `PLATEFORME_PROXY_DE_CONFIANCE` | le démarrage **lève**, et le message **nomme la variable** |

🔴 **Lire QUELLE assertion rougit à chaque rouge.** Une rouge qui rougit pour la
mauvaise raison ne prouve rien, et elle est indiscernable d'une bonne si l'on ne
lit que le code de sortie.

- [ ] **Step 4: Écrire le document de résultats**

Il doit porter, nommément : ce qui a été mesuré, **avec sa commande** ; ce qui
ne l'a pas été ; et un § « Ce que cette recette n'établit PAS » ouvrant sur :

🔴 **LE CRITÈRE ⑦ D'`auth-pomerium` N'EST TOUJOURS PAS JOUÉ.** Ce lot retire le
blocage ① — le défaut de conception. Le blocage ② demeure : le flux OAuth Google
exige un humain, et il relève du **lot 4** du cadrage.

- [ ] **Step 5: Ajouter la ligne à l'index des chantiers de `CLAUDE.md`**

Une ligne, dans § « Index des chantiers ». **Une ligne à l'index, le récit au
journal — jamais l'inverse.**

- [ ] **Step 6: Commit**

```bash
cd /home/mallanic/Projects/Guacamole
git add docs/superpowers/plans/2026-08-22-page-derriere-pomerium-resultats.md docs/superpowers/plans/journaux-page-pomerium CLAUDE.md
git commit -m 'recette(page) : les huit criteres joues, chaque rouge comprise — et le critere sept reste DU'
```

---

### Task 9: Revue transverse de fin de lot

🔴 **Une revue par tâche ne peut pas voir un défaut qui franchit une frontière
de tâche** — chacun est correct des deux côtés pris séparément. La revue
transverse a trouvé jusqu'à **vingt-sept** affirmations devenues fausses en une
branche.

- [ ] **Step 1: Relever les tailles APRÈS la dernière édition**

```bash
cd /home/mallanic/Projects/Guacamole
{ git ls-files; git ls-files --others --exclude-standard; } \
  | grep -vE 'node_modules|package-lock|Cargo.lock|/dist/|testdata/|^docs/|^CLAUDE.md' \
  | xargs wc -l 2>/dev/null | sort -rn | awk '$1>500'
```

Attendu : la même liste qu'avant le lot. **Et corriger le tableau de dette de
`CLAUDE.md` dans le même mouvement** si elle a changé.

- [ ] **Step 2: Relire le diff du lot contre les messages de commit**

```bash
git log --oneline main..HEAD
git diff main..HEAD --stat
```

🔴 **Relire les journaux CONTRE le message, jamais l'inverse.** Sept écarts sur
neuf, dans un audit passé, n'existaient que dans le message de commit.

- [ ] **Step 3: Chercher les affirmations devenues fausses, par le SENS**

```bash
grep -rn "ne sert pas\|aucun fichier statique\|404 introuvable\|facultative" \
  plateforme/src deploiement CLAUDE.md --include='*.ts' --include='*.md' --include='*.conf'
```

Lire chaque occurrence. ⚠️ **Une négation se dit de plusieurs façons**, et c'est
celle qu'on n'a pas listée qui survit.

- [ ] **Step 4: La suite entière, une dernière fois**

```bash
cd /home/mallanic/Projects/Guacamole && env -u TURN_URL -u TURN_SECRET ./scripts/verify-all.sh
```

- [ ] **Step 5: Commit des corrections de revue, s'il y en a**

```bash
git add <les fichiers NOMMÉS>
git commit -m 'revue(transverse) : <ce qui a ete trouve, nommement>'
```
