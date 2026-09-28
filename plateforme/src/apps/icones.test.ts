import { createHash, randomUUID } from 'node:crypto';
import { existsSync, mkdtempSync, readdirSync, rmSync, utimesSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, describe, expect, it } from 'vitest';
import { AGE_EVICTION_ICONE_MS, empreinteValide, ouvrirMagasin } from './icones';

let racines: string[] = [];
function magasinNeuf() {
    const r = mkdtempSync(join(tmpdir(), 'g2-icones-'));
    racines.push(r);
    return ouvrirMagasin(join(r, 'icones'), () => {});
}
afterEach(() => {
    for (const r of racines) rmSync(r, { recursive: true, force: true });
    racines = [];
});

const OCTETS = Buffer.from('\x89PNG\r\n\x1a\n-des-octets');
const EMPREINTE = createHash('sha256').update(OCTETS).digest('hex');

describe('the icon store on disk', () => {
    it('writes, re-reads, and knows what it holds', () => {
        const m = magasinNeuf();
        expect(m.possede(EMPREINTE)).toBe(false);
        m.write(EMPREINTE, OCTETS);
        expect(m.possede(EMPREINTE)).toBe(true);
        expect(m.lire(EMPREINTE)).toEqual(OCTETS);
    });

    it('🔴 REFUSES bytes that do not match their fingerprint', () => {
        // 🔴 WITHOUT THIS RECOMPUTATION, CONTENT ADDRESSING WOULD NOT BE ONE: a
        // faulty agent would poison the store with a file that does not
        // match its name, and `Cache-Control: immutable` would make the
        // poisoning PERMANENT in the caches.
        const m = magasinNeuf();
        const mensonge = createHash('sha256').update('something else').digest('hex');
        expect(() => m.write(mensonge, OCTETS)).toThrow(/announced fingerprint/);
        // And the partial file is NEVER written.
        expect(m.possede(mensonge)).toBe(false);
        expect(readdirSync(m.repertoire)).toEqual([]);
    });

    it('🔴 REFUSES a fingerprint that could escape the store', () => {
        // 🔴 WITHOUT `empreinteValide`, `:sha256` IS A PATH COMPONENT
        // SUPPLIED BY THE NETWORK, and `..` is meaningful in it.
        const m = magasinNeuf();
        for (const mauvaise of [
            '../../../etc/passwd',
            '..%2f..%2fx',
            'a'.repeat(63),
            'a'.repeat(65),
            'A'.repeat(64), // upper case: refused, see the comment
            `${'a'.repeat(63)}/`,
            '',
            '.',
            '..',
        ]) {
            expect(empreinteValide(mauvaise)).toBe(false);
            expect(() => m.write(mauvaise, OCTETS)).toThrow(/invalid/);
            expect(m.possede(mauvaise)).toBe(false);
            expect(m.lire(mauvaise)).toBeUndefined();
        }
        expect(readdirSync(m.repertoire)).toEqual([]);
        expect(empreinteValide(EMPREINTE)).toBe(true);
    });

    it('the write is ATOMIC: no partial file remains', () => {
        const m = magasinNeuf();
        m.write(EMPREINTE, OCTETS);
        // 🔴 A TRUNCATED FILE UNDER A NAME THAT PROMISES ITS CONTENT would be
        // served without ever being read back. The only file in the directory is
        // the hash itself — no leftover `.part`.
        expect(readdirSync(m.repertoire)).toEqual([EMPREINTE]);
    });

    it('🔴 `manquantes` queries the DISK, not an in-memory list', () => {
        const m = magasinNeuf();
        m.write(EMPREINTE, OCTETS);
        expect(m.manquantes([EMPREINTE])).toEqual([]);

        // 🔴 THE FILE IS DELETED FROM UNDER THE STORE. A bookkeeping
        // table would not see it, and the icon would be lost
        // FOREVER. It is the property of criterion ⑦: the store
        // rebuilds itself.
        rmSync(join(m.repertoire, EMPREINTE));
        expect(m.manquantes([EMPREINTE])).toEqual([EMPREINTE]);
        expect(m.possede(EMPREINTE)).toBe(false);
    });

    it('`manquantes` keeps the announcement order and merges duplicates', () => {
        const m = magasinNeuf();
        const a = createHash('sha256').update('a').digest('hex');
        const b = createHash('sha256').update('b').digest('hex');
        const c = createHash('sha256').update('c').digest('hex');
        m.write(b, Buffer.from('b'));
        expect(m.manquantes([c, a, c, b, a])).toEqual([c, a]);
    });

    it('a MALFORMED fingerprint is not « missing »: it is ignored', () => {
        // Asking for it again would make the agent loop on a value the route
        // would refuse anyway.
        const m = magasinNeuf();
        expect(m.manquantes(['../x', 'ZZZ'])).toEqual([]);
    });

    it('an EMPTY store makes everything be asked again — the first start, and the loss', () => {
        const m = magasinNeuf();
        const a = createHash('sha256').update('a').digest('hex');
        const b = createHash('sha256').update('b').digest('hex');
        expect(m.manquantes([a, b])).toEqual([a, b]);
    });

    it('the directory is CREATED if missing, and its path is LOGGED', () => {
        const r = mkdtempSync(join(tmpdir(), 'g2-icones-'));
        racines.push(r);
        const vu: string[] = [];
        const cible = join(r, 'profond', randomUUID());
        expect(existsSync(cible)).toBe(false);
        ouvrirMagasin(cible, (c) => vu.push(c));
        expect(existsSync(cible)).toBe(true);
        // ⚠️ THE LOG LINE IS NOT DECORATIVE: the variable being
        // optional, an operator can get the directory wrong without
        // anything breaking — the store would rebuild elsewhere, silently.
        expect(vu).toEqual([cible]);
    });

    it('a foreign file already present is READ as is, without being revalidated', () => {
        // ⚠️ A DECLARED PROPERTY, NOT A HIDDEN GAP: `lire` recomputes
        // nothing. The check lives at WRITE time, which is the only path through
        // which a peer can drop something. A file placed by hand
        // in the store is the responsibility of whoever placed it.
        const m = magasinNeuf();
        writeFileSync(join(m.repertoire, EMPREINTE), 'not the right content');
        expect(m.lire(EMPREINTE)?.toString()).toBe('not the right content');
    });
});

