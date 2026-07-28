// Hygiène de l'origin avant le spike. Module ESM pur : importé tel quel par
// reset.html dans le navigateur, et par Vitest sous Node — d'où l'injection des
// API plutôt qu'un accès direct à `navigator`.
//
// Raison d'être : l'ancienne application enregistre un service worker sur ce même
// origin (web/index.js:418). Un service worker actif intercepterait les requêtes
// du spike et rendrait tous les verdicts ininterprétables.

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
            // La portée est relevée AVANT la suppression : c'est elle qui dira si
            // l'ancienne application était bien là.
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
