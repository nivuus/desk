import { describe, expect, it } from 'vitest';
import { nettoyerOrigin } from '../public/lib/reset-origin.js';

// Fake `navigator.serviceWorker`: two registrations, one of which from the old app.
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
    it('unregisters all the service workers and reports their scopes', async () => {
        const sw = fauxServiceWorker(['https://app.allanic.me/', 'https://app.allanic.me/excel/']);
        const cache = fauxCaches([]);

        const rapport = await nettoyerOrigin({ serviceWorker: sw.api, caches: cache.api });

        expect(rapport.serviceWorkers).toEqual([
            'https://app.allanic.me/',
            'https://app.allanic.me/excel/',
        ]);
        expect(sw.desenregistres).toHaveLength(2);
    });

    it('empties the caches and reports their names', async () => {
        const sw = fauxServiceWorker([]);
        const cache = fauxCaches(['guacamole-v1', 'images']);

        const rapport = await nettoyerOrigin({ serviceWorker: sw.api, caches: cache.api });

        expect(rapport.caches).toEqual(['guacamole-v1', 'images']);
        expect(cache.supprimes).toEqual(['guacamole-v1', 'images']);
    });

    it('reports an already clean origin without failing', async () => {
        const rapport = await nettoyerOrigin({
            serviceWorker: fauxServiceWorker([]).api,
            caches: fauxCaches([]).api,
        });

        expect(rapport).toEqual({ serviceWorkers: [], caches: [], supporte: true });
    });

    it("reports the absence of the API instead of throwing", async () => {
        const rapport = await nettoyerOrigin({ serviceWorker: undefined, caches: undefined });

        expect(rapport.supporte).toBe(false);
        expect(rapport.serviceWorkers).toEqual([]);
    });
});
