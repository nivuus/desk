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
import { verifyEnrolment } from '../agents/enrolement';
import { verify } from '../identite/mot-de-passe';
import { analyserArguments, enrolerLaVm, roterLeSecret } from './enroler-agent';

let base: Pilote | undefined;

afterEach(async () => {
    await base?.fermer();
    base = undefined;
});

describe('analyserArguments of admin:agent', () => {
    it('reads --vm and --adresse', () => {
        expect(analyserArguments(['--vm', 'w1', '--adresse', '192.168.3.2']))
            .toEqual({ mode: 'enroler', vm: 'w1', adresse: '192.168.3.2' });
    });

    it('--roter requires ONLY --vm: it creates no VM, it fixes one', () => {
        expect(analyserArguments(['--vm', 'w1', '--roter']))
            .toEqual({ mode: 'roter', vm: 'w1' });
        // Et --adresse, s'il traîne, ne change rien : la rotation ne touche
        // pas la table `vm`.
        expect(analyserArguments(['--vm', 'w1', '--roter', '--adresse', '10.0.0.1']))
            .toEqual({ mode: 'roter', vm: 'w1' });
    });

    it('🔴 --roter without --vm is refused, rather than rotating at random', () => {
        // 🔴 LA ROUGE : laisser passer. Une rotation sans cible nommée ne peut
        // pas deviner LAQUELLE des VMs enrôlées doit changer de secret.
        expect('refus' in analyserArguments(['--roter'])).toBe(true);
    });

    it('🔴 REFUSES --secret EVEN together with --roter', () => {
        // 🔴 LA ROUGE : ne contrôler les drapeaux interdits que sur le chemin
        // d'enrôlement. Le secret d'une ROTATION est tout aussi sensible que
        // celui d'un enrôlement — `ps` l'exposerait de la même façon —, et
        // c'est justement le chemin qu'on emprunte quand un secret a fuité.
        const r = analyserArguments(['--vm', 'w1', '--roter', '--secret', 'chut']);
        expect('refus' in r).toBe(true);
        if (!('refus' in r)) return;
        expect(r.refus).toMatch(/drawn at random/i);
        expect(r.refus).not.toContain('chut');
    });

    it('🔴 REFUSES --secret on the command line, with its reason', () => {
        // 🔴 La rouge la plus utile du fichier : l'accepter. Le motif ne
        // RECOPIE PAS la valeur refusée — la réécrire dans un journal après
        // l'avoir refusée dans un argv n'aurait aucun sens.
        const r = analyserArguments(['--vm', 'w1', '--adresse', '10.0.0.1', '--secret', 'chut']);
        expect('refus' in r).toBe(true);
        if (!('refus' in r)) return;
        expect(r.refus).toMatch(/drawn at random/i);
        expect(r.refus).not.toContain('chut');
    });

    it('also refuses the variants of the same flag', () => {
        for (const drapeau of ['--secret-enrolement', '--password', '--mdp', '-s']) {
            const r = analyserArguments(['--vm', 'w1', '--adresse', '10.0.0.1', drapeau, 'chut']);
            expect('refus' in r).toBe(true);
        }
    });

    it('refuses a missing --vm or --adresse, rather than returning undefined', () => {
        expect('refus' in analyserArguments([])).toBe(true);
        expect('refus' in analyserArguments(['--vm', 'w1'])).toBe(true);
        expect('refus' in analyserArguments(['--adresse', '10.0.0.1'])).toBe(true);
        expect('refus' in analyserArguments(['--vm', '', '--adresse', '10.0.0.1'])).toBe(true);
    });
});

describe(`enrolerLaVm, engine=${MOTEUR}`, () => {
    it('🔴 draws the secret AT RANDOM: two calls with the SAME ARGUMENTS differ', async () => {
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

    it('🔴 writes the FINGERPRINT to the database, NEVER the plaintext secret', async () => {
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
        expect(await verify(secret, ligne!.empreinte_secret)).toBe(true);
        // Une VM enrôlée n'a pas encore battu.
        expect(ligne!.vu_a).toBeNull();
    });
});

describe(`roterLeSecret, engine=${MOTEUR}`, () => {
    it("🔴 (a) the OLD secret is refused and the NEW one accepted, end to end", async () => {
        // 🔴 LA ROUGE : écrire le secret en clair au lieu de son empreinte, ou
        // ne rien écrire du tout. Le juge n'est pas la colonne mais
        // `verifyEnrolment`, c'est-à-dire le chemin RÉEL du canal /agent :
        // c'est la seule façon de savoir que la rotation a produit une
        // empreinte que le service sait vérifier.
        base = await baseNeuve('admin-roter-bout-en-bout');
        const { vmId, secret: ancien } = await enrolerLaVm(
            base, 'w1', '192.168.3.2', 1_787_136_773_742);

        const r = await roterLeSecret(base, vmId);
        expect('refus' in r).toBe(false);
        if ('refus' in r) return;

        expect((await verifyEnrolment(base, vmId, ancien, () => {})).ok).toBe(false);
        expect((await verifyEnrolment(base, vmId, r.secret, () => {})).ok).toBe(true);
    });

    it('🔴 (b) the SESSION PREFIX is unchanged — re-read on both sides', async () => {
        // 🔴 LA ROUGE : faire tourner le préfixe aussi. Il compose le nom des
        // sessions VIVANTES de la VM : le changer les couperait toutes. Le
        // test relit la colonne AVANT et APRÈS l'appel, sans quoi il ne
        // mesurerait rien.
        base = await baseNeuve('admin-roter-prefixe');
        const { vmId, prefixe: before } = await enrolerLaVm(
            base, 'w1', '192.168.3.2', 1_787_136_773_742);

        const r = await roterLeSecret(base, vmId);
        expect('refus' in r).toBe(false);

        const apres = (await lireParVm(base, vmId))!.prefixe_session;
        expect(apres).toBe(before);
        expect(apres).toHaveLength(22);
    });

    it('🔴 (b bis) the new secret is DRAWN AT RANDOM: two rotations differ', async () => {
        // 🔴 LA ROUGE : le dériver de l'identifiant de VM. Il serait devinable
        // par quiconque le connaît, et la rotation ne réparerait rien.
        base = await baseNeuve('admin-roter-alea');
        const { vmId } = await enrolerLaVm(base, 'w1', '192.168.3.2', 1_787_136_773_742);
        const un = await roterLeSecret(base, vmId);
        const deux = await roterLeSecret(base, vmId);
        expect('refus' in un).toBe(false);
        expect('refus' in deux).toBe(false);
        if ('refus' in un || 'refus' in deux) return;
        expect(un.secret).not.toBe(deux.secret);
    });

    it('🔴 (c) an UNKNOWN VM gives a REASONED refusal, never an exception', async () => {
        // 🔴 LA ROUGE : laisser l'UPDATE toucher zéro ligne en silence et
        // rendre un succès. L'administrateur croirait avoir fait tourner un
        // secret compromis, et l'ancien resterait valide — le pire résultat
        // possible pour cette commande, puisqu'on ne l'emploie QUE lorsqu'un
        // secret a fuité.
        base = await baseNeuve('admin-roter-inconnue');
        const r = await roterLeSecret(base, 'no-vm-by-this-name');
        expect('refus' in r).toBe(true);
        if (!('refus' in r)) return;
        expect(r.refus).toMatch(/enrolled/i);
    });
});
