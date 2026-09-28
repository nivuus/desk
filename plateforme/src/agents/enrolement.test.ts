// The check of an enrolment secret, and the refusal that does not enumerate.
//
// 🔴 THE CENTRAL RED OF THIS FILE is the INDISTINCT refusal: an unknown VM
// and a wrong secret must return the SAME refusal, word for word. Two distinct
// reasons would be an enumeration oracle — the caller would learn by
// trial and error which VMs exist —, and it is literally what spec
// §4 P3 ② names as the red of this criterion.

import { afterEach, describe, expect, it } from 'vitest';
import { baseNeuve, MOTEUR } from '../base/harnais';
import type { Pilote } from '../base/pilote';
import { hacher } from '../identite/mot-de-passe';
import { enroler } from '../depot/agent';
import { verifyEnrolment } from './enrolement';

let base: Pilote | undefined;

const PREFIXE = 'RhH1x2QmTz9kLpVbNc7dAw';
const SECRET = 'un-secret-d-enrolement-tire-au-sort-et-long';

afterEach(async () => {
    await base?.fermer();
    base = undefined;
});

async function baseEnrolee(nom: string): Promise<Pilote> {
    const p = await baseNeuve(nom);
    await p.executer('INSERT INTO vm(id,nom,adresse) VALUES(?,?,?)', ['v-1', 'vm-1', '192.168.3.2']);
    await enroler(p, 'v-1', await hacher(SECRET), PREFIXE);
    return p;
}

describe(`enrolment of an agent, engine=${MOTEUR}`, () => {
    it('accepts the right pair, and returns the prefix of the VM', async () => {
        base = await baseEnrolee('enrol-bon');
        const journal: string[] = [];
        const v = await verifyEnrolment(base, 'v-1', SECRET, (l) => journal.push(l));
        expect(v).toEqual({ ok: true, vmId: 'v-1', prefixe: PREFIXE });
        // A success logs no refusal.
        expect(journal).toEqual([]);
    });

    it('🔴 refuses an UNKNOWN VM and a WRONG SECRET with the SAME refusal, word for word', async () => {
        // 🔴 The red: returning `'vm-inconnue'` on one side and `'secret-invalide'`
        // on the other. It is an enumeration oracle, and ONE character of
        // difference is enough for it to become one again — hence the
        // strict comparison of the two whole objects.
        base = await baseEnrolee('enrol-indistinct');
        const inconnue = await verifyEnrolment(base, 'v-jamais-enrolee', SECRET, () => {});
        const fauxSecret = await verifyEnrolment(base, 'v-1', 'pas-le-bon-secret', () => {});
        expect(inconnue.ok).toBe(false);
        expect(fauxSecret.ok).toBe(false);
        expect(inconnue).toEqual(fauxSecret);
    });

    it('logs the refusal WITH the requested VM name', async () => {
        // 🔴 The red: logging nothing. The refusal then becomes
        // undiagnosable — and it is the exact price of the indistinction
        // above: what the requester does not learn, the operator must
        // be able to read on their side. Same `message` / `journal` split as
        // `identite/garde.ts`.
        base = await baseEnrolee('enrol-journal');
        const journal: string[] = [];
        await verifyEnrolment(base, 'v-jamais-enrolee', SECRET, (l) => journal.push(l));
        expect(journal).toHaveLength(1);
        expect(journal[0]).toContain('v-jamais-enrolee');
        // 🔴 AND IT NEVER COPIES THE SECRET. The sweep of P2's criterion ④
        // applies as is: we search for the field NAME as much as the
        // value.
        expect(journal[0]).not.toContain(SECRET);
        expect(journal[0]).not.toContain('secret=');
    });

    it('🔴 returns a refusal, NEVER an exception, on a TRUNCATED fingerprint', async () => {
        // 🔴 The red: comparing without equalising the lengths first. MEASURED in
        // P2: `timingSafeEqual` THROWS `Input buffers must have the same byte
        // length`, and the caller would answer an internal error where it must
        // answer a refusal — the difference in behaviour would on its own be an
        // oracle. `identite/mot-de-passe.ts` already handles this case; this test
        // checks that enrolment does not undo it.
        base = await baseNeuve('enrol-tronquee');
        await base.executer('INSERT INTO vm(id,nom,adresse) VALUES(?,?,?)', ['v-1', 'vm-1', '10.0.0.1']);
        const entiere = await hacher(SECRET);
        await enroler(base, 'v-1', entiere.slice(0, entiere.length - 10), PREFIXE);

        const v = await verifyEnrolment(base, 'v-1', SECRET, () => {});
        expect(v.ok).toBe(false);
    });

    it('LETS THROUGH the exception of an unknown algorithm, without turning it into a refusal', async () => {
        // ⚠️ `identite/mot-de-passe.ts::verify` deliberately THROWS on an
        // unknown algorithm: a silent refusal there would be indistinguishable from a wrong
        // secret, and nobody could diagnose a database written by a
        // future version of the service. Enrolment must therefore NOT swallow it —
        // it is the channel that will translate it into a refusal, logging it WITH its
        // cause.
        base = await baseNeuve('enrol-algo');
        await base.executer('INSERT INTO vm(id,nom,adresse) VALUES(?,?,?)', ['v-1', 'vm-1', '10.0.0.1']);
        await enroler(base, 'v-1', 'argon2id$1$2$3$sel$empreinte', PREFIXE);
        await expect(verifyEnrolment(base, 'v-1', SECRET, () => {}))
            .rejects.toThrow(/unknown hash algorithm/i);
    });
});
