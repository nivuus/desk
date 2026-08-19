import { describe, expect, it } from 'vitest';
import { lireConfig } from './config';

describe('lireConfig', () => {
    it("refuse de démarrer sans PLATEFORME_HOTE — il n'y a pas de défaut", () => {
        // Le défaut DOIT être l'absence de défaut (spec §4, critère ④).
        // Poser '0.0.0.0' par défaut ferait passer un test d'écoute sans rien
        // garantir : c'est exactement la panne muette que ce dépôt combat.
        expect(() => lireConfig({})).toThrow(/PLATEFORME_HOTE/);
    });

    it("n'invente pas d'adresse quand la variable est vide", () => {
        expect(() => lireConfig({ PLATEFORME_HOTE: '' })).toThrow(/PLATEFORME_HOTE/);
    });

    it('lit les quatre champs, avec leurs défauts non permissifs', () => {
        const c = lireConfig({ PLATEFORME_HOTE: '127.0.0.1' });
        expect(c).toEqual({
            hote: '127.0.0.1',
            port: 8080,
            base: 'sqlite',
            urlBase: ':memory:',
        });
    });

    it('refuse un PLATEFORME_BASE inconnu, plutôt que de retomber sur sqlite', () => {
        expect(() => lireConfig({ PLATEFORME_HOTE: '::1', PLATEFORME_BASE: 'mysql' }))
            .toThrow(/PLATEFORME_BASE/);
    });

    it('refuse un port qui n’est pas un entier', () => {
        expect(() => lireConfig({ PLATEFORME_HOTE: '::1', PLATEFORME_PORT: 'huit-mille' }))
            .toThrow(/PLATEFORME_PORT/);
    });
});
