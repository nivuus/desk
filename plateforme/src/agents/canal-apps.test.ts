// The `/agent` channel, APPLICATIONS side: the catalogue an agent pushes, and
// the launch outcome it reports.
//
// 🔴 THIS FILE WAS BORN OF AN EXTRACTION, NOT A DUPLICATION: the harness
// it shares with `canal.test.ts` lives in `canal-harnais.ts`, extracted before
// these cases were written. Copying them would have produced two `ouvrirUrl`
// that would diverge at the first fix applied to only one of the two.
//
// 🔴 THE TWO `sequence` REFUSALS ARE THE HALF THAT COUNTS. A peer that
// presented no secret could otherwise write into the `application` table
// of a VM it did not authenticate — it is the exact hole that the heartbeat
// refusal already closes, through another door.

import { createHash } from 'node:crypto';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { WebSocketServer } from 'ws';
import {
    PLATEFORME_VERSION,
    encodeCatalogue,
    encodeEnroler,
    encodeLancee,
    type Application,
} from '../../../proto/ts/plateforme';
import { baseNeuve } from '../base/harnais';
import type { Pilote } from '../base/pilote';
import { lireParVm } from '../depot/application';
import { servirLeCanalAgent } from './canal';
import { enrolerUneVm, ouvrir, SECRET, SECRET_VM, T0, type Pair } from './canal-harnais';
import { RegistreAgents } from './registre';
import { Frein } from '../securite/frein';
import { ouvrirMagasin, type Magasin } from '../apps/icones';

let base: Pilote | undefined;
let wss: WebSocketServer | undefined;
let registre = new RegistreAgents();
let maintenant = T0;

afterEach(async () => {
    if (wss) {
        for (const socket of wss.clients) socket.terminate();
        await new Promise<void>((r) => wss!.close(() => r()));
        wss = undefined;
    }
    await base?.fermer();
    base = undefined;
    vi.restoreAllMocks();
});

/// The store of the current mount — `undefined` as long as no test asks for one.
let magasin: Magasin | undefined;
let racinesIcones: string[] = [];

async function start(p: Pilote, withStore = false): Promise<number> {
    maintenant = T0;
    registre = new RegistreAgents();
    magasin = undefined;
    if (withStore) {
        const r = mkdtempSync(join(tmpdir(), 'g2-canal-icones-'));
        racinesIcones.push(r);
        magasin = ouvrirMagasin(join(r, 'icones'), () => {});
    }
    wss = new WebSocketServer({ port: 0, host: '127.0.0.1' });
    await new Promise<void>((r) => wss!.once('listening', () => r()));
    servirLeCanalAgent(wss, {
        base: p,
        secretJeton: SECRET,
        maintenant: () => maintenant,
        registre,
        // A FRESH brake per mount: this file tests the catalogue and the
        // launch, not the braking, and a brake shared between tests would make
        // the ADDRESS budgets overflow (127.0.0.1 is the same for all).
        frein: new Frein(),
        // No proxy declared: the address key is that of the real peer.
        proxyDeConfiance: new Set(),
        magasin,
    });
    const adresse = wss.address();
    return typeof adresse === 'object' && adresse ? adresse.port : 0;
}

function app(nom: string, cle: string, icone: string | null = null): Application {
    return {
        cle,
        nom,
        chemin: `C:\\Users\\guacamole\\Desktop\\${nom}.lnk`,
        cible: `c:\\program files\\${nom}\\${nom}.exe`,
        arguments: '',
        repertoire: `c:\\program files\\${nom}`,
        icone,
        source_max: icone === null ? 'non-mesuree' : { pixels: 256 },
        accent: null,
        associations: [],
    };
}

/// Waits until the VM's catalogue satisfies `predicat`, or FAILS.
///
/// ⚠️ BOUNDED, AND FAILING ON EXPIRY — same pattern as `attendreVu`, and for
/// the same reason: the catalogue write is deliberately launched WITHOUT being
/// awaited, so a lost write only shows as a state that
/// never arrives. An unbounded loop would hang instead of turning red.
async function attendreCatalogue(
    p: Pilote,
    vmId: string,
    predicat: (noms: string[]) => boolean,
    quoi: string,
    borneMs = 2000,
): Promise<string[]> {
    const fin = Date.now() + borneMs;
    for (;;) {
        const noms = (await lireParVm(p, vmId)).map((l) => l.nom);
        if (predicat(noms)) return noms;
        if (Date.now() > fin) {
            throw new Error(`catalogue ${quoi} never reached for ${vmId} (seen=${noms.join(',')})`);
        }
        await new Promise((r) => setTimeout(r, 25));
    }
}

