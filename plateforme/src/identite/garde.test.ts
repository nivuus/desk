// La règle complète de la poignée de main : qui passe, qui est refusé, avec
// quel motif.

import { describe, expect, it } from 'vitest';
import { signer, DUREE_JETON_ACCES_MS } from './jeton';
import { ProprieteDeSession } from '../signaling/propriete';
import { garde } from './garde';

const SECRET = 'un-secret-de-plateforme-de-quarante-octets';
const T0 = 1_787_000_000_000;

function neuve(maintenant: () => number = () => T0) {
    const proprietes = new ProprieteDeSession();
    return { proprietes, g: garde(SECRET, maintenant, proprietes) };
}

describe('garde de la poignée de main', () => {
    it('accepte un pair `agent` SANS jeton — la fenêtre déclarée jusqu’à P3', () => {
        // ⚠️ Ce test EXISTE pour rendre visible la moitié du trou que P2 ne
        // ferme pas : `role:'agent'` demeure un chemin ANONYME vers des
        // identifiants TURN de 24 h. L'agent Rust n'a pas d'identité avant P3,
        // et lui en exiger une casserait le chantier D en cours. Le jour où P3
        // l'inversera, il faudra réécrire ce test À DESSEIN, pas par surprise.
        const { g } = neuve();
        expect(g.verifier({ role: 'agent', session: 'bureau' })).toEqual({ ok: true });
    });

    it('REFUSE un `client` sans jeton', () => {
        const { g } = neuve();
        const v = g.verifier({ role: 'client', session: 's-1' });
        expect(v.ok).toBe(false);
        if (v.ok) return;
        expect(v.motif).toBe('jeton-absent');
    });

    it('REFUSE un jeton mal formé ou mal signé', () => {
        const { g } = neuve();
        expect(g.verifier({ role: 'client', session: 's-1', jeton: 'pas.un.jeton' }))
            .toMatchObject({ ok: false, motif: 'jeton-invalide' });
        const autre = signer('u1', 'un-AUTRE-secret-de-quarante-caracteres-ou-plus', T0);
        expect(g.verifier({ role: 'client', session: 's-1', jeton: autre }))
            .toMatchObject({ ok: false, motif: 'jeton-invalide' });
    });

    it('REFUSE un jeton d’un type inattendu, sans jamais LEVER', () => {
        // Un `String(jeton)` sans contrôle ferait passer un objet pour une
        // chaîne, ou lèverait sur `null`.
        const { g } = neuve();
        for (const jeton of [42, { sub: 'u1' }, null, [], true]) {
            expect(() => g.verifier({ role: 'client', session: 's-1', jeton }))
                .not.toThrow();
            expect(g.verifier({ role: 'client', session: 's-1', jeton }).ok).toBe(false);
        }
    });

    it('accepte un `client` au jeton valide sur une session LIBRE', () => {
        const { g } = neuve();
        const jeton = signer('u1', SECRET, T0);
        expect(g.verifier({ role: 'client', session: 's-1', jeton }))
            .toEqual({ ok: true, utilisateurId: 'u1' });
    });

    it('accepte le MÊME utilisateur sur SA session, et REFUSE un autre', () => {
        const { g, proprietes } = neuve();
        proprietes.revendiquer('s-1', 'u1');
        expect(g.verifier({ role: 'client', session: 's-1', jeton: signer('u1', SECRET, T0) }))
            .toEqual({ ok: true, utilisateurId: 'u1' });

        const refus = g.verifier({
            role: 'client',
            session: 's-1',
            jeton: signer('u2', SECRET, T0),
        });
        expect(refus).toMatchObject({ ok: false, motif: 'session-refusee' });
        if (refus.ok) return;
        // 🔴 Le message SUR LE FIL ne nomme ni le propriétaire ni l'existence
        // de la session : ce serait un oracle. Le JOURNAL, lui, porte le nom
        // de session et l'identifiant du demandeur — c'est ce qui sépare un
        // diagnostic d'un oracle.
        expect(refus.message).not.toContain('u1');
        expect(refus.message).not.toContain('s-1');
        expect(refus.journal).toContain('s-1');
        expect(refus.journal).toContain('u2');
    });

    it('REFUSE un jeton EXPIRÉ, sur une horloge qui VARIE', () => {
        // 🔴 Le critère ② vécu de bout en bout : le MÊME jeton, deux instants.
        // Figer l'horloge rendrait le second appel vert.
        let maintenant = T0;
        const { g } = neuve(() => maintenant);
        const jeton = signer('u1', SECRET, T0);
        expect(g.verifier({ role: 'client', session: 's-1', jeton }))
            .toEqual({ ok: true, utilisateurId: 'u1' });
        maintenant = T0 + DUREE_JETON_ACCES_MS;
        expect(g.verifier({ role: 'client', session: 's-1', jeton }))
            .toMatchObject({ ok: false, motif: 'jeton-expire' });
    });

    it('verifier N’A AUCUN EFFET DE BORD : deux appels ne revendiquent rien', () => {
        // 🔴 Y mettre la revendication laisserait une appartenance FANTÔME
        // derrière un pair que `Appariement::declarer` refuse ensuite pour
        // cause de rôle déjà occupé.
        const { g, proprietes } = neuve();
        const jeton = signer('u1', SECRET, T0);
        g.verifier({ role: 'client', session: 's-1', jeton });
        g.verifier({ role: 'client', session: 's-1', jeton });
        expect(proprietes.proprietaire('s-1')).toBeUndefined();
        // Et un AUTRE utilisateur passe encore, preuve que rien n'a été posé.
        expect(g.verifier({ role: 'client', session: 's-1', jeton: signer('u2', SECRET, T0) }).ok)
            .toBe(true);
    });

    it('revendiquer puis liberer : la session change de main', () => {
        const { g, proprietes } = neuve();
        g.revendiquer('s-1', 'u1');
        expect(proprietes.proprietaire('s-1')).toBe('u1');
        // Un `agent` n'a pas d'identifiant : il ne revendique rien.
        g.revendiquer('s-2', undefined);
        expect(proprietes.proprietaire('s-2')).toBeUndefined();
        g.liberer('s-1');
        expect(proprietes.proprietaire('s-1')).toBeUndefined();
    });
});
