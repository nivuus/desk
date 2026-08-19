// Le registre d'appartenance de session, pur et synchrone.

import { describe, expect, it } from 'vitest';
import { ProprieteDeSession } from './propriete';

describe('ProprieteDeSession', () => {
    it('une session libre n’a pas de propriétaire', () => {
        expect(new ProprieteDeSession().proprietaire('s-1')).toBeUndefined();
    });

    it('revendiquée par u1, elle rend u1 — et pas u2', () => {
        // 🔴 La seconde assertion est le point : un registre qui rendrait
        // `undefined` quoi qu'il arrive passerait la première, et plus
        // personne ne serait jamais refusé.
        const r = new ProprieteDeSession();
        r.revendiquer('s-1', 'u1');
        expect(r.proprietaire('s-1')).toBe('u1');
        expect(r.proprietaire('s-1')).not.toBe('u2');
    });

    it('revendiquer deux fois par le MÊME utilisateur est sans effet, jamais une erreur', () => {
        // Une reconnexion du même utilisateur casserait sinon sa propre
        // session.
        const r = new ProprieteDeSession();
        r.revendiquer('s-1', 'u1');
        expect(() => r.revendiquer('s-1', 'u1')).not.toThrow();
        expect(r.proprietaire('s-1')).toBe('u1');
    });

    it('libérer rend la session à nouveau libre, et revendicable par un autre', () => {
        // Ne jamais libérer perdrait un nom de session à vie.
        const r = new ProprieteDeSession();
        r.revendiquer('s-1', 'u1');
        r.liberer('s-1');
        expect(r.proprietaire('s-1')).toBeUndefined();
        r.revendiquer('s-1', 'u2');
        expect(r.proprietaire('s-1')).toBe('u2');
    });

    it('deux sessions ne se mélangent pas', () => {
        const r = new ProprieteDeSession();
        r.revendiquer('s-1', 'u1');
        r.revendiquer('s-2', 'u2');
        expect(r.proprietaire('s-1')).toBe('u1');
        expect(r.proprietaire('s-2')).toBe('u2');
        r.liberer('s-1');
        expect(r.proprietaire('s-2')).toBe('u2');
    });
});