/// Enrols the peer and waits for the answer — prerequisite of all nominal cases.
async function enrole(pair: Pair): Promise<void> {
    const rep = await pair.dire(encodeEnroler('v-1', SECRET_VM));
    expect(rep.type).toBe('enrole');
}

describe('the /agent channel, applications side', () => {
    it('🔴 a `catalogue` BEFORE any enrolment is refused, reason `sequence`', async () => {
        // 🔴 ACCEPTING IT WOULD LET AN ANONYMOUS PEER WRITE INTO THE
        // `application` TABLE OF A VM IT DID NOT AUTHENTICATE. It is the
        // exact hole the heartbeat's `sequence` refusal already closes, through another
        // door — and this one writes to the database, where that one only handed out a
        // token.
        base = await baseNeuve('canal-apps-sequence-catalogue');
        await enrolerUneVm(base, 'v-1');
        const pair = await ouvrir(await start(base));

        const rep = await pair.dire(encodeCatalogue(true, [app('Intrus', 'cle-intrus')], []));
        expect(rep).toEqual({ type: 'refus', v: PLATEFORME_VERSION, motif: 'sequence' });
        // And NOTHING was written: that is the half that decides. A refusal that
        // still let the write through would be an open door
        // with a "closed" sign.
        expect(await lireParVm(base, 'v-1')).toEqual([]);
        pair.socket.terminate();
    });

    it('🔴 a `lancee` before any enrolment is refused, reason `sequence`', async () => {
        // Same reason: an anonymous peer could otherwise resolve another's
        // request, and fake a successful launch that never happened.
        base = await baseNeuve('canal-apps-sequence-lancee');
        await enrolerUneVm(base, 'v-1');
        const pair = await ouvrir(await start(base));

        const rep = await pair.dire(encodeLancee('d-1', 'raccourci'));
        expect(rep).toEqual({ type: 'refus', v: PLATEFORME_VERSION, motif: 'sequence' });
        pair.socket.terminate();
    });

    it('a valid `catalogue` is MERGED and WRITTEN', async () => {
        base = await baseNeuve('canal-apps-ecriture');
        await enrolerUneVm(base, 'v-1');
        const pair = await ouvrir(await start(base));
        await enrole(pair);

        pair.socket.send(encodeCatalogue(true, [app('Firefox', 'c-1'), app('Excel', 'c-2')], []));
        expect(await attendreCatalogue(base, 'v-1', (n) => n.length === 2, 'with two entries'))
            .toEqual(['Excel', 'Firefox']);

        // And a second COMPLETE message without Excel removes it — the
        // merge decides, and it is wired on the real path.
        maintenant = T0 + 30_000;
        pair.socket.send(encodeCatalogue(true, [app('Firefox', 'c-1')], []));
        expect(await attendreCatalogue(base, 'v-1', (n) => n.length === 1, 'with one entry'))
            .toEqual(['Firefox']);
        pair.socket.terminate();
    });

    it("🔴 writing the catalogue is NOT AWAITED, and its failure does not bring the connection down", async () => {
        // 🔴 AN `await` IN THE `message` HANDLER WOULD MEAN THAT A DATABASE
        // MOMENTARILY UNAVAILABLE WOULD TAKE DOWN THE CONNECTION OF AN AGENT THAT IS
        // PERFECTLY FINE — and a promise rejected WITHOUT `catch` would take down the whole
        // Node process. It is the rule this channel has held itself to since P3 for
        // `marquerVu`.
        //
        // The database is CLOSED under the channel's feet: every write throws.
        // The peer, for its part, must keep being served.
        base = await baseNeuve('canal-apps-echec-ecriture');
        await enrolerUneVm(base, 'v-1');
        const journal = vi.spyOn(console, 'error').mockImplementation(() => {});
        const pair = await ouvrir(await start(base));
        await enrole(pair);

        await base.fermer();
        pair.socket.send(encodeCatalogue(true, [app('Firefox', 'c-1')], []));

        // The connection is still alive, and still answers: the refusal of a
        // malformed message is the least ambiguous proof that the loop runs.
        const rep = await pair.dire('not json');
        expect(rep).toEqual({ type: 'refus', v: PLATEFORME_VERSION, motif: 'forme' });
        // And the failure is IN THE LOG, never silent: a lost write
        // shows nowhere else.
        expect(journal.mock.calls.map((c) => String(c[0])).join('\n')).toContain('v-1');

        pair.socket.terminate();
        base = undefined;
    });

    it("enrolment REGISTERS the VM in the registry, and closing REMOVES it", async () => {
        // 🔴 Forgetting the registration would make every launch return
        // `agent-injoignable`, for a perfectly connected VM. Forgetting the removal
        // would do the opposite: a dead VM would stay "reachable" until the
        // next enrolment, and each launch would cost five seconds
        // of waiting before failing.
        base = await baseNeuve('canal-apps-registre');
        await enrolerUneVm(base, 'v-1');
        const pair = await ouvrir(await start(base));

        // BEFORE enrolment: nobody. It is the witness without which
        // the next assertion would hold for a registry that accepted everything.
        await expect(registre.lancer('v-1', 'c-1', 'd-0')).resolves.toBe('agent-injoignable');

        await enrole(pair);
        const enVol = registre.lancer('v-1', 'c-1', 'd-1');
        // The order did LEAVE on the socket: the peer receives it.
        const ordre = await pair.recevoir();
        expect(ordre).toEqual({
            type: 'lancer',
            v: PLATEFORME_VERSION,
            demande: 'd-1',
            cle: 'c-1',
        });

        // Closing removes the VM — and rejects the request in flight.
        pair.socket.close();
        expect(await enVol).toBe('agent-injoignable');
        await expect(registre.lancer('v-1', 'c-1', 'd-2')).resolves.toBe('agent-injoignable');
    });

    it('a `lancee` RESOLVES the request in flight, with its outcome', async () => {
        base = await baseNeuve('canal-apps-lancee');
        await enrolerUneVm(base, 'v-1');
        const pair = await ouvrir(await start(base));
        await enrole(pair);

        const enVol = registre.lancer('v-1', 'c-1', 'd-1');
        const ordre = await pair.recevoir();
        expect(ordre.demande).toBe('d-1');

        pair.socket.send(encodeLancee('d-1', 'raccourci'));
        // 🔴 `raccourci` AND NOT `true`: it is the PATH taken that makes the
        // acceptance criterion decidable — launching through the rebuilt target instead
        // of the `.lnk` would pass a criterion that would only say "something
        // launched".
        expect(await enVol).toBe('raccourci');
        pair.socket.terminate();
    });
});

