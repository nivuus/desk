import { describe, expect, it } from 'vitest';
import tokensCss from '../design/tokens/couleurs.css?raw';
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
        if (r === undefined) throw new Error(`no fake answer for ${url}`);
        return {
            ok: r.ok ?? true,
            status: r.status ?? 200,
            json: r.json ?? (async () => ({})),
            arrayBuffer: r.arrayBuffer ?? (async () => new ArrayBuffer(0)),
        };
    };
    return { fetch, appels };
}

/// 🔴 AUCUNE COULEUR N'EST ÉCRITE DANS CE FICHIER, ET §7.2 L'EXIGE : son
/// balayage couvre les `.ts` autant que les `.css`, et il a relevé deux
/// littérales que ce test portait. **Élargir son exclusion aurait satisfait le
/// contrôle en le VIDANT** ; la valeur est donc LUE sur `tokens/couleurs.css`
/// (`tokens.css` avant l'extraction de la tâche 6, 25 août 2026), comme
/// dans `manifeste.test.ts` — il n'existe qu'une source de vérité pour une
/// couleur.
const ACCENT = (() => {
    const racine = tokensCss.slice(tokensCss.indexOf(':root'));
    const trouve = /--accent:\s*([^;]+);/.exec(racine);
    if (trouve === null) throw new Error('tokens/couleurs.css no longer declares --accent');
    return trouve[1].trim();
})();

/// L'URL SIGNÉE telle que la plateforme la frappe. ⚠️ ELLE EST ÉCRITE ICI EN
/// DUR, ET C'EST CORRECT : `client/` ne peut pas importer `plateforme/`, et ce
/// module ne la FABRIQUE pas — il la RELAIE. Ce qu'on éprouve est justement
/// qu'il la relaie sans y toucher.
const URL_SIGNEE = '/application/u-1/icone?e=abc123&v=vm-1&x=1787136780000&s=une-signature';

const APP: ApplicationListee = {
    id: 'u-1',
    nom: 'Bloc-notes',
    icone: 'abc123',
    icone_url: URL_SIGNEE,
    source_max: '256',
    accent: null,
    associations: [],
};

