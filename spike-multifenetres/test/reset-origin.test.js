import { describe, expect, it } from 'vitest';
import { nettoyerOrigin } from '../public/lib/reset-origin.js';

// Faux `navigator.serviceWorker` : deux enregistrements, dont un de l'ancienne app.
function fauxServiceWorker(portees) {
    const desenregistres = [];
    return {
        desenregistres,
        api: {
            getRegistrations: async () =>
                portees.map((portee) => ({
                    scope: portee,
                    unregister: async () => {
                        desenregistres.push(portee);
                        return true;
                    },
                })),
        },
    };
}

function fauxCaches(noms) {
    const supprimes = [];
    return {
        supprimes,
        api: {
            keys: async () => noms,
            delete: async (nom) => {
                supprimes.push(nom);
                return true;
            },
        },
    };
}

describe('nettoyerOrigin', () => {
    it('désenregistre tous les service workers et rapporte leurs portées', async () => {
        const sw = fauxServiceWorker(['https://app.allanic.me/', 'https://app.allanic.me/excel/']);
        const cache = fauxCaches([]);

        const rapport = await nettoyerOrigin({ serviceWorker: sw.api, caches: cache.api });

        expect(rapport.serviceWorkers).toEqual([
            'https://app.allanic.me/',
            'https://app.allanic.me/excel/',
        ]);
        expect(sw.desenregistres).toHaveLength(2);
    });

    it('vide les caches et rapporte leurs noms', async () => {
        const sw = fauxServiceWorker([]);
        const cache = fauxCaches(['guacamole-v1', 'images']);

        const rapport = await nettoyerOrigin({ serviceWorker: sw.api, caches: cache.api });

        expect(rapport.caches).toEqual(['guacamole-v1', 'images']);
        expect(cache.supprimes).toEqual(['guacamole-v1', 'images']);
    });

    it('rapporte un origin déjà propre sans échouer', async () => {
        const rapport = await nettoyerOrigin({
            serviceWorker: fauxServiceWorker([]).api,
            caches: fauxCaches([]).api,
        });

        expect(rapport).toEqual({ serviceWorkers: [], caches: [], supporte: true });
    });

    it("signale l'absence d'API au lieu de lever", async () => {
        const rapport = await nettoyerOrigin({ serviceWorker: undefined, caches: undefined });

        expect(rapport.supporte).toBe(false);
        expect(rapport.serviceWorkers).toEqual([]);
    });
});
