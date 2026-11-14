// `POST /auth/deconnexion` (password mode): signing out revokes the family of
// the presented refresh token, so a refresh token (thirty days) can be ended
// before it expires. The client still has to call this route.

import { afterEach, describe, expect, it } from 'vitest';
import { mkdtempSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { baseNeuve } from '../base/harnais';
import type { Pilote } from '../base/pilote';
import type { Config } from '../config';
import { createUser } from '../depot/utilisateur';
import { hacher } from '../identite/mot-de-passe';
import { startServer, type ServicePlateforme } from './serveur';

const EMAIL = 'ada@exemple.test';
const MOT_DE_PASSE = 'un-mot-de-passe-ordinaire-42';

let base: Pilote | undefined;
let service: ServicePlateforme | undefined;

afterEach(async () => {
    await service?.close();
    service = undefined;
    await base?.fermer();
    base = undefined;
});

async function servir(nom: string): Promise<string> {
    base = await baseNeuve(nom);
    await createUser(base, EMAIL, await hacher(MOT_DE_PASSE), 1_787_136_773_742);
    const config: Config = {
        hote: '127.0.0.1',
        port: 0,
        base: 'sqlite',
        urlBase: ':memory:',
        secretJeton: 'un-secret-de-plateforme-de-quarante-octets',
        proxyDeConfiance: new Set(),
        repertoireIcones: join(mkdtempSync(join(tmpdir(), 'deco-icones-')), 'icones'),
        repertoireTeleversements: join(mkdtempSync(join(tmpdir(), 'deco-tranches-')), 'televersements'),
        auth: 'motdepasse',
    };
    service = await startServer(config, base);
    return `http://127.0.0.1:${service.port}`;
}

function poster(url: string, corps: unknown): Promise<Response> {
    return fetch(url, {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify(corps),
    });
}

async function connecter(url: string): Promise<string> {
    const r = await poster(`${url}/auth/connexion`, { email: EMAIL, motdepasse: MOT_DE_PASSE });
    expect(r.status).toBe(200);
    return ((await r.json()) as { rafraichissement: string }).rafraichissement;
}

describe('POST /auth/deconnexion', () => {
    it('revokes the family: the token, and its successor, answer `revoque`', async () => {
        const url = await servir('deco-famille');
        const premier = await connecter(url);
        // Witness: before signing out, the token rotates.
        const r1 = await poster(`${url}/auth/rafraichir`, { rafraichissement: premier });
        expect(r1.status).toBe(200);
        const second = ((await r1.json()) as { rafraichissement: string }).rafraichissement;

        const sortie = await poster(`${url}/auth/deconnexion`, { rafraichissement: second });
        expect(sortie.status).toBe(204);

        const apres = await poster(`${url}/auth/rafraichir`, { rafraichissement: second });
        expect([apres.status, await apres.json()]).toEqual([401, { refus: 'revoque' }]);
    });

    it('leaves the other families of the account alive', async () => {
        const url = await servir('deco-autres');
        const ici = await connecter(url);
        const ailleurs = await connecter(url);
        expect((await poster(`${url}/auth/deconnexion`, { rafraichissement: ici })).status).toBe(204);
        expect((await poster(`${url}/auth/rafraichir`, { rafraichissement: ailleurs })).status).toBe(200);
    });

    it('answers 204 for an unknown token, and 400 for a malformed body', async () => {
        const url = await servir('deco-forme');
        expect((await poster(`${url}/auth/deconnexion`, { rafraichissement: 'never-issued' })).status)
            .toBe(204);
        expect((await poster(`${url}/auth/deconnexion`, { autre: 1 })).status).toBe(400);
    });
});
