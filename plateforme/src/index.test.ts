// The startup order, and the refusal to start degraded.
//
// Spec §6, first case: the service REFUSES to start, with the cause. It does not
// start degraded — "a signaling that pairs without recording anything
// would be indistinguishable from correct operation". It is the class of defect
// this whole repository is written against.

import net from 'node:net';
import { afterEach, describe, expect, it } from 'vitest';
import type { Config } from './config';
import { start, type Service } from './demarrage';
import { baseNeuve } from './base/harnais';
import { ouvrirSession } from './depot/session';
import { mkdtempSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

let service: Service | undefined;

afterEach(async () => {
    await service?.arreter();
    service = undefined;
});

/// Attempts a bare TCP connection, and says whether something listens.
function connecterA(port: number): Promise<void> {
    return new Promise((resolve, reject) => {
        const s = net.connect({ host: '127.0.0.1', port });
        s.once('connect', () => {
            s.destroy();
            resolve();
        });
        s.once('error', (e) => {
            s.destroy();
            reject(e);
        });
    });
}

const PORT_MORT = 45_137;

// An EXPLICIT test secret, never `''`: `lireConfig` refuses the empty
// string, and a `Config` literal built by hand must carry a value
// a service would really accept.
const SECRET = 'un-secret-de-plateforme-de-quarante-octets';

describe('service startup', () => {
    it("refuses to start when the database is unreachable, and opens no port", async () => {
        const config: Config = {
            hote: '127.0.0.1',
            port: PORT_MORT,
            base: 'postgres',
            // A port on which nothing listens: the connection is refused.
            urlBase: 'postgres://x:y@127.0.0.1:1/x',
            secretJeton: SECRET,
            // No proxy declared: see `config.ts`, the empty set is the
            // default and means "trust nobody's announced address".
            proxyDeConfiance: new Set(),
            repertoireIcones: join(mkdtempSync(join(tmpdir(), 'g2-icones-')), 'icones'),
            repertoireTeleversements: join(mkdtempSync(join(tmpdir(), 'g3-tranches-')), 'televersements'),
            auth: 'pomerium',
        };
        // Two DISTINCT assertions, and the second is the point of this test.
        await expect(start(config)).rejects.toThrow(/base/i);
        // 🔴 Without this one, a service that opens its port THEN dies would pass
        // for correct. It is what exercises the startup order.
        await expect(connecterA(PORT_MORT)).rejects.toThrow(/ECONNREFUSED/);
    });

    it('closes at startup the sessions left open, and says how many', async () => {
        // A database carrying two open sessions, as after an abrupt
        // stop of the service.
        const base = await baseNeuve('demarrage-balai');
        await ouvrirSession(base, 'survivante-1', 1_000);
        await ouvrirSession(base, 'survivante-2', 2_000);

        const balayees = await import('./depot/session').then((m) =>
            m.balayerLesOuvertes(base, 9_000),
        );
        expect(balayees).toBe(2);
        await base.fermer();
    });

    it('opens the port and serves the relay when the database is ready', async () => {
        service = await start({
            hote: '127.0.0.1',
            port: 0,
            base: 'sqlite',
            urlBase: ':memory:',
            secretJeton: SECRET,
            // No proxy declared: the service will trust nobody's
            // `X-Forwarded-For` header, which is the default of
            // `lireConfig` and the state of a deployment without a reverse proxy.
            proxyDeConfiance: new Set(),
            repertoireIcones: join(mkdtempSync(join(tmpdir(), 'g2-icones-')), 'icones'),
            repertoireTeleversements: join(mkdtempSync(join(tmpdir(), 'g3-tranches-')), 'televersements'),
            auth: 'pomerium',
        });
        expect(service.port).toBeGreaterThan(0);
        await expect(connecterA(service.port)).resolves.toBeUndefined();
        // The migrations are applied: the table exists and is readable. The
        // count is hardcoded for the reason given in
        // `base/pilotes.test.ts` — P2 brought it from 1 to 2 by adding
        // `0002-identite.sql`, P3 from 2 to 3 by adding `0003-agents.sql`, and
        // G1 from 3 to 4 by adding `0004-applications.sql`.
        //
        // ⚠️ It is the SECOND place in the repository that pins this count, and the only one
        // `pilotes.test.ts` does not name: updating one without
        // the other leaves a red whose cause is elsewhere than where one
        // looks for it. Both are found by
        // `grep -rn "schema_migration" src/ | grep -i test`.
        expect(await service.base.interroger('SELECT version FROM schema_migration', []))
            .toHaveLength(7);
    });
});
