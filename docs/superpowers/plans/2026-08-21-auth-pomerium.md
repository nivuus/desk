# L'authentification derrière Pomerium — plan d'implémentation

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** L'identité des humains vient de Pomerium au lieu d'un mot de passe, le
jeton interne et l'agent Windows continuent de fonctionner sans changement de
protocole.

**Architecture:** Une variable de mode (`PLATEFORME_AUTH`) choisit entre deux
délivrances du **même** jeton interne : le mot de passe d'hier, ou le courriel
posé par le proxy. Ni `porteur.ts`, ni `garde.ts`, ni les cinq routeurs porteurs,
ni la poignée de main WebSocket ne changent. En parallèle, le relais de signaling
quitte la racine pour `/signal`, afin que Pomerium puisse garder la racine sans
couper l'agent.

**Tech Stack:** TypeScript / Node 24 (`plateforme/`, `client/` + Vite), Rust
(`agent/`), Vitest, nginx, Pomerium.

**Spec:** [`docs/superpowers/specs/2026-08-21-auth-pomerium-design.md`](../specs/2026-08-21-auth-pomerium-design.md)

---

## Global Constraints

- **500 lignes par fichier de code source** (`agent/src/`, `client/src/`,
  `plateforme/`, `proto/`, `scripts/`). `routes-auth.ts` pèse **397** lignes :
  la route neuve naît dans un fichier neuf. Relevé par la commande du
  `CLAUDE.md`, jamais recopié.
- **Aucune horloge lue dans un module** : `maintenant` est un paramètre, partout.
- **Un module de règle est PUR** : ni base, ni socket, ni `Date.now`.
- **Deux commandes de test, jamais une** :
  `cd client && npx vitest run` **et** `cd client && npx vitest run --dir ../proto`.
- **`tsc --noEmit` est une étape distincte** : Vitest transpile sans vérifier
  les types.
- **La double passe de base** : `npm run test:sqlite` **et** `npm run test:postgres`.
- **`env -u TURN_URL -u TURN_SECRET ./scripts/verify-all.sh`** — le script n'est
  pas hermétique.
- **Commits** : nommer les fichiers, jamais `git add -A` (arbre partagé).

---

## Deux écarts avec la spec, découverts au relevé et tranchés ici

🔴 **Ces deux points ne sont pas dans la spec, et ils la CORRIGENT. Les lire
avant la tâche 5.**

### Écart ① — `SIGNALING_URL` ne change PAS de valeur

La spec dit « l'agent emprunte `ws://192.168.3.1:8080/signal` ». **Pris à la
lettre, cela casse le canal d'enrôlement.** Relevé le 21 août 2026 :
`agent/src/plateforme.rs:187` compose l'URL du canal par
`format!("{}/agent", signaling_url.trim_end_matches('/'))`. Un `SIGNALING_URL`
portant `/signal` donnerait donc `ws://h:8080/signal/agent`, que la plateforme
compare **exactement** et refuse.

**Décision : `SIGNALING_URL` reste la BASE du service, et le suffixe `/signal`
se DÉRIVE**, exactement comme `/agent` se dérive déjà. Conséquences :
`scripts/run-agent.sh` ne change pas, `.env` ne change pas, et les recettes qui
posent `SIGNALING_URL` restent valables.

✅ **`base_http` n'est pas affectée** : `agent/src/apps/icone/televersement.rs:135`
fait `sans.split('/').next()`, donc elle retire déjà tout chemin. Vérifié par
lecture le 21 août 2026, et la tâche 5 le fige par un test.

### Écart ② — le coffre du client ne sait pas poser un jeton SEUL

`client/src/jeton.ts:57` expose `poser(coffre, paire)` où
`Paire = { acces: string; rafraichissement: string }` — **les deux sont
obligatoires**. Le mode `pomerium` ne délivre aucun jeton de rafraîchissement
(spec § 3). Il faut donc un `poserAcces` qui pose l'accès **et EFFACE** la clé
de rafraîchissement : un jeton de rafraîchissement laissé par un montage
antérieur survivrait au changement de mode et serait présenté à une route qui
rend désormais `404`.

---

## Structure des fichiers

| Fichier | Responsabilité | Tâche |
| --- | --- | --- |
| `plateforme/src/config.ts` *(modifié)* | lit `PLATEFORME_AUTH`, refuse l'écoute universelle | 1 |
| `plateforme/src/http/routes-identite.ts` *(créé)* | la règle de lecture de l'en-tête **et** la route `GET /auth/moi` | 2 |
| `plateforme/src/http/serveur.ts` *(modifié)* | chaîne le routeur neuf ; route la montée sur `/signal` | 2, 5 |
| `plateforme/src/http/routes-auth.ts` *(modifié)* | se retire en mode `pomerium` | 3 |
| `client/src/jeton.ts` *(modifié)* | `poserAcces` — la règle, testée | 4 |
| `client/src/connexion.ts` *(modifié)* | câblage : demande `/auth/moi` avant d'afficher le formulaire | 4 |
| `client/src/adresse-plateforme.ts` *(modifié)* | dérive `/signal` | 5 |
| `agent/src/signaling.rs` *(modifié)* | `url_du_relais` — la dérivation, testée sur l'hôte | 5 |
| `agent/src/demarrage.rs`, `agent/src/pont.rs` *(modifiés)* | appellent la dérivation | 5 |
| `deploiement/nginx.conf` *(modifié)* | la table `map` disparaît | 6 |
| `/opt/nivuus/Pomerium/config.yaml` *(modifié)* | trois routes | 7 |

---

### Task 1: `PLATEFORME_AUTH` et la garde d'écoute

**Files:**
- Modify: `plateforme/src/config.ts`
- Test: `plateforme/src/config.test.ts`

**Interfaces:**
- Consumes: rien.
- Produces: `Config.auth: 'pomerium' | 'motdepasse'` — lu par les tâches 2 et 3.

- [ ] **Step 1: Write the failing tests**

Ajouter à `plateforme/src/config.test.ts`. ⚠️ Relever d'abord comment les tests
existants construisent leur environnement minimal (`PLATEFORME_HOTE` et
`PLATEFORME_SECRET_JETON` sont obligatoires) et **réemployer ce montage**,
jamais en écrire un second.

```ts
describe('PLATEFORME_AUTH', () => {
    /// Le montage minimal — copié du haut de ce fichier, jamais réinventé.
    const base = {
        PLATEFORME_HOTE: '127.0.0.1',
        // ⚠️ `SECRET` EST LA CONSTANTE DÉJÀ DÉCLARÉE EN TÊTE DE CE FICHIER, ET
        // NON UN LITTÉRAL NEUF. Un littéral affecté à `PLATEFORME_SECRET_JETON`
        // fait ROUGIR `securite/secrets.test.ts`, qui balaie TOUS les fichiers
        // versionnés — y compris `docs/`. Mesuré le 21 août 2026 : la première
        // rédaction de ce plan écrivait le littéral, et le scanner a dénoncé le
        // plan lui-même, à deux endroits.
        PLATEFORME_SECRET_JETON: SECRET,
    };

    it('vaut pomerium par défaut', () => {
        expect(lireConfig({ ...base }).auth).toBe('pomerium');
    });

    it('accepte motdepasse', () => {
        expect(lireConfig({ ...base, PLATEFORME_AUTH: 'motdepasse' }).auth).toBe('motdepasse');
    });

    it('retombe sur le défaut quand la valeur est VIDE', () => {
        expect(lireConfig({ ...base, PLATEFORME_AUTH: '' }).auth).toBe('pomerium');
    });

    // 🔴 LA ROUGE DU CRITÈRE ④ : une coquille de casse ne doit PAS retomber
    // sur le défaut, sinon le mode mot de passe tournerait sous le nom du
    // mode Pomerium — et le second sens est une OUVERTURE.
    it('LÈVE sur une valeur inconnue, jamais un repli', () => {
        expect(() => lireConfig({ ...base, PLATEFORME_AUTH: 'Pomerium' })).toThrow(
            /PLATEFORME_AUTH/,
        );
    });
});

describe("la garde d'écoute du mode pomerium", () => {
    const base = { PLATEFORME_SECRET_JETON: SECRET };

    // 🔴 LA ROUGE DU CRITÈRE ⑤, et elle décrit le montage RÉEL du
    // 21 août 2026 : le service tourne aujourd'hui sur 0.0.0.0:8080.
    it('REFUSE 0.0.0.0 en mode pomerium', () => {
        expect(() => lireConfig({ ...base, PLATEFORME_HOTE: '0.0.0.0' })).toThrow(
            /écoute universelle/,
        );
    });

    it('REFUSE :: en mode pomerium', () => {
        expect(() => lireConfig({ ...base, PLATEFORME_HOTE: '::' })).toThrow(
            /écoute universelle/,
        );
    });

    // La garde est liée au MODE, pas universelle : sans ce test, on ne saurait
    // pas si elle refuse 0.0.0.0 ou si elle refuse toujours.
    it('LAISSE PASSER 0.0.0.0 en mode motdepasse', () => {
        const c = lireConfig({ ...base, PLATEFORME_HOTE: '0.0.0.0', PLATEFORME_AUTH: 'motdepasse' });
        expect(c.hote).toBe('0.0.0.0');
    });

    // Le déploiement Docker pose un NOM DE SERVICE, pas une adresse : la garde
    // doit le laisser passer, sinon elle casse le montage le plus sûr des trois.
    it('LAISSE PASSER un nom de service de réseau interne', () => {
        const c = lireConfig({ ...base, PLATEFORME_HOTE: 'plateforme' });
        expect(c.hote).toBe('plateforme');
    });

    // Le montage retenu par la spec § 7.1.
    it("LAISSE PASSER l'adresse du pont libvirt", () => {
        expect(lireConfig({ ...base, PLATEFORME_HOTE: '192.168.3.1' }).hote).toBe('192.168.3.1');
    });
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd plateforme && npx vitest run src/config.test.ts`
Expected: FAIL — `auth` n'existe pas sur `Config`, et aucun `throw` sur `0.0.0.0`.

