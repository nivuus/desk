// Ces tests tournent sous `test:sqlite` ET sous `test:postgres`, sans être
// écrits deux fois : ils emploient le même harnais que `pilotes.test.ts`.
//
// 🔴 L'HORLOGE EST UN PARAMÈTRE, et c'est ce que ces valeurs exactes
// vérifient. Un `Date.now()` caché dans le dépôt les ferait toutes échouer.

import { afterEach, describe, expect, it } from 'vitest';
import { baseNeuve, MOTEUR } from '../base/harnais';
import type { Pilote } from '../base/pilote';
import {
    balayerLesOuvertes,
    clore,
    compterOuvertesDe,
    lireParNom,
    ouvrirSession,
} from './session';

let base: Pilote | undefined;

afterEach(async () => {
    await base?.fermer();
    base = undefined;
});

describe(`session repository, engine=${MOTEUR}`, () => {
    it('opens a row with ouverte_a set and fermee_a null', async () => {
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

    it('closes the row: fermee_a set, reason recorded', async () => {
        base = await baseNeuve('dep-clot');
        const id = await ouvrirSession(base, 'bureau', 1_000_000);
        await clore(base, id, 1_000_500, 'both peers left');
        const [ligne] = await lireParNom(base, 'bureau');
        expect(Number(ligne.fermee_a)).toBe(1_000_500);
        expect(ligne.motif).toBe('both peers left');
    });

    it('does not close twice: the second closing does not change fermee_a', async () => {
        base = await baseNeuve('dep-double');
        const id = await ouvrirSession(base, 'bureau', 1_000_000);
        await clore(base, id, 1_000_500, 'premier');
        await clore(base, id, 9_000_000, 'second');
        const [ligne] = await lireParNom(base, 'bureau');
        expect(Number(ligne.fermee_a)).toBe(1_000_500);
        expect(ligne.motif).toBe('premier');
    });

    it('sweeps at startup the rows left open, and returns their count', async () => {
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
        expect(a.motif).toBe('platform restarted');
        // La ligne déjà close garde SON instant et SON motif.
        const [c] = await lireParNom(base, 'c');
        expect(Number(c.fermee_a)).toBe(4_000);
        expect(c.motif).toBe('normale');
    });

    it('two successive sessions of the same name are two distinct rows', async () => {
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

    it('writes utilisateur_id NULL when none is given — P1 is intact', async () => {
        // 🔴 Le paramètre est FACULTATIF : le rendre requis casserait tous les
        // appels de P1, et une session de contrôle `bureau` où l'agent arrive
        // seul n'a personne à inscrire.
        base = await baseNeuve('dep-without-user');
        await ouvrirSession(base, 'bureau', 1_000_000);
        const [ligne] = await lireParNom(base, 'bureau');
        expect(ligne.utilisateur_id).toBeNull();
    });

    it('writes vm_id NULL when none is given — the local trial mode', async () => {
        // 🔴 La rouge : rendre le paramètre OBLIGATOIRE. Une session `bureau`
        // ouverte par un agent NON enrôlé — le mode d'essai local que la spec
        // §10 pose comme légitime — n'a aucune VM honnête à inscrire, et la
        // colonne reste NULLABLE pour cette raison, pas par dette.
        base = await baseNeuve('dep-sans-vm');
        await ouvrirSession(base, 'bureau', 1_000_000);
        const [ligne] = await lireParNom(base, 'bureau');
        expect(ligne.vm_id).toBeNull();
    });

    it('writes vm_id when the trace resolved the prefix — legacy item no. 3 of P2', async () => {
        // `session.vm_id` restait entièrement NULL au sortir de P2. C'est la
        // trace qui le résout (préfixe du nom de session -> VM), et c'est ici
        // qu'elle l'inscrit.
        base = await baseNeuve('dep-with-vm');
        await ouvrirSession(base, 'RhH1x2QmTz9kLpVbNc7dAw:bureau', 1_787_136_773_742, undefined, 'v-42');
        const [ligne] = await lireParNom(base, 'RhH1x2QmTz9kLpVbNc7dAw:bureau');
        expect(ligne.vm_id).toBe('v-42');
        // L'utilisateur reste NULL : un agent seul n'a personne à inscrire.
        expect(ligne.utilisateur_id).toBeNull();
        // Et rien d'autre n'a bougé — la magnitude d'époque comprise.
        expect(Number(ligne.ouverte_a)).toBe(1_787_136_773_742);
        expect(ligne.fermee_a).toBeNull();
    });

    it('writes BOTH when the guard and the trace each established theirs', async () => {
        base = await baseNeuve('dep-with-both');
        await ouvrirSession(base, 'P:w-1', 1_000_000, 'u-42', 'v-42');
        const [ligne] = await lireParNom(base, 'P:w-1');
        expect(ligne.utilisateur_id).toBe('u-42');
        expect(ligne.vm_id).toBe('v-42');
    });

    it('writes utilisateur_id when the guard established one', async () => {
        // C'est ce qui rend le mot « enregistrée » du critère ③ littéralement
        // vrai, et c'est ce dont P4 aura besoin pour attribuer une VM.
        base = await baseNeuve('dep-with-user');
        await ouvrirSession(base, 'bureau', 1_000_000, 'u-42');
        const [ligne] = await lireParNom(base, 'bureau');
        expect(ligne.utilisateur_id).toBe('u-42');
        // Et rien d'autre n'a bougé.
        expect(Number(ligne.ouverte_a)).toBe(1_000_000);
        expect(ligne.fermee_a).toBeNull();
    });
});

describe(`compterOuvertesDe, engine=${MOTEUR}`, () => {
    /// Une époque réelle, jamais un petit nombre commode : leçon de P1.
    const MS = 1_787_136_773_742;

    it('🔴 the count goes from 0 TO 1 — the TRANSITION is seen', async () => {
        // 🔴 La rouge : rendre une constante. Le test doit voir le compte
        // BOUGER, pas lire un nombre — c'est la forme du critère ④ de P3,
        // appliquée ici. Un test qui n'asserterait que `1` serait vert sur un
        // `return 1`.
        base = await baseNeuve('sess-compte-transition');
        expect(await compterOuvertesDe(base, 'u-ada')).toBe(0);
        await ouvrirSession(base, 'PREFIXE:bureau', MS, 'u-ada');
        expect(await compterOuvertesDe(base, 'u-ada')).toBe(1);
        // 🔴 ET C'EST UN NOMBRE. Sans le `setTypeParser` de
        // `base/pilote-postgres.ts`, un `COUNT(*)` — un `int8` — reviendrait
        // en CHAÎNE, et `'0' == 0` mais `'0' !== 0`. Cette valeur
        // n'appartenant à AUCUNE colonne, le balayage colonne par colonne de
        // `pilotes.test.ts` ne la couvre pas.
        expect(typeof (await compterOuvertesDe(base, 'u-ada'))).toBe('number');
    });

    it('🔴 a CLOSED session is not counted', async () => {
        // 🔴 La rouge : omettre `AND fermee_a IS NULL`. Le compte deviendrait
        // un historique, et le hub dirait « vous avez une session ouverte » à
        // qui n'en a plus depuis des semaines.
        base = await baseNeuve('sess-compte-close');
        const id = await ouvrirSession(base, 'PREFIXE:bureau', MS, 'u-ada');
        expect(await compterOuvertesDe(base, 'u-ada')).toBe(1);
        await clore(base, id, MS + 60_000, 'both peers left');
        expect(await compterOuvertesDe(base, 'u-ada')).toBe(0);
    });

    it('🔴 the session of ANOTHER user is not counted', async () => {
        // 🔴 La rouge : omettre le `WHERE utilisateur_id = ?`. Le compte
        // deviendrait global, et chacun verrait le nombre de sessions de tous.
        base = await baseNeuve('sess-compte-autrui');
        await ouvrirSession(base, 'PREFIXE:bureau', MS, 'u-bob');
        expect(await compterOuvertesDe(base, 'u-ada')).toBe(0);
        expect(await compterOuvertesDe(base, 'u-bob')).toBe(1);
    });

    it('🔴 a session with a NULL `utilisateur_id` is counted for NOBODY', async () => {
        // 🔴 La rouge : traiter `NULL` comme appartenant au demandeur (par
        // exemple `utilisateur_id = ? OR utilisateur_id IS NULL`). C'est le cas
        // NOMINAL d'une session de contrôle appariée par l'agent seul — chacun
        // se verrait attribuer les sessions de toutes les VMs de la flotte.
        base = await baseNeuve('sess-compte-nul');
        await ouvrirSession(base, 'PREFIXE:bureau', MS);
        expect(await compterOuvertesDe(base, 'u-ada')).toBe(0);
        expect(await compterOuvertesDe(base, '')).toBe(0);
    });
});
