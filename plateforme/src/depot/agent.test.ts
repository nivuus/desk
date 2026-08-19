// Ces tests tournent sous `test:sqlite` ET sous `test:postgres`, sans être
// écrits deux fois : ils emploient le même harnais que `pilotes.test.ts`.
//
// 🔴 LES VALEURS SONT RÉALISTES, JAMAIS COMMODES. C'est la leçon la plus chère
// de P1 : la double passe n'écrivait que des `1_000`, et déclarait portable un
// schéma que Postgres refusait pour toute écriture réelle. `vu_a` reçoit donc
// une MAGNITUDE D'ÉPOQUE, et l'empreinte une vraie longueur de `scrypt`.

import { afterEach, describe, expect, it } from 'vitest';
import { baseNeuve, MOTEUR } from '../base/harnais';
import type { Pilote } from '../base/pilote';
import { hacher } from '../identite/mot-de-passe';
import { enroler, lireParPrefixe, lireParVm, marquerVu } from './agent';

let base: Pilote | undefined;

/// La magnitude qui a réellement cassé Postgres en P1 (`pilotes.test.ts:111`).
const MS = 1_787_136_773_742;
/// Un préfixe de la VRAIE longueur que `agents/prefixe.ts` produit.
const PREFIXE = 'RhH1x2QmTz9kLpVbNc7dAw';

afterEach(async () => {
    await base?.fermer();
    base = undefined;
});

/// Une VM à laquelle s'accrocher : `agent_enrole.vm_id` la RÉFÉRENCE, et
/// SQLite applique la clé étrangère (`PRAGMA foreign_keys=ON` à l'ouverture).
async function avecVm(p: Pilote, id: string): Promise<void> {
    await p.executer('INSERT INTO vm(id,nom,adresse) VALUES(?,?,?)', [id, `vm-${id}`, '192.168.3.2']);
}

describe(`dépôt agent_enrole, moteur=${MOTEUR}`, () => {
    it('enrôle, puis relit par VM', async () => {
        base = await baseNeuve('agent-enrole');
        await avecVm(base, 'v-1');
        // Une empreinte RÉELLE, produite par la même dérivation que les
        // comptes humains — pas une chaîne courte qui ne mesure aucune
        // longueur de colonne.
        const empreinte = await hacher('un-secret-d-enrolement-de-la-vraie-longueur');
        await enroler(base, 'v-1', empreinte, PREFIXE);

        const ligne = await lireParVm(base, 'v-1');
        expect(ligne).toBeDefined();
        expect(ligne!.vm_id).toBe('v-1');
        expect(ligne!.empreinte_secret).toBe(empreinte);
        expect(ligne!.prefixe_session).toBe(PREFIXE);
        // Une VM enrôlée qui n'a jamais battu : NULL, pas zéro.
        expect(ligne!.vu_a).toBeNull();
    });

    it('relit la MÊME ligne par son PRÉFIXE', async () => {
        // 🔴 La rouge : interroger sur `vm_id`. Le préfixe est ce que le nom de
        // session porte — c'est la seule clé dont `signaling/trace.ts`
        // dispose pour remonter à la VM.
        base = await baseNeuve('agent-prefixe');
        await avecVm(base, 'v-1');
        await enroler(base, 'v-1', await hacher('secret-un'), PREFIXE);

        const parPrefixe = await lireParPrefixe(base, PREFIXE);
        const parVm = await lireParVm(base, 'v-1');
        expect(parPrefixe).toEqual(parVm);
        expect(parPrefixe!.vm_id).toBe('v-1');
    });

    it('rend undefined sur un inconnu, SANS LEVER, des deux côtés', async () => {
        // 🔴 Lever ferait répondre 500 au canal là où il doit répondre un
        // refus — et l'écart de comportement serait à lui seul un oracle
        // d'énumération. Précédent : `depot/utilisateur.ts::lireParEmail`.
        base = await baseNeuve('agent-inconnu');
        await expect(lireParVm(base, 'v-jamais-vue')).resolves.toBeUndefined();
        await expect(lireParPrefixe(base, 'PrefixeQuiNExistePas22')).resolves.toBeUndefined();
    });

    it('REFUSE un second enrôlement sur le MÊME préfixe', async () => {
        // 🔴 La rouge : retirer UNIQUE de la migration. Deux VMs de même
        // préfixe rendraient `lireParPrefixe` ambiguë, et le choix de la VM
        // arbitraire — le problème exact que le préfixe existe pour fermer.
        base = await baseNeuve('agent-unique');
        await avecVm(base, 'v-1');
        await avecVm(base, 'v-2');
        await enroler(base, 'v-1', await hacher('secret-un'), PREFIXE);
        await expect(enroler(base, 'v-2', await hacher('secret-deux'), PREFIXE)).rejects.toThrow();
    });

    it('marque vu_a à une MAGNITUDE D’ÉPOQUE, et la relit', async () => {
        // 🔴 Écrire `1_000` passerait sur les deux moteurs sans rien prouver :
        // c'est l'angle mort exact que P1 a payé. `pg` rend les BIGINT en
        // `string`, d'où le `Number(...)` — comme `pilotes.test.ts:100-140`.
        base = await baseNeuve('agent-vu');
        await avecVm(base, 'v-1');
        await enroler(base, 'v-1', await hacher('secret-un'), PREFIXE);

        await marquerVu(base, 'v-1', MS);
        expect(Number((await lireParVm(base, 'v-1'))!.vu_a)).toBe(MS);
        // Le battement suivant AVANCE la valeur, il ne l'ajoute pas.
        await marquerVu(base, 'v-1', MS + 30_000);
        expect(Number((await lireParVm(base, 'v-1'))!.vu_a)).toBe(MS + 30_000);
    });
});
