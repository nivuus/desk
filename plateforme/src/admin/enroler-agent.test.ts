// La partie PURE de l'enrôlement en ligne de commande, et la garde qui vaut :
// un secret ne passe JAMAIS par l'argv.
//
// 🔴 La rouge de ce fichier est `--secret` : l'accepter « pour la commodité »
// exposerait le secret d'enrôlement à TOUT utilisateur de la machine, `ps`
// donnant l'argv de tout processus — puis l'historique du shell le garderait.
// C'est le jumeau exact de `DRAPEAUX_INTERDITS` de `creer-utilisateur.ts`.

import { afterEach, describe, expect, it } from 'vitest';
import { baseNeuve, MOTEUR } from '../base/harnais';
import type { Pilote } from '../base/pilote';
import { lireParVm } from '../depot/agent';
import { verifier } from '../identite/mot-de-passe';
import { analyserArguments, enrolerLaVm } from './enroler-agent';

let base: Pilote | undefined;

afterEach(async () => {
    await base?.fermer();
    base = undefined;
});

describe('analyserArguments de admin:agent', () => {
    it('lit --vm et --adresse', () => {
        expect(analyserArguments(['--vm', 'w1', '--adresse', '192.168.3.2']))
            .toEqual({ vm: 'w1', adresse: '192.168.3.2' });
    });

    it('🔴 REFUSE --secret sur la ligne de commande, avec son motif', () => {
        // 🔴 La rouge la plus utile du fichier : l'accepter. Le motif ne
        // RECOPIE PAS la valeur refusée — la réécrire dans un journal après
        // l'avoir refusée dans un argv n'aurait aucun sens.
        const r = analyserArguments(['--vm', 'w1', '--adresse', '10.0.0.1', '--secret', 'chut']);
        expect('refus' in r).toBe(true);
        if (!('refus' in r)) return;
        expect(r.refus).toMatch(/tiré au sort|tire au sort/i);
        expect(r.refus).not.toContain('chut');
    });

    it('refuse aussi les variantes du même drapeau', () => {
        for (const drapeau of ['--secret-enrolement', '--password', '--mdp', '-s']) {
            const r = analyserArguments(['--vm', 'w1', '--adresse', '10.0.0.1', drapeau, 'chut']);
            expect('refus' in r).toBe(true);
        }
    });

    it('refuse --vm ou --adresse absents, plutôt que de rendre undefined', () => {
        expect('refus' in analyserArguments([])).toBe(true);
        expect('refus' in analyserArguments(['--vm', 'w1'])).toBe(true);
        expect('refus' in analyserArguments(['--adresse', '10.0.0.1'])).toBe(true);
        expect('refus' in analyserArguments(['--vm', '', '--adresse', '10.0.0.1'])).toBe(true);
    });
});

describe(`enrolerLaVm, moteur=${MOTEUR}`, () => {
    it('🔴 tire le secret AU SORT : deux appels de MÊMES ARGUMENTS diffèrent', async () => {
        // 🔴 La rouge : le dériver du nom de VM. Il deviendrait devinable par
        // quiconque connaît ce nom, et l'enrôlement n'authentifierait plus rien.
        //
        // ⚠️ LES DEUX APPELS PORTENT LES MÊMES ARGUMENTS, ET C'EST TOUT LE
        // TEST. Une première rédaction employait deux noms de VM distincts
        // (`w1`, `w2`) : un secret dérivé du nom aurait alors différé lui
        // aussi, et le test serait resté VERT sous la mutation — MESURÉ, il
        // l'est resté. Un contrôle qu'on n'a jamais vu rouge n'est pas un
        // contrôle, et celui-là ne pouvait pas l'être.
        //
        // `vm.nom` n'est pas UNIQUE (`0001-socle.sql`) : deux enrôlements du
        // même nom sont donc possibles, et c'est ce qui rend l'appel répétable.
        base = await baseNeuve('admin-agent-alea');
        const un = await enrolerLaVm(base, 'w1', '192.168.3.2', 1_787_136_773_742);
        const deux = await enrolerLaVm(base, 'w1', '192.168.3.2', 1_787_136_773_742);
        expect(un.secret).not.toBe(deux.secret);
        // Et les préfixes non plus : deux VMs ne se disputent pas un espace
        // de noms, fût-ce sous le même nom d'affichage.
        expect(un.prefixe).not.toBe(deux.prefixe);
        expect(un.prefixe).toHaveLength(22);
        // Deux lignes distinctes, donc deux identifiants distincts.
        expect(un.vmId).not.toBe(deux.vmId);
    });

    it('🔴 écrit l’EMPREINTE en base, JAMAIS le secret en clair', async () => {
        // 🔴 La rouge : écrire le clair. Le test relit la colonne et l'y
        // trouverait — une base volée livrerait alors toutes les VMs.
        base = await baseNeuve('admin-agent-empreinte');
        const { secret, prefixe } = await enrolerLaVm(base, 'w1', '192.168.3.2', 1_787_136_773_742);

        const [vm] = await base.interroger<{ id: string }>(
            'SELECT id FROM vm WHERE nom = ?', ['w1']);
        const ligne = await lireParVm(base, vm.id);
        expect(ligne).toBeDefined();
        expect(ligne!.empreinte_secret).not.toContain(secret);
        expect(ligne!.prefixe_session).toBe(prefixe);
        // Et l'empreinte VÉRIFIE bien le secret rendu : sans cette assertion,
        // écrire n'importe quoi passerait la précédente.
        expect(await verifier(secret, ligne!.empreinte_secret)).toBe(true);
        // Une VM enrôlée n'a pas encore battu.
        expect(ligne!.vu_a).toBeNull();
    });
});
