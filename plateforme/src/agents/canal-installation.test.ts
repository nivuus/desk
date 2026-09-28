// The `/agent` channel, INSTALLATION side: re-emission at enrolment, and the
// two upstream messages the agent reports.
//
// 🔴 A SEPARATE FILE, NOT AN ADDITION TO `canal-apps.test.ts`. That one is at
// 373 lines; adding a hundred and twenty lines would have brought it within reach of the cap,
// and this repository writes four times that the margin regained by an extraction is
// lost again if treated as granted. The harness is SHARED
// (`canal-harnais.ts`, extracted by G2 for this exact reason): copying it
// would have produced two `ouvrirUrl` that would diverge at the first fix
// applied to only one of the two.

import { afterEach, describe, expect, it, vi } from 'vitest';
import { WebSocketServer } from 'ws';
import {
    encodeEnroler,
    encodeProgression,
    encodeTermine,
    PLATEFORME_VERSION,
} from '../../../proto/ts/plateforme';
import { baseNeuve } from '../base/harnais';
import type { Pilote } from '../base/pilote';
import { Frein } from '../securite/frein';
import { create as createInstallation, lireParId } from '../depot/installation';
import { create as createUpload, sceller } from '../depot/televersement';
import { servirLeCanalAgent } from './canal';
import { enrolerUneVm, ouvrir, SECRET, SECRET_VM, T0, type Pair } from './canal-harnais';
import { RegistreAgents } from './registre';

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

async function start(p: Pilote): Promise<number> {
    maintenant = T0;
    registre = new RegistreAgents();
    wss = new WebSocketServer({ port: 0, host: '127.0.0.1' });
    await new Promise<void>((r) => wss!.once('listening', () => r()));
    servirLeCanalAgent(wss, {
        base: p,
        secretJeton: SECRET,
        maintenant: () => maintenant,
        registre,
        frein: new Frein(),
        proxyDeConfiance: new Set(),
        magasin: undefined,
    });
    const adresse = wss.address();
    return typeof adresse === 'object' && adresse ? adresse.port : 0;
}

async function enrole(pair: Pair): Promise<void> {
    const rep = await pair.dire(encodeEnroler('v-1', SECRET_VM));
    expect(rep.type).toBe('enrole');
}

/// A sealed upload, and an `en_attente` installation for VM `v-1`.
async function unOrdreEnAttente(p: Pilote): Promise<{ installation: string; tel: string }> {
    await p.executer(
        'INSERT INTO utilisateur(id,email,empreinte_mdp,cree_a) VALUES(?,?,?,?)',
        ['u-1', 'a@b.c', 'scrypt$1$1$1$x$y', T0],
    );
    const tel = await createUpload(
        p,
        {
            userId: 'u-1',
            nom: 'Firefox Setup 130.0.exe',
            taille: 3_221_225_472,
            sha256: 'a'.repeat(64),
            chunkSize: 8 * 1024 * 1024,
        },
        T0,
    );
    await sceller(p, tel.id, T0 + 1);
    const inst = await createInstallation(p, { vmId: 'v-1', televersementId: tel.id }, T0 + 2);
    return { installation: inst.id, tel: tel.id };
}