// ---------------------------------------------------------------------------
// Sub-block G2 — the inventory of missing icons.
// ---------------------------------------------------------------------------

/// 🔴 THE HASHES ARE DERIVED FROM THEIR CONTENT, NEVER INVENTED. A
/// first draft set `'a'.repeat(64)` and dropped arbitrary
/// bytes under it: `write` REFUSED it — the recomputation guard
/// doing exactly its job, on the test that ignored it.
const OCTETS_1 = Buffer.from('\x89PNG-un');
const OCTETS_2 = Buffer.from('\x89PNG-deux');
const E1 = createHash('sha256').update(OCTETS_1).digest('hex');
const E2 = createHash('sha256').update(OCTETS_2).digest('hex');

/// Waits for the next pushed message, or returns `undefined` if none
/// comes.
///
/// 🔴 A POSITIVE WAIT IS NEEDED TO BE ABLE TO CONCLUDE ABSENCE. The
/// message is pushed AFTER the catalogue write, which is deliberately
/// launched WITHOUT being awaited: concluding too early would turn criterion ⑤ green
/// on a product that does push a message. `recevoir()` is bounded to
/// 2,000 ms and THROWS on expiry — that throw is what means "none".
async function pousseOuRien(pair: Pair): Promise<Record<string, unknown> | undefined> {
    try {
        return await pair.recevoir();
    } catch {
        return undefined;
    }
}

