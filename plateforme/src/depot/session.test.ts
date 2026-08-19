// Ces tests tournent sous `test:sqlite` ET sous `test:postgres`, sans être
// écrits deux fois : ils emploient le même harnais que `pilotes.test.ts`.
//
// 🔴 L'HORLOGE EST UN PARAMÈTRE, et c'est ce que ces valeurs exactes
// vérifient. Un `Date.now()` caché dans le dépôt les ferait toutes échouer.

import { afterEach, describe, expect, it } from 'vitest';
import { baseNeuve, MOTEUR } from '../base/harnais';
import type { Pilote } from '../base/pilote';
import { balayerLesOuvertes, clore, lireParNom, ouvrirSession } from './session';

let base: Pilote | undefined;

afterEach(async () => {
    await base?.fermer();
    base = undefined;
});

describe(`dépôt session, moteur=${MOTEUR}`, () => {
    it('ouvre une ligne avec ouverte_a posé et fermee_a nul', async () => {
        base = await baseNeuve('dep-ouvre');
        const id = await ouvrirSession(base, 'bureau', 1_000_000);
        const [ligne] = await lireParNom(base, 'bureau');
        expect(ligne.id).toBe(id);
        expect(ligne.nom_session).toBe('bureau');
        // Valeur EXACTE : c'est l'assertion qui interdit un `Date.now()` caché.
        expect(Number(ligne.ouverte_a)).toBe(1_000_000);
        expect(ligne.fermee_a).toBeNull();
        expect(ligne.motif).toBeNull();
    });

    it('clôt la ligne : fermee_a posé, motif enregistré', async () => {
        base = await baseNeuve('dep-clot');
        const id = await ouvrirSession(base, 'bureau', 1_000_000);
        await clore(base, id, 1_000_500, 'les deux pairs sont partis');
        const [ligne] = await lireParNom(base, 'bureau');
        expect(Number(ligne.fermee_a)).toBe(1_000_500);
        expect(ligne.motif).toBe('les deux pairs sont partis');
    });

    it('ne clôt pas deux fois : la seconde clôture ne change pas fermee_a', async () => {
        base = await baseNeuve('dep-double');
        const id = await ouvrirSession(base, 'bureau', 1_000_000);
        await clore(base, id, 1_000_500, 'premier');
        await clore(base, id, 9_000_000, 'second');
        const [ligne] = await lireParNom(base, 'bureau');
        expect(Number(ligne.fermee_a)).toBe(1_000_500);
        expect(ligne.motif).toBe('premier');
    });

    it('balaie au démarrage les lignes restées ouvertes, et rend leur compte', async () => {
        base = await baseNeuve('dep-balai');
        await ouvrirSession(base, 'a', 1_000);
        await ouvrirSession(base, 'b', 2_000);
        const close = await ouvrirSession(base, 'c', 3_000);
        await clore(base, close, 4_000, 'normale');

        expect(await balayerLesOuvertes(base, 5_000)).toBe(2);
        // Idempotent : un second balayage ne trouve plus rien.
        expect(await balayerLesOuvertes(base, 6_000)).toBe(0);

        const [a] = await lireParNom(base, 'a');
        expect(Number(a.fermee_a)).toBe(5_000);
        expect(a.motif).toBe('plateforme redémarrée');
        // La ligne déjà close garde SON instant et SON motif.
        const [c] = await lireParNom(base, 'c');
        expect(Number(c.fermee_a)).toBe(4_000);
        expect(c.motif).toBe('normale');
    });

    it('deux sessions du même nom successives sont deux lignes distinctes', async () => {
        // Le nom de session n'est PAS unique dans le temps : `bureau` revient à
        // chaque démarrage d'agent (`agent/src/superviseur/protocole.rs`,
        // constante SESSION_DE_CONTROLE). La clé primaire est un UUID, jamais
        // le nom.
        base = await baseNeuve('dep-homonymes');
        const un = await ouvrirSession(base, 'bureau', 1_000);
        await clore(base, un, 2_000, 'fin');
        const deux = await ouvrirSession(base, 'bureau', 3_000);
        expect(deux).not.toBe(un);
        expect(await lireParNom(base, 'bureau')).toHaveLength(2);
    });

    it('écrit utilisateur_id NULL quand aucun n’est fourni — P1 est intact', async () => {
        // 🔴 Le paramètre est FACULTATIF : le rendre requis casserait tous les
        // appels de P1, et une session de contrôle `bureau` où l'agent arrive
        // seul n'a personne à inscrire.
        base = await baseNeuve('dep-sans-utilisateur');
        await ouvrirSession(base, 'bureau', 1_000_000);
        const [ligne] = await lireParNom(base, 'bureau');
        expect(ligne.utilisateur_id).toBeNull();
    });

    it('écrit vm_id NULL quand aucun n’est fourni — le mode d’essai local', async () => {
        // 🔴 La rouge : rendre le paramètre OBLIGATOIRE. Une session `bureau`
        // ouverte par un agent NON enrôlé — le mode d'essai local que la spec
        // §10 pose comme légitime — n'a aucune VM honnête à inscrire, et la
        // colonne reste NULLABLE pour cette raison, pas par dette.
        base = await baseNeuve('dep-sans-vm');
        await ouvrirSession(base, 'bureau', 1_000_000);
        const [ligne] = await lireParNom(base, 'bureau');
        expect(ligne.vm_id).toBeNull();
    });

    it('écrit vm_id quand la trace a résolu le préfixe — le legs n°3 de P2', async () => {
        // `session.vm_id` restait entièrement NULL au sortir de P2. C'est la
        // trace qui le résout (préfixe du nom de session -> VM), et c'est ici
        // qu'elle l'inscrit.
        base = await baseNeuve('dep-avec-vm');
        await ouvrirSession(base, 'RhH1x2QmTz9kLpVbNc7dAw:bureau', 1_787_136_773_742, undefined, 'v-42');
        const [ligne] = await lireParNom(base, 'RhH1x2QmTz9kLpVbNc7dAw:bureau');
        expect(ligne.vm_id).toBe('v-42');
        // L'utilisateur reste NULL : un agent seul n'a personne à inscrire.
        expect(ligne.utilisateur_id).toBeNull();
        // Et rien d'autre n'a bougé — la magnitude d'époque comprise.
        expect(Number(ligne.ouverte_a)).toBe(1_787_136_773_742);
        expect(ligne.fermee_a).toBeNull();
    });

    it('écrit les DEUX quand la garde et la trace ont chacune établi la leur', async () => {
        base = await baseNeuve('dep-avec-les-deux');
        await ouvrirSession(base, 'P:w-1', 1_000_000, 'u-42', 'v-42');
        const [ligne] = await lireParNom(base, 'P:w-1');
        expect(ligne.utilisateur_id).toBe('u-42');
        expect(ligne.vm_id).toBe('v-42');
    });

    it('écrit utilisateur_id quand la garde en a établi un', async () => {
        // C'est ce qui rend le mot « enregistrée » du critère ③ littéralement
        // vrai, et c'est ce dont P4 aura besoin pour attribuer une VM.
        base = await baseNeuve('dep-avec-utilisateur');
        await ouvrirSession(base, 'bureau', 1_000_000, 'u-42');
        const [ligne] = await lireParNom(base, 'bureau');
        expect(ligne.utilisateur_id).toBe('u-42');
        // Et rien d'autre n'a bougé.
        expect(Number(ligne.ouverte_a)).toBe(1_000_000);
        expect(ligne.fermee_a).toBeNull();
    });
});
