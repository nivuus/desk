// P1's criterion ②: a paired session leaves a row in the database, and
// closes it on leaving.
//
// Two stages are tested here, and both are needed:
//   - the observer ALONE, with an injected clock, on EXACT VALUES;
//   - the WHOLE service, two peers on `/signal` (the relay, moved from the
//     root on 21 August 2026 — see the header of `http/serveur.ts`), where the
//     only possible proof is a BOUNDED wait.
//
// ⚠️ The wait is bounded and fails on expiry, never an infinite loop:
// the write is deliberately launched without being awaited (see `trace.ts`), so
// a lost write only shows as a row that does not arrive. A
// test that looped without a bound would hang instead of turning red.

import { afterEach, describe, expect, it, vi } from 'vitest';
import { WebSocket } from 'ws';
import { baseNeuve } from '../base/harnais';
import type { Pilote } from '../base/pilote';
import type { Config } from '../config';
import { enroler } from '../depot/agent';
import { lireParNom, type LigneSession } from '../depot/session';
import { startServer, type ServicePlateforme } from '../http/serveur';
import { signer } from '../identite/jeton';
import { hacher } from '../identite/mot-de-passe';
import { MOTIF_DEPART, observateurDeSession } from './trace';
import { mkdtempSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

// An EXPLICIT test secret, never `''`: `lireConfig` refuses the empty
// string, and a `Config` literal built by hand must carry a value
// a service would really accept.
const CONFIG: Config = {
    hote: '127.0.0.1',
    port: 0,
    base: 'sqlite',
    urlBase: ':memory:',
    secretJeton: 'un-secret-de-plateforme-de-quarante-octets',
    // No proxy declared — see `config.ts`: the empty set is the default,
    // and it means "trust nobody's announced address".
    proxyDeConfiance: new Set(),
    repertoireIcones: join(mkdtempSync(join(tmpdir(), 'g2-icones-')), 'icones'),
    repertoireTeleversements: join(mkdtempSync(join(tmpdir(), 'g3-tranches-')), 'televersements'),
    auth: 'pomerium',
};

let base: Pilote | undefined;
let service: ServicePlateforme | undefined;

afterEach(async () => {
    await service?.close();
    service = undefined;
    await base?.fermer();
    base = undefined;
});

/// Waits for a row to satisfy `predicat`, or FAILS after `borneMs`.
async function attendreLigne(
    p: Pilote,
    nom: string,
    predicat: (l: LigneSession) => boolean,
    quoi: string,
    borneMs = 2000,
): Promise<LigneSession> {
    const fin = Date.now() + borneMs;
    for (;;) {
        const lignes = await lireParNom(p, nom);
        const trouvee = lignes.find(predicat);
        if (trouvee) return trouvee;
        if (Date.now() > fin) {
            throw new Error(`no session ${quoi} row for ${nom} within ${borneMs} ms`);
        }
        await new Promise((r) => setTimeout(r, 25));
    }
}

/// The prefix of the simulated VM. All the sessions of this file carry it,
/// because the guard now requires the agent token's subject to PREFIX the
/// requested session (sub-block P3).
const P = 'RhH1x2QmTz9kLpVbNc7dAw';

function connecter(
    url: string,
    role: string,
    session: string,
    sujet = 'u-trace',
): Promise<WebSocket> {
    return new Promise((resolve) => {
        const w = new WebSocket(url);
        w.once('open', () => {
            // 🔴 BOTH ROLES REQUIRE A TOKEN. The `client` role since P2;
            // the `agent` role since P3, which closed E2's anonymous window.
            // Without a token, the guard refuses, the socket closes, and NO
            // pairing takes place — hence no trace row, and these tests
            // would measure a service that never pairs anything.
            //
            // The agent token is of TYPE `agent` and its subject is the VM's
            // PREFIX: the guard requires this subject to prefix the session.
            const jeton = role === 'client'
                ? signer(sujet, CONFIG.secretJeton, Date.now())
                : signer(P, CONFIG.secretJeton, Date.now(), undefined, 'agent');
            w.send(JSON.stringify({ role, session, jeton }));
            resolve(w);
        });
    });
}

function fermer(w: WebSocket): Promise<void> {
    return new Promise((resolve) => {
        w.once('close', () => resolve());
        w.close();
    });
}

describe('session observer', () => {
    it('opens at pairing and closes at departure, at EXACT instants', async () => {
        base = await baseNeuve('trace-unite');
        let instant = 5_000_000_000;
        const obs = observateurDeSession(base, () => instant);

        obs.apparie('u-1');
        await attendreLigne(base, 'u-1', (l) => l.fermee_a === null, 'ouverte');

        instant = 5_000_000_900;
        obs.separe('u-1');
        const close = await attendreLigne(base, 'u-1', (l) => l.fermee_a !== null, 'close');
        // EXACT values: it is the assertion that forbids a `Date.now()`
        // hidden in the observer.
        expect(Number(close.ouverte_a)).toBe(5_000_000_000);
        expect(Number(close.fermee_a)).toBe(5_000_000_900);
        expect(close.motif).toBe(MOTIF_DEPART);
    });

    it('a second pairing does not reopen a second row', async () => {
        // A peer that reconnects while the other stays in place
        // re-pairs the session. Without this guard, the first row would be
        // orphaned — never closed, until the sweep of the next startup.
        base = await baseNeuve('trace-rappari');
        const obs = observateurDeSession(base, () => 7_000);
        obs.apparie('u-2');
        await attendreLigne(base, 'u-2', () => true, 'ouverte');
        obs.apparie('u-2');
        await new Promise((r) => setTimeout(r, 100));
        expect(await lireParNom(base, 'u-2')).toHaveLength(1);
    });

    it('a departure without prior pairing writes nothing', async () => {
        base = await baseNeuve('trace-sans');
        const obs = observateurDeSession(base, () => 7_000);
        obs.separe('jamais-apparie');
        await new Promise((r) => setTimeout(r, 100));
        expect(await lireParNom(base, 'jamais-apparie')).toHaveLength(0);
    });
});

/// Enrols a VM to which the session can attach.
///
/// ⚠️ THE `vm` ROW FIRST: `agent_enrole.vm_id` REFERENCES it
/// (`0003-agents.sql`), and SQLite applies the foreign key. The hash is
/// a REAL `scrypt` hash, never a short string — same rule as
/// `depot/agent.test.ts`, and for the same reason: a convenient value
/// measures no column length.
async function enrolerUneVm(p: Pilote, vmId: string, prefixe: string): Promise<void> {
    await p.executer('INSERT INTO vm(id,nom,adresse) VALUES(?,?,?)', [
        vmId,
        `vm-${vmId}`,
        '192.168.3.2',
    ]);
    await enroler(p, vmId, await hacher('un-secret-d-enrolement-de-la-vraie-longueur'), prefixe);
}

describe('the session.vm_id column', () => {
    // 🔴 IT IS P2'S LEGACY ITEM NO. 3 THAT CLOSES HERE: "`session.vm_id` stays
    // entirely NULL". The prefix of the session name designates the VM, and
    // it is the TRACE that resolves it — never the relay, whose `apparie` stays
    // synchronous and without return (E10). The resolution is therefore tested at the
    // level of the observer, where it lives.

    it('a session prefixed by an ENROLLED VM records its vm_id', async () => {
        base = await baseNeuve('trace-vm-connue');
        await enrolerUneVm(base, 'v-1', P);
        const obs = observateurDeSession(base, () => 5_000_000_000);

        obs.apparie(`${P}:bureau`);
        const ligne = await attendreLigne(base, `${P}:bureau`, () => true, 'ouverte');
        expect(ligne.vm_id).toBe('v-1');
    });

    it('a session WITHOUT a prefix leaves vm_id at `null`', async () => {
        // The local trial mode that spec §10 sets as LEGITIMATE: an
        // agent launched without `AGENT_VM` names its session plain `bureau`.
        // Throwing, or recording an empty string, would break this mode — and an
        // empty string would lie by claiming to know a VM.
        base = await baseNeuve('trace-vm-sans-prefixe');
        await enrolerUneVm(base, 'v-1', P);
        const obs = observateurDeSession(base, () => 5_000_000_000);

        obs.apparie('bureau');
        const ligne = await attendreLigne(base, 'bureau', () => true, 'ouverte');
        expect(ligne.vm_id).toBeNull();
    });

    it('🔴 the instant is read SYNCHRONOUSLY at pairing, NOT after the database read', async () => {
        // 🔴 THIS TEST EXISTS BECAUSE A MUTATION STAYED GREEN WITHOUT IT.
        // The prefix resolution inserts a database read between
        // `apparie()` and the INSERT: reading the clock in the call to
        // `ouvrirSession` would therefore date `ouverte_a` from the END OF A QUERY and
        // not from the pairing. None of the three tests above saw it —
        // they do not move their clock —, and the comment of `trace.ts`
        // stated the rule without anything holding it.
        base = await baseNeuve('trace-instant-synchrone');
        await enrolerUneVm(base, 'v-1', P);
        let instant = 5_000_000_000;
        const obs = observateurDeSession(base, () => instant);

        obs.apparie(`${P}:bureau`);
        // 🔴 THIS LINE RUNS BEFORE THE RESOLUTION HAS COMPLETED, and it is
        // what makes the test decidable: `resoudreVm` is asynchronous, so
        // `apparie` has returned control here without the database having answered. A
        // clock read later would see 9 000 000 000.
        instant = 9_000_000_000;

        const ligne = await attendreLigne(base, `${P}:bureau`, () => true, 'ouverte');
        expect(Number(ligne.ouverte_a)).toBe(5_000_000_000);
        expect(ligne.vm_id).toBe('v-1');
    });

    it('🔴 a session with an UNKNOWN prefix leaves vm_id at `null`, and LOGS it', async () => {
        // 🔴 BOTH HALVES COUNT. Recording anyway would make the
        // column LIE — it would name a VM the database does not know. And staying
        // silent would make the case indistinguishable from the previous one: an agent whose
        // enrolment was revoked would pair sessions without anything,
        // anywhere, flagging it.
        base = await baseNeuve('trace-vm-inconnue');
        const journal = vi.spyOn(console, 'warn').mockImplementation(() => {});
        const inconnu = 'Zz9QmRhH1x2kLpVbNc7dAw';
        const obs = observateurDeSession(base, () => 5_000_000_000);

        obs.apparie(`${inconnu}:bureau`);
        const ligne = await attendreLigne(base, `${inconnu}:bureau`, () => true, 'ouverte');
        expect(ligne.vm_id).toBeNull();

        const lignes = journal.mock.calls.map((c) => String(c[0])).join('\n');
        expect(lignes).toContain(inconnu);
        journal.mockRestore();
    });
});

describe('the whole service', () => {
    it('writes a row at pairing, and closes it when both peers disconnect', async () => {
        base = await baseNeuve('trace-service');
        service = await startServer(CONFIG, base);
        const url = `ws://127.0.0.1:${service.port}/signal`;

        const agent = await connecter(url, 'agent', `${P}:trace-1`);
        const client = await connecter(url, 'client', `${P}:trace-1`);

        const ouverte = await attendreLigne(base, `${P}:trace-1`, () => true, 'ouverte');
        expect(Number(ouverte.ouverte_a)).toBeGreaterThan(0);
        expect(ouverte.fermee_a).toBeNull();

        await fermer(agent);
        await fermer(client);

        const close = await attendreLigne(base, `${P}:trace-1`, (l) => l.fermee_a !== null, 'close');
        expect(Number(close.fermee_a)).toBeGreaterThanOrEqual(Number(close.ouverte_a));
        expect(await lireParNom(base, `${P}:trace-1`)).toHaveLength(1);
    });

    it('writes nothing when a single peer has declared itself', async () => {
        // 🔴 The first test would be GREEN with a write set too early —
        // from the first declaration. It is this test that distinguishes
        // "pairing" from "connection", and the supervisor declares itself alone
        // on `bureau` for hours at VM startup.
        base = await baseNeuve('trace-solitaire');
        service = await startServer(CONFIG, base);
        const url = `ws://127.0.0.1:${service.port}/signal`;

        const agent = await connecter(url, 'agent', `${P}:trace-2`);
        await new Promise((r) => setTimeout(r, 300));
        expect(await lireParNom(base, `${P}:trace-2`)).toHaveLength(0);

        await fermer(agent);
        await new Promise((r) => setTimeout(r, 300));
        expect(await lireParNom(base, `${P}:trace-2`)).toHaveLength(0);
    });

    it('records in the database the user of the authenticated client that pairs', async () => {
        // ⚠️ THIS TEST CANNOT BE ONE OF AN ANONYMOUS CLIENT: since
        // the guard is wired, an anonymous client never reaches
        // pairing. A test assuming one would measure a state
        // the product can no longer produce — vacuous by construction. The real
        // case is this one: the `bureau` control session, where the `agent`
        // arrives alone and without identity, and where the CLIENT, for its part, is authenticated.
        base = await baseNeuve('trace-appartenance');
        service = await startServer(CONFIG, base);
        const url = `ws://127.0.0.1:${service.port}/signal`;

        const agent = await connecter(url, 'agent', `${P}:bureau`);
        const client = await connecter(url, 'client', `${P}:bureau`, 'u-proprietaire');

        const ligne = await attendreLigne(base, `${P}:bureau`, () => true, 'ouverte');
        expect(ligne.utilisateur_id).toBe('u-proprietaire');

        await fermer(agent);
        await fermer(client);
    });
});