describe("the inventory of missing icons", () => {
    afterEach(() => {
        for (const r of racinesIcones) rmSync(r, { recursive: true, force: true });
        racinesIcones = [];
    });

    it('asks for the fingerprints the store does NOT have', async () => {
        base = await baseNeuve('canal-icones-manque');
        const port = await start(base, true);
        await enrolerUneVm(base, 'v-1');
        const pair = await ouvrir(port);
        await enrole(pair);
        pair.socket.send(encodeCatalogue(true, [app('A', 'c-a', E1), app('B', 'c-b', E2)], []));
        const message = await pousseOuRien(pair);
        expect(message, "the inventory must be pushed").toBeDefined();
        expect(message!.type).toBe('icones-manquantes');
        expect(message!.empreintes).toEqual([E1, E2]);
        expect(message!.v).toBe(PLATEFORME_VERSION);
        pair.socket.close();
    });

    it('🔴 PUSHES NOTHING when the store already has everything — criterion ⑤', async () => {
        // 🔴 AN EMPTY LIST WOULD COST ONE MESSAGE PER RECONCILIATION ON AN
        // IDLE DISK, that is every thirty seconds, forever.
        // It is exactly what G1's diff exists to
        // avoid, and it is where criterion ⑤ is judged.
        base = await baseNeuve('canal-icones-rien');
        const port = await start(base, true);
        magasin!.write(E1, OCTETS_1);
        magasin!.write(E2, OCTETS_2);
        await enrolerUneVm(base, 'v-1');
        const pair = await ouvrir(port);
        await enrole(pair);
        pair.socket.send(encodeCatalogue(true, [app('A', 'c-a', E1), app('B', 'c-b', E2)], []));
        // 🔴 THE CATALOGUE DID ARRIVE — otherwise this "nothing" would be that
        // of a BROKEN product, and would say nothing at all.
        await attendreCatalogue(base, 'v-1', (n) => n.includes('A'), 'written');
        expect(await pousseOuRien(pair)).toBeUndefined();
        pair.socket.close();
    });

    it('asks ONLY for what is missing, and ignores the applications WITHOUT an icon', async () => {
        base = await baseNeuve('canal-icones-partiel');
        const port = await start(base, true);
        magasin!.write(E1, OCTETS_1);
        await enrolerUneVm(base, 'v-1');
        const pair = await ouvrir(port);
        await enrole(pair);
        pair.socket.send(
            encodeCatalogue(true, [app('A', 'c-a', E1), app('B', 'c-b', E2), app('C', 'c-c')], []),
        );
        const message = await pousseOuRien(pair);
        expect(message!.empreintes).toEqual([E2]);
        pair.socket.close();
    });

    it('🔴 ASKS AGAIN for an icon whose FILE has disappeared — criterion ⑦', async () => {
        // 🔴 THE INVENTORY QUERIES THE DISK, NOT A TABLE. A bookkeeping
        // table would not see the loss, and the icon would be lost
        // FOREVER. That is what makes the store SELF-REBUILDING, and therefore
        // the disk acceptable.
        base = await baseNeuve('canal-icones-perdu');
        const port = await start(base, true);
        magasin!.write(E1, OCTETS_1);
        rmSync(join(magasin!.repertoire, E1));
        await enrolerUneVm(base, 'v-1');
        const pair = await ouvrir(port);
        await enrole(pair);
        pair.socket.send(encodeCatalogue(true, [app('A', 'c-a', E1)], []));
        const message = await pousseOuRien(pair);
        expect(message!.empreintes).toEqual([E1]);
        pair.socket.close();
    });

    it('without a store, no inventory — and the catalogue is written anyway', async () => {
        base = await baseNeuve('canal-icones-sans-magasin');
        const port = await start(base, false);
        await enrolerUneVm(base, 'v-1');
        const pair = await ouvrir(port);
        await enrole(pair);
        pair.socket.send(encodeCatalogue(true, [app('A', 'c-a', E1)], []));
        await attendreCatalogue(base, 'v-1', (n) => n.includes('A'), 'written');
        expect(await pousseOuRien(pair)).toBeUndefined();
        pair.socket.close();
    });
});