describe('re-sending the installations at enrolment', () => {
    // 🔴 THE RED OF CRITERION ④, AND IT IS FREE ON G1'S BINARY —
    // which has no `installer` variant at all. A WebSocket `push` has
    // NO delivery guarantee: without this re-emission, an order emitted
    // during an outage would be lost WITH NO END, and the user
    // would wait for an installation nobody would ever relaunch.
    //
    // ⚠️ THIS TEST SENDS NOTHING AFTER ENROLMENT, and that is the point: it
    // waits for a message the channel PUSHES on its own. `recevoir()` exists
    // for that — waiting for it through a `dire('')` would be a race, and
    // would cause a `forme` refusal one time in two.
    it('🔴 PUSHES the pending order, without the peer asking for anything', async () => {
        base = await baseNeuve('canal-inst-reemission');
        await enrolerUneVm(base, 'v-1');
        const { installation, tel } = await unOrdreEnAttente(base);
        const pair = await ouvrir(await start(base));

        await enrole(pair);
        const ordre = await pair.recevoir();
        expect(ordre).toEqual({
            type: 'installer',
            v: PLATEFORME_VERSION,
            installation,
            // ⚠️ THE URL IS RELATIVE, and that is deliberate: the agent resolves it
            // against the address of its own channel, as it already derives that
            // of the icon upload. Two variables for the same address
            // would diverge the day one of the two was changed.
            url: `/televersement/${tel}/contenu`,
            nom: 'Firefox Setup 130.0.exe',
            taille: 3_221_225_472,
            sha256: 'a'.repeat(64),
        });
        pair.socket.terminate();
    });

    // 🔴 THE HALF THAT PREVENTS DOUBLE EXECUTION, and without it the test
    // above would hold for a service that replays FOREVER. As soon as an
    // agent has reported a progress, the row goes `en_cours` and stops
    // being re-emitted. It is the FIRST of the two belts; the second is the
    // marker on the VM's disk, and it protects against the case where the first has
    // lost its database.
    it('🔴 NO LONGER RE-SENDS an installation already started', async () => {
        base = await baseNeuve('canal-inst-pas-deux-fois');
        await enrolerUneVm(base, 'v-1');
        const { installation } = await unOrdreEnAttente(base);
        const premier = await ouvrir(await start(base));
        await enrole(premier);
        // The first enrolment does receive it — it is the witness without which
        // the next assertion would hold for an entirely silent service.
        expect((await premier.recevoir()).installation).toBe(installation);

        // The agent reports a progress: the row leaves `en_attente`.
        // ⚠️ `send`, NEVER `dire`: a progress calls for NO answer,
        // and `dire` would wait for it two seconds before turning red for the wrong
        // reason. It is what the catalogue test already does, for the same
        // reason — we wait for the EFFECT in the database, not an acknowledgement that does not exist.
        premier.socket.send(encodeProgression(installation, 'transfert', 8_388_608, 3_221_225_472, 1_200));
        // ⚠️ THE WRITE IS NOT AWAITED BY THE CHANNEL (that is the rule of the
        // file), so we wait for the EFFECT rather than a duration.
        await attendreEtat(base, installation, 'en_cours');
        premier.socket.terminate();

        // A second enrolment must receive NOTHING.
        const second = await ouvrir(await start(base));
        await enrole(second);
        await expect(second.recevoir()).rejects.toThrow(/no message pushed/);
        second.socket.terminate();
    });
});

