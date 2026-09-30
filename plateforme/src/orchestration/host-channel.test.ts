import { afterEach, describe, expect, it } from 'vitest';
import { createServer, type Server } from 'node:net';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { envoyerAuCanal } from './host-channel';

const nettoyages: Array<() => void> = [];
afterEach(() => {
    while (nettoyages.length > 0) nettoyages.pop()!();
});

/// A real Unix socket that responds with `reponse` to the first line received and
/// retains what it received.
function ecouter(reponse: string | null): Promise<{ chemin: string; recu: string[] }> {
    const dossier = mkdtempSync(join(tmpdir(), 'host-channel-'));
    const chemin = join(dossier, 'vm-control.sock');
    const recu: string[] = [];
    const serveur: Server = createServer((socket) => {
        socket.setEncoding('utf8');
        let tampon = '';
        socket.on('data', (morceau) => {
            tampon += morceau;
            const fin = tampon.indexOf('\n');
            if (fin === -1) return;
            recu.push(tampon.slice(0, fin));
            if (reponse !== null) socket.end(reponse);
        });
    });
    nettoyages.push(() => {
        serveur.close();
        rmSync(dossier, { recursive: true, force: true });
    });
    return new Promise((resoudre) => serveur.listen(chemin, () => resoudre({ chemin, recu })));
}

describe('envoyerAuCanal', () => {
    it('sends the verb followed by a newline and returns ok on "ok"', async () => {
        const { chemin, recu } = await ecouter('ok\n');
        expect(await envoyerAuCanal(chemin, 'wake')).toEqual({ ok: true });
        expect(recu).toEqual(['wake']);
    });

    it('returns the cause of an "err <cause>"', async () => {
        const { chemin } = await ecouter('err wake-failed\n');
        expect(await envoyerAuCanal(chemin, 'wake')).toEqual({ ok: false, cause: 'wake-failed' });
    });

    it('returns `reponse-inattendue` for an out-of-protocol response', async () => {
        const { chemin } = await ecouter('peut-etre\n');
        expect(await envoyerAuCanal(chemin, 'busy')).toEqual({ ok: false, cause: 'reponse-inattendue' });
    });

    it('🔴 missing socket: refusal `ENOENT`, never an exception', async () => {
        const r = await envoyerAuCanal('/tmp/host-channel-inexistant.sock', 'wake');
        expect(r).toEqual({ ok: false, cause: 'ENOENT' });
    });

    it('🔴 path present but no listener: refusal `ECONNREFUSED`, never an exception', async () => {
        // Under Linux, connecting to a file that is not a socket returns
        // ECONNREFUSED: it is exactly the case of a stopped service whose
        // file remains.
        const dossier = mkdtempSync(join(tmpdir(), 'host-channel-'));
        nettoyages.push(() => rmSync(dossier, { recursive: true, force: true }));
        const fichier = join(dossier, 'pas-un-socket');
        writeFileSync(fichier, '');
        expect(await envoyerAuCanal(fichier, 'wake')).toEqual({ ok: false, cause: 'ECONNREFUSED' });
    });

    it('🔴 host does not respond: `delai`, without blocking', async () => {
        const { chemin } = await ecouter(null);
        const debut = Date.now();
        expect(await envoyerAuCanal(chemin, 'wake', 100)).toEqual({ ok: false, cause: 'delai' });
        expect(Date.now() - debut).toBeLessThan(1_000);
    });

    it('host closes without responding: `ferme-sans-reponse`', async () => {
        const dossier = mkdtempSync(join(tmpdir(), 'host-channel-'));
        const chemin = join(dossier, 'ferme.sock');
        const serveur = createServer((socket) => socket.end());
        nettoyages.push(() => {
            serveur.close();
            rmSync(dossier, { recursive: true, force: true });
        });
        await new Promise<void>((r) => serveur.listen(chemin, () => r()));
        expect(await envoyerAuCanal(chemin, 'wake')).toEqual({ ok: false, cause: 'ferme-sans-reponse' });
    });

    it('🔴 a path Node rejects synchronously resolves to a value, never rejects', async () => {
        const reponse = await envoyerAuCanal('/tmp/bad\u0000path.sock', 'wake');
        expect(reponse.ok).toBe(false);
        if (reponse.ok) return;
        expect(typeof reponse.cause).toBe('string');
        expect(reponse.cause.length).toBeGreaterThan(0);
    });

    it('🔴 an empty path, which `connect` rejects synchronously, resolves to a value', async () => {
        const reponse = await envoyerAuCanal('', 'wake');
        expect(reponse.ok).toBe(false);
        if (reponse.ok) return;
        expect(reponse.cause.length).toBeGreaterThan(0);
    });
});