describe('listerApplications', () => {
    it('calls GET /applications?vm=… WITH the bearer, and returns the list', async () => {
        const { fetch, appels } = faux({
            'https://x/applications?vm=vm-1': {
                json: async () => ({
                    applications: [
                        {
                            id: 'u-1',
                            nom: 'Bloc-notes',
                            icone: 'abc123',
                            icone_url: URL_SIGNEE,
                            source_max: '256',
                            accent: ACCENT,
                            associations: ['.txt', '.log'],
                        },
                        { id: 'u-2', nom: 'Paint', icone: null, source_max: 'non-mesuree' },
                    ],
                }),
            },
        });
        const deps: DepsCatalogue = { base: 'https://x', jeton: 'J', fetch };
        const issue = await listerApplications('vm-1', deps);
        expect(issue.etat).toBe('ok');
        if (issue.etat !== 'ok') return;
        expect(issue.value.map((a) => a.nom)).toEqual(['Bloc-notes', 'Paint']);
        expect(issue.value[1].icone).toBeNull();
        // 🔴 L'URL SIGNÉE TRAVERSE, ET UNE ENTRÉE SANS ELLE REND `null` —
        //    jamais `undefined`, jamais une URL fabriquée ici. Le hub ne doit
        //    pas avoir à distinguer « pas d'icône » de « champ absent », et il
        //    ne doit surtout pas se croire capable d'en écrire une.
        expect(issue.value[0].icone_url).toBe(URL_SIGNEE);
        expect(issue.value[1].icone_url).toBeNull();
        expect(issue.value[0].accent).toBe(ACCENT);
        expect(issue.value[0].associations).toEqual(['.txt', '.log']);
        // ⚠️ UNE ENTRÉE SANS LES DEUX CHAMPS RETOMBE SUR DES VALEURS NEUTRES,
        //    et non sur `undefined` : le hub ne doit pas avoir à distinguer
        //    « aucune association » de « champ absent ».
        expect(issue.value[1].accent).toBeNull();
        expect(issue.value[1].associations).toEqual([]);
        // 🔴 L'EN-TÊTE EST LA MOITIÉ QUI COMPTE : toute la voie V1 repose sur
        //    le fait que la page lit AUTHENTIFIÉE ce que le navigateur ne
        //    saurait pas aller chercher lui-même.
        expect(appels[0].init?.headers).toEqual({ authorization: 'Bearer J' });
    });

    it("escapes the VM identifier in the request", async () => {
        const { fetch, appels } = faux({
            'https://x/applications?vm=a%2Fb%3Fc': { json: async () => ({ applications: [] }) },
        });
        const issue = await listerApplications('a/b?c', { base: 'https://x', jeton: 'J', fetch });
        expect(issue.etat).toBe('ok');
        expect(appels[0].url).toBe('https://x/applications?vm=a%2Fb%3Fc');
    });

    it('returns the service REASON on a refusal, never an exception', async () => {
        const { fetch } = faux({
            'https://x/applications?vm=vm-1': { ok: false, status: 404, json: async () => ({ refus: 'vm-inconnue' }) },
        });
        const issue = await listerApplications('vm-1', { base: 'https://x', jeton: 'J', fetch });
        expect(issue).toEqual({ etat: 'refus', refus: { source: 'service', statut: 404, motif: 'vm-inconnue' } });
    });

    it('returns the CODE alone when the refusal body is unreadable', async () => {
        const { fetch } = faux({
            'https://x/applications?vm=vm-1': {
                ok: false,
                status: 503,
                json: async () => {
                    throw new Error('not JSON');
                },
            },
        });
        const issue = await listerApplications('vm-1', { base: 'https://x', jeton: 'J', fetch });
        expect(issue).toEqual({ etat: 'refus', refus: { source: 'service', statut: 503, motif: 'status 503' } });
    });

    it('refuses a 200 body that carries no `applications` array', async () => {
        const { fetch } = faux({ 'https://x/applications?vm=vm-1': { json: async () => ({ applications: 'non' }) } });
        const issue = await listerApplications('vm-1', { base: 'https://x', jeton: 'J', fetch });
        expect(issue.etat).toBe('refus');
        if (issue.etat !== 'refus') return;
        expect(issue.refus).toEqual({
            source: 'client',
            motif: 'reponse-illisible',
            detail: "'applications' is not an array",
        });
    });

    it("refuses an entry without `id` or `nom` rather than making one up", async () => {
        const { fetch } = faux({
            'https://x/applications?vm=vm-1': { json: async () => ({ applications: [{ nom: 'sans id' }] }) },
        });
        const issue = await listerApplications('vm-1', { base: 'https://x', jeton: 'J', fetch });
        expect(issue.etat).toBe('refus');
    });
});

describe('lireIcone', () => {
    it("🔴 follows the SIGNED URL, and sends NO header", async () => {
        // 🔴 C'EST LA PROPRIÉTÉ QUE LE LOT DU 30 AOÛT 2026 LIVRE : la même URL
        // se pose dans un `<img src>`, qui ne peut rien porter d'autre. Un
        // `authorization` envoyé quand même ferait vivre une seconde voie
        // d'autorisation que la plateforme a retirée.
        const octets = new Uint8Array([0x89, 0x50, 0x4e, 0x47]);
        const { fetch, appels } = faux({
            [`https://x${URL_SIGNEE}`]: {
                arrayBuffer: async () => octets.buffer.slice(0),
            },
        });
        const issue = await lireIcone(APP, { base: 'https://x', jeton: 'J', fetch });
        expect(issue.etat).toBe('ok');
        if (issue.etat !== 'ok') return;
        expect(Array.from(issue.value)).toEqual([0x89, 0x50, 0x4e, 0x47]);
        // 🔴 L'URL EST RELAYÉE TELLE QUELLE, jamais reconstruite : le client
        // n'a pas la clé, et une URL qu'il fabriquerait serait refusée.
        expect(appels[0].url).toBe(`https://x${URL_SIGNEE}`);
        expect(appels[0].init?.headers).toBeUndefined();
    });

    it("refuses without calling when the application has no icon", async () => {
        const { fetch, appels } = faux({});
        const issue = await lireIcone(
            { ...APP, icone: null, icone_url: null },
            { base: 'https://x', jeton: 'J', fetch },
        );
        expect(issue.etat).toBe('refus');
        // 🔴 ZÉRO APPEL : le contrôle qui vaut n'est pas le refus, c'est
        //    l'ABSENCE de requête. Un refus rendu APRÈS un aller-retour
        //    inutile passerait la première assertion et pas celle-ci.
        expect(appels).toHaveLength(0);
    });
});

