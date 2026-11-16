// The service shuts down cleanly on SIGTERM, not only on SIGINT.
//
// 🔴 THE DEFECT: only `SIGINT` (Ctrl-C) ran `arreter()`. `systemctl stop`,
// `systemctl restart` and `docker stop` send `SIGTERM`, so in production the
// process died on the default action: sockets cut without a close frame,
// sessions left open in the database, the clean path tested only by hand.
//
// `index.ts` runs on import, so it is started as a real child process — the
// pattern of `signaling/resilience.test.ts`. `node --import tsx` and not the
// `tsx` binary: the signal must reach the service process itself, not a
// launcher that would relay it and report its own exit.

import { spawn } from 'node:child_process';
import { mkdtempSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

const racine = path.join(path.dirname(fileURLToPath(import.meta.url)), '..');
/// A named constant, never a literal after the variable name: the sweep of
/// `securite/secrets.test.ts` reports any literal assignment to it.
const SECRET_ENFANT = 'un-secret-de-plateforme-de-quarante-octets';

function demarrer() {
    const donnees = mkdtempSync(path.join(tmpdir(), 'signaux-'));
    const enfant = spawn(process.execPath, ['--import', 'tsx', path.join(racine, 'src', 'index.ts')], {
        cwd: racine,
        env: {
            ...process.env,
            PLATEFORME_HOTE: '127.0.0.1',
            PLATEFORME_PORT: '0',
            PLATEFORME_BASE: 'sqlite',
            PLATEFORME_BASE_URL: ':memory:',
            PLATEFORME_SECRET_JETON: SECRET_ENFANT,
            PLATEFORME_AUTH: 'motdepasse',
            PLATEFORME_ICONES: path.join(donnees, 'icones'),
            PLATEFORME_TELEVERSEMENTS: path.join(donnees, 'televersements'),
        },
    });
    const pret = new Promise<void>((resolve, reject) => {
        let sortie = '';
        let erreurs = '';
        enfant.stdout.on('data', (b: Buffer) => {
            sortie += b.toString();
            if (/port \d+/.test(sortie)) resolve();
        });
        enfant.stderr.on('data', (b: Buffer) => {
            erreurs += b.toString();
        });
        enfant.once('exit', (code, signal) =>
            reject(new Error(`exited before listening: code=${code} signal=${signal} ${erreurs}`)));
        setTimeout(() => reject(new Error(`startup timed out: ${erreurs}`)), 15_000);
    });
    const fin = new Promise<{ code: number | null; signal: NodeJS.Signals | null }>((resolve) =>
        enfant.once('exit', (code, signal) => resolve({ code, signal })));
    return { enfant, pret, fin };
}

describe('process signals', () => {
    for (const signal of ['SIGTERM', 'SIGINT'] as const) {
        it(`🔴 ${signal} runs the clean shutdown and exits 0`, async () => {
            const { enfant, pret, fin } = demarrer();
            await pret;
            enfant.kill(signal);
            expect(await fin).toEqual({ code: 0, signal: null });
        }, 20_000);
    }
});
