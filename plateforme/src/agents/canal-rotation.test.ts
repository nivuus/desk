// Rotating a VM's enrolment secret (`npm run admin:agent -- --roter`) must
// end what the OLD secret opened: a socket enrolled before the rotation is
// closed at its next heartbeat, with the enrolment refusal, and receives no
// fresh token. Without this, the socket would heartbeat forever and every
// beat would mint a new agent token, so "issued tokens stay valid until they
// expire" would bound nothing.

import { afterEach, describe, expect, it } from 'vitest';
import { WebSocketServer } from 'ws';
import { encodeBattement, encodeEnroler, PLATEFORME_VERSION } from '../../../proto/ts/plateforme';
import { roterLeSecret } from '../admin/enroler-agent';
import { baseNeuve } from '../base/harnais';
import type { Pilote } from '../base/pilote';
import { Frein } from '../securite/frein';
import { servirLeCanalAgent } from './canal';
import { enrolerUneVm, ouvrir, SECRET, SECRET_VM, T0 } from './canal-harnais';
import { RegistreAgents } from './registre';

let base: Pilote | undefined;
let wss: WebSocketServer | undefined;

afterEach(async () => {
    if (wss) {
        for (const socket of wss.clients) socket.terminate();
        await new Promise<void>((r) => wss!.close(() => r()));
        wss = undefined;
    }
    await base?.fermer();
    base = undefined;
});

async function start(p: Pilote): Promise<number> {
    wss = new WebSocketServer({ port: 0, host: '127.0.0.1' });
    await new Promise<void>((r) => wss!.once('listening', () => r()));
    servirLeCanalAgent(wss, {
        base: p,
        secretJeton: SECRET,
        maintenant: () => T0,
        registre: new RegistreAgents(),
        frein: new Frein(),
        proxyDeConfiance: new Set(),
        magasin: undefined,
    });
    const adresse = wss.address();
    return typeof adresse === 'object' && adresse ? adresse.port : 0;
}

describe('rotating the enrolment secret', () => {
    it('🔴 the next heartbeat of a socket of the old secret is refused and closes it', async () => {
        base = await baseNeuve('canal-rotation');
        await enrolerUneVm(base, 'v-1');
        const pair = await ouvrir(await start(base));
        expect((await pair.dire(encodeEnroler('v-1', SECRET_VM))).type).toBe('enrole');

        // Witness: before the rotation, a heartbeat gets a fresh token.
        expect((await pair.dire(encodeBattement())).type).toBe('battement-recu');

        const rotation = await roterLeSecret(base, 'v-1');
        expect('refus' in rotation).toBe(false);

        const rep = await pair.dire(encodeBattement());
        expect(rep).toEqual({ type: 'refus', v: PLATEFORME_VERSION, motif: 'enrolement' });
        expect(rep).not.toHaveProperty('jeton');
        await pair.ferme;
        expect(pair.ordre.at(-1)).toBe('close');
    });
});
