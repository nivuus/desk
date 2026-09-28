// L'attribution d'une VM par ligne de commande d'administration.
//
// 🔴 ICI ON NOMME LA CAUSE, ET C'EST L'INVERSE DES ROUTES. `http/routes-vm.ts`
// rend le MÊME refus pour « VM inconnue » et « VM d'autrui », parce qu'une
// route publique qui les distinguerait serait un oracle d'énumération. Cette
// commande est une commande d'ADMINISTRATION : l'énumération n'y est pas un
// risque — l'appelant a déjà l'accès à la base et au secret de configuration —
// et lui cacher la cause le ferait chercher ailleurs (D8, E8).
//
// ⚠️ SEPT TESTS, comme le plan l'annonce.

import { afterEach, describe, expect, it } from 'vitest';
import { baseNeuve, MOTEUR } from '../base/harnais';
import type { Pilote } from '../base/pilote';
import { enroler } from '../depot/agent';
import { lireParId } from '../depot/vm';
import { createUser } from '../depot/utilisateur';
import { analyserArguments, appliquer } from './attribuer-vm';

const MS = 1_787_136_773_742;

let base: Pilote | undefined;

afterEach(async () => {
    await base?.fermer();
    base = undefined;
});

async function poserVm(p: Pilote, id: string, nom: string): Promise<void> {
    await p.executer('INSERT INTO vm(id, nom, adresse) VALUES(?, ?, ?)', [id, nom, '192.168.3.2']);
    await enroler(p, id, 'empreinte-opaque', `PREFIXE${id}`);
}

describe('analyserArguments of admin:attribuer', () => {
    it('🔴 requires --email AND --vm, with NO DEFAULT for either', () => {
        // 🔴 La rouge : donner un défaut à l'un des deux. Un défaut sur
        // `--email` attribuerait la VM à un compte que l'opérateur n'a pas
        // nommé ; un défaut sur `--vm` en choisirait une au hasard. Les deux
        // sont des gestes irréversibles sans confirmation.
        expect(analyserArguments(['--email', 'ada@exemple.test', '--vm', 'w1'])).toEqual({
            action: 'attribuer',
            email: 'ada@exemple.test',
            vm: 'w1',
        });
        expect('refus' in analyserArguments([])).toBe(true);
        expect('refus' in analyserArguments(['--email', 'ada@exemple.test'])).toBe(true);
        expect('refus' in analyserArguments(['--vm', 'w1'])).toBe(true);
        // Une valeur VIDE n'est pas une valeur.
        expect('refus' in analyserArguments(['--email', '', '--vm', 'w1'])).toBe(true);
        expect('refus' in analyserArguments(['--email', 'ada@exemple.test', '--vm', ''])).toBe(true);
    });

    it('reuses AS IS the list of refused flags of the two other commands', () => {
        // ⚠️ AUCUN SECRET N'EST EN JEU DANS CETTE COMMANDE, et la liste est
        // reprise quand même : une commande d'administration qui accepterait
        // `--mot-de-passe` sans s'en servir laisserait tout de même la chaîne
        // dans `ps`, où tout utilisateur de la machine la lirait. Le refus est
        // explicite et porte son motif.
        for (const drapeau of ['--mot-de-passe', '--motdepasse', '--password', '--mdp', '-p',
                               '--secret', '--secret-enrolement', '-s']) {
            const r = analyserArguments(['--email', 'ada@exemple.test', '--vm', 'w1', drapeau, 'chut']);
            expect('refus' in r).toBe(true);
            if (!('refus' in r)) return;
            // Le motif ne RECOPIE PAS la valeur refusée.
            expect(r.refus).not.toContain('chut');
        }
    });

    it('🔴 `--detacher` does NOT require an email, and says so', () => {
        // 🔴 La rouge : ignorer le drapeau, ou exiger `--email` avec lui. On
        // détache une VM DE quelqu'un ; exiger de nommer ce quelqu'un
        // obligerait l'opérateur à savoir d'avance ce que la commande va lui
        // apprendre.
        expect(analyserArguments(['--detacher', '--vm', 'w1'])).toEqual({
            action: 'detacher',
            vm: 'w1',
        });
        expect('refus' in analyserArguments(['--detacher'])).toBe(true);
    });
});

