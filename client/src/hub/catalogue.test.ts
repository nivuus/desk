import { describe, expect, it } from 'vitest';
import {
    lancerApplication,
    listerVms,
    lireIcone,
    listerApplications,
    type ApplicationListee,
    type DepsCatalogue,
    type Fetch,
    type InitHttp,
    type ReponseHttp,
} from './catalogue';

/// Un `fetch` factice qui MÉMORISE ce qu'on lui demande — c'est ce qui permet
/// d'éprouver l'URL et l'en-tête, et pas seulement le retour.
function faux(reponses: Record<string, Partial<ReponseHttp>>): {
    fetch: Fetch;
    appels: { url: string; init?: InitHttp }[];
} {
    const appels: { url: string; init?: InitHttp }[] = [];
    const fetch: Fetch = async (url, init) => {
        appels.push({ url, init });
        const r = reponses[url];
        if (r === undefined) throw new Error(`aucune réponse factice pour ${url}`);
        return {
            ok: r.ok ?? true,
            status: r.status ?? 200,
            json: r.json ?? (async () => ({})),
            arrayBuffer: r.arrayBuffer ?? (async () => new ArrayBuffer(0)),
        };
    };
    return { fetch, appels };
}

const APP: ApplicationListee = { id: 'u-1', nom: 'Bloc-notes', icone: 'abc123', source_max: '256' };

describe('listerApplications', () => {
    it('appelle GET /applications?vm=… AVEC le porteur, et rend la liste', async () => {
        const { fetch, appels } = faux({
            'https://x/applications?vm=vm-1': {
                json: async () => ({
                    applications: [
                        { id: 'u-1', nom: 'Bloc-notes', icone: 'abc123', source_max: '256' },
                        { id: 'u-2', nom: 'Paint', icone: null, source_max: 'non-mesuree' },
                    ],
                }),
            },
        });
        const deps: DepsCatalogue = { base: 'https://x', jeton: 'J', fetch };
        const issue = await listerApplications('vm-1', deps);
        expect(issue.etat).toBe('ok');
        if (issue.etat !== 'ok') return;
        expect(issue.valeur.map((a) => a.nom)).toEqual(['Bloc-notes', 'Paint']);
        expect(issue.valeur[1].icone).toBeNull();
        // 🔴 L'EN-TÊTE EST LA MOITIÉ QUI COMPTE : toute la voie V1 repose sur
        //    le fait que la page lit AUTHENTIFIÉE ce que le navigateur ne
        //    saurait pas aller chercher lui-même.
        expect(appels[0].init?.headers).toEqual({ authorization: 'Bearer J' });
    });

    it("échappe l'identifiant de VM dans la requête", async () => {
        const { fetch, appels } = faux({
            'https://x/applications?vm=a%2Fb%3Fc': { json: async () => ({ applications: [] }) },
        });
        const issue = await listerApplications('a/b?c', { base: 'https://x', jeton: 'J', fetch });
        expect(issue.etat).toBe('ok');
        expect(appels[0].url).toBe('https://x/applications?vm=a%2Fb%3Fc');
    });

    it('rend le MOTIF du service sur un refus, jamais une exception', async () => {
        const { fetch } = faux({
            'https://x/applications?vm=vm-1': { ok: false, status: 404, json: async () => ({ refus: 'vm-inconnue' }) },
        });
        const issue = await listerApplications('vm-1', { base: 'https://x', jeton: 'J', fetch });
        expect(issue).toEqual({ etat: 'refus', refus: { source: 'service', statut: 404, motif: 'vm-inconnue' } });
    });

    it('rend le CODE seul quand le corps du refus est illisible', async () => {
        const { fetch } = faux({
            'https://x/applications?vm=vm-1': {
                ok: false,
                status: 503,
                json: async () => {
                    throw new Error('pas du JSON');
                },
            },
        });
        const issue = await listerApplications('vm-1', { base: 'https://x', jeton: 'J', fetch });
        expect(issue).toEqual({ etat: 'refus', refus: { source: 'service', statut: 503, motif: 'statut 503' } });
    });

    it('refuse un corps 200 qui ne porte pas de tableau `applications`', async () => {
        const { fetch } = faux({ 'https://x/applications?vm=vm-1': { json: async () => ({ applications: 'non' }) } });
        const issue = await listerApplications('vm-1', { base: 'https://x', jeton: 'J', fetch });
        expect(issue.etat).toBe('refus');
        if (issue.etat !== 'refus') return;
        expect(issue.refus).toEqual({
            source: 'client',
            motif: 'reponse-illisible',
            detail: "'applications' n'est pas un tableau",
        });
    });

    it("refuse une entrée sans `id` ni `nom` plutôt que d'en fabriquer", async () => {
        const { fetch } = faux({
            'https://x/applications?vm=vm-1': { json: async () => ({ applications: [{ nom: 'sans id' }] }) },
        });
        const issue = await listerApplications('vm-1', { base: 'https://x', jeton: 'J', fetch });
        expect(issue.etat).toBe('refus');
    });
});

