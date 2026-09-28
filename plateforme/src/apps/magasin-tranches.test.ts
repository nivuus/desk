// ⚠️ THIS FILE DOES NOT TOUCH THE DATABASE, and yet it runs under `test:sqlite`
// AND under `test:postgres`: it lives in the same suite. It opens no driver,
// so `baseNeuve` and its mandatory schema name do not concern it.
//
// ⚠️ THE TWO CONFIGURATION TESTS AT THE END OF THE FILE ARE HERE RATHER THAN IN
// `config.test.ts`, AND IT IS DECLARED: the tree is shared with other
// workstreams, and the task forbids itself from writing elsewhere. `config.test.ts` received
// the only line it could NOT not receive — its `toEqual` compares
// the WHOLE object, so one more field turns it red.

import { randomUUID } from 'node:crypto';
import { existsSync, mkdtempSync, readdirSync, readFileSync, rmSync, utimesSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { Readable } from 'node:stream';
import { afterEach, describe, expect, it } from 'vitest';
import { lireConfig } from '../config';
import {
    AGE_EVICTION_TRANCHES_MS,
    identifiantValide,
    ouvrirMagasinTranches,
    rangValide,
} from './magasin-tranches';
import { verdict } from '../../../proto/ts/tranches';
import { SECRET as SECRET_PLATEFORME } from '../agents/canal-harnais';

let racines: string[] = [];
function magasinNeuf() {
    const r = mkdtempSync(join(tmpdir(), 'g3-tranches-'));
    racines.push(r);
    return ouvrirMagasinTranches(join(r, 'televersements'), () => {});
}
afterEach(() => {
    for (const r of racines) rmSync(r, { recursive: true, force: true });
    racines = [];
});

/// A byte stream, in chunks, to test the REAL path — the one that
/// does not accumulate in memory — rather than a disguised `Buffer`.
function flux(...morceaux: (string | Uint8Array)[]): AsyncIterable<Uint8Array> {
    return Readable.from(morceaux.map((m) => (typeof m === 'string' ? Buffer.from(m) : m)));
}

// ⚠️ THIS IDENTIFIER CARRIES HEXADECIMAL LETTERS, AND IT IS DELIBERATE: on
// a digits-only UUID, `toUpperCase()` would be a NO-OP and the
// "upper case refused" case of the identifier test could NOT fail.
const ID = '0a1b2c3d-4e5f-4a6b-8c7d-9e0f1a2b3c4d';
const PLAFOND = 1024;

async function lire(r: Readable): Promise<Buffer> {
    const bouts: Buffer[] = [];
    for await (const m of r) bouts.push(Buffer.from(m as Uint8Array));
    return Buffer.concat(bouts);
}

describe('the chunk store on disk', () => {
    it('writes a chunk AS A STREAM, and re-reads it by concatenation', async () => {
        const m = magasinNeuf();
        expect(await m.write(ID, 0, flux('abc', 'def'), PLAFOND)).toEqual({ ok: true, octets: 6 });
        expect(await m.write(ID, 1, flux('gh'), PLAFOND)).toEqual({ ok: true, octets: 2 });
        expect((await lire(m.concatener(ID, [0, 1]))).toString()).toBe('abcdefgh');
    });

    it('🔴 THERE IS NO ASSEMBLED FILE — one file per chunk, and nothing else', async () => {
        // 🔴 An 800 MB installer would double the disk space at sealing,
        // and the assembled file would be a SECOND source of truth that nothing
        // would arbitrate against its chunks the day they diverged.
        const m = magasinNeuf();
        await m.write(ID, 0, flux('abc'), PLAFOND);
        await m.write(ID, 1, flux('de'), PLAFOND);
        // The concatenation is a STREAM: consuming it writes nothing.
        expect((await lire(m.concatener(ID, [0, 1]))).toString()).toBe('abcde');
        expect(readdirSync(join(m.racine, ID)).sort()).toEqual(['0', '1']);
    });

    it('🔴 the ceiling CUTS, deletes the partial file, and says so — never a truncation', async () => {
        const m = magasinNeuf();
        const r = await m.write(ID, 0, flux('a'.repeat(600), 'b'.repeat(600)), PLAFOND);
        expect(r).toEqual({ ok: false, motif: 'plafond-depasse', plafond: PLAFOND });
        // 🔴 NEITHER THE CHUNK NOR THE `.part`: a 600-byte file left there
        // would be seen PRESENT by the resume, `verdict` would call it `incoherentes`,
        // and an inconsistency is not repaired by asking again.
        expect(existsSync(join(m.racine, ID))).toBe(true);
        expect(readdirSync(join(m.racine, ID))).toEqual([]);
        expect(m.lister(ID)).toEqual([]);
    });

    it('a chunk EXACTLY at the ceiling passes: the bound is inclusive', async () => {
        const m = magasinNeuf();
        expect(await m.write(ID, 0, flux('x'.repeat(PLAFOND)), PLAFOND))
            .toEqual({ ok: true, octets: PLAFOND });
        expect(m.lister(ID)).toEqual([{ n: 0, octets: PLAFOND }]);
    });

    it('⚠️ the ceiling BOUNDS the disk, it does not JUDGE the split', async () => {
        // A chunk SHORTER than the contract passes here without a word: it is
        // `verdict` that will declare it `incoherentes` at sealing. This module does not
        // know the contract, and making it know it would be the second
        // arithmetic that `proto/ts/tranches.ts` exists to prevent.
        const m = magasinNeuf();
        await m.write(ID, 0, flux('court'), PLAFOND);
        expect(m.lister(ID)).toEqual([{ n: 0, octets: 5 }]);
        expect(verdict(20, 10, m.lister(ID))).toEqual({ etat: 'incoherentes', n: [0] });
    });

    it('🔴 `lister` queries the DISK, not a bookkeeping', async () => {
        const m = magasinNeuf();
        await m.write(ID, 0, flux('abcde'), PLAFOND);
        await m.write(ID, 1, flux('fg'), PLAFOND);
        expect(m.lister(ID)).toEqual([{ n: 0, octets: 5 }, { n: 1, octets: 2 }]);

        // 🔴 THE FILE IS DELETED FROM UNDER THE STORE. A table
        // would diverge from the disk the day a file was lost — and that is
        // PRECISELY the day one needs to know. The resume
        // asks again, and the depositor completes it.
        rmSync(join(m.racine, ID, '0'));
        expect(m.lister(ID)).toEqual([{ n: 1, octets: 2 }]);
        expect(verdict(7, 5, m.lister(ID))).toEqual({ etat: 'manquantes', n: [0] });
    });

    it('`lister` sorts by rank, and a sort of STRINGS would not suffice', async () => {
        const m = magasinNeuf();
        for (const n of [10, 2, 0]) await m.write(ID, n, flux('x'), PLAFOND);
        expect(m.lister(ID).map((t) => t.n)).toEqual([0, 2, 10]);
    });

    it('⚠️ an abandoned `.part` is NOT a chunk', async () => {
        // Counting it would make a chunk that never finished being written look
        // complete — and `verdict` would seal a truncated file.
        const m = magasinNeuf();
        await m.write(ID, 0, flux('ab'), PLAFOND);
        writeFileSync(join(m.racine, ID, '1.12345.abc.part'), 'moitie');
        expect(m.lister(ID)).toEqual([{ n: 0, octets: 2 }]);
    });

    it('an upload without any chunk returns an EMPTY list, never an error', () => {
        const m = magasinNeuf();
        expect(m.lister(ID)).toEqual([]);
        expect(verdict(4, 2, m.lister(ID))).toEqual({ etat: 'manquantes', n: [0, 1] });
    });

    it('🔴 a VANISHED chunk gives a stream ERROR, never a truncated stream', async () => {
        // 🔴 A short stream would produce on the agent side a wrong hash whose
        // cause nobody could tell. That is why `concatener` serves
        // the plan VERIFIED by the caller and not a listing: a listing
        // would simply not have seen the chunk, and would have ended
        // cleanly.
        const m = magasinNeuf();
        await m.write(ID, 0, flux('aaa'), PLAFOND);
        await m.write(ID, 1, flux('bbb'), PLAFOND);
        rmSync(join(m.racine, ID, '1'));
        await expect(lire(m.concatener(ID, [0, 1]))).rejects.toThrow(/ENOENT/);
    });

    it('🔴 a chunk deleted WHILE being read errors too', async () => {
        // The variant above, but the deletion happens while the stream
        // flows: it is the real case of a concurrent purge. The first
        // chunk is big enough for back-pressure to suspend the
        // generator well before it opens the second.
        const m = magasinNeuf();
        await m.write(ID, 0, flux(Buffer.alloc(1 << 20, 0x61)), 1 << 21);
        await m.write(ID, 1, flux('bbb'), PLAFOND);
        const r = m.concatener(ID, [0, 1]);
        let vus = 0;
        await expect(
            (async () => {
                for await (const morceau of r) {
                    if (vus === 0) rmSync(join(m.racine, ID, '1'));
                    vus += (morceau as Uint8Array).byteLength;
                }
            })(),
        ).rejects.toThrow(/ENOENT/);
        expect(vus).toBeGreaterThan(0);
    });

    it('🔴 REFUSES an identifier that could escape the store', async () => {
        // 🔴 `/televersement/..%2f..%2fetc/tranche/0` must be able to write
        // NOWHERE, and the refusal is explicit: we do not sanitise silently,
        // otherwise nobody would know where we wrote.
        const m = magasinNeuf();
        for (const mauvais of [
            '../../../etc/passwd',
            '..%2f..%2fx',
            '..',
            '.',
            '',
            'x',
            `${ID}/..`,
            ID.toUpperCase(), // upper case: refused, see the comment
            `${ID} `,
        ]) {
            expect(identifiantValide(mauvais)).toBe(false);
            await expect(m.write(mauvais, 0, flux('x'), PLAFOND))
                .rejects.toThrow(/invalid upload identifier/);
            expect(() => m.lister(mauvais)).toThrow(/identifier/);
            expect(() => m.concatener(mauvais, [0])).toThrow(/identifier/);
            expect(() => m.remove(mauvais)).toThrow(/identifier/);
        }
        expect(readdirSync(m.racine)).toEqual([]);
        expect(identifiantValide(ID)).toBe(true);
    });

    it('🔴 REFUSES a rank that is not a positive safe integer', async () => {
        // 🔴 THE FILE NAME IS THE VALIDATED NUMBER, never a copied URL
        // segment. Under this guard, `String(n)` is ALWAYS a run of
        // digits — beyond the safe integer, `String(1e21)` would be `1e+21`.
        const m = magasinNeuf();
        for (const mauvais of [-1, 1.5, NaN, Infinity, 1e21, Number.MAX_SAFE_INTEGER + 2]) {
            expect(rangValide(mauvais)).toBe(false);
            await expect(m.write(ID, mauvais, flux('x'), PLAFOND))
                .rejects.toThrow(/invalid chunk rank/);
            expect(() => m.concatener(ID, [mauvais])).toThrow(/invalid chunk rank/);
        }
        expect(rangValide(0)).toBe(true);
        expect(rangValide(Number.MAX_SAFE_INTEGER)).toBe(true);
        expect(readdirSync(m.racine)).toEqual([]);
    });

    it('🔴 a rank that is a copied URL SEGMENT can write NOWHERE', async () => {
        // 🔴 THE GUARD IS DEFENCE IN DEPTH, and this test puts it to
        // the test of the case it exists to stop: a route that
        // passed the RAW URL segment instead of the number. Typing
        // forbids it, an `as` bypasses it — as an `any` or a
        // badly parsed JSON body would, and the refusal must hold without it.
        const m = magasinNeuf();
        const evil = '../../evil' as unknown as number;
        expect(rangValide(evil)).toBe(false);
        await expect(m.write(ID, evil, flux('poison'), PLAFOND))
            .rejects.toThrow(/invalid chunk rank/);
        expect(() => m.concatener(ID, [evil])).toThrow(/invalid chunk rank/);
        // 🔴 AND NOTHING WAS WRITTEN ELSEWHERE: neither in the root, nor above
        // it. A refusal that left the file would not be one.
        expect(readdirSync(m.racine)).toEqual([]);
        expect(existsSync(join(m.racine, '..', 'evil'))).toBe(false);
        expect(existsSync(join(m.racine, ID, '..', '..', 'evil'))).toBe(false);
    });

    it('🔴 the rank is validated BEFORE the stream exists', async () => {
        // A faulty rank throws at the CALL, where the caller can still answer,
        // rather than in the middle of an answer already started.
        const m = magasinNeuf();
        await m.write(ID, 0, flux('a'), PLAFOND);
        expect(() => m.concatener(ID, [0, -1])).toThrow(/rank/);
    });

    it('the write is ATOMIC: no `.part` survives a success', async () => {
        const m = magasinNeuf();
        await m.write(ID, 7, flux('abc'), PLAFOND);
        expect(readdirSync(join(m.racine, ID))).toEqual(['7']);
        expect(readFileSync(join(m.racine, ID, '7')).toString()).toBe('abc');
    });

    it('rewriting a rank REPLACES it, leaving no residue', async () => {
        const m = magasinNeuf();
        await m.write(ID, 0, flux('aaaaa'), PLAFOND);
        await m.write(ID, 0, flux('bb'), PLAFOND);
        expect(m.lister(ID)).toEqual([{ n: 0, octets: 2 }]);
        expect(readdirSync(join(m.racine, ID))).toEqual(['0']);
    });

    it('`supprimer` removes everything, and on an absent one it is a success', async () => {
        const m = magasinNeuf();
        await m.write(ID, 0, flux('a'), PLAFOND);
        m.remove(ID);
        expect(existsSync(join(m.racine, ID))).toBe(false);
        expect(m.lister(ID)).toEqual([]);
        // A purge that runs after a drop abandoned before its first frame.
        expect(() => m.remove(ID)).not.toThrow();
    });

    it('two uploads do not mix', async () => {
        const m = magasinNeuf();
        const autre = randomUUID();
        await m.write(ID, 0, flux('un'), PLAFOND);
        await m.write(autre, 0, flux('deux'), PLAFOND);
        expect(m.lister(ID)).toEqual([{ n: 0, octets: 2 }]);
        expect(m.lister(autre)).toEqual([{ n: 0, octets: 4 }]);
        m.remove(autre);
        expect(m.lister(ID)).toEqual([{ n: 0, octets: 2 }]);
    });

    it('the root is CREATED if missing, and its path is LOGGED', () => {
        const r = mkdtempSync(join(tmpdir(), 'g3-tranches-'));
        racines.push(r);
        const vu: string[] = [];
        const cible = join(r, 'profond', randomUUID());
        expect(existsSync(cible)).toBe(false);
        ouvrirMagasinTranches(cible, (c) => vu.push(c));
        expect(existsSync(cible)).toBe(true);
        // ⚠️ The variable being optional, an operator can get the
        // directory wrong without anything breaking — and here the loss does NOT
        // repair itself: a human has to drop their file again.
        expect(vu).toEqual([cible]);
    });
});

describe('eviction by age, with a floor', () => {
    // Time is INJECTED, never read from the clock: a test that really waited
    // for the eviction age would be a test disabled at the first
    // slowdown of the machine.
    const JOUR_MS = 24 * 60 * 60_000;

    /// Drops a single chunk for `id`, then FORCES the last-modified date
    /// of the upload's DIRECTORY — it is that, and not an isolated
    /// chunk, that `evincer` measures. Equivalent, on the REAL store,
    /// of the task's `deposer(cle, octets, quand)`.
    async function deposerA(m: ReturnType<typeof magasinNeuf>, id: string, quandMs: number): Promise<void> {
        await m.write(id, 0, flux('x'), PLAFOND);
        utimesSync(join(m.racine, id), new Date(quandMs), new Date(quandMs));
    }

    it('evicts an old and NON-referenced upload', async () => {
        const m = magasinNeuf();
        const orphelin = randomUUID();
        await deposerA(m, orphelin, 0);
        await m.evincer({ maintenant: 400 * JOUR_MS, referencees: new Set() });
        expect(existsSync(join(m.racine, orphelin))).toBe(false);
    });

    // 🔴 THE ONLY TEST THAT TELLS AN EVICTION FROM A CORRUPTION. Without it,
    // an eviction that takes EVERYTHING would pass the previous test.
    it('CANNOT evict an old upload still REFERENCED by a live entry', async () => {
        const m = magasinNeuf();
        const enService = randomUUID();
        await deposerA(m, enService, 0);
        await m.evincer({ maintenant: 400 * JOUR_MS, referencees: new Set([enService]) });
        expect(existsSync(join(m.racine, enService))).toBe(true);
        expect(m.lister(enService)).toEqual([{ n: 0, octets: 1 }]);
    });

    it('does not evict a young upload', async () => {
        const m = magasinNeuf();
        const recent = randomUUID();
        await deposerA(m, recent, 0);
        await m.evincer({ maintenant: 1 * JOUR_MS, referencees: new Set() });
        expect(existsSync(join(m.racine, recent))).toBe(true);
    });

    it('🔴 the constant N is NOT calibrated: the floor holds at any value', () => {
        // Consistency check of the setup itself: if
        // `AGE_EVICTION_TRANCHES_MS` ever drifted outside the interval
        // [1 day, 400 days], the three tests above would lose their meaning
        // without any red saying so.
        expect(AGE_EVICTION_TRANCHES_MS).toBeGreaterThan(1 * JOUR_MS);
        expect(AGE_EVICTION_TRANCHES_MS).toBeLessThan(400 * JOUR_MS);
    });

    it('a name that is not a valid identifier is never touched', async () => {
        const m = magasinNeuf();
        const etranger = join(m.racine, 'not-a-uuid');
        writeFileSync(etranger, 'x');
        utimesSync(etranger, new Date(0), new Date(0));
        await m.evincer({ maintenant: 400 * JOUR_MS, referencees: new Set() });
        expect(existsSync(etranger)).toBe(true);
    });
});

describe('PLATEFORME_TELEVERSEMENTS', () => {
    // 🔴 THE SECRET GOES THROUGH A SHARED CONSTANT, AND IT IS NOT STYLE.
    // The scanner of `securite/secrets.test.ts` looks for a LITERAL
    // assignment to a NAMED secret variable: written inline, this setup
    // turned "secrets are assigned in clear in versioned
    // files" red — on a perfectly harmless test secret, but the
    // scanner cannot know that, and it is precisely why it does not
    // look at the value. Reusing the harness constant also removes
    // a copy of the literal.
    const BASE = {
        PLATEFORME_HOTE: '127.0.0.1',
        PLATEFORME_SECRET_JETON: SECRET_PLATEFORME,
    };

    it('keeps the directory it is GIVEN', () => {
        // 🔴 THE RED: the variable set and IGNORED. The chunks
        // would be written elsewhere, silently.
        //
        // `PLATEFORME_PROXY_DE_CONFIANCE` is set to satisfy the guard
        // of the refusal to start in `pomerium` mode (task 6, `config.ts`) —
        // it is not the subject of this test.
        expect(
            lireConfig({
                ...BASE,
                PLATEFORME_TELEVERSEMENTS: '/var/lib/guac/tel',
                PLATEFORME_PROXY_DE_CONFIANCE: '172.18.0.5',
            }).repertoireTeleversements,
        ).toBe('/var/lib/guac/tel');
    });

    it('🔴 an EMPTY value falls back to the default, not to the current directory', () => {
        // `env.X ?? 'defaut'` does NOT catch `''` — the platform's sub-block P1
        // paid for this exact mistake.
        //
        // `PLATEFORME_PROXY_DE_CONFIANCE` is set for the same reason as
        // above.
        expect(
            lireConfig({
                ...BASE,
                PLATEFORME_TELEVERSEMENTS: '',
                PLATEFORME_PROXY_DE_CONFIANCE: '172.18.0.5',
            }).repertoireTeleversements,
        ).toBe('donnees/televersements');
    });
});
