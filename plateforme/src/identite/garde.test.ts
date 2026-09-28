// La règle complète de la poignée de main : qui passe, qui est refusé, avec
// quel motif.

import { describe, expect, it } from 'vitest';
import { signer, DUREE_JETON_ACCES_MS } from './jeton';
import { ProprieteDeSession } from '../signaling/propriete';
import { garde } from './garde';

const SECRET = 'un-secret-de-plateforme-de-quarante-octets';
const T0 = 1_787_000_000_000;

/// Deux préfixes de la VRAIE longueur que `agents/prefixe.ts` produit — 22
/// caractères de base64url. Une chaîne courte ne mesurerait pas la même chose.
const P = 'RhH1x2QmTz9kLpVbNc7dAw';
const Q = 'Zk4pQw8sXt2vBn6mLr0eYu';

function neuve(maintenant: () => number = () => T0) {
    const proprietes = new ProprieteDeSession();
    return { proprietes, g: garde(SECRET, maintenant, proprietes) };
}

describe('handshake guard', () => {
    it('🔴 REFUSES an `agent` peer WITHOUT a token — the P2 window is CLOSED', () => {
        // 🔴 CETTE ASSERTION EST L'INVERSE EXACT DE CELLE QUE P2 LIVRAIT, et
        // c'est le geste central de P3. P2 écrivait ici « accepte un pair
        // `agent` SANS jeton », en annonçant dans son propre commentaire que
        // « le jour où P3 l'inversera, il faudra réécrire ce test À DESSEIN,
        // pas par surprise ». C'est fait, et à dessein.
        //
        // 🔴 LA ROUGE EST GRATUITE : le binaire de P2 la porte. `garde.ts`
        // ouvrait sur `if (role === 'agent') return { ok: true };`, et un pair
        // qui se déclarait `{"role":"agent"}` obtenait des identifiants TURN
        // valables 86 400 s sans présenter la moindre identité.
        const { g } = neuve();
        const v = g.verify({ role: 'agent', session: 'bureau' });
        expect(v.ok).toBe(false);
        if (v.ok) return;
        expect(v.motif).toBe('jeton-absent');
    });

    it('accepts an `agent` whose token PREFIXES the requested session', () => {
        const { g } = neuve();
        const jeton = signer(P, SECRET, T0, DUREE_JETON_ACCES_MS, 'agent');
        expect(g.verify({ role: 'agent', session: `${P}:bureau`, jeton }))
            .toEqual({ ok: true });
        // La même VM sur une de SES fenêtres.
        expect(g.verify({ role: 'agent', session: `${P}:w-1`, jeton }).ok).toBe(true);
    });

    it('🔴 REFUSES the SAME agent token on the session of ANOTHER VM', () => {
        // 🔴 La rouge : omettre la comparaison de préfixe. Un agent enrôlé
        // occuperait alors la session de TOUTE autre VM — c'est le pendant
        // `agent` du critère ③ de P2, et sans lui l'enrôlement n'authentifie
        // que l'existence d'une VM, jamais LAQUELLE.
        const { g } = neuve();
        const jeton = signer(P, SECRET, T0, DUREE_JETON_ACCES_MS, 'agent');
        const refus = g.verify({ role: 'agent', session: `${Q}:bureau`, jeton });
        expect(refus).toMatchObject({ ok: false, motif: 'session-refusee' });
        if (refus.ok) return;
        // Le message SUR LE FIL ne distingue pas les causes : il est le même
        // que celui d'un client refusé pour appartenance. Le JOURNAL, lui,
        // porte de quoi diagnostiquer.
        expect(refus.message).toBe('access refused to the requested session');
        expect(refus.journal).toContain(`${Q}:bureau`);
    });

    it('🔴 REFUSES a prefix that is only a START of the subject, without the separator', () => {
        // 🔴 La rouge : comparer par `session.startsWith(sujet)` SANS le
        // séparateur. Un agent de préfixe `AB` occuperait alors les sessions
        // de la VM `ABC`, dont le préfixe le prolonge — une collision qui ne
        // se produirait qu'entre deux VMs précises, donc jamais en essai et
        // toujours en production.
        const { g } = neuve();
        const jeton = signer('AB', SECRET, T0, DUREE_JETON_ACCES_MS, 'agent');
        expect(g.verify({ role: 'agent', session: 'ABC:bureau', jeton }))
            .toMatchObject({ ok: false, motif: 'session-refusee' });
    });

    it('🔴 REFUSES a HUMAN token presented as `role:agent` — confusion, direction 1', () => {
        // 🔴 La rouge : omettre `type === 'agent'`. Les deux jetons sont signés
        // par le MÊME secret : un jeton humain volé ouvrirait un rôle `agent`,
        // donc un `ice-config` sur toute session dont il préfixerait le nom.
        //
        // ⚠️ DEUX SENS DE CONFUSION, DEUX TESTS, jamais un seul à deux
        // assertions : `expect` interrompt à la première, et la seconde ne
        // serait éprouvée par rien. C'est la leçon ①A-bis de P2, appliquée
        // d'avance.
        const { g } = neuve();
        const humain = signer(P, SECRET, T0);
        expect(g.verify({ role: 'agent', session: `${P}:bureau`, jeton: humain }))
            .toMatchObject({ ok: false, motif: 'session-refusee' });
    });

    it('🔴 REFUSES an AGENT token presented as `role:client` — confusion, direction 2', () => {
        // 🔴 La rouge : omettre `type !== 'agent'`. Un jeton d'agent ouvrirait
        // un rôle `client`, contournant l'appartenance de session que P2 a
        // posée (`signaling/propriete.ts`) : l'agent deviendrait un
        // utilisateur, sur la session de n'importe qui.
        const { g } = neuve();
        const agent = signer(P, SECRET, T0, DUREE_JETON_ACCES_MS, 'agent');
        expect(g.verify({ role: 'client', session: `${P}:bureau`, jeton: agent }))
            .toMatchObject({ ok: false, motif: 'session-refusee' });
    });

    it('REFUSES a `client` without a token', () => {
        const { g } = neuve();
        const v = g.verify({ role: 'client', session: 's-1' });
        expect(v.ok).toBe(false);
        if (v.ok) return;
        expect(v.motif).toBe('jeton-absent');
    });

    it('REFUSES a malformed or badly signed token', () => {
        const { g } = neuve();
        expect(g.verify({ role: 'client', session: 's-1', jeton: 'not.a.token' }))
            .toMatchObject({ ok: false, motif: 'jeton-invalide' });
        const autre = signer('u1', 'ANOTHER-secret-of-forty-characters-or-more', T0);
        expect(g.verify({ role: 'client', session: 's-1', jeton: autre }))
            .toMatchObject({ ok: false, motif: 'jeton-invalide' });
    });

    it('REFUSES a token of an unexpected type, without ever THROWING', () => {
        // Un `String(jeton)` sans contrôle ferait passer un objet pour une
        // chaîne, ou lèverait sur `null`.
        const { g } = neuve();
        for (const jeton of [42, { sub: 'u1' }, null, [], true]) {
            expect(() => g.verify({ role: 'client', session: 's-1', jeton }))
                .not.toThrow();
            expect(g.verify({ role: 'client', session: 's-1', jeton }).ok).toBe(false);
        }
    });

    it('accepts a `client` with a valid token on a FREE session', () => {
        const { g } = neuve();
        const jeton = signer('u1', SECRET, T0);
        expect(g.verify({ role: 'client', session: 's-1', jeton }))
            .toEqual({ ok: true, userId: 'u1' });
    });

    it('accepts the SAME user on THEIR session, and REFUSES another', () => {
        const { g, proprietes } = neuve();
        proprietes.revendiquer('s-1', 'u1');
        expect(g.verify({ role: 'client', session: 's-1', jeton: signer('u1', SECRET, T0) }))
            .toEqual({ ok: true, userId: 'u1' });

        const refus = g.verify({
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

    it('REFUSES an EXPIRED token, on a clock that VARIES', () => {
        // 🔴 Le critère ② vécu de bout en bout : le MÊME jeton, deux instants.
        // Figer l'horloge rendrait le second appel vert.
        let maintenant = T0;
        const { g } = neuve(() => maintenant);
        const jeton = signer('u1', SECRET, T0);
        expect(g.verify({ role: 'client', session: 's-1', jeton }))
            .toEqual({ ok: true, userId: 'u1' });
        maintenant = T0 + DUREE_JETON_ACCES_MS;
        expect(g.verify({ role: 'client', session: 's-1', jeton }))
            .toMatchObject({ ok: false, motif: 'jeton-expire' });
    });

    it('verify HAS NO SIDE EFFECT: two calls claim nothing', () => {
        // 🔴 Y mettre la revendication laisserait une appartenance FANTÔME
        // derrière un pair que `Appariement::declarer` refuse ensuite pour
        // cause de rôle déjà occupé.
        const { g, proprietes } = neuve();
        const jeton = signer('u1', SECRET, T0);
        g.verify({ role: 'client', session: 's-1', jeton });
        g.verify({ role: 'client', session: 's-1', jeton });
        expect(proprietes.proprietaire('s-1')).toBeUndefined();
        // Et un AUTRE utilisateur passe encore, preuve que rien n'a été posé.
        expect(g.verify({ role: 'client', session: 's-1', jeton: signer('u2', SECRET, T0) }).ok)
            .toBe(true);
    });

    it('claim then release: the session changes hands', () => {
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
