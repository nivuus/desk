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
import { enroler, lireParPrefixe, lireParVm, marquerVu, remplacerEmpreinte } from './agent';

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

    it('marque vu_a à une MAGNITUDE D’ÉPOQUE, et la relit EN `number`', async () => {
        // 🔴 Écrire `1_000` passerait sur les deux moteurs sans rien prouver :
        // c'est l'angle mort exact que P1 a payé.
        //
        // ⚠️ CE TEST ENVELOPPAIT SES DEUX LECTURES DANS `Number(...)`, et ce
        // `Number` CACHAIT le défaut au lieu de le mesurer : `pg` rendait ce
        // `BIGINT` en **chaîne**, si bien que `LigneAgent.vu_a` déclarait
        // `number | null` une valeur qui était une `string` en PRODUCTION.
        // Relevé par la recette de P3, corrigé au PILOTE
        // (`base/pilote-postgres.ts`, `setTypeParser`) parce que le défaut
        // était de classe. L'assertion est donc désormais NUE — c'est elle
        // qui tient la déclaration de type honnête.
        base = await baseNeuve('agent-vu');
        await avecVm(base, 'v-1');
        await enroler(base, 'v-1', await hacher('secret-un'), PREFIXE);

        await marquerVu(base, 'v-1', MS);
        expect((await lireParVm(base, 'v-1'))!.vu_a).toBe(MS);
        // Le battement suivant AVANCE la valeur, il ne l'ajoute pas.
        await marquerVu(base, 'v-1', MS + 30_000);
        expect((await lireParVm(base, 'v-1'))!.vu_a).toBe(MS + 30_000);
    });
});

describe(`remplacerEmpreinte, moteur=${MOTEUR}`, () => {
    it("remplace l'empreinte de la VM nommée, et rend le nombre de lignes touchées", async () => {
        base = await baseNeuve('agent-rotation');
        await avecVm(base, 'v-1');
        const ancienne = await hacher('le-secret-d-origine-de-la-vraie-longueur');
        await enroler(base, 'v-1', ancienne, PREFIXE);

        const neuve = await hacher('le-secret-de-remplacement-tout-aussi-long');
        expect(await remplacerEmpreinte(base, 'v-1', neuve)).toBe(1);

        const ligne = await lireParVm(base, 'v-1');
        expect(ligne!.empreinte_secret).toBe(neuve);
    });

    it("🔴 NE TOUCHE PAS le préfixe de session — relu des DEUX côtés de l'appel", async () => {
        // 🔴 LA ROUGE : faire tourner le préfixe en même temps que le secret.
        // Il compose le nom des sessions VIVANTES de cette VM
        // (`agents/prefixe.ts`) : le changer couperait toute session en cours.
        // Rotation du secret n'est pas rotation de l'identité.
        base = await baseNeuve('agent-rotation-prefixe');
        await avecVm(base, 'v-1');
        await enroler(base, 'v-1', await hacher('le-secret-d-origine-tres-long'), PREFIXE);

        const avant = (await lireParVm(base, 'v-1'))!.prefixe_session;
        await remplacerEmpreinte(base, 'v-1', await hacher('un-tout-autre-secret-aussi-long'));
        const apres = (await lireParVm(base, 'v-1'))!.prefixe_session;

        expect(apres).toBe(avant);
        expect(apres).toBe(PREFIXE);
    });

    it("🔴 ne touche AUCUNE autre VM, et ne lève pas sur une VM inconnue", async () => {
        // 🔴 LA ROUGE : oublier la clause WHERE. Toutes les VMs partageraient
        // alors le même secret, ce qu'aucun test à une seule VM ne verrait.
        base = await baseNeuve('agent-rotation-portee');
        await avecVm(base, 'v-1');
        await avecVm(base, 'v-2');
        const gardee = await hacher('le-secret-de-la-vm-voisine-bien-long');
        await enroler(base, 'v-1', await hacher('le-secret-a-remplacer-bien-long'), PREFIXE);
        await enroler(base, 'v-2', gardee, 'Zk4pQ7mNr2xTvB9wLcHd1s');

        await remplacerEmpreinte(base, 'v-1', await hacher('le-secret-neuf-tout-aussi-long'));
        expect((await lireParVm(base, 'v-2'))!.empreinte_secret).toBe(gardee);

        // Une VM inconnue : zéro ligne touchée, et surtout AUCUNE exception —
        // c'est ce qui permet à l'appelant de rendre un refus motivé.
        expect(await remplacerEmpreinte(base, 'v-jamais-enrolee', 'peu-importe')).toBe(0);
    });
});
