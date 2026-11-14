// The `/agent` channel answers for ONE VM per socket: the VM its enrolment
// proved. A launch outcome, an installation progress or an installation
// outcome sent by VM B's socket about VM A's request or installation must
// change nothing. Each case carries its witness: the same message from the
// right VM does take effect, otherwise "nothing changed" would hold for a
// channel that ignores everything.

import { afterEach, describe, expect, it, vi } from 'vitest';
import { WebSocketServer } from 'ws';
import {
    encodeEnroler,
    encodeLancee,
    encodeProgression,
    encodeTermine,
} from '../../../proto/ts/plateforme';
import { baseNeuve } from '../base/harnais';
import type { Pilote } from '../base/pilote';
import { enroler } from '../depot/agent';
import { create as createInstallation, lireParId } from '../depot/installation';
import { create as createUpload, sceller } from '../depot/televersement';
import { hacher } from '../identite/mot-de-passe';
import { Frein } from '../securite/frein';
import { servirLeCanalAgent } from './canal';
import { ouvrir, SECRET, SECRET_VM, T0, type Pair } from './canal-harnais';
import { RegistreAgents } from './registre';

let base: Pilote | undefined;
let wss: WebSocketServer | undefined;
let registre = new RegistreAgents();

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
    registre = new RegistreAgents();
    wss = new WebSocketServer({ port: 0, host: '127.0.0.1' });
    await new Promise<void>((r) => wss!.once('listening', () => r()));
    servirLeCanalAgent(wss, {
        base: p,
        secretJeton: SECRET,
        maintenant: () => T0,
        registre,
        frein: new Frein(),
        proxyDeConfiance: new Set(),
        magasin: undefined,
    });
    const adresse = wss.address();
    return typeof adresse === 'object' && adresse ? adresse.port : 0;
}

/// Two enrolled VMs, each with its own session prefix (the column is UNIQUE).
async function deuxVms(p: Pilote): Promise<void> {
    const empreinte = await hacher(SECRET_VM);
    for (const [vm, prefixe] of [['v-a', 'PrefixA0000000000000000'], ['v-b', 'PrefixB0000000000000000']]) {
        await p.executer('INSERT INTO vm(id,nom,adresse) VALUES(?,?,?)', [vm, `vm-${vm}`, '192.168.3.2']);
        await enroler(p, vm, empreinte, prefixe);
    }
}

async function enrole(port: number, vm: string): Promise<Pair> {
    const pair = await ouvrir(port);
    expect((await pair.dire(encodeEnroler(vm, SECRET_VM))).type).toBe('enrole');
    return pair;
}

/// An `en_cours` installation for `vm` (so no order is re-sent at enrolment).
async function installationPour(p: Pilote, vm: string): Promise<string> {
    await p.executer(
        'INSERT INTO utilisateur(id,email,empreinte_mdp,cree_a) VALUES(?,?,?,?) ON CONFLICT DO NOTHING',
        ['u-1', 'a@b.c', 'scrypt$1$1$1$x$y', T0],
    );
    const tel = await createUpload(p, {
        userId: 'u-1', nom: 'setup.exe', taille: 3, sha256: 'a'.repeat(64), chunkSize: 8,
    }, T0);
    await sceller(p, tel.id, T0);
    const inst = await createInstallation(p, { vmId: vm, televersementId: tel.id }, T0);
    await p.executer('UPDATE installation SET etat = ? WHERE id = ?', ['en_cours', inst.id]);
    return inst.id;
}

async function attendre(predicat: () => Promise<boolean>, quoi: string): Promise<void> {
    for (let i = 0; i < 200; i += 1) {
        if (await predicat()) return;
        await new Promise((r) => setTimeout(r, 10));
    }
    throw new Error(`${quoi} did not happen within 2000 ms`);
}

describe('agent messages are scoped to the VM of the socket', () => {
    it("VM B's socket cannot finish or advance VM A's installation", async () => {
        base = await baseNeuve('canal-portee-installation');
        await deuxVms(base);
        const deA = await installationPour(base, 'v-a');
        const deB = await installationPour(base, 'v-b');
        const port = await start(base);
        const b = await enrole(port, 'v-b');

        b.socket.send(encodeProgression(deA, 'transfert', 1, 3, 5));
        b.socket.send(encodeTermine(deA, 'reussie', null, 0, 'forged', false));
        // Witness, sent AFTER on the same socket: B's own installation is written.
        b.socket.send(encodeTermine(deB, 'reussie', null, 0, '', false));
        const p = base;
        await attendre(async () => (await lireParId(p, deB))?.etat === 'terminee', "B's own outcome");

        const ligneA = await lireParId(base, deA);
        expect([ligneA?.etat, ligneA?.issue, ligneA?.phase]).toEqual(['en_cours', null, '']);
        b.socket.terminate();
    });

    it("VM B's socket cannot resolve a launch waiting on VM A", async () => {
        base = await baseNeuve('canal-portee-lancement');
        await deuxVms(base);
        const port = await start(base);
        const a = await enrole(port, 'v-a');
        const b = await enrole(port, 'v-b');
        vi.spyOn(console, 'warn').mockImplementation(() => {});

        let issue: string | undefined;
        const attente = registre.lancer('v-a', 'cle-1', 'd-1').then((i) => { issue = i; });
        const ordre = await a.recevoir();
        expect(ordre).toMatchObject({ type: 'lancer', demande: 'd-1' });

        b.socket.send(encodeLancee('d-1', 'raccourci'));
        // B's answer has had time to be handled: a round trip on B's socket.
        expect((await b.dire(JSON.stringify({ type: 'battement', v: ordre.v }))).type).toBe('battement-recu');
        expect(issue).toBeUndefined();

        // Witness: A's own answer resolves the wait.
        a.socket.send(encodeLancee('d-1', 'cible'));
        await attente;
        expect(issue).toBe('cible');
        a.socket.terminate();
        b.socket.terminate();
    });
});