describe('the two upstream messages of the installation', () => {
    // 🔴 NEITHER PROGRESS NOR OUTCOME WITHOUT ENROLMENT. Accepting them would let an
    // anonymous peer write into the `installation` table of a VM that is not
    // its own — hence DECLARE SOMEONE ELSE'S INSTALLATION SUCCEEDED, OR REFUSED.
    // It is the hole that the heartbeat's `sequence` refusal already closes, through two
    // other doors.
    it('🔴 a `progression` before any enrolment is refused, reason `sequence`', async () => {
        base = await baseNeuve('canal-inst-seq-progression');
        await enrolerUneVm(base, 'v-1');
        const { installation } = await unOrdreEnAttente(base);
        const pair = await ouvrir(await start(base));

        const rep = await pair.dire(encodeProgression(installation, 'transfert', 1, 2, 3));
        expect(rep).toEqual({ type: 'refus', v: PLATEFORME_VERSION, motif: 'sequence' });
        // And NOTHING was written: that is the half that decides. A refusal that
        // still let the write through would be an open door
        // with a "closed" sign.
        expect((await lireParId(base, installation))?.etat).toBe('en_attente');
        pair.socket.terminate();
    });

    it('🔴 a `termine` before any enrolment is refused, reason `sequence`', async () => {
        base = await baseNeuve('canal-inst-seq-termine');
        await enrolerUneVm(base, 'v-1');
        const { installation } = await unOrdreEnAttente(base);
        const pair = await ouvrir(await start(base));

        const rep = await pair.dire(
            encodeTermine(installation, 'reussie', null, 0, '', false),
        );
        expect(rep).toEqual({ type: 'refus', v: PLATEFORME_VERSION, motif: 'sequence' });
        expect((await lireParId(base, installation))?.issue).toBeNull();
        pair.socket.terminate();
    });

    it('a valid `termine` writes the outcome, the reason and the exit code', async () => {
        base = await baseNeuve('canal-inst-termine');
        await enrolerUneVm(base, 'v-1');
        const { installation } = await unOrdreEnAttente(base);
        const pair = await ouvrir(await start(base));
        await enrole(pair);
        await pair.recevoir(); // the re-emitted order

        maintenant = T0 + 90_000;
        pair.socket.send(
            // ⚠️ 3010 IS A SUCCESS THAT ASKS FOR A REBOOT, and it is
            // REPORTED next to the outcome without anything deducing anything
            // from it: it is the agent, and the agent alone, that counted the applications
            // that appeared during its window.
            encodeTermine(installation, 'reussie', null, 3010, 'Restart required.', false),
        );
        await attendreEtat(base, installation, 'terminee');
        const ligne = await lireParId(base, installation);
        expect(ligne?.issue).toBe('reussie');
        expect(ligne?.code_sortie).toBe(3010);
        expect(ligne?.terminee_a).toBe(T0 + 90_000);
        pair.socket.terminate();
    });

    // ⚠️ A `termine` WHOSE WRITE FAILS MUST NOT TAKE DOWN THE CONNECTION.
    // An `await` on the path of a message would mean that a momentarily
    // unavailable database would kill the channel of an agent that is perfectly fine, and a
    // promise rejected without `catch` would take down the whole Node process.
    it("🔴 writing the outcome is NOT AWAITED, and its failure does not bring the connection down", async () => {
        base = await baseNeuve('canal-inst-echec-ecriture');
        await enrolerUneVm(base, 'v-1');
        const { installation } = await unOrdreEnAttente(base);
        const pair = await ouvrir(await start(base));
        await enrole(pair);
        await pair.recevoir();

        vi.spyOn(base, 'executer').mockRejectedValue(new Error('database unavailable'));
        vi.spyOn(console, 'error').mockImplementation(() => {});
        pair.socket.send(encodeTermine(installation, 'reussie', null, 0, '', false));
        // We let the handler run: the write is launched without being
        // awaited, and that is precisely the property under test.
        await new Promise((r) => setTimeout(r, 50));

        // The connection is still alive: the next heartbeat gets its answer.
        vi.restoreAllMocks();
        const rep = await pair.dire(JSON.stringify({ type: 'battement', v: PLATEFORME_VERSION }));
        expect(rep.type).toBe('battement-recu');
        pair.socket.terminate();
    });
});

/// Waits until the state of an installation reaches `attendu`, BOUNDED.
///
/// 🔴 WE WAIT FOR THE FACT, NEVER A DURATION. This channel's writes are
/// deliberately NOT AWAITED (`void … .catch(…)`), so an `await` on an
/// arbitrary duration would be a race: too short it would turn red on a slow
/// database, too long it would slow down the whole suite.
async function attendreEtat(p: Pilote, id: string, attendu: string): Promise<void> {
    for (let i = 0; i < 200; i += 1) {
        const ligne = await lireParId(p, id);
        if (ligne?.etat === attendu) return;
        await new Promise((r) => setTimeout(r, 10));
    }
    throw new Error(`installation ${id} did not reach the state ${attendu} within 2000 ms`);
}