describe('lancerApplication', () => {
    it('POSTs to /application/:id/lancer with the bearer', async () => {
        const { fetch, appels } = faux({ 'https://x/application/u-1/lancer': {} });
        const issue = await lancerApplication('u-1', { base: 'https://x', jeton: 'J', fetch });
        expect(issue.etat).toBe('ok');
        expect(appels[0].init?.method).toBe('POST');
        expect(appels[0].init?.headers).toEqual({ authorization: 'Bearer J' });
    });

    it('returns the service reason on a refusal', async () => {
        const { fetch } = faux({
            'https://x/application/u-1/lancer': { ok: false, status: 503, json: async () => ({ refus: 'vm-injoignable' }) },
        });
        const issue = await lancerApplication('u-1', { base: 'https://x', jeton: 'J', fetch });
        expect(issue).toEqual({ etat: 'refus', refus: { source: 'service', statut: 503, motif: 'vm-injoignable' } });
    });
});

describe('shape check', () => {
    it('the REAL fetch satisfies the Fetch type', () => {
        // Aucune requête n'est émise : c'est une assertion de TYPAGE, jouée à
        // la compilation. Le même geste que `televersement.test.ts`.
        const _: Fetch = globalThis.fetch as unknown as Fetch;
        expect(typeof _).toBe('function');
    });
});

describe('listerVms', () => {
    it('calls GET /vm with the bearer and returns the list', async () => {
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
        expect(issue.value).toEqual([{ id: 'vm-1', nom: 'poste', etat: 'prete', prefixe: 'AAA' }]);
        expect(appels[0].init?.headers).toEqual({ authorization: 'Bearer J' });
    });

    it('returns an EMPTY list rather than a refusal when no VM is assigned', async () => {
        // ⚠️ AUCUNE VM N'EST UN ÉTAT NORMAL, pas une panne : `routes-vm.ts` rend
        //    200 avec un tableau vide. Le confondre avec un refus ferait dire au
        //    hub qu'il est cassé là où il n'a rien à montrer.
        const { fetch } = faux({ 'https://x/vm': { json: async () => ({ vms: [] }) } });
        const issue = await listerVms({ base: 'https://x', jeton: 'J', fetch });
        expect(issue).toEqual({ etat: 'ok', value: [] });
    });

    it('returns the service reason on a refusal', async () => {
        const { fetch } = faux({
            'https://x/vm': { ok: false, status: 401, json: async () => ({ refus: 'jeton-expire' }) },
        });
        const issue = await listerVms({ base: 'https://x', jeton: 'J', fetch });
        expect(issue).toEqual({ etat: 'refus', refus: { source: 'service', statut: 401, motif: 'jeton-expire' } });
    });

    it('refuses a 200 body without a `vms` array', async () => {
        const { fetch } = faux({ 'https://x/vm': { json: async () => ({}) } });
        const issue = await listerVms({ base: 'https://x', jeton: 'J', fetch });
        expect(issue.etat).toBe('refus');
    });
});

describe('the two fields of slice F', () => {
    it("DISCARDS an `associations` entry that is not a string", async () => {
        // ⚠️ Une entrée non textuelle atterrirait dans un `accept` de
        //    manifeste, où le navigateur la rejetterait sans qu'on sache d'où
        //    elle vient.
        const { fetch } = faux({
            'https://x/applications?vm=v': {
                json: async () => ({
                    applications: [
                        { id: 'u', nom: 'N', icone: null, source_max: 'non-mesuree', associations: ['.a', 3, null, '.b'] },
                    ],
                }),
            },
        });
        const issue = await listerApplications('v', { base: 'https://x', jeton: 'J', fetch });
        expect(issue.etat).toBe('ok');
        if (issue.etat !== 'ok') return;
        expect(issue.value[0].associations).toEqual(['.a', '.b']);
    });

    it("falls back to `[]` when `associations` is not an array", async () => {
        const { fetch } = faux({
            'https://x/applications?vm=v': {
                json: async () => ({
                    applications: [
                        { id: 'u', nom: 'N', icone: null, source_max: 'non-mesuree', associations: 'non' },
                    ],
                }),
            },
        });
        const issue = await listerApplications('v', { base: 'https://x', jeton: 'J', fetch });
        expect(issue.etat).toBe('ok');
        if (issue.etat !== 'ok') return;
        expect(issue.value[0].associations).toEqual([]);
    });
});