- [ ] **Step 3: Write the implementation**

Dans `plateforme/src/config.ts`, ajouter au champ de l'interface `Config`,
après `base` :

```ts
    /// PLATEFORME_AUTH, défaut 'pomerium'. Une valeur inconnue LÈVE.
    ///
    /// ⚠️ CE N'EST PAS UN ARMEMENT, C'EST UN CHOIX DE MODE — la convention
    /// `=0 désarme` de `agent/` ne s'applique pas ici. Le précédent est
    /// `PLATEFORME_BASE` quinze lignes plus haut, et pour la même raison : un
    /// repli silencieux ferait tourner un mode sous le nom de l'autre, et
    /// l'un des deux sens est une OUVERTURE.
    auth: 'pomerium' | 'motdepasse';
```

Puis, à côté de `const BASES` :

```ts
const AUTHS = ['pomerium', 'motdepasse'] as const;

/// Les adresses qui font écouter le service sur TOUTES les interfaces.
///
/// 🔴 CE N'EST PAS « L'ÉCOUTE EST BORNÉE », ET LA DIFFÉRENCE EST ÉCRITE PLUTÔT
/// QUE MAQUILLÉE. Le contrôle qu'on aimerait — « ce doit être une adresse de
/// bouclage » — casserait le déploiement livré, qui pose `PLATEFORME_HOTE:
/// plateforme`, un nom de service Docker sans port publié, et qui est le
/// montage le plus sûr des trois. Ce qui est décidable est le refus de
/// l'écoute UNIVERSELLE ; que seul Pomerium atteigne le port reste à la charge
/// de l'exploitant, et c'est dit au § 9 de la spec.
const ECOUTES_UNIVERSELLES = new Set(['0.0.0.0', '::', '[::]', '*']);
```

Puis, dans `lireConfig`, **après** la lecture de `base` (et donc après celle de
`hote`, qui est la première) :

```ts
    const brutAuth = env.PLATEFORME_AUTH;
    const auth = (brutAuth === undefined || brutAuth === '' ? 'pomerium' : brutAuth) as Config['auth'];
    if (!(AUTHS as readonly string[]).includes(auth)) {
        throw new Error(
            `PLATEFORME_AUTH doit valoir ${AUTHS.join(' ou ')}, reçu : ${brutAuth}`,
        );
    }

    // 🔴 LIÉE AU MODE, ET NON UNIVERSELLE. En mode `motdepasse`, le service
    // s'authentifie lui-même et une écoute large ne le rend pas anonyme ; en
    // mode `pomerium`, l'identité arrive dans un en-tête EN CLAIR, et une
    // écoute universelle l'offre à quiconque atteint la machine.
    if (auth === 'pomerium' && ECOUTES_UNIVERSELLES.has(hote.trim())) {
        throw new Error(
            `PLATEFORME_HOTE=${hote} est une écoute universelle, refusée en mode ` +
                "pomerium : l'identité arrive dans un en-tête en clair, que seul le " +
                'proxy doit pouvoir poser. Nommer une adresse précise ' +
                '(192.168.3.1, 127.0.0.1) ou un nom de service de réseau interne.',
        );
    }
```

⚠️ **Ajouter `auth` à l'objet rendu par `lireConfig`**, sans quoi le champ
existe au type et vaut `undefined` à l'exécution — `tsc` l'attrape, mais
seulement si l'objet rendu est typé `Config`. Le vérifier.

🔴 **CE CHAMP CASSE TOUS LES LITTÉRAUX `Config` DES TESTS, ET C'EST TANT MIEUX :
`tsc` les nomme un par un.** Les énumérer avant de les corriger, plutôt que d'en
corriger un et croire l'affaire close — c'est le naufrage du « 487 » :

```bash
cd plateforme && grep -rn ": Config = {" src/
```

Chacun reçoit `auth: 'pomerium'`, **sauf** ceux qui éprouvent les routes de mot
de passe, qui reçoivent `auth: 'motdepasse'` (tâche 3).

- [ ] **Step 4: Run the tests**

Run: `cd plateforme && npx vitest run src/config.test.ts && npx tsc --noEmit`
Expected: PASS, et `tsc` muet.

- [ ] **Step 5: Play the red on the REAL service**

🔴 **La rouge du critère ⑤ ne se raisonne pas, elle se joue.** Le service qui
tourne aujourd'hui écoute sur `0.0.0.0:8080`.

```bash
cd plateforme && PLATEFORME_HOTE=0.0.0.0 PLATEFORME_SECRET_JETON="$(openssl rand -base64 48)" npm start
```

Expected: le service **refuse de démarrer** et nomme l'écoute universelle.
Recommencer avec `PLATEFORME_HOTE=127.0.0.1` : il démarre. **Consigner les deux
sorties**, l'une ne vaut rien sans l'autre.

- [ ] **Step 6: Commit**

```bash
git add plateforme/src/config.ts plateforme/src/config.test.ts
git commit -m "config(auth) : PLATEFORME_AUTH, et le refus de l ecoute universelle en mode pomerium"
```

---

### Task 2: `GET /auth/moi`

**Files:**
- Create: `plateforme/src/http/routes-identite.ts`
- Create: `plateforme/src/http/routes-identite.test.ts`
- Modify: `plateforme/src/http/serveur.ts`

**Interfaces:**
- Consumes: `Config.auth` (tâche 1) ; `creerUtilisateur(p, email, empreinteMdp, maintenant): Promise<string>` et `lireParEmail(p, email): Promise<LigneUtilisateur | undefined>` (`depot/utilisateur.ts`) ; `signer(sujet, secret, maintenant): string` (`identite/jeton.ts`).
- Produces: `servirIdentite(req, rep, deps): Promise<boolean>`, `lireIdentitePomerium(entetes): VerdictIdentite`, `CHEMIN_MOI = '/auth/moi'`, `ENTETE_IDENTITE = 'x-pomerium-claim-email'`, `MARQUEUR_SANS_MOT_DE_PASSE`.

- [ ] **Step 1: Write the failing test for the PURE rule**

Créer `plateforme/src/http/routes-identite.test.ts` :

```ts
import { describe, expect, it } from 'vitest';
import { lireIdentitePomerium, ENTETE_IDENTITE } from './routes-identite';

