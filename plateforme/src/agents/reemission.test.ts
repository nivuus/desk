// The re-emission of installation orders at enrolment, against BOTH engines.
//
// 🔴 THE CASE THIS FILE EXISTS FOR: an installation the agent has started
// (`en_cours`) and whose `termine` never reached the database — the agent was
// restarted, the network dropped while the installer ran, or the write failed.
// Before this fix only `en_attente` rows were re-sent, so such a row stayed
// `en_cours` FOREVER: nothing on either side ever asked the agent again, and the
// agent only reports a verdict it holds on disk when it receives the order again.
//
// ⚠️ THE ROW STATES ARE SET BY SQL, NOT THROUGH `avancer`/`terminer`: this file
// tests the re-emission, not those writes, and must not depend on their signatures.

import { afterEach, describe, expect, it } from 'vitest';
import { baseNeuve, MOTEUR } from '../base/harnais';
import type { Pilote } from '../base/pilote';
import { create as createInstallation } from '../depot/installation';
import { create as createUpload, sceller } from '../depot/televersement';
import { reemettreLesInstallations } from './canal-apps';

let base: Pilote | undefined;

afterEach(async () => {
    await base?.fermer();
    base = undefined;
});

const MS = 1_787_136_773_742;

async function socle(p: Pilote): Promise<{ vm: string; tel: string }> {
    await p.executer('INSERT INTO utilisateur(id,email,empreinte_mdp,cree_a) VALUES(?,?,?,?)', [
        'u1', 'u1@exemple.test', 'scrypt$1$1$1$x$y', MS,
    ]);
    await p.executer('INSERT INTO vm(id,nom,adresse) VALUES(?,?,?)', ['v1', 'vm-1', '192.168.3.2']);
    const tel = await createUpload(
        p,
        { userId: 'u1', nom: 'setup.exe', taille: 10, sha256: 'a'.repeat(64), chunkSize: 4 },
        MS,
    );
    await sceller(p, tel.id, MS);
    return { vm: 'v1', tel: tel.id };
}

async function ordresEmis(p: Pilote, vmId: string): Promise<string[]> {
    const emis: string[] = [];
    await reemettreLesInstallations({ base: p, vmId, envoyer: (brut) => emis.push(brut) });
    return emis.map((brut) => (JSON.parse(brut) as { installation: string }).installation);
}

describe(`re-emission of installation orders, engine=${MOTEUR}`, () => {
    it('🔴 re-sends an installation left `en_cours`, so a lost `termine` can be recovered', async () => {
        base = await baseNeuve('reemission-en-cours');
        const { vm, tel } = await socle(base);
        const enAttente = await createInstallation(base, { vmId: vm, televersementId: tel }, MS);
        const enCours = await createInstallation(base, { vmId: vm, televersementId: tel }, MS + 1);
        await base.executer('UPDATE installation SET etat = ? WHERE id = ?', ['en_cours', enCours.id]);

        expect(await ordresEmis(base, vm)).toEqual([enAttente.id, enCours.id]);
    });

    it('never re-sends a finished installation', async () => {
        base = await baseNeuve('reemission-terminee');
        const { vm, tel } = await socle(base);
        const finie = await createInstallation(base, { vmId: vm, televersementId: tel }, MS);
        await base.executer('UPDATE installation SET etat = ? WHERE id = ?', ['terminee', finie.id]);
        // Witness: the same VM DOES receive an order that is not finished, so an
        // empty list below cannot come from a re-emission that sends nothing.
        const temoin = await createInstallation(base, { vmId: vm, televersementId: tel }, MS + 6);

        expect(await ordresEmis(base, vm)).toEqual([temoin.id]);
    });
});
