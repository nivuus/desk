import { describe, expect, it } from 'vitest';
import { cibleDeRedirection } from './redirection';

describe('cibleDeRedirection', () => {
    it('keeps the query string', () => {
        // 🔴 LE POINT DE TOUTE LA TACHE : les PWA DEJA installees portent
        // `shell.html?app=<id>` dans leur id (fige, jamais mis a jour -- voir
        // hub/manifeste.ts), pas dans start_url depuis ce lot, et le
        // manifeste blob: qu elles portent ne sera jamais relu. Perdre
        // `?app=` les casserait aussi surement que supprimer le fichier.
        expect(cibleDeRedirection('?app=u-1')).toBe('/?app=u-1');
    });

    it('without a query, leads to the bare root', () => {
        expect(cibleDeRedirection('')).toBe('/');
    });

    it('keeps SEVERAL parameters', () => {
        expect(cibleDeRedirection('?app=u-1&plateforme=https%3A%2F%2Fx')).toBe(
            '/?app=u-1&plateforme=https%3A%2F%2Fx',
        );
    });
});