describe('lireIdentitePomerium', () => {
    it('rend le courriel', () => {
        const v = lireIdentitePomerium({ [ENTETE_IDENTITE]: 'a@b.c' });
        expect(v).toEqual({ ok: true, email: 'a@b.c' });
    });

    it('rogne les espaces', () => {
        const v = lireIdentitePomerium({ [ENTETE_IDENTITE]: '  a@b.c  ' });
        expect(v).toEqual({ ok: true, email: 'a@b.c' });
    });

    // 🔴 AUCUN REPLI SUR UN UTILISATEUR PAR DÉFAUT : une mauvaise
    // configuration du proxy doit être bruyante et refusante.
    it("refuse l'en-tête absent", () => {
        expect(lireIdentitePomerium({})).toEqual({ ok: false, motif: 'identite-absente' });
    });

    it("refuse l'en-tête vide", () => {
        expect(lireIdentitePomerium({ [ENTETE_IDENTITE]: '   ' })).toEqual({
            ok: false,
            motif: 'identite-absente',
        });
    });

    // Précédent littéral de `porteur.ts` : en choisir un serait prendre une
    // décision qu'un attaquant exploite dès que deux couches n'en prennent pas
    // la même.
    it("REFUSE un en-tête RÉPÉTÉ, jamais ne le désambiguïse", () => {
        expect(lireIdentitePomerium({ [ENTETE_IDENTITE]: ['a@b.c', 'mechant@x.y'] })).toEqual({
            ok: false,
            motif: 'identite-absente',
        });
    });
});
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cd plateforme && npx vitest run src/http/routes-identite.test.ts`
Expected: FAIL — le module n'existe pas.

- [ ] **Step 3: Write the module**

Créer `plateforme/src/http/routes-identite.ts` :

```ts
// `GET /auth/moi` : l'identité posée par Pomerium, échangée contre le jeton
// interne — le MÊME jeton que celui du mot de passe, à l'octet près.
//
// 🔴 POURQUOI CE FICHIER EXISTE PLUTÔT QU'UNE ROUTE DE PLUS DANS
// `routes-auth.ts` : ce dernier pesait 397 lignes au 21 août 2026, et la règle
// des 500 lignes veut qu'une addition substantielle s'accompagne d'une
// extraction. Ce sont par ailleurs deux DÉLIVRANCES différentes du même jeton,
// et elles ne partagent aucune règle : l'une hache un mot de passe, l'autre
// lit un en-tête.
//
// 🔴 LE JETON INTERNE N'EST PAS REMPLACÉ, ET C'EST LE CŒUR DE TOUT LE
// CHANTIER. Il authentifie la poignée de main du relais, que Pomerium ne peut
// pas garder — l'agent Windows n'a ni navigateur, ni cookie, ni session
// Google. Le retirer couperait l'agent.
//
// ⚠️ AUCUN JETON DE RAFRAÎCHISSEMENT N'EST DÉLIVRÉ, et ce n'est pas un oubli :
// à l'expiration, le client rappelle cette route, et le cookie Pomerium vit
// 8640 h. Tenir une chaîne rotative anti-rejeu dont plus personne n'a besoin
// serait du code vivant que rien n'exerce.

import type { IncomingMessage, ServerResponse } from 'node:http';
import type { Pilote } from '../base/pilote';
import { creerUtilisateur, lireParEmail } from '../depot/utilisateur';
import { signer } from '../identite/jeton';
import { entetesCors } from './cors';
import { ENTETES_SECURITE } from './entetes';

export const CHEMIN_MOI = '/auth/moi';

/// L'en-tête que Pomerium pose quand la route déclare
/// `pass_identity_headers: true`. **En minuscules** : Node normalise les noms
/// d'en-tête entrants, et une comparaison sur la casse d'origine ne
/// correspondrait jamais.
export const ENTETE_IDENTITE = 'x-pomerium-claim-email';

/// Ce qu'on écrit dans `empreinte_mdp`, qui est `NOT NULL` (`0001-socle.sql`).
///
/// ⚠️ JAMAIS UNE CHAÎNE VIDE : elle pourrait un jour croiser un vérificateur
/// permissif. Ce marqueur ne peut correspondre à aucun format que
/// `identite/mot-de-passe.ts` sait lire (`scrypt$N$r$p$sel$empreinte`).
export const MARQUEUR_SANS_MOT_DE_PASSE = 'pomerium$aucun-mot-de-passe';

export type VerdictIdentite =
    | { ok: true; email: string }
    | { ok: false; motif: 'identite-absente' };

export interface DependancesIdentite {
    base: Pilote;
    secretJeton: string;
    origineClient?: string;
    maintenant: () => number;
    auth: 'pomerium' | 'motdepasse';
}

/// 🔴 PURE : ni base, ni socket, ni horloge. C'est ce qui la rend éprouvable
/// sans monter de serveur, et c'est la convention de tout ce répertoire.
export function lireIdentitePomerium(
    entetes: Record<string, string | string[] | undefined>,
): VerdictIdentite {
    const brut = entetes[ENTETE_IDENTITE];
    // Un en-tête RÉPÉTÉ est refusé, jamais désambiguïsé — précédent littéral
    // de `porteur.ts`. Node rend un tableau quand il a vu plusieurs en-têtes
    // du même nom ; en choisir un serait prendre une décision qu'un attaquant
    // exploite dès que deux couches n'en prennent pas la même.
    if (Array.isArray(brut) || brut === undefined) {
        return { ok: false, motif: 'identite-absente' };
    }
    const email = brut.trim();
    if (email === '') return { ok: false, motif: 'identite-absente' };
    return { ok: true, email };
}

function repondre(
    rep: ServerResponse,
    code: number,
    corps: unknown,
    cors: Record<string, string> | undefined,
): void {
    rep.writeHead(code, {
        'content-type': 'application/json; charset=utf-8',
        ...ENTETES_SECURITE,
        ...(cors ?? {}),
    });
    rep.end(JSON.stringify(corps));
}

/// Rend `true` si la requête a été servie.
///
/// 🔴 EN MODE `motdepasse`, ELLE REND `false` — donc le 404 GÉNÉRIQUE du
/// serveur. C'est délibéré, et c'est ce qui porte le mode jusqu'au client : la
/// page est bâtie statiquement par Vite et ne peut lire aucune variable du
/// serveur, alors elle DEMANDE. Un `403` dirait « la route existe, tu n'y as
/// pas droit », ce qui inviterait à réessayer ; `404` dit la vérité.
export async function servirIdentite(
    req: IncomingMessage,
    rep: ServerResponse,
    deps: DependancesIdentite,
): Promise<boolean> {
    const chemin = new URL(req.url ?? '/', 'http://placeholder').pathname;
    // Comparaison EXACTE, jamais un `startsWith`.
    if (chemin !== CHEMIN_MOI) return false;
    if (deps.auth !== 'pomerium') return false;

    const cors = entetesCors(req.headers.origin, deps.origineClient);

    if (req.method === 'OPTIONS') {
        rep.writeHead(204, { ...ENTETES_SECURITE, ...(cors ?? {}) });
        rep.end();
        return true;
    }
    if (req.method !== 'GET') {
        repondre(rep, 405, { refus: 'methode' }, cors);
        return true;
    }

    const identite = lireIdentitePomerium(req.headers);
    if (!identite.ok) {
        repondre(rep, 401, { refus: identite.motif }, cors);
        return true;
    }

    const maintenant = deps.maintenant();
    const id = await identifiantDe(deps.base, identite.email, maintenant);
    repondre(rep, 200, { acces: signer(id, deps.secretJeton, maintenant) }, cors);
    return true;
}

