import { describe, expect, it } from 'vitest';
import { cibleDeRedirection } from './redirection';

describe('cibleDeRedirection', () => {
    it('conserve la chaine de requete', () => {
        // 🔴 LE POINT DE TOUTE LA TACHE : les PWA DEJA installees portent
        // `shell.html?app=<id>` dans leur id (fige, jamais mis a jour -- voir
        // hub/manifeste.ts), pas dans start_url depuis ce lot, et le
        // manifeste blob: qu elles portent ne sera jamais relu. Perdre
        // `?app=` les casserait aussi surement que supprimer le fichier.
        expect(cibleDeRedirection('?app=u-1')).toBe('/?app=u-1');
    });

    it('sans requete, mene a la racine nue', () => {
        expect(cibleDeRedirection('')).toBe('/');
    });

    it('conserve PLUSIEURS parametres', () => {
        expect(cibleDeRedirection('?app=u-1&plateforme=https%3A%2F%2Fx')).toBe(
            '/?app=u-1&plateforme=https%3A%2F%2Fx',
        );
    });
});
