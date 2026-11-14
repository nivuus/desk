// JSON request bodies are read as BYTES and decoded ONCE, through the real
// server. A body arrives in network chunks, and a multi-byte UTF-8
// character can straddle two of them: decoding chunk by chunk turned it into
// U+FFFD, so a password or an e-mail in any non-ASCII script failed to sign
// in (and burnt a throttle attempt). The size cap counts bytes, whatever the
// script.

import { afterEach, describe, expect, it } from 'vitest';
import { request } from 'node:http';
import { mkdtempSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { baseNeuve } from '../base/harnais';
import type { Pilote } from '../base/pilote';
import type { Config } from '../config';
import { createUser } from '../depot/utilisateur';
import { hacher } from '../identite/mot-de-passe';
import { startServer, type ServicePlateforme } from './serveur';

const EMAIL = 'zoé@exemple.test';
const MOT_DE_PASSE = 'clé-密码-كلمة-42';

let base: Pilote | undefined;
let service: ServicePlateforme | undefined;

afterEach(async () => {
    await service?.close();
    service = undefined;
    await base?.fermer();
    base = undefined;
});

async function servir(nom: string): Promise<number> {
    base = await baseNeuve(nom);
    await createUser(base, EMAIL, await hacher(MOT_DE_PASSE), 1_787_136_773_742);
    const config: Config = {
        hote: '127.0.0.1',
        port: 0,
        base: 'sqlite',
        urlBase: ':memory:',
        secretJeton: 'un-secret-de-plateforme-de-quarante-octets',
        proxyDeConfiance: new Set(),
        repertoireIcones: join(mkdtempSync(join(tmpdir(), 'corps-icones-')), 'icones'),
        repertoireTeleversements: join(mkdtempSync(join(tmpdir(), 'corps-tranches-')), 'televersements'),
        auth: 'motdepasse',
    };
    service = await startServer(config, base);
    return service.port;
}

/// POSTs `corps` in two writes, the cut falling INSIDE a multi-byte character.
function posterEnDeux(port: number, corps: Buffer, coupure: number): Promise<number> {
    return new Promise((resoudre, rejeter) => {
        const req = request({
            host: '127.0.0.1', port, method: 'POST', path: '/auth/connexion',
            headers: { 'content-type': 'application/json', 'content-length': corps.length },
        }, (rep) => {
            rep.resume();
            rep.on('end', () => resoudre(rep.statusCode ?? 0));
        });
        req.on('error', rejeter);
        req.on('socket', (s) => s.setNoDelay(true));
        req.write(corps.subarray(0, coupure));
        setTimeout(() => req.end(corps.subarray(coupure)), 50);
    });
}

describe('JSON body reading', () => {
    it('signs in when the body is cut inside an "é" and inside a CJK character', async () => {
        const port = await servir('corps-coupure');
        const corps = Buffer.from(JSON.stringify({ email: EMAIL, motdepasse: MOT_DE_PASSE }), 'utf8');
        for (const caractere of ['é', '密']) {
            const debut = corps.indexOf(Buffer.from(caractere, 'utf8'));
            expect(debut).toBeGreaterThan(0);
            // One byte into the character: neither chunk holds it whole.
            expect(await posterEnDeux(port, corps, debut + 1)).toBe(200);
        }
    });

    it('caps the body in BYTES: 1,400 CJK characters (4,200 bytes) exceed 4 KiB', async () => {
        const port = await servir('corps-plafond');
        const motdepasse = '密'.repeat(1_400);
        expect(motdepasse.length).toBeLessThan(4 * 1024);
        const r = await fetch(`http://127.0.0.1:${port}/auth/connexion`, {
            method: 'POST',
            headers: { 'content-type': 'application/json' },
            body: JSON.stringify({ email: EMAIL, motdepasse }),
        });
        expect(r.status).toBe(413);
    });
});