/// L'identifiant du compte, créé s'il n'existe pas.
///
/// ⚠️ LA SECONDE LECTURE N'EST PAS DÉFENSIVE, ELLE FERME UNE COURSE RÉELLE :
/// deux requêtes simultanées d'un même utilisateur inconnu passeraient toutes
/// deux la première lecture, et la seconde insertion violerait l'index UNIQUE
/// du courriel (`0001-socle.sql`) — un `500` sur la toute première ouverture
/// de page. On relit alors, et on ne relève l'erreur que si le compte est
/// toujours introuvable, auquel cas elle dit autre chose qu'une course.
async function identifiantDe(base: Pilote, email: string, maintenant: number): Promise<string> {
    const existant = await lireParEmail(base, email);
    if (existant !== undefined) return existant.id;
    try {
        return await creerUtilisateur(base, email, MARQUEUR_SANS_MOT_DE_PASSE, maintenant);
    } catch (cause) {
        const rattrape = await lireParEmail(base, email);
        if (rattrape !== undefined) return rattrape.id;
        throw cause;
    }
}
```

- [ ] **Step 4: Run the pure-rule tests**

Run: `cd plateforme && npx vitest run src/http/routes-identite.test.ts`
Expected: PASS.

- [ ] **Step 5: Write the failing route tests**

Le montage est celui de `plateforme/src/http/routes-sante.test.ts` — `baseNeuve`,
`demarrerServeur`, `fetch` sur le port réel —, **réemployé et non réinventé** :
il éprouve le service ENTIER, donc aussi le chaînage de l'étape 6.

```ts
import { afterEach, describe, expect, it } from 'vitest';
import { baseNeuve } from '../base/harnais';
import type { Pilote } from '../base/pilote';
import { demarrerServeur, type ServicePlateforme } from './serveur';
import type { Config } from '../config';
import { mkdtempSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const SECRET = 'un-secret-de-plateforme-de-quarante-octets';

const CONFIG: Config = {
    hote: '127.0.0.1',
    port: 0,
    base: 'sqlite',
    urlBase: ':memory:',
    secretJeton: SECRET,
    auth: 'pomerium',
    proxyDeConfiance: new Set(),
    repertoireIcones: join(mkdtempSync(join(tmpdir(), 'moi-icones-')), 'icones'),
    repertoireTeleversements: join(mkdtempSync(join(tmpdir(), 'moi-tranches-')), 'televersements'),
};

let base: Pilote | undefined;
let service: ServicePlateforme | undefined;

afterEach(async () => {
    await service?.close();
    service = undefined;
    await base?.fermer();
    base = undefined;
});

/// Compte les comptes. C'est LE COMPTE qui dit si l'upsert a créé une fois ou
/// deux — jamais la seule absence d'erreur.
async function combienDeComptes(p: Pilote): Promise<number> {
    return (await p.interroger<{ id: string }>('SELECT id FROM utilisateur', [])).length;
}

describe('GET /auth/moi', () => {
    it('① compte inconnu ⇒ 200, et le compte est CRÉÉ', async () => {
        base = await baseNeuve('moi-inconnu');
        service = await demarrerServeur(CONFIG, base);
        expect(await combienDeComptes(base)).toBe(0);

        const r = await fetch(`http://127.0.0.1:${service.port}/auth/moi`, {
            headers: { 'x-pomerium-claim-email': 'a@b.c' },
        });

        expect(r.status).toBe(200);
        const corps = (await r.json()) as { acces?: unknown };
        expect(typeof corps.acces).toBe('string');
        expect(await combienDeComptes(base)).toBe(1);
    });

    it('② compte connu ⇒ 200, et AUCUN compte de plus', async () => {
        base = await baseNeuve('moi-connu');
        service = await demarrerServeur(CONFIG, base);
        const url = `http://127.0.0.1:${service.port}/auth/moi`;
        const en = { 'x-pomerium-claim-email': 'a@b.c' };

        await fetch(url, { headers: en });
        const r = await fetch(url, { headers: en });

        expect(r.status).toBe(200);
        expect(await combienDeComptes(base)).toBe(1);
    });

    // 🔴 LA ROUGE DU CRITÈRE ① : sans `pass_identity_headers` chez Pomerium,
    // le service REFUSE — il ne se replie sur aucun utilisateur par défaut.
    it('③ en-tête absent ⇒ 401 identite-absente', async () => {
        base = await baseNeuve('moi-absent');
        service = await demarrerServeur(CONFIG, base);

        const r = await fetch(`http://127.0.0.1:${service.port}/auth/moi`);

        expect(r.status).toBe(401);
        expect(await r.json()).toEqual({ refus: 'identite-absente' });
        expect(await combienDeComptes(base)).toBe(0);
    });

    // 🔴 LES ROUGES DES CRITÈRES ② ET ③ EN UN SEUL TEST, ET L'EN-TÊTE EST
    // PRÉSENT À DESSEIN : c'est ce qui prouve qu'un en-tête FORGÉ est ignoré
    // en mode `motdepasse`, et pas seulement que la route est absente.
    it('④ mode motdepasse ⇒ 404, en-tête forgé IGNORÉ', async () => {
        base = await baseNeuve('moi-motdepasse');
        service = await demarrerServeur({ ...CONFIG, auth: 'motdepasse' }, base);

        const r = await fetch(`http://127.0.0.1:${service.port}/auth/moi`, {
            headers: { 'x-pomerium-claim-email': 'forge@x.y' },
        });

        expect(r.status).toBe(404);
        expect(await combienDeComptes(base)).toBe(0);
    });
});
```

⚠️ **Le littéral `'x-pomerium-claim-email'` est écrit EN TOUTES LETTRES ici, et
non importé de `ENTETE_IDENTITE`** : un test qui importe la constante qu'il
éprouve ne peut pas voir un renommage. C'est le seul endroit du dépôt où le nom
d'en-tête est écrit deux fois, et c'est délibéré.

- [ ] **Step 6: Run them, then chain the router**

Run: `cd plateforme && npx vitest run src/http/routes-identite.test.ts`
Expected: les quatre PASS (le module est déjà écrit).

Puis, dans `plateforme/src/http/serveur.ts` : ajouter `auth: config.auth` à
l'objet `deps`, importer `servirIdentite`, et le chaîner **en tête** de
`servirTout`, avant `servirAuth` :

```ts
        if (await servirIdentite(requete, reponse, deps)) return true;
        if (await servirAuth(requete, reponse, deps)) return true;
```

🔴 **EN TÊTE, ET CE N'EST PAS INDIFFÉRENT** : `/auth/moi` et les deux chemins de
`servirAuth` sont disjoints aujourd'hui, mais les trois partagent le préfixe
`/auth/`. Le jour où l'un comparerait par préfixe, c'est cet ordre qui
trancherait — en silence.

- [ ] **Step 7: Prove the chaining is the ONLY thing that makes it live**

🔴 **LA ROUGE DU CHAÎNAGE**, patron déjà payé six fois dans ce fichier : sans la
ligne, les routes tombent dans le 404 générique — « la panne la plus discrète
possible : le service répond, écoute, et sert les autres ».

Retirer la ligne, relancer `npx vitest run src/http/serveur.test.ts`, **vérifier
qu'un test tombe et LEQUEL**, remettre la ligne. Consigner la sortie.

⚠️ Cela suppose qu'un test de `serveur.test.ts` couvre la route. **S'il n'y en a
pas, en écrire un** — sur le modèle du test dédié que ce fichier a déjà pour le
canal `/agent`.

- [ ] **Step 8: Full test pass and commit**

Run: `cd plateforme && npm run test:sqlite && npm run test:postgres && npx tsc --noEmit`
Expected: tout PASS.

```bash
git add plateforme/src/http/routes-identite.ts plateforme/src/http/routes-identite.test.ts plateforme/src/http/serveur.ts plateforme/src/http/serveur.test.ts
git commit -m "identite(auth/moi) : l en-tete de Pomerium delivre le MEME jeton interne"
```

---

### Task 3: Les deux routes de mot de passe se retirent en mode `pomerium`

**Files:**
- Modify: `plateforme/src/http/routes-auth.ts`
- Test: `plateforme/src/http/routes-auth.test.ts`

**Interfaces:**
- Consumes: `Config.auth` (tâche 1), passé dans `deps` par `serveur.ts` (tâche 2).
- Produces: rien de neuf.

- [ ] **Step 1: Write the failing tests**

Ajouter à `plateforme/src/http/routes-auth.test.ts` :

```ts
// 🔴 LA ROUGE DU CRITÈRE ③, DANS LES DEUX SENS. Un seul sens laisserait
// l'autre route vivante dans le mauvais mode : `/auth/connexion` ouverte
// derrière Pomerium serait une SECONDE porte, avec un mot de passe que plus
// personne ne tourne.
it('rend false — donc le 404 générique — en mode pomerium', async () => {
    // Réemployer le montage de requête/réponse déjà présent dans ce fichier.
    const servie = await servirAuth(requete('POST', '/auth/connexion'), reponse, {
        ...deps,
        auth: 'pomerium',
    });
    expect(servie).toBe(false);
});

it('rend false pour /auth/rafraichir en mode pomerium', async () => {
    const servie = await servirAuth(requete('POST', '/auth/rafraichir'), reponse, {
        ...deps,
        auth: 'pomerium',
    });
    expect(servie).toBe(false);
});

// Le témoin qui rend les deux précédents interprétables : en mode motdepasse,
// la route sert TOUJOURS. Sans lui, un `false` pourrait venir d'un routeur
// entièrement cassé.
it('sert TOUJOURS en mode motdepasse', async () => {
    const servie = await servirAuth(requete('POST', '/auth/connexion'), reponse, {
        ...deps,
        auth: 'motdepasse',
    });
    expect(servie).toBe(true);
});
```

⚠️ **Les tests existants de ce fichier construisent `deps` sans `auth`.** Leur
ajouter `auth: 'motdepasse'` — sinon `tsc` échoue, et surtout ils éprouveraient
un mode qu'ils ne nomment pas.

- [ ] **Step 2: Run to verify they fail**

Run: `cd plateforme && npx vitest run src/http/routes-auth.test.ts`
Expected: FAIL — `auth` n'existe pas sur `DependancesAuth`.

- [ ] **Step 3: Implement**

Dans `plateforme/src/http/routes-auth.ts`, ajouter à `DependancesAuth` :

```ts
    /// Le mode d'authentification (`config.ts`). En `pomerium`, ce routeur se
    /// RETIRE : voir le garde en tête de `servirAuth`.
    auth: 'pomerium' | 'motdepasse';
```

Puis, dans `servirAuth`, **immédiatement après** la ligne
`if (!CHEMINS.has(chemin)) return false;` :

```ts
    // 🔴 EN MODE `pomerium`, CES DEUX ROUTES N'EXISTENT PAS — `false`, donc le
    // 404 générique du serveur. Les laisser vivantes derrière le proxy serait
    // une SECONDE porte d'authentification, avec des mots de passe que plus
    // personne ne tourne et un frein que plus personne ne regarde.
    //
    // ⚠️ LE GARDE EST APRÈS LA COMPARAISON DE CHEMIN ET NON AVANT, à dessein :
    // un routeur qui rendrait `false` pour TOUT chemin en mode pomerium serait
    // indiscernable d'un routeur débranché, et la rouge du chaînage ne
    // pourrait plus rien dire.
    if (deps.auth !== 'motdepasse') return false;
```

- [ ] **Step 4: Run the tests**

Run: `cd plateforme && npx vitest run src/http/routes-auth.test.ts && npx tsc --noEmit`
Expected: PASS, `tsc` muet.

- [ ] **Step 5: Commit**

```bash
git add plateforme/src/http/routes-auth.ts plateforme/src/http/routes-auth.test.ts
git commit -m "auth(mode) : les deux routes de mot de passe se retirent derriere Pomerium"
```

---

### Task 4: Le client demande son identité avant d'afficher le formulaire

**Files:**
- Modify: `client/src/jeton.ts`
- Modify: `client/src/jeton.test.ts`
- Modify: `client/src/connexion.ts`

**Interfaces:**
- Consumes: `GET /auth/moi` → `200 {acces}` ou `404` (tâche 2).
- Produces: `poserAcces(coffre: Coffre, acces: string): void`.

- [ ] **Step 1: Write the failing test for `poserAcces`**

Ajouter à `client/src/jeton.test.ts`, en réemployant le coffre factice en
mémoire déjà présent en tête du fichier :

```ts
describe('poserAcces', () => {
    it("pose l'accès", () => {
        const c = coffreFactice();
        poserAcces(c, 'J');
        expect(c.getItem(CLE_ACCES)).toBe('J');
    });

    // 🔴 CE TEST EST LA RAISON D'ÊTRE DE LA FONCTION. Un jeton de
    // rafraîchissement laissé par un montage `motdepasse` antérieur survivrait
    // au changement de mode et serait présenté à une route qui rend désormais
    // 404 — une panne dont le symptôme serait une déconnexion inexpliquée dix
    // minutes après chaque ouverture de page.
    it('EFFACE le jeton de rafraîchissement laissé par un montage antérieur', () => {
        const c = coffreFactice();
        c.setItem(CLE_RAFRAICHISSEMENT, 'vieux');
        poserAcces(c, 'J');
        expect(c.getItem(CLE_RAFRAICHISSEMENT)).toBeNull();
    });
});
```

⚠️ **Relever ce que le coffre factice rend pour une clé absente** — `null` ou
`undefined` — et asserter **cette** valeur, pas celle qu'on suppose.

- [ ] **Step 2: Run to verify it fails**

Run: `cd client && npx vitest run src/jeton.test.ts`
Expected: FAIL — `poserAcces` n'est pas exportée.

- [ ] **Step 3: Implement**

Dans `client/src/jeton.ts`, sous `poser` :

```ts
/// Pose le seul jeton d'accès, et EFFACE celui de rafraîchissement.
///
/// 🔴 L'EFFACEMENT EST LE POINT, PAS UN NETTOYAGE DE CONFORT. Le mode
/// `pomerium` ne délivre aucun jeton de rafraîchissement — à l'expiration, le
/// client rappelle `GET /auth/moi`, et le cookie du proxy vit 8640 h. Un jeton
/// laissé par un montage `motdepasse` antérieur serait présenté à une route
/// qui rend désormais 404, et le symptôme serait une déconnexion inexpliquée
/// dix minutes après chaque ouverture de page.
export function poserAcces(coffre: Coffre, acces: string): void {
    coffre.setItem(CLE_ACCES, acces);
    coffre.removeItem(CLE_RAFRAICHISSEMENT);
}
```

⚠️ **Vérifier que `Coffre` déclare `removeItem`.** S'il ne le déclare pas,
l'ajouter à l'interface — et relever que `vider` l'emploie déjà, auquel cas il
est là.

- [ ] **Step 4: Run the tests**

Run: `cd client && npx vitest run src/jeton.test.ts`
Expected: PASS.

- [ ] **Step 5: Extract the post-token flow in `connexion.ts`**

Le corps du `submit` fait deux choses : obtenir un jeton, puis chercher la
session. Le second devient une fonction, appelée par les **deux** chemins.

⚠️ **Extraction, jamais recopie** — le dépôt a payé douze fois le contraire. Le
corps déplacé est celui qui va de `afficher('recherche de votre machine…', …)`
jusqu'à `window.location.href = suite;` inclus, **verbatim**.

**Ancrer l'extraction par un relevé, jamais par un numéro de ligne recopié :**

```bash
grep -n "recherche de votre machine\|window.location.href = suite" client/src/connexion.ts
```

Les deux numéros rendus bornent le corps à déplacer, inclus. Il devient :

```ts
/// Ce qui suit l'obtention d'un jeton, quel que soit le chemin qui l'a obtenu.
///
/// ⚠️ CE N'EST PAS UNE RÈGLE, C'EST DU CÂBLAGE — au sens du critère posé en
/// tête de ce fichier : ces branches ne font que router une décision prise par
/// `routes-session.ts` et couverte par SES tests. La clause reste donc
/// resserrée, pas assouplie.
///
/// ⚠️ CORPS DÉPLACÉ VERBATIM. Trois substitutions, et TROIS SEULEMENT :
///   ① `corps.acces` devient le paramètre `acces` ;
///   ② les `return` de sortie anticipée restent des `return` — la fonction
///      rend `void`, donc leur sens ne change pas ;
///   ③ le `catch` et le `finally` du `submit` RESTENT chez l'appelant : les
///      déplacer ici ferait réactiver `bouton.disabled = false` sur le chemin
///      Pomerium, où aucun bouton n'a jamais été désactivé.
async function chercherLaSession(acces: string): Promise<void> {
    // …
}
```

🔴 **UNE EXTRACTION N'EST JAMAIS RIGOUREUSEMENT VERBATIM** — elle laisse ses
imports derrière elle, déplace les visibilités et casse les déictiques.
Après l'extraction : `npx tsc --noEmit` **et** relire les commentaires déplacés,
dont plusieurs disent « ci-dessous » et « ce fichier ».

- [ ] **Step 6: Add the Pomerium path at load**

En tête de module, après la déclaration de `suite` :

```ts
/// Demande l'identité au service AVANT de montrer le formulaire.
///
/// 🔴 C'EST LE 404 QUI PORTE LE MODE JUSQU'ICI, et c'est pourquoi cette page
/// n'a aucune variable de mode à connaître. Elle est bâtie statiquement par
/// Vite et ne peut lire aucune configuration du serveur : elle DEMANDE. Un
/// 404 signifie « ce montage authentifie par mot de passe » ; un 200, « le
/// proxy m'a déjà identifié ».
///
/// ⚠️ TOUT ÉCHEC RETOMBE SUR LE FORMULAIRE, y compris un échec réseau. C'est
/// le repli le moins surprenant : l'utilisateur voit un écran sur lequel il
/// peut agir, plutôt qu'une page vide dont rien ne dit ce qu'elle attend.
async function tenterPomerium(): Promise<boolean> {
    try {
        const reponse = await fetch(`${plateformeUrl}/auth/moi`);
        if (!reponse.ok) return false;
        const corps = await reponse.json().catch(() => undefined);
        if (typeof corps?.acces !== 'string' || corps.acces === '') return false;
        poserAcces(window.localStorage, corps.acces);
        await chercherLaSession(corps.acces);
        return true;
    } catch {
        return false;
    }
}

// ⚠️ Le formulaire est CACHÉ le temps de la tentative, puis remontré si elle
// échoue : l'afficher d'abord ferait clignoter un écran de connexion sur un
// montage qui n'en demande aucun.
formulaire.hidden = true;
afficher('identification…', 'neutre');
void tenterPomerium().then((abouti) => {
    if (abouti) return;
    formulaire.hidden = false;
    afficher('', 'neutre');
});
```

⚠️ **Relever le nom réel de la variable du formulaire** dans le fichier
(`formulaire` d'après la ligne `formulaire.addEventListener`), et **vérifier que
`connexion.html` n'a pas déjà un attribut qui entre en conflit avec `hidden`**.

- [ ] **Step 7: Typecheck and the two test commands**

Run:
```bash
cd client && npx tsc --noEmit && npx vitest run && npx vitest run --dir ../proto
```
Expected: tout PASS. ⚠️ **Deux commandes vitest, jamais une** : la racine Vitest
est `client/`, et `proto/ts/` n'est pas couvert par la première.

- [ ] **Step 8: Commit**

```bash
git add client/src/jeton.ts client/src/jeton.test.ts client/src/connexion.ts
git commit -m "client(auth) : la page demande son identite avant de montrer un formulaire"
```

---

### Task 5: Le relais quitte `/` pour `/signal`

🔴 **UNE SEULE TÂCHE POUR TROIS COMPOSANTS, ET C'EST DÉLIBÉRÉ.** Un relecteur ne
peut pas approuver une moitié : une plateforme déplacée sans agent déplacé donne
un bureau qui n'arrive jamais, et l'inverse aussi. Le déplacement est atomique.

**Files:**
- Modify: `plateforme/src/http/serveur.ts`
- Test: `plateforme/src/http/serveur.test.ts`
- Modify: `client/src/adresse-plateforme.ts`
- Test: `client/src/adresse-plateforme.test.ts`
- Modify: `agent/src/signaling.rs`
- Modify: `agent/src/demarrage.rs:106`, `agent/src/pont.rs:85`

**Interfaces:**
- Consumes: rien des tâches précédentes.
- Produces: `url_du_relais(signaling_url: &str) -> String` (Rust) ; `adresseSignaling` rend désormais une URL portant `/signal`.

- [ ] **Step 1: Write the failing test — plateforme**

Dans `plateforme/src/http/serveur.test.ts`, en réemployant le montage de montée
WebSocket déjà présent pour `/agent` :

`serveur.test.ts` porte déjà un `tenter(url)` qui rend `'ouvert'` ou `'ferme'`,
et un `servir(nom)` qui monte le service. **Les réemployer.**

```ts
it('🔴 accepte la montée WebSocket sur /signal — le relais a déménagé', async () => {
    service = await servir('http-signal');
    await expect(tenter(`ws://127.0.0.1:${service.port}/signal`)).resolves.toBe('ouvert');
});

// 🔴 LA ROUGE DU CRITÈRE ⑥, ET C'EST ELLE QUI FAIT LA DIFFÉRENCE ENTRE UN
// DÉPLACEMENT ET UNE ADDITION. Sans ce test, la racine pourrait rester
// ouverte : Pomerium croirait garder le relais, et le relais répondrait à
// côté de sa garde.
it('🔴 REFUSE désormais la montée sur la RACINE', async () => {
    service = await servir('http-racine-fermee');
    await expect(tenter(`ws://127.0.0.1:${service.port}/`)).resolves.toBe('ferme');
});

// Le témoin qui rend les deux précédents interprétables : `/agent` n'a PAS
// bougé. Sans lui, un `'ferme'` pourrait venir d'un routage entièrement cassé.
it('/agent est INCHANGÉ', async () => {
    service = await servir('http-agent-inchange');
    await expect(tenter(`ws://127.0.0.1:${service.port}/agent`)).resolves.toBe('ouvert');
});
```

⚠️ **Le test d'appariement de bout en bout de ce fichier ouvre deux sockets sur
`/`** (« Un pair 'agent' et un pair 'client' sur '/' »). Il devient faux dans ce
commit : **le corriger, et corriger son commentaire**, plutôt que le laisser
rougir.

- [ ] **Step 2: Write the failing test — agent (pure, sur l'hôte)**

Dans `agent/src/signaling.rs`, sous un `#[cfg(test)] mod` :

```rust
#[test]
fn le_relais_derive_du_signaling() {
    assert_eq!(url_du_relais("ws://h:8080"), "ws://h:8080/signal");
}

/// ⚠️ MÊME RAISON QUE `url_du_canal` : `ws://h:8080/` suivi de `/signal`
/// donnerait `//signal`, et la plateforme compare le chemin EXACTEMENT.
#[test]
fn la_barre_finale_ne_double_pas() {
    assert_eq!(url_du_relais("ws://h:8080/"), "ws://h:8080/signal");
}

/// 🔴 LE TEST QUI FIGE L'ÉCART ① DU PLAN : `SIGNALING_URL` reste la BASE, donc
/// le canal `/agent` continue de se dériver juste. Sans lui, quelqu'un
/// pourrait un jour mettre `/signal` dans la variable et casser l'enrôlement
/// sans qu'aucun test ne bronche.
#[test]
fn le_canal_agent_n_est_pas_affecte() {
    assert_eq!(crate::plateforme::url_du_canal("ws://h:8080"), "ws://h:8080/agent");
}
```

- [ ] **Step 3: Run both to verify they fail**

Run: `cd plateforme && npx vitest run src/http/serveur.test.ts`
Expected: FAIL — la montée sur `/signal` rend 404.

Run: `cargo test -p agent url_du_relais`
Expected: FAIL — la fonction n'existe pas.

- [ ] **Step 4: Implement — plateforme**

Dans `plateforme/src/http/serveur.ts`, à côté de `CHEMIN_AGENT` :

```ts
/// Le chemin du relais de signaling. ⚠️ Il est comparé EXACTEMENT.
///
/// 🔴 IL A QUITTÉ LA RACINE LE 21 AOÛT 2026, ET LA RAISON EST LE PROXY, PAS LE
/// GOÛT. Sur la racine, la page et la montée WebSocket se disputaient le même
/// chemin, départagées par le seul en-tête `Upgrade` — un critère sur lequel
/// Pomerium ne sait pas router. Le relais servant DEUX pairs de natures
/// différentes (le navigateur, que le proxy authentifie ; l'agent Windows, qui
/// n'a ni navigateur ni cookie), il fallait un chemin distinct pour que le
/// proxy puisse garder la racine sans couper l'agent.
///
/// ✅ CE DÉPLACEMENT SOLDE UN LEGS DÉCLARÉ de `deploiement/nginx.conf`, que le
/// sous-bloc P5 avait écarté faute d'avoir le droit de toucher `agent/`.
const CHEMIN_SIGNAL = '/signal';
```

Puis, dans le `http.on('upgrade', …)` :

```ts
        const wss =
            chemin === CHEMIN_SIGNAL ? wssRacine : chemin === CHEMIN_AGENT ? wssAgent : undefined;
```

⚠️ **Corriger aussi le commentaire de tête du fichier**, qui écrit « Tout chemin
qui n'est ni `/` ni `/agent` reçoit un `404` » et « la page de session, la
page-shell et l'agent visent tous les trois le chemin racine ». **Les deux
phrases deviennent fausses dans ce commit** — c'est le naufrage du « 487 » :
chercher l'affirmation, pas la corriger là où on nous l'a montrée.

```bash
grep -n "racine\|'/'\|/agent" plateforme/src/http/serveur.ts
```

- [ ] **Step 5: Implement — client**

Dans `client/src/adresse-plateforme.ts` :

```ts
/// Le chemin du relais sur le service. Voir `plateforme/src/http/serveur.ts`.
const CHEMIN_SIGNAL = '/signal';

export function adresseSignaling(
    emplacement: Emplacement,
    explicite?: string | null,
): string {
    // ⚠️ UN EXPLICITE RESTE EXPLICITE, ET NE REÇOIT PAS LE SUFFIXE. C'est le
    // contrat de ce paramètre depuis P5 : il remplace l'adresse ENTIÈRE, pas
    // son autorité. Y ajouter `/signal` ferait `/signal/signal` chez qui l'a
    // déjà écrit, et personne ne saurait lequel des deux comportements est le
    // bon. CONSÉQUENCE À CONNAÎTRE : une recette qui pose `?signaling=` doit
    // désormais écrire le chemin. Relevé le 21 août 2026 — aucune n'en pose
    // (`grep -rn 'signaling=' client/*.mjs client/recette/*.mjs scripts/*.sh`).
    if (explicite) return explicite;
    const schema = emplacement.protocol === 'https:' ? 'wss' : 'ws';
    return `${schema}://${emplacement.host}${CHEMIN_SIGNAL}`;
}
```

⚠️ **Les tests existants de `adresse-plateforme.test.ts` assertent l'ancienne
valeur.** Les mettre à jour — et **relire chacun**, plutôt que substituer en
masse.

- [ ] **Step 6: Implement — agent**

Dans `agent/src/signaling.rs`, au-dessus de `run_signaling` :

```rust
/// Compose l'URL du relais à partir de celle du service.
///
/// 🔴 `SIGNALING_URL` EST LA BASE DU SERVICE, JAMAIS L'URL DU RELAIS, ET C'EST
/// CE QUI REND `scripts/run-agent.sh` INCHANGÉ. La même variable sert à
/// dériver le canal d'enrôlement (`plateforme::url_du_canal`, qui ajoute
/// `/agent`) et l'adresse HTTP du téléversement d'icônes
/// (`apps::icone::televersement::base_http`, qui retire tout chemin). Y écrire
/// `/signal` casserait le premier : `ws://h:8080/signal/agent` n'est pas
/// `/agent`, et la plateforme compare le chemin EXACTEMENT.
///
/// Le `trim_end_matches` a la même raison que chez son jumeau : `ws://h:8080/`
/// suivi de `/signal` donnerait `//signal`, refusé de la même façon.
pub fn url_du_relais(signaling_url: &str) -> String {
    format!("{}/signal", signaling_url.trim_end_matches('/'))
}
```

Puis les deux appelants :

- `agent/src/demarrage.rs:106` — `&config.signaling_url` devient
  `&crate::signaling::url_du_relais(&config.signaling_url)` ;
- `agent/src/pont.rs:85` — même substitution sur son argument d'URL.

⚠️ **`grep -rn "run_signaling(" agent/src` pour n'en oublier aucun**, plutôt que
se fier à ces deux numéros de ligne : ils auront dérivé.

- [ ] **Step 7: Run every check**

```bash
cd plateforme && npx vitest run src/http/serveur.test.ts
cd ../client && npx vitest run && npx tsc --noEmit
cd .. && cargo test --workspace
cargo check --target x86_64-pc-windows-gnu
```
Expected: tout PASS. ⚠️ `cargo check` : **vérifier la NATURE des avertissements,
jamais leur nombre** — il dérive avec la fraîcheur du build.

- [ ] **Step 8: Commit**

```bash
git add plateforme/src/http/serveur.ts plateforme/src/http/serveur.test.ts \
        client/src/adresse-plateforme.ts client/src/adresse-plateforme.test.ts \
        agent/src/signaling.rs agent/src/demarrage.rs agent/src/pont.rs
git commit -m "signal(chemin) : le relais quitte la racine — et SIGNALING_URL reste la BASE, sinon le canal /agent casse"
```

---

### Task 6: `nginx.conf` — la table `map` disparaît

**Files:**
- Modify: `deploiement/nginx.conf`

**Interfaces:**
- Consumes: le chemin `/signal` (tâche 5).
- Produces: rien.

- [ ] **Step 1: Read the whole file first**

⚠️ **Ce fichier n'est couvert par AUCUN test** — c'est une configuration nginx.
Le seul juge est `nginx -t` **et** un démarrage réel : le fichier documente
lui-même qu'une configuration déclarée « ok » par `nginx -t` peut refuser de
démarrer (`daemon off` en double).

```bash
sed -n '1,120p' deploiement/nginx.conf
```

- [ ] **Step 2: Remove the map and the `if`, add a plain location**

Retirer la table `map $http_upgrade $vers_plateforme`, le `if ($vers_plateforme)`
du `location /`, et le `location @relais` nommé qu'il servait. Les remplacer par
un `location` ordinaire, sur le modèle de `location = /agent` juste en dessous :

```nginx
        # Le relais de signaling. Depuis le 21 août 2026 il vit sur `/signal`
        # et non plus sur la racine, ce qui retire à ce fichier la table
        # `map $http_upgrade` et le `location` nommé qu'elle servait : la page
        # et la montée ne se disputent plus le même chemin.
        location = /signal {
            proxy_pass http://plateforme;
            proxy_http_version 1.1;
            proxy_set_header Upgrade    $http_upgrade;
            proxy_set_header Connection $connexion_amont;
            proxy_set_header Host       $host;
        }
```

⚠️ **Recopier les `proxy_set_header` réellement présents dans `location = /agent`**,
pas cette liste : elle est un modèle, pas un relevé. Et **garder la table
`map $http_upgrade $connexion_amont`**, qui sert les DEUX montées et qui n'est
pas celle qu'on retire.

- [ ] **Step 3: Verify the syntax**

```bash
docker run --rm -v "$PWD/deploiement/nginx.conf:/etc/nginx/nginx.conf:ro" nginx:alpine nginx -t
```

🔴 **CETTE COMMANDE, TELLE QUELLE, ÉCHOUE — ET LA PREMIÈRE RÉDACTION DE CE PLAN
NE LE DISAIT PAS** (mesuré le 21 août 2026) : `cannot load certificate
.../fullchain.pem`. `deploiement/tls/` est **gitignoré**, donc absent de tout
checkout neuf, et `nginx -t` charge réellement les certificats.

Monter des certificats **auto-signés jetables** dans le même conteneur jetable
lève l'obstacle. ⚠️ Les générer hors de l'arbre versionné, et vérifier par
`git status --porcelain` qu'ils n'y ont rien laissé.

Expected, une fois les certificats fournis : `syntax is ok` / `test is successful`.

⚠️ **`nginx -t` ne prouve pas le démarrage**, et ce fichier le dit. Le contrôle
qui vaut est le § 8 de la recette (tâche 8).

- [ ] **Step 4: Commit**

```bash
git add deploiement/nginx.conf
git commit -m "deploiement(nginx) : le legs declare est solde — la table map disparait avec la racine partagee"
```

---

### Task 7: Pomerium

🔴 **CETTE TÂCHE TOUCHE UN SYSTÈME TIERS EN PRODUCTION.** `config.yaml` fronte
**sept services** — Home Assistant, Grocy, MediaManager, Sunshine, deux services
`personas`, le site racine. Une régression sur l'un d'eux serait un dommage
causé par un chantier qui ne le concerne pas.

**Files:**
- Modify: `/opt/nivuus/Pomerium/config.yaml`

- [ ] **Step 1: Relever l'état AVANT, et le garder**

```bash
cp -a /opt/nivuus/Pomerium/config.yaml /opt/nivuus/Pomerium/config.yaml.bak-20260821
grep -c '^  - from:' /opt/nivuus/Pomerium/config.yaml
for h in media.allanic.me allanic.me home.allanic.me grocy.allanic.me personas.allanic.me game.allanic.me; do
    printf '%s ' "$h"; curl -s -o /dev/null -w '%{http_code}\n' -m 10 "https://$h/"
done | tee /tmp/pomerium-avant.txt
```

🔴 **CE RELEVÉ EST LE TÉMOIN, ET SANS LUI L'ÉTAT D'APRÈS N'EST PAS
INTERPRÉTABLE.** Un service déjà en panne avant l'édition serait attribué à
l'édition. Le garder.

- [ ] **Step 2: Éditer**

Remplacer le bloc `app.allanic.me` existant par les **trois** routes du § 7.2 de
la spec — la première garde la politique actuelle **mot pour mot** (elle
whiteliste déjà `manifest.json`, `.ico`, `.png`), change sa cible pour
`http://192.168.3.1:8080` et ajoute `pass_identity_headers: true`.

⚠️ **`pass_identity_headers: true` EST LA LIGNE QUI PORTE TOUT LE CHANTIER.**
Sans elle, la page rend `identite-absente` et rien ne fonctionne — c'est la
rouge du critère ① de la spec, et elle se joue en la retirant.

🔴 **LES DEUX ROUTES PRÉFIXÉES S'ÉCRIVENT AVANT LA ROUTE NUE, et cette ligne
disait L'INVERSE jusqu'au 21 août 2026.** Pomerium évalue dans l'ordre du
fichier, premier appariement gagnant : une route nue en tête avalerait `/signal`
et `/agent`, qui partiraient vers la redirection Google — que l'agent Windows ne
peut jamais satisfaire. Le précédent invoqué (`grocy.allanic.me`) dit lui aussi
**préfixe d'abord** : il avait été mal lu. Détail et mesure au § 7.2 de la spec.

- [ ] **Step 3: Recharger, puis contrôler les SEPT autres routes**

```bash
docker compose -f /opt/nivuus/Pomerium/docker-compose.yml up -d pomerium
docker logs --tail 40 pomerium
for h in media.allanic.me allanic.me home.allanic.me grocy.allanic.me personas.allanic.me game.allanic.me; do
    printf '%s ' "$h"; curl -s -o /dev/null -w '%{http_code}\n' -m 10 "https://$h/"
done | tee /tmp/pomerium-apres.txt
diff /tmp/pomerium-avant.txt /tmp/pomerium-apres.txt && echo "AUCUNE REGRESSION"
```

Expected: `diff` vide.

🔴 **SI LE `diff` N'EST PAS VIDE : restaurer la sauvegarde, relancer, et
DIAGNOSTIQUER** — jamais poursuivre. La restauration est
`cp -a /opt/nivuus/Pomerium/config.yaml.bak-20260821 /opt/nivuus/Pomerium/config.yaml`.

- [ ] **Step 4: Consigner**

Le bloc appliqué et les deux relevés `curl` vont dans le document de résultats
(tâche 8). 🔴 **Une preuve ne doit jamais vivre dans un fichier gitignoré** — un
espace de travail a déjà emporté six constats de revue.

---

### Task 8: La recette, et le document de résultats

**Files:**
- Create: `docs/superpowers/plans/2026-08-21-auth-pomerium-resultats.md`
- Modify: `CLAUDE.md`

- [ ] **Step 1: Démarrer la VM et recompiler l'agent**

🔴 **SANS CETTE ÉTAPE, LA TÂCHE 5 EST LIVRÉE À MOITIÉ** : un agent non recompilé
vise encore la racine et ne se connecte plus.

```bash
virsh list --all
virsh start Windows
until timeout 3 bash -c 'echo > /dev/tcp/192.168.3.2/5985' 2>/dev/null; do sleep 5; done
until ls /media/vm/dev >/dev/null 2>&1; do sleep 5; done
set -a && source .env && set +a
git status --porcelain agent/ proto/
cargo clean --release -p proto -p agent
scripts/build-agent.sh
```

⚠️ **Sourcer `.env` AVANT `build-agent.sh`** : sans cela il s'arrête EN SILENCE
après « sources synchronisées », et le symptôme se lit exactement comme une
compilation réussie et muette.

⚠️ **`git status --porcelain agent/ proto/` avant CHAQUE build** : le script
rsynchronise l'arbre entier, travail non commité des voisins compris.

- [ ] **Step 2: Prouver que le binaire est bien le neuf**

🔴 **LA TAILLE DU BINAIRE NE PROUVE RIEN, DANS LES DEUX SENS.** Ce qui tranche
est une chaîne posée soi-même, cherchée sur le chemin que `run-agent.sh` lance,
**avec son témoin négatif** :

```bash
node scripts/winrm.js 'Select-String -Path C:\dev\target\release\agent.exe -Pattern "/signal" -Encoding Byte | Measure-Object | % Count'
```

Expected: non nul pour `/signal`, **et zéro pour une chaîne qu'on sait absente**
(le témoin négatif). Les deux, jamais l'un seul.

- [ ] **Step 3: Jouer la recette navigateur — critère ⑦**

🔴 **C'EST LE SEUL CRITÈRE QUI JUGE LE PRODUIT**, et aucun test Node ne peut le
voir : `client/src/adresse-plateforme.ts` porte la démonstration que le
déploiement de P5 aurait été « livré non fonctionnel ET VERT ».

**Deux exécutions par bras**, et les deux bras :

| Bras | Montage | Attendu |
| --- | --- | --- |
| **VERT** | Pomerium avec `pass_identity_headers: true` | la page **ne montre jamais** le formulaire ; le bureau s'affiche |
| **ROUGE ①** | la même ligne **retirée** de `config.yaml` | la page rend `identite-absente`, **et le dit** |

⚠️ **Toute mesure de plus de 5 minutes** exige
`--disable-background-timer-throttling`, `--disable-backgrounding-occluded-windows`
et `--disable-renderer-backgrounding`.

⚠️ **Restaurer la ligne après la rouge**, et le vérifier par un second bras vert
— une rouge qu'on ne referme pas laisse le produit cassé.

- [ ] **Step 4: La passe complète**

```bash
env -u TURN_URL -u TURN_SECRET ./scripts/verify-all.sh
```

⚠️ **Le script compte DIX étapes et affiche DIX-HUIT en-têtes `==>`.** Dire
lequel on annonce.

- [ ] **Step 5: Écrire le document de résultats**

Il porte, et **rien d'autre ne fait foi** : les deux relevés `curl` de la
tâche 7, les sorties des rouges des tâches 1, 2, 3 et 5, les deux bras du
critère ⑦, et ce que la recette **n'a pas** établi.

🔴 **NE JAMAIS FABRIQUER UNE PIÈCE** : deux transcriptions ont déjà été
assemblées à la main dans ce dépôt et présentées comme des relevés. Chaque
sortie est celle de sa propre commande, relancée.

- [ ] **Step 6: Mettre `CLAUDE.md` à jour**

Dans cet ordre, et **en relevant chaque chiffre au moment où on l'écrit** :

1. la ligne d'index du chantier, dans « Index des chantiers » ;
2. `PLATEFORME_AUTH` dans le tableau « Variables du SERVEUR », avec sa
   convention de MODE et son lever sur valeur inconnue ;
3. la mention du chemin `/signal` là où `CLAUDE.md` parle du relais ;
4. le legs « TURNS sur 443 n'est pas livré » — **inchangé**, Pomerium ne le
   solde pas ;
5. **relever à nouveau le tableau de dette des 500 lignes**, par la commande du
   fichier, **après la dernière édition de la ronde**.

⚠️ **Un relevé daté n'est pas une consigne** : le récit va dans
`docs/JOURNAL.md`, l'index ne prend qu'une ligne.

- [ ] **Step 7: Commit**

```bash
git add docs/superpowers/plans/2026-08-21-auth-pomerium-resultats.md CLAUDE.md
git commit -F <fichier de message>
```

⚠️ **Message long par fichier (`-F`)** : des backticks dans un `git commit -m`
exécutent une commande, et les accents graves mutilent les phrases.

---

## Ce que ce plan ne fait PAS

- **Il ne retire pas la machinerie de mots de passe** — arbitrage ④ de la spec.
  `mot-de-passe.ts`, `depot/jeton.ts`, la table `jeton_rafraichissement` et
  `npm run admin:utilisateur` restent, vivants et testés, exercés par leurs
  seuls tests.
- **Il ne vérifie aucune signature** — arbitrage ② ; l'en-tête reste falsifiable
  par quiconque atteint le port, VM Windows comprise (spec § 9).
- **Il ne solde pas le legs ④** « le hub n'est pas installable » : la politique
  Pomerium lève l'obstacle côté proxy, l'`<img src>` sans `Authorization` reste
  entier côté service.
- **Il ne mesure aucune latence** : le legs « la latence de bout en bout n'a
  jamais été mesurée, depuis D1 » reste entier.
