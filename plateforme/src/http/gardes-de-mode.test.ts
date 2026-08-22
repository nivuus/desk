// LES DEUX GARDES DE MODE, ÉPROUVÉES DANS LEUR MODE INACTIF ET LA PAGE ARMÉE.
//
// 🔴 C'EST LA COMPOSITION QUE RIEN N'ÉPROUVAIT, ET C'EST ELLE QUI ROUGIT. Les
// deux gardes étaient testées, le servant de page était testé, et le défaut
// vivait EXACTEMENT à leur frontière : chaque moitié était juste, et aucune
// revue par tâche ne pouvait le voir. `routes-identite.test.ts` et
// `routes-auth.test.ts` ne posent jamais `racinePage` ; `routes-page.test.ts`
// ne visite jamais `/auth/*`.
//
// 🔴 MESURÉ AVANT CORRECTION, service en mode `motdepasse` avec la page armée :
// `/auth/moi` rendait `200 text/html` au lieu de `404`. Le repli SPA du
// servant, chaîné en dernier, avalait le `404` générique que les deux gardes
// appelaient — et ce `404` est le mécanisme documenté qui porte le MODE
// jusqu'au client (`client/src/connexion.ts` : « un 404 signifie ce montage
// authentifie par mot de passe »).
//
// ⚠️ LA MÉTHODE EST `GET`, ET C'EST CE QUI REND CES TESTS DISCRIMINANTS. Le
// servant se retire hors `GET`/`HEAD`, si bien qu'un `POST /auth/connexion`
// rendait `404` de toute façon — par accident, et sans rien prouver. C'est le
// `GET` qui traverse le repli SPA, donc le seul qui puisse rougir.

import { mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, describe, expect, it } from 'vitest';
import { baseNeuve } from '../base/harnais';
import type { Pilote } from '../base/pilote';
import type { Config } from '../config';
import { demarrerServeur, type ServicePlateforme } from './serveur';

const CONFIG: Config = {
    hote: '127.0.0.1',
    port: 0,
    base: 'sqlite',
    urlBase: ':memory:',
    secretJeton: 'un-secret-de-plateforme-de-quarante-octets',
    proxyDeConfiance: new Set(),
    repertoireIcones: join(mkdtempSync(join(tmpdir(), 'garde-icones-')), 'icones'),
    repertoireTeleversements: join(mkdtempSync(join(tmpdir(), 'garde-tranches-')), 'televersements'),
    auth: 'pomerium',
};

let base: Pilote | undefined;
let service: ServicePlateforme | undefined;

afterEach(async () => {
    await service?.close();
    service = undefined;
    await base?.fermer();
    base = undefined;
});

/// Une racine bâtie à la main, qui porte le `index.html` du repli SPA : sans
/// lui, le servant échouerait au `stat` et le `404` reviendrait pour une
/// raison qui n'est PAS celle qu'on éprouve.
function racineArmee(): string {
    const racine = mkdtempSync(join(tmpdir(), 'garde-page-'));
    writeFileSync(join(racine, 'index.html'), '<!doctype html><title>page</title>');
    return racine;
}

function requeteFermee(url: string): Promise<Response> {
    return fetch(url, { headers: { Connection: 'close' } });
}

async function servir(auth: 'pomerium' | 'motdepasse', nom: string): Promise<string> {
    base = await baseNeuve(nom);
    service = await demarrerServeur({ ...CONFIG, auth, racinePage: racineArmee() }, base);
    return `http://127.0.0.1:${service.port}`;
}

describe("la garde de mode d'`/auth/moi`, en mode motdepasse", () => {
    it('rend 404, jamais la page', async () => {
        const url = await servir('motdepasse', 'garde-moi-statut');
        expect((await requeteFermee(`${url}/auth/moi`)).status).toBe(404);
    });

    // 🔴 SÉPARÉE, ET ANCRÉE SUR LE TYPE : `expect` s'arrête au premier échec,
    // et cette assertion-ci rougirait MÊME si un futur défaut rendait le bon
    // statut avec le mauvais corps. C'est elle qui dit que le `404` vient de
    // la garde et non d'un servant qui aurait échoué pour une autre raison.
    it("rend le 404 du service — text/plain, jamais text/html", async () => {
        const url = await servir('motdepasse', 'garde-moi-type');
        const r = await requeteFermee(`${url}/auth/moi`);
        expect(r.headers.get('content-type')).toContain('text/plain');
    });

    it('rend le corps du 404 du service, mot pour mot', async () => {
        const url = await servir('motdepasse', 'garde-moi-corps');
        expect(await (await requeteFermee(`${url}/auth/moi`)).text()).toBe('introuvable\n');
    });
});

describe("la garde de mode de `/auth/connexion`, en mode pomerium", () => {
    // 🔴 LE JUMEAU SYMÉTRIQUE. Les deux gardes ont des polarités OPPOSÉES et
    // PARTITIONNENT les modes : elles ont donc le même défaut, chacune dans
    // l'autre mode. En éprouver une seule laisserait l'autre entière.
    it('rend 404, jamais la page', async () => {
        const url = await servir('pomerium', 'garde-connexion-statut');
        expect((await requeteFermee(`${url}/auth/connexion`)).status).toBe(404);
    });

    it("rend le 404 du service — text/plain, jamais text/html", async () => {
        const url = await servir('pomerium', 'garde-connexion-type');
        const r = await requeteFermee(`${url}/auth/connexion`);
        expect(r.headers.get('content-type')).toContain('text/plain');
    });

    it('rend le corps du 404 du service, mot pour mot', async () => {
        const url = await servir('pomerium', 'garde-connexion-corps');
        expect(await (await requeteFermee(`${url}/auth/connexion`)).text()).toBe('introuvable\n');
    });

    it('`/auth/rafraichir` est gardée de la même façon', async () => {
        const url = await servir('pomerium', 'garde-rafraichir');
        expect((await requeteFermee(`${url}/auth/rafraichir`)).status).toBe(404);
    });
});

// 🔴 LE TÉMOIN NÉGATIF DE TOUT CE FICHIER. Sans lui, les sept `404` ci-dessus
// seraient rendus à l'identique par une page qui ne serait PAS armée — donc
// par un montage où le défaut n'existe pas. Ce test prouve que la racine
// employée ci-dessus sert réellement quelque chose.
describe('la page est bien armée dans ce montage', () => {
    it('GET / rend 200 sur la même configuration', async () => {
        const url = await servir('motdepasse', 'garde-temoin-negatif');
        expect((await requeteFermee(`${url}/`)).status).toBe(200);
    });
});