describe('eviction by age, with a floor', () => {
    // Time is INJECTED, never read from the clock: a test that really waited
    // for the eviction age would be a test disabled at the first
    // slowdown of the machine.
    const JOUR_MS = 24 * 60 * 60_000;

    /// An icon whose content and hash match, as
    /// `write` requires.
    function icone(texte: string): { empreinte: string; octets: Buffer } {
        const octets = Buffer.from(texte);
        return { empreinte: createHash('sha256').update(octets).digest('hex'), octets };
    }

    /// Drops an icon, then FORCES its last-modified date: it is
    /// the equivalent, on the REAL store, of the task's `deposer(cle, octets, quand)`
    /// — `write` alone has no grip on the disk's clock.
    function deposerA(m: ReturnType<typeof magasinNeuf>, texte: string, quandMs: number): string {
        const { empreinte, octets } = icone(texte);
        m.write(empreinte, octets);
        utimesSync(join(m.repertoire, empreinte), new Date(quandMs), new Date(quandMs));
        return empreinte;
    }

    it('evicts an old and NON-referenced icon', async () => {
        const m = magasinNeuf();
        const orpheline = deposerA(m, 'orpheline', 0);
        await m.evincer({ maintenant: 400 * JOUR_MS, referencees: new Set() });
        expect(m.possede(orpheline)).toBe(false);
    });

    // 🔴 THE ONLY TEST THAT TELLS AN EVICTION FROM A CORRUPTION. Without it,
    // an eviction that takes EVERYTHING would pass the previous test.
    it('CANNOT evict an old icon still REFERENCED by a live entry', async () => {
        const m = magasinNeuf();
        const enService = deposerA(m, 'en-service', 0);
        await m.evincer({ maintenant: 400 * JOUR_MS, referencees: new Set([enService]) });
        expect(m.possede(enService)).toBe(true);
    });

    it('does not evict a young icon', async () => {
        const m = magasinNeuf();
        const recente = deposerA(m, 'recente', 0);
        await m.evincer({ maintenant: 1 * JOUR_MS, referencees: new Set() });
        expect(m.possede(recente)).toBe(true);
    });

    it('🔴 the constant N is NOT calibrated: the floor holds at any value', () => {
        // Consistency check of the setup itself: if `AGE_EVICTION_ICONE_MS`
        // ever drifted outside the interval [1 day, 400 days], the two
        // tests above would lose their meaning without any red saying so.
        expect(AGE_EVICTION_ICONE_MS).toBeGreaterThan(1 * JOUR_MS);
        expect(AGE_EVICTION_ICONE_MS).toBeLessThan(400 * JOUR_MS);
    });

    it('a name that is not a valid fingerprint is never touched', async () => {
        const m = magasinNeuf();
        writeFileSync(join(m.repertoire, 'etranger'), 'not a fingerprint');
        utimesSync(join(m.repertoire, 'etranger'), new Date(0), new Date(0));
        await m.evincer({ maintenant: 400 * JOUR_MS, referencees: new Set() });
        expect(existsSync(join(m.repertoire, 'etranger'))).toBe(true);
    });
});