describe(`appliquer of admin:attribuer, engine=${MOTEUR}`, () => {
    it('🔴 success → code 0, and the RE-READ row carries the owner', async () => {
        base = await baseNeuve('adm-attrib-ok');
        await poserVm(base, 'v1', 'w1');
        const ada = await createUser(base, 'ada@exemple.test', 'empreinte', MS);
        const issue = await appliquer(base, {
            action: 'attribuer',
            email: 'ada@exemple.test',
            vm: 'w1',
        });
        expect(issue.code).toBe(0);
        // L'ÉTAT relu, jamais le seul code de sortie : un `return 0` qui
        // n'écrirait rien passerait la première assertion.
        expect((await lireParId(base, 'v1'))?.utilisateur_id).toBe(ada);
        // `--vm` accepte un NOM ou un IDENTIFIANT, le nom d'abord :
        // l'administrateur connaît celui qu'il a donné.
        expect(issue.sortie).toContain('v1');
    });

    it('🔴 `--detacher` returns the VM to the pool, and a NEW assignment becomes possible again', async () => {
        // 🔴 La rouge : ignorer le drapeau. La VM resterait prise à vie, et il
        // n'existerait aucun chemin pour la rendre — `depot/vm.ts::detacher`
        // n'a pas d'autre appelant.
        base = await baseNeuve('adm-detacher');
        await poserVm(base, 'v1', 'w1');
        await createUser(base, 'ada@exemple.test', 'empreinte', MS);
        const bob = await createUser(base, 'bob@exemple.test', 'empreinte', MS);
        await appliquer(base, { action: 'attribuer', email: 'ada@exemple.test', vm: 'w1' });

        expect((await appliquer(base, { action: 'detacher', vm: 'w1' })).code).toBe(0);
        expect((await lireParId(base, 'v1'))?.utilisateur_id).toBeNull();

        const encore = await appliquer(base, {
            action: 'attribuer',
            email: 'bob@exemple.test',
            vm: 'w1',
        });
        expect(encore.code).toBe(0);
        expect((await lireParId(base, 'v1'))?.utilisateur_id).toBe(bob);
    });

    it('UNKNOWN email → code 2, and the message NAMES the cause', async () => {
        // ⚠️ On nomme, ici. Voir l'en-tête : ce n'est pas une route publique.
        base = await baseNeuve('adm-courriel-inconnu');
        await poserVm(base, 'v1', 'w1');
        const issue = await appliquer(base, {
            action: 'attribuer',
            email: 'personne@exemple.test',
            vm: 'w1',
        });
        expect(issue.code).toBe(2);
        expect(issue.error).toMatch(/personne@exemple\.test/);
        expect(issue.error).toMatch(/account|email/i);
        // Et rien n'a été écrit.
        expect((await lireParId(base, 'v1'))?.utilisateur_id).toBeNull();
    });

    it('UNKNOWN VM → code 2, message naming the VM', async () => {
        base = await baseNeuve('adm-vm-inconnue');
        await createUser(base, 'ada@exemple.test', 'empreinte', MS);
        const issue = await appliquer(base, {
            action: 'attribuer',
            email: 'ada@exemple.test',
            vm: 'w-jamais-creee',
        });
        expect(issue.code).toBe(2);
        expect(issue.error).toMatch(/w-jamais-creee/);
    });

    it('🔴 VM ALREADY TAKEN → code 2, message naming the OWNER', async () => {
        // 🔴 La rouge : ne pas nommer le propriétaire. L'opérateur saurait que
        // ça a échoué sans savoir qui détacher — il lui faudrait ouvrir la base
        // à la main, ce que cette commande existe pour éviter.
        base = await baseNeuve('adm-vm-prise');
        await poserVm(base, 'v1', 'w1');
        const ada = await createUser(base, 'ada@exemple.test', 'empreinte', MS);
        await createUser(base, 'bob@exemple.test', 'empreinte', MS);
        await appliquer(base, { action: 'attribuer', email: 'ada@exemple.test', vm: 'w1' });

        const issue = await appliquer(base, {
            action: 'attribuer',
            email: 'bob@exemple.test',
            vm: 'w1',
        });
        expect(issue.code).toBe(2);
        expect(issue.error).toContain(ada);
        // Et la VM n'a PAS changé de main.
        expect((await lireParId(base, 'v1'))?.utilisateur_id).toBe(ada);
    });

    it('🔴 user who ALREADY has a VM → code 2 and a message, NEVER a stack trace', async () => {
        // 🔴 La rouge : laisser l'exception d'unicité remonter. L'administrateur
        // verrait `duplicate key value violates unique constraint
        // "vm_un_utilisateur"` — ou son jumeau SQLite, qui ne dit pas la même
        // chose —, ce qui ne lui apprend pas quoi faire. Le refus typé de
        // l'orchestrateur est traduit en une phrase.
        base = await baseNeuve('adm-deja-servi');
        await poserVm(base, 'v1', 'w1');
        await poserVm(base, 'v2', 'w2');
        await createUser(base, 'ada@exemple.test', 'empreinte', MS);
        await appliquer(base, { action: 'attribuer', email: 'ada@exemple.test', vm: 'w1' });

        const issue = await appliquer(base, {
            action: 'attribuer',
            email: 'ada@exemple.test',
            vm: 'w2',
        });
        expect(issue.code).toBe(2);
        expect(issue.error).toMatch(/already/i);
        // Aucune trace de pile, et aucun texte de moteur : les deux moteurs
        // n'écrivent pas le même, et l'un des deux serait donc faux.
        expect(issue.error).not.toMatch(/UNIQUE constraint|duplicate key|at Object|\bat \w+\./);
        expect((await lireParId(base, 'v2'))?.utilisateur_id).toBeNull();
    });
});
