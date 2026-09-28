// Origin hygiene before the spike. Pure ESM module: imported as is by
// reset.html in the browser, and by Vitest under Node — hence the injection of the
// APIs rather than direct access to `navigator`.
//
// Reason to exist: the old application registers a service worker on this same
// origin (web/index.js:418). An active service worker would intercept the requests
// of the spike and make every verdict uninterpretable.

/**
 * @param {{ serviceWorker?: { getRegistrations(): Promise<Array<{scope: string, unregister(): Promise<boolean>}>> },
 *           caches?: { keys(): Promise<string[]>, delete(nom: string): Promise<boolean> } }} api
 * @returns {Promise<{ serviceWorkers: string[], caches: string[], supporte: boolean }>}
 */
export async function nettoyerOrigin(api) {
    const rapport = { serviceWorkers: [], caches: [], supporte: false };

    if (!api?.serviceWorker && !api?.caches) return rapport;
    rapport.supporte = true;

    if (api.serviceWorker) {
        const enregistrements = await api.serviceWorker.getRegistrations();
        for (const enregistrement of enregistrements) {
            // The scope is recorded BEFORE the removal: it is what will tell whether
            // the old application was really there.
            rapport.serviceWorkers.push(enregistrement.scope);
            await enregistrement.unregister();
        }
    }

    if (api.caches) {
        const noms = await api.caches.keys();
        for (const nom of noms) {
            rapport.caches.push(nom);
            await api.caches.delete(nom);
        }
    }

    return rapport;
}
