// The "DROP" family of upload routes: the rank, the hard bound
// of the `PUT`, and the idempotence of dropping again.
//
// 🔴 EXTRACTED FROM `routes-televersement.test.ts`, WHICH REACHED 504 LINES for
// a cap of 500. The repository paid TWICE in D9 for having caught up a
// crossing by a COMPRESSION it forbids by name; extraction
// is the gesture its doctrine prescribes, and EACH CASE TAKES WITH IT THE
// COMMENT THAT JUSTIFIES IT — no assertion line was reworded.
//
// 🔴 THE FIXTURES COME FROM `routes-televersement-harnais.ts`, NEVER FROM A
// COPY: two setups would diverge, and the day one of the two changed
// step, the other would test a contract the product no longer has.

import { readdirSync } from 'node:fs';
import { join } from 'node:path';
import { afterEach, describe, expect, it } from 'vitest';
import { MOTEUR } from '../base/harnais';
import { withIt, jetonDe } from './routes-harnais';
import {
    deposer,
    magasin,
    monter,
    nettoyer,
    PAS,
    poser,
    racine,
    TRANCHES,
    user,
} from './routes-televersement-harnais';

afterEach(nettoyer);

describe(`upload routes — drop, engine=${MOTEUR}`, () => {
    /* ── ③ DROP ───────────────────────────────────────────────────── */

    it('drops, re-reads the state, and DROPPING AGAIN overwrites (idempotence)', async () => {
        const { url, base } = await monter('tel-deposer');
        const ada = await user(base, 'ada@exemple.test');
        const jeton = jetonDe(ada);
        const id = await poser(base, ada);

        const un = await deposer(url, id, 1, TRANCHES[1], jeton);
        expect(un.status).toBe(200);
        expect(await un.json()).toEqual({ n: 1, octets: 4 });

        const etat = await fetch(`${url}/televersement/${id}`, { headers: withIt(jeton) });
        expect(((await etat.json()) as { tranches_presentes: unknown }).tranches_presentes).toEqual([
            { n: 1, octets: 4 },
        ]);

        // Dropping the SAME rank again, shorter: the second write wins.
        const bis = await deposer(url, id, 1, Buffer.from('xy'), jeton);
        expect(bis.status).toBe(200);
        expect(magasin.lister(id)).toEqual([{ n: 1, octets: 2 }]);
    });

    it('🔴 refuses a chunk beyond the step, and leaves NO file', async () => {
        // 🔴 THE RED: without the bound, we return 200 and write a chunk of
        // five bytes where the step is worth four.
        const { url, base } = await monter('tel-borne');
        const ada = await user(base, 'ada@exemple.test');
        const id = await poser(base, ada);

        const r = await deposer(url, id, 0, Buffer.alloc(PAS + 1, 0x41), jetonDe(ada));
        expect(r.status).toBe(413);
        expect(await r.json()).toEqual({ refus: 'tranche-trop-grande', maximum: PAS });

        // ⚠️ NO CHUNK: the `rename` never happened, so nothing exists
        // under the final name. It is THIS check that kills the red — without the
        // bound, file `0` exists and `lister` returns it.
        expect(magasin.lister(id)).toEqual([]);

        // 🔴 THE DIRECTORY IS EMPTY, AND THIS ASSERTION WAS RESTORED TO ITS
        // STRONG FORM AFTER THE DEFECT IT WORKED AROUND WAS FIXED.
        //
        // It first stopped at "what remains can only be a
        // `.part`", on a MEASURED defect of the store: `magasin-tranches.ts`
        // claimed that "THE PARTIAL FILE IS DELETED, whatever the
        // cause", and it was false — a RACE between the `rmSync` of the
        // error path and the ASYNCHRONOUS `open(2)` of `createWriteStream`, which created
        // the file just after its deletion. It is what made this
        // test fail intermittently under the load of the full suite, and never
        // in isolation.
        //
        // ✅ THE RACE IS REMOVED AT ITS SOURCE — the descriptor is opened by
        // `openSync` BEFORE the `pipeline`, so the inode already exists when the
        // `catch` deletes. Differential measured by a direct probe on `write`,
        // outside HTTP, **one run of 400 overflows per arm**:
        // **100 non-empty directories out of 400 BEFORE, 0 out of 400 AFTER**.
        // ⚠️ The rate depends on the load — an earlier measurement under another
        // load recorded 42 out of 400 —, so NO rate is claimed; what
        // is established is the disappearance, not a frequency.
        //
        // ⚠️ It was NOT a protocol hole: `lister` ignores non-numeric
        // names, so no fake chunk was ever counted and
        // sealing saw nothing of it. It was a DISK LEAK, on a
        // service that accepts 4 GiB.
        expect(readdirSync(join(racine, id)), 'no residue, .part included').toEqual([]);
    });

    it('refuses a rank that is not an integer, or that falls outside the plan', async () => {
        const { url, base } = await monter('tel-rang');
        const ada = await user(base, 'ada@exemple.test');
        const jeton = jetonDe(ada);
        const id = await poser(base, ada);

        for (const rang of ['%2B1', '1e3', '01x', '-1']) {
            const r = await deposer(url, id, rang, Buffer.from("a"), jeton);
            expect(r.status, rang).toBe(400);
            expect(await r.json(), rang).toEqual({ refus: 'rang-invalide' });
        }
        // Three chunks (0, 1, 2): rank 3 will never become consistent.
        const hors = await deposer(url, id, 3, Buffer.from('a'), jeton);
        expect(hors.status).toBe(409);
        expect(await hors.json()).toEqual({ refus: 'rang-hors-plan', tranches: 3 });
        expect(magasin.lister(id)).toEqual([]);
    });
});