describe('lireIcone', () => {
    it("appelle la route d'icône AVEC l'empreinte et le porteur", async () => {
        const octets = new Uint8Array([0x89, 0x50, 0x4e, 0x47]);
        const { fetch, appels } = faux({
            'https://x/application/u-1/icone?e=abc123': {
                arrayBuffer: async () => octets.buffer.slice(0),
            },
        });
        const issue = await lireIcone(APP, { base: 'https://x', jeton: 'J', fetch });
        expect(issue.etat).toBe('ok');
        if (issue.etat !== 'ok') return;
        expect(Array.from(issue.valeur)).toEqual([0x89, 0x50, 0x4e, 0x47]);
        expect(appels[0].init?.headers).toEqual({ authorization: 'Bearer J' });
    });

    it("refuse sans appeler quand l'application n'a pas d'icône", async () => {
        const { fetch, appels } = faux({});
        const issue = await lireIcone({ ...APP, icone: null }, { base: 'https://x', jeton: 'J', fetch });
        expect(issue.etat).toBe('refus');
        // 🔴 ZÉRO APPEL : le contrôle qui vaut n'est pas le refus, c'est
        //    l'ABSENCE de requête. Un refus rendu APRÈS un aller-retour
        //    inutile passerait la première assertion et pas celle-ci.
        expect(appels).toHaveLength(0);
    });
});

describe('lancerApplication', () => {
    it('POSTe sur /application/:id/lancer avec le porteur', async () => {
        const { fetch, appels } = faux({ 'https://x/application/u-1/lancer': {} });
        const issue = await lancerApplication('u-1', { base: 'https://x', jeton: 'J', fetch });
        expect(issue.etat).toBe('ok');
        expect(appels[0].init?.method).toBe('POST');
        expect(appels[0].init?.headers).toEqual({ authorization: 'Bearer J' });
    });

    it('rend le motif du service sur un refus', async () => {
        const { fetch } = faux({
            'https://x/application/u-1/lancer': { ok: false, status: 503, json: async () => ({ refus: 'vm-injoignable' }) },
        });
        const issue = await lancerApplication('u-1', { base: 'https://x', jeton: 'J', fetch });
        expect(issue).toEqual({ etat: 'refus', refus: { source: 'service', statut: 503, motif: 'vm-injoignable' } });
    });
});

describe('contrôle de forme', () => {
    it('la VRAIE fetch satisfait le type Fetch', () => {
        // Aucune requête n'est émise : c'est une assertion de TYPAGE, jouée à
        // la compilation. Le même geste que `televersement.test.ts`.
        const _: Fetch = globalThis.fetch as unknown as Fetch;
        expect(typeof _).toBe('function');
    });
});

describe('listerVms', () => {
    it('appelle GET /vm avec le porteur et rend la liste', async () => {
        const { fetch, appels } = faux({
            'https://x/vm': {
                json: async () => ({
                    vms: [{ id: 'vm-1', nom: 'poste', etat: 'prete', prefixe: 'AAA', sessions_ouvertes: 0 }],
                }),
            },
        });
        const issue = await listerVms({ base: 'https://x', jeton: 'J', fetch });
        expect(issue.etat).toBe('ok');
        if (issue.etat !== 'ok') return;
        expect(issue.valeur).toEqual([{ id: 'vm-1', nom: 'poste', etat: 'prete', prefixe: 'AAA' }]);
        expect(appels[0].init?.headers).toEqual({ authorization: 'Bearer J' });
    });

    it('rend une liste VIDE plutôt qu\'un refus quand aucune VM n\'est attribuée', async () => {
        // ⚠️ AUCUNE VM N'EST UN ÉTAT NORMAL, pas une panne : `routes-vm.ts` rend
        //    200 avec un tableau vide. Le confondre avec un refus ferait dire au
        //    hub qu'il est cassé là où il n'a rien à montrer.
        const { fetch } = faux({ 'https://x/vm': { json: async () => ({ vms: [] }) } });
        const issue = await listerVms({ base: 'https://x', jeton: 'J', fetch });
        expect(issue).toEqual({ etat: 'ok', valeur: [] });
    });

    it('rend le motif du service sur un refus', async () => {
        const { fetch } = faux({
            'https://x/vm': { ok: false, status: 401, json: async () => ({ refus: 'jeton-expire' }) },
        });
        const issue = await listerVms({ base: 'https://x', jeton: 'J', fetch });
        expect(issue).toEqual({ etat: 'refus', refus: { source: 'service', statut: 401, motif: 'jeton-expire' } });
    });

    it('refuse un corps 200 sans tableau `vms`', async () => {
        const { fetch } = faux({ 'https://x/vm': { json: async () => ({}) } });
        const issue = await listerVms({ base: 'https://x', jeton: 'J', fetch });
        expect(issue.etat).toBe('refus');
    });
});
