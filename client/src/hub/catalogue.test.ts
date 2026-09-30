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

/// A fake `fetch` that MEMORISES what it is asked — that is what makes it possible
/// to exercise the URL and the header, and not only the return value.
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

/// 🔴 NO COLOUR IS WRITTEN IN THIS FILE, AND §7.2 REQUIRES IT: its
/// scan covers the `.ts` as much as the `.css`, and it reported two
/// literals this test carried. **Widening its exclusion would have satisfied the
/// check by EMPTYING it**; the value is therefore READ from `tokens/couleurs.css`
/// (`tokens.css` before the extraction of task 6, August 25th, 2026), as
/// in `manifeste.test.ts` — there is only one source of truth for a
/// colour.
const ACCENT = (() => {
    const racine = tokensCss.slice(tokensCss.indexOf(':root'));
    const trouve = /--accent:\s*([^;]+);/.exec(racine);
    if (trouve === null) throw new Error('tokens/couleurs.css no longer declares --accent');
    return trouve[1].trim();
})();

/// The SIGNED URL as the platform mints it. ⚠️ IT IS WRITTEN HERE
/// HARDCODED, AND THAT IS CORRECT: `client/` cannot import `plateforme/`, and this
/// module does not MAKE it — it RELAYS it. What is exercised is precisely
/// that it relays it without touching it.
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
        // 🔴 THE SIGNED URL GOES THROUGH, AND AN ENTRY WITHOUT IT RETURNS `null` —
        //    never `undefined`, never a URL made up here. The hub must not
        //    have to tell "no icon" from "absent field", and above all it
        //    must not believe itself able to write one.
        expect(issue.value[0].icone_url).toBe(URL_SIGNEE);
        expect(issue.value[1].icone_url).toBeNull();
        expect(issue.value[0].accent).toBe(ACCENT);
        expect(issue.value[0].associations).toEqual(['.txt', '.log']);
        // ⚠️ AN ENTRY WITHOUT BOTH FIELDS FALLS BACK TO NEUTRAL VALUES,
        //    and not to `undefined`: the hub must not have to tell
        //    "no association" from "absent field".
        expect(issue.value[1].accent).toBeNull();
        expect(issue.value[1].associations).toEqual([]);
        // 🔴 THE HEADER IS THE HALF THAT MATTERS: the whole V1 route rests on
        //    the fact that the page reads AUTHENTICATED what the browser could not
        //    fetch by itself.
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
        // 🔴 IT IS THE PROPERTY THE BATCH OF AUGUST 30TH, 2026 DELIVERS: the same URL
        // goes into an `<img src>`, which can carry nothing else. An
        // `authorization` sent anyway would keep alive a second authorisation
        // route the platform removed.
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
        // 🔴 THE URL IS RELAYED AS IS, never rebuilt: the client
        // does not have the key, and a URL it made up would be refused.
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
        // 🔴 ZERO CALLS: the check that counts is not the refusal, it is the
        //    ABSENCE of a request. A refusal returned AFTER a useless
        //    round trip would pass the first assertion and not this one.
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

    it('returns `etat` when the service carries it (503 starting)', async () => {
        const { fetch } = faux({
            'https://x/application/u-1/lancer': {
                ok: false,
                status: 503,
                json: async () => ({ refus: 'agent-injoignable', etat: 'demarrage' }),
            },
        });
        const issue = await lancerApplication('u-1', { base: 'https://x', jeton: 'J', fetch });
        expect(issue).toEqual({
            etat: 'refus',
            refus: { source: 'service', statut: 503, motif: 'agent-injoignable', etat: 'demarrage' },
        });
    });

    it('a refusal without `etat` does not carry the field', async () => {
        const { fetch } = faux({
            'https://x/application/u-1/lancer': { ok: false, status: 504, json: async () => ({ refus: 'delai' }) },
        });
        const issue = await lancerApplication('u-1', { base: 'https://x', jeton: 'J', fetch });
        expect(issue).toEqual({ etat: 'refus', refus: { source: 'service', statut: 504, motif: 'delai' } });
    });
});

describe('shape check', () => {
    it('the REAL fetch satisfies the Fetch type', () => {
        // No request is emitted: it is a TYPING assertion, played at
        // compile time. The same gesture as `televersement.test.ts`.
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
        // ⚠️ NO VM IS A NORMAL STATE, not a failure: `routes-vm.ts` returns
        //    200 with an empty array. Confusing it with a refusal would make the
        //    hub say it is broken where it has nothing to show.
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
        // ⚠️ A non-text entry would land in a manifest `accept`,
        //    where the browser would reject it without anyone knowing where
        //    it came from.
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
