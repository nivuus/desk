// The loop of the `/agent` channel, on REAL `WebSocket`s, port 0 assigned by
// the system and an INJECTED clock — same setup as `garde-fil.test.ts`, and
// for the same reason: two of the seven criteria below require time to
// MOVE between two messages, and a frozen clock would make them inert.
//
// 🔴 THIS FILE CROSSES TWO MODULES ON PURPOSE. The second test takes the token
// the channel delivers and presents it to the relay's GUARD. It is the only
// place in the repository where the full chain — enrolment, signature, type
// claim, session prefix — is tested end to end; each of its links
// is right on its own, and it is precisely the class of defect that crosses
// a task boundary that this repository pays for on every branch.

import { afterEach, describe, expect, it, vi } from 'vitest';
import { WebSocket, WebSocketServer } from 'ws';
import {
    PLATEFORME_VERSION,
    encodeBattement,
    encodeEnroler,
} from '../../../proto/ts/plateforme';
import { baseNeuve, piloteCompteur } from '../base/harnais';
import type { Pilote } from '../base/pilote';
import type { Config } from '../config';
import { startServer, type ServicePlateforme } from '../http/serveur';
import { garde as fabriquerGarde } from '../identite/garde';
import { mkdtempSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { DUREE_JETON_ACCES_MS, verifyToken } from '../identite/jeton';
import { ProprieteDeSession } from '../signaling/propriete';
import { servirLeCanalAgent } from './canal';
import { RegistreAgents } from './registre';
import { ECHECS_MAX_ADRESSE, ECHECS_MAX_COMPTE, Frein } from '../securite/frein';
import {
    attendreVu,
    enrolerUneVm,
    ouvrir,
    ouvrirUrl,
    P,
    SECRET,
    SECRET_VM,
    T0,
} from './canal-harnais';

let base: Pilote | undefined;
let wss: WebSocketServer | undefined;
let service: ServicePlateforme | undefined;
let maintenant = T0;

afterEach(async () => {
    if (wss) {
        for (const socket of wss.clients) socket.terminate();
        await new Promise<void>((r) => wss!.close(() => r()));
        wss = undefined;
    }
    await service?.close();
    service = undefined;
    await base?.fermer();
    base = undefined;
    vi.restoreAllMocks();
});

/// Mounts the channel on a system-assigned port, with the file's
/// clock. We wait for `listening`: reading `address()` before the socket is
/// bound would return `null`, and the test would connect to a nonexistent port.
async function start(p: Pilote, frein: Frein = new Frein()): Promise<number> {
    maintenant = T0;
    wss = new WebSocketServer({ port: 0, host: '127.0.0.1' });
    await new Promise<void>((r) => wss!.once('listening', () => r()));
    servirLeCanalAgent(wss, {
        base: p,
        secretJeton: SECRET,
        maintenant: () => maintenant,
        registre: new RegistreAgents(),
        frein,
        // No proxy declared: `adresseSource` will therefore ignore any
        // `X-Forwarded-For`, and the address key will be that of the real peer.
        proxyDeConfiance: new Set(),
    });
    const adresse = wss.address();
    return typeof adresse === 'object' && adresse ? adresse.port : 0;
}

describe('the loop of the /agent channel', () => {
    it('`enroler` with the RIGHT secret returns `enrole`, with prefix and token', async () => {
        base = await baseNeuve('canal-enrole');
        await enrolerUneVm(base, 'v-1');
        const pair = await ouvrir(await start(base));

        const rep = await pair.dire(encodeEnroler('v-1', SECRET_VM));
        expect(rep.type).toBe('enrole');
        expect(rep.v).toBe(PLATEFORME_VERSION);
        expect(rep.prefixe).toBe(P);
        expect(typeof rep.jeton).toBe('string');
        // The expiry is EXACT, not "greater than zero": it is
        // the assertion that forbids a `Date.now()` hidden in the channel.
        expect(rep.expire_a).toBe(T0 + DUREE_JETON_ACCES_MS);
        pair.socket.terminate();
    });

    it('🔴 the returned token is VERIFIABLE BY THE GUARD, and of type `agent`', async () => {
        // 🔴 THIS TEST CROSSES TWO MODULES ON PURPOSE. The red is signing
        // without the type claim: the token would remain a perfectly
        // valid JWT, and the guard would refuse it for the `agent` role — a channel
        // that delivers tokens nothing accepts. Each of the two modules
        // is right on its own; it is their JUNCTION that is tested only here.
        base = await baseNeuve('canal-jeton-garde');
        await enrolerUneVm(base, 'v-1');
        const pair = await ouvrir(await start(base));
        const rep = await pair.dire(encodeEnroler('v-1', SECRET_VM));

        // The token's SUBJECT is the PREFIX, and its type is `agent`.
        expect(verifyToken(rep.jeton, SECRET, T0)).toEqual({
            ok: true,
            sujet: P,
            type: 'agent',
        });

        // And the relay's guard accepts it for a session this prefix
        // carries — that is, for the REAL use of the token.
        const g = fabriquerGarde(SECRET, () => T0, new ProprieteDeSession());
        expect(g.verify({ role: 'agent', session: `${P}:bureau`, jeton: rep.jeton })).toEqual({
            ok: true,
        });
        // WITNESS, in the same run: the guard refuses this same token for
        // a session the prefix does NOT carry. Without it, the acceptance
        // above would hold for a guard that looked at nothing.
        expect(g.verify({ role: 'agent', session: 'bureau', jeton: rep.jeton }).ok).toBe(false);
        pair.socket.terminate();
    });

    it('🔴 `enroler` with the WRONG secret returns `refus` and CLOSES the socket, without writing the secret to the log', async () => {
        // 🔴 CLOSING IS THE FREE HALF. A refused peer that
        // kept its socket open could retry without limit on the
        // same connection. ✅ THE OTHER HALF HAS EXISTED SINCE P5 — the channel's
        // brake, tested by `describe('the brake of the /agent channel')` further down
        // in this file. This line used to say "the denial of service P5 must
        // brake": it is done, and both halves now live side by
        // side here.
        base = await baseNeuve('canal-mauvais-secret');
        await enrolerUneVm(base, 'v-1');
        const journal = vi.spyOn(console, 'warn').mockImplementation(() => {});
        const pair = await ouvrir(await start(base));

        const rep = await pair.dire(encodeEnroler('v-1', 'un-autre-secret-de-la-vraie-longueur'));
        expect(rep).toEqual({ type: 'refus', v: PLATEFORME_VERSION, motif: 'enrolement' });

        await pair.ferme;
        // SEND THEN CLOSE, never the reverse: a peer that saw a
        // close with no reason would not know whether to correct itself or
        // retry.
        expect(pair.ordre).toEqual(['message', 'close']);
        expect(pair.socket.readyState).toBe(WebSocket.CLOSED);

        // ⚠️ THE LOG NAMES THE VM, NEVER THE SECRET. We sweep on the NAME of the
        // field and not on its value: searching for the value would pass a
        // log that wrote "secret=" followed by something else.
        const lignes = journal.mock.calls.map((c) => String(c[0])).join('\n');
        expect(lignes).toContain('v-1');
        expect(lignes).not.toContain('secret');
    });

    it('🔴 a message of version PLATEFORME_VERSION + 1 is refused, reason `version`', async () => {
        // 🔴 It is the TypeScript half of criterion ③. Omitting it would make
        // the fields of a message from a future version be interpreted with the
        // meaning of ours.
        base = await baseNeuve('canal-version');
        await enrolerUneVm(base, 'v-1');
        const pair = await ouvrir(await start(base));

        const rep = await pair.dire(
            JSON.stringify({ type: 'battement', v: PLATEFORME_VERSION + 1 }),
        );
        expect(rep).toEqual({ type: 'refus', v: PLATEFORME_VERSION, motif: 'version' });
        // ⚠️ AND THE SOCKET CLOSES, a decision of this module and not of the plan: a
        // peer that does not speak our version will NEVER succeed on this
        // connection, and leaving it open would make it loop instead of its
        // exponential-backoff reconnection.
        await pair.ferme;
        expect(pair.ordre).toEqual(['message', 'close']);
    });

    it('🔴 `battement` BEFORE `enroler` is refused, reason `sequence`, and issues NO token', async () => {
        // 🔴 BOTH HALVES COUNT, and the second is the real one. A
        // `battement-recu` returned to an anonymous peer would carry a TOKEN — that is,
        // the channel would sign an agent identity for a peer that
        // presented no secret. It is exactly E12's leak in another
        // form.
        base = await baseNeuve('canal-sequence');
        await enrolerUneVm(base, 'v-1');
        const pair = await ouvrir(await start(base));

        const rep = await pair.dire(encodeBattement());
        expect(rep).toEqual({ type: 'refus', v: PLATEFORME_VERSION, motif: 'sequence' });
        expect(rep.jeton).toBeUndefined();
        // The socket stays OPEN: unlike the wrong secret, a sequence
        // error can be corrected — the peer can enrol then resume. Same
        // split as the relay between the malformed message and the refused
        // handshake.
        expect(pair.socket.readyState).toBe(WebSocket.OPEN);
        pair.socket.terminate();
    });

    it('🔴 `battement` after `enroler` ADVANCES `vu_a` in the database', async () => {
        // 🔴 Writing nothing would leave `vu_a` frozen, and the VM would be
        // forever `injoignable` (`agents/fraicheur.ts`) while it
        // beats. The clock MOVES between the two messages: without that, a
        // `vu_a` simply copied from the enrolment would pass the test.
        base = await baseNeuve('canal-vu-a');
        await enrolerUneVm(base, 'v-1');
        const pair = await ouvrir(await start(base));

        await pair.dire(encodeEnroler('v-1', SECRET_VM));
        await attendreVu(base, 'v-1', (vu) => vu === T0, 'set at enrolment');

        maintenant = T0 + 30_000;
        await pair.dire(encodeBattement());
        expect(await attendreVu(base, 'v-1', (vu) => vu === T0 + 30_000, 'advanced by the heartbeat')).toBe(
            T0 + 30_000,
        );
        pair.socket.terminate();
    });

    it('🔴 `battement` returns a FRESH token, valid at an instant when the previous one has EXPIRED', async () => {
        // 🔴 Returning the same token would make the agent drop at the expiry of the
        // first one — ten minutes after enrolment —, without seeing it coming.
        // Comparing the two `expire_a` is not enough to say so: what says so
        // is that at the instant the OLD one is refused, the NEW one passes.
        base = await baseNeuve('canal-jeton-frais');
        await enrolerUneVm(base, 'v-1');
        const pair = await ouvrir(await start(base));

        const premier = await pair.dire(encodeEnroler('v-1', SECRET_VM));
        maintenant = T0 + 30_000;
        const second = await pair.dire(encodeBattement());

        expect(second.type).toBe('battement-recu');
        expect(second.expire_a).toBe(T0 + 30_000 + DUREE_JETON_ACCES_MS);
        expect(Number(second.expire_a)).toBeGreaterThan(Number(premier.expire_a));

        // The EXACT instant the first one dies: the bound is strict
        // (`maintenant >= exp`), so the first is refused and the second passes.
        const instantCritique = T0 + DUREE_JETON_ACCES_MS;
        expect(verifyToken(premier.jeton, SECRET, instantCritique)).toEqual({
            ok: false,
            motif: 'expire',
        });
        expect(verifyToken(second.jeton, SECRET, instantCritique).ok).toBe(true);
        pair.socket.terminate();
    });
});

describe('the channel, WIRED into the whole service', () => {
    it('🔴 the `/agent` path of the service really serves the channel', async () => {
        // 🔴 WITHOUT THIS TEST, FORGETTING THE CALL IN `http/serveur.ts` WOULD TURN
        // RED NOWHERE. Task 13 only tests the MOUNTING of the `/agent` path —
        // a `WebSocketServer` that accepts the connection and listens to nothing would
        // pass it. It is the exact silent failure `serveur.ts` already invokes
        // to make `base` and `garde` REQUIRED.
        //
        // The clock is not injectable here (`startServer` reads `Date.now`),
        // so the expiry is not asserted on an exact value: this test
        // measures the WIRING, the seven above measure the loop.
        const config: Config = {
            hote: '127.0.0.1',
            port: 0,
            base: 'sqlite',
            urlBase: ':memory:',
            secretJeton: SECRET,
            // No proxy declared: see `config.ts`, the empty set is the
            // default and means "trust nobody's announced address".
            proxyDeConfiance: new Set(),
            repertoireIcones: join(mkdtempSync(join(tmpdir(), 'g2-icones-')), 'icones'),
            repertoireTeleversements: join(mkdtempSync(join(tmpdir(), 'g3-tranches-')), 'televersements'),
            auth: 'pomerium',
        };
        base = await baseNeuve('canal-service');
        await enrolerUneVm(base, 'v-1');
        service = await startServer(config, base);

        const surLeChemin = await ouvrirUrl(`ws://127.0.0.1:${service.port}/agent`);
        const rep = await surLeChemin.dire(encodeEnroler('v-1', SECRET_VM));
        expect(rep.type).toBe('enrole');
        expect(rep.prefixe).toBe(P);
        surLeChemin.socket.terminate();
    });
});

describe('the brake of the /agent channel', () => {
    /// A complete enrolment attempt: open, say, read the refusal.
    ///
    /// ⚠️ A NEW SOCKET EACH TIME, and it is not zeal: the reason
    /// `enrolement` CLOSES the socket (`MOTIFS_FERMANTS`), and reusing the peer
    /// would measure a dead socket.
    async function tenter(port: number, vm: string, secret: string): Promise<Record<string, unknown>> {
        const pair = await ouvrir(port);
        const rep = await pair.dire(encodeEnroler(vm, secret));
        pair.socket.terminate();
        return rep;
    }

    it('(a) the n+1th attempt on the SAME VM is refused by the brake', async () => {
        base = await baseNeuve('canal-frein-vm');
        await enrolerUneVm(base, 'v-1');
        const port = await start(base);
        for (let i = 0; i < ECHECS_MAX_COMPTE; i++) {
            expect((await tenter(port, 'v-1', 'ce-n-est-pas-le-bon-secret')).type).toBe('refus');
        }
        // The refusal is the SAME (see (c)); what changed is its COST,
        // which (e) measures.
        expect((await tenter(port, 'v-1', 'ce-n-est-pas-le-bon-secret')).type).toBe('refus');
    }, 30000);

    it("(b) the n+1th from the SAME address, VMs all DISTINCT, is braked", async () => {
        const reel = await baseNeuve('canal-frein-adresse');
        base = reel;
        const compteur = piloteCompteur(reel);
        const port = await start(compteur.pilote);
        // No VM budget can bite: each name is tried ONCE.
        for (let i = 0; i < ECHECS_MAX_ADRESSE; i++) {
            await tenter(port, `inconnue-${i}`, 'peu-importe');
        }
        compteur.remettre();
        expect((await tenter(port, 'yet-another-one', 'peu-importe')).type).toBe('refus');
        // 🔴 The discriminant: the braked attempt read NOTHING from the database.
        expect(compteur.acces()).toBe(0);
    }, 60000);

    it('(c) 🔴 the braked refusal is the SAME MESSAGE as the enrolment refusal', async () => {
        // 🔴 A DISTINCT `frein` REASON WOULD GIVE THE ATTACKER THE INFORMATION
        // "this VM exists and I made it trigger": it is exactly the
        // ENUMERATION ORACLE that `agents/enrolement.ts` closes over three
        // paragraphs, reopened through the brake's door. The two refusals are
        // produced IN THE SAME TEST and compared object for object.
        base = await baseNeuve('canal-frein-oracle');
        await enrolerUneVm(base, 'v-1');
        const port = await start(base);

        const refusNonFreine = await tenter(port, 'v-1', 'ce-n-est-pas-le-bon-secret');
        for (let i = 0; i < ECHECS_MAX_COMPTE; i++) {
            await tenter(port, 'v-1', 'ce-n-est-pas-le-bon-secret');
        }
        const refusFreine = await tenter(port, 'v-1', 'ce-n-est-pas-le-bon-secret');
        expect(refusFreine).toEqual(refusNonFreine);
    }, 30000);

    it('(d) the LOG, however, tells the two apart', async () => {
        // Same split as `identite/garde.ts`: `message` on the wire,
        // `journal` on our side. The requester learns nothing; the operator does.
        const avertir = vi.spyOn(console, 'warn').mockImplementation(() => {});
        base = await baseNeuve('canal-frein-journal');
        await enrolerUneVm(base, 'v-1');
        const port = await start(base);
        for (let i = 0; i < ECHECS_MAX_COMPTE; i++) {
            await tenter(port, 'v-1', 'ce-n-est-pas-le-bon-secret');
        }
        await tenter(port, 'v-1', 'ce-n-est-pas-le-bon-secret');
        const lignes = avertir.mock.calls.map((c) => String(c[0]));
        // The ordinary enrolment refusal, written by `enrolement.ts`.
        expect(lignes.some((l) => l.includes('enrolment refused for VM v-1'))).toBe(true);
        // And the brake's line, which exists ONLY on the operator's side.
        const freinees = lignes.filter((l) => l.startsWith('frein '));
        expect(freinees.length).toBeGreaterThanOrEqual(1);
        expect(freinees[0]).toContain('route=/agent');
        expect(freinees[0]).toContain('adresse=');
    }, 30000);

    it('(e) 🔴 the braked refusal reads NOTHING from the database — so derives no `scrypt`', async () => {
        // 🔴 IT IS THE POINT OF THE WHOLE TASK. `verifyEnrolment` reads
        // `agent_enrole` THEN derives a `scrypt` hash, memory-hard and
        // deliberately expensive (68 ms measured on 20 August 2026). A brake placed
        // AFTER protects nothing: it counts attempts it has already paid for,
        // and an attacker exhausts the service without ever guessing a secret.
        const reel = await baseNeuve('canal-frein-sans-scrypt');
        base = reel;
        await enrolerUneVm(reel, 'v-1');
        const compteur = piloteCompteur(reel);
        const port = await start(compteur.pilote);
        for (let i = 0; i < ECHECS_MAX_COMPTE; i++) {
            await tenter(port, 'v-1', 'ce-n-est-pas-le-bon-secret');
        }
        compteur.remettre();
        await tenter(port, 'v-1', 'ce-n-est-pas-le-bon-secret');
        expect(compteur.acces()).toBe(0);
    }, 30000);

    it('(f) a SUCCESSFUL enrolment resets the VM counter to zero', async () => {
        // Same rule as `/auth/connexion`: success clears the VM's key,
        // NEVER the address's — otherwise an attacker who owns a valid VM
        // would launder themselves between two bursts.
        base = await baseNeuve('canal-frein-succes');
        await enrolerUneVm(base, 'v-1');
        const port = await start(base);
        for (let i = 0; i < ECHECS_MAX_COMPTE - 1; i++) {
            await tenter(port, 'v-1', 'ce-n-est-pas-le-bon-secret');
        }
        expect((await tenter(port, 'v-1', SECRET_VM)).type).toBe('enrole');
        // Without the reset, the last one of this second series would be
        // braked, hence would never reach the database.
        const reel2 = base;
        const compteur = piloteCompteur(reel2);
        void compteur;
        for (let i = 0; i < ECHECS_MAX_COMPTE - 1; i++) {
            expect((await tenter(port, 'v-1', 'ce-n-est-pas-le-bon-secret')).type).toBe('refus');
        }
        // And success remains possible: the VM is not locked.
        expect((await tenter(port, 'v-1', SECRET_VM)).type).toBe('enrole');
    }, 30000);
});
