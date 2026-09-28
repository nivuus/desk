// The two icon routes, tested THROUGH a real HTTP server.
//
// 🔴 THE PATH CHECK COMPARES THE BODY, NEVER THE STATUS ALONE, and it is
// not a precaution: G1 MEASURED that a `startsWith('/application')`
// left its tests GREEN — the route ate the whole family and returned ITS
// OWN typed 404, indistinguishable from the generic 404 as long as one only read the
// status (commit `a97f902`).

import { createHash } from 'node:crypto';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, describe, expect, it } from 'vitest';
import { ouvrirMagasin, type Magasin } from '../apps/icones';
import { MOTEUR } from '../base/harnais';
import {
    attribuer,
    withIt,
    demonter,
    jetonDe,
    monterRoute,
    MS,
    poserApp,
    poserVm,
    SECRET,
    type Montage,
} from './routes-harnais';
import { signerUrlIcone } from '../apps/url-icone';
import { ICONE_MAX_OCTETS, servirIcone } from './routes-icone';

const PNG = Buffer.from('\x89PNG\r\n\x1a\n-des-octets-d-icone');
const EMPREINTE = createHash('sha256').update(PNG).digest('hex');
const AUTRE = createHash('sha256').update('autre').digest('hex');

let montage: Montage | undefined;
let magasin: Magasin;
let racines: string[] = [];

afterEach(async () => {
    await demonter(montage);
    montage = undefined;
    for (const r of racines) rmSync(r, { recursive: true, force: true });
    racines = [];
});

async function servir(nom: string): Promise<string> {
    const r = mkdtempSync(join(tmpdir(), 'g2-routes-icone-'));
    racines.push(r);
    magasin = ouvrirMagasin(join(r, 'icones'), () => {});
    montage = await monterRoute(nom, (req, rep, base) =>
        servirIcone(req, rep, {
            base,
            secretJeton: SECRET,
            magasin,
            maintenant: () => MS,
        }),
    );
    return montage.url;
}

describe(`icon routes, engine=${MOTEUR}`, () => {
    it('🔴 eats ONLY its two paths — the body of the 404 is compared', async () => {
        // 🔴 IF THIS MUTATION SURVIVES, THE TEST IS WRONG, NOT THE MUTATION.
        // Four declined paths, two of which existed in no
        // first draft.
        const url = await servir('icone-chemins');
        for (const chemin of [
            '/iconedetournee',
            '/icone/',
            '/icone/a/b',
            '/application/x/icone/y',
            '/applications',
            '/application/x/lancer',
        ]) {
            const r = await fetch(`${url}${chemin}`);
            expect(r.status, chemin).toBe(404);
            // The GENERIC 404 of `serveur.ts`, word for word — not a typed 404
            // this router would have returned.
            expect(await r.text(), chemin).toBe('not found\n');
        }
    });

    it('serves `/application/:id/icone` WITHOUT `?e=` with a typed 400', async () => {
        const url = await servir('icone-sans-e');
        const jeton = jetonDe('u1');
        const r = await fetch(`${url}/application/x/icone`, { headers: withIt(jeton) });
        // The path IS recognised — hence not the generic 404.
        expect(r.status).toBe(400);
        expect(await r.json()).toEqual({ refus: 'empreinte-absente' });
    });

    describe('PUT /icone/:sha256 — the agent drops off', () => {
        it('accepts an AGENT token and returns 204', async () => {
            const url = await servir('icone-put');
            const r = await fetch(`${url}/icone/${EMPREINTE}`, {
                method: 'PUT',
                headers: withIt(jetonDe('vm-1', 'agent')),
                body: PNG,
            });
            expect(r.status).toBe(204);
            expect(magasin.lire(EMPREINTE)).toEqual(PNG);
        });

        it('🔴 REFUSES a USER token with 403, never 401', async () => {
            // 🔴 SYMMETRICAL TO `jeton-agent` in `porteur.ts`, and argued the
            // same way: the token is VALID, it simply is not
            // an agent's. A 401 would invite reconnecting for nothing.
            const url = await servir('icone-put-humain');
            const r = await fetch(`${url}/icone/${EMPREINTE}`, {
                method: 'PUT',
                headers: withIt(jetonDe('u1')),
                body: PNG,
            });
            expect(r.status).toBe(403);
            expect(await r.json()).toEqual({ refus: 'jeton-utilisateur' });
            expect(magasin.lire(EMPREINTE)).toBeUndefined();
        });

        it('refuses without a token, and with an unreadable token', async () => {
            const url = await servir('icone-put-nu');
            const sans = await fetch(`${url}/icone/${EMPREINTE}`, { method: 'PUT', body: PNG });
            expect(sans.status).toBe(401);
            expect(await sans.json()).toEqual({ refus: 'jeton-absent' });
            const faux = await fetch(`${url}/icone/${EMPREINTE}`, {
                method: 'PUT',
                headers: { authorization: 'Bearer not-a-token' },
                body: PNG,
            });
            expect(faux.status).toBe(401);
            expect(await faux.json()).toEqual({ refus: 'jeton-invalide' });
        });

        it('🔴 RECOMPUTES the fingerprint and REFUSES if it lies', async () => {
            // 🔴 WITHOUT THIS RECOMPUTATION, CONTENT ADDRESSING WOULD NOT BE ONE,
            // and `Cache-Control: immutable` would make the poisoning PERMANENT
            // in the caches. Tested at the store, REPLAYED HERE end to end.
            const url = await servir('icone-put-menteur');
            const r = await fetch(`${url}/icone/${AUTRE}`, {
                method: 'PUT',
                headers: withIt(jetonDe('vm-1', 'agent')),
                body: PNG,
            });
            expect(r.status).toBe(400);
            expect(await r.json()).toEqual({ refus: 'empreinte' });
            expect(magasin.lire(AUTRE)).toBeUndefined();
        });

        it('🔴 REFUSES a fingerprint that could escape the store', async () => {
            const url = await servir('icone-put-chemin');
            // `..%2f..%2fx`: the decoded path would go out of the directory.
            const r = await fetch(`${url}/icone/..%2f..%2fx`, {
                method: 'PUT',
                headers: withIt(jetonDe('vm-1', 'agent')),
                body: PNG,
            });
            expect(r.status).toBe(400);
            expect(await r.json()).toEqual({ refus: 'empreinte-invalide' });
        });

        it('refuses a body beyond the ceiling, with a TYPED 413', async () => {
            const url = await servir('icone-put-gros');
            const gros = Buffer.alloc(ICONE_MAX_OCTETS + 1, 7);
            const r = await fetch(`${url}/icone/${createHash('sha256').update(gros).digest('hex')}`, {
                method: 'PUT',
                headers: withIt(jetonDe('vm-1', 'agent')),
                body: gros,
            });
            expect(r.status).toBe(413);
            expect(await r.json()).toEqual({ refus: 'taille' });
        });

        it('refuses a method other than PUT with 405', async () => {
            const url = await servir('icone-put-methode');
            const r = await fetch(`${url}/icone/${EMPREINTE}`, { headers: withIt(jetonDe('u1')) });
            expect(r.status).toBe(405);
            expect(await r.json()).toEqual({ refus: 'methode' });
        });
    });

    describe('GET /application/:id/icone — the SIGNED URL, without a header', () => {
        async function poser(nom: string): Promise<{ url: string; id: string; u: string }> {
            const url = await servir(nom);
            const base = montage!.base;
            await poserVm(base, 'vm-1');
            const u = await attribuer(base, 'vm-1', 'ada@exemple.test');
            const id = await poserApp(base, 'vm-1', 'Bloc-notes', 'a1b2', EMPREINTE, {
                pixels: 256,
            });
            magasin.write(EMPREINTE, PNG);
            return { url, id, u };
        }

        /// The signed URL as `routes-applications.ts` mints it — the SAME
        /// function as the product, never a `fetch` reimplementation that
        /// would only test itself.
        function signee(id: string, vm = 'vm-1', empreinte = EMPREINTE, t = MS): string {
            return signerUrlIcone(id, vm, empreinte, SECRET, t);
        }

        it('🔴 serves the PNG WITHOUT ANY HEADER — that is the whole point of the batch', async () => {
            // 🔴 THE CHECK THAT MATTERS IS THE ABSENCE OF `Authorization`, NOT THE
            // 200: an `<img src>` can carry nothing other than its URL, and
            // it is exactly what this request reproduces.
            const { url, id } = await poser('icone-get-signee');
            const r = await fetch(`${url}${signee(id)}`);
            expect(r.status).toBe(200);
            expect(r.headers.get('content-type')).toBe('image/png');
            expect(r.headers.get('cache-control')).toBe('private, max-age=31536000, immutable');
            // 🔴 THE EXCEPTION DOES NOT WIDEN: `cache-control` is overridden,
            // `nosniff` IS NOT. It is the only one of the two security headers
            // that is a guard — without it, a browser could
            // guess a type other than `image/png` on bytes a peer
            // dropped.
            expect(r.headers.get('x-content-type-options')).toBe('nosniff');
            expect(Buffer.from(await r.arrayBuffer())).toEqual(PNG);
        });

        it('🔴 REFUSES a FORGED signature, with a typed 403', async () => {
            const { url, id } = await poser('icone-get-faussaire');
            const chemin = signee(id);
            // A single character of the signature changes, and it really changes.
            const p = new URL(chemin, 'http://interne');
            const vraie = p.searchParams.get('s')!;
            p.searchParams.set('s', (vraie[0] === 'a' ? 'b' : 'a') + vraie.slice(1));
            const r = await fetch(`${url}${p.pathname}${p.search}`);
            expect(r.status).toBe(403);
            expect(await r.json()).toEqual({ refus: 'signature-invalide' });
        });

        it('🔴 REFUSES an EXPIRED URL, with a typed 403', async () => {
            // 🔴 THE EXPIRY IS JUDGED ON THE SERVER SIDE, against `deps.maintenant`
            // — never by trusting the client's `x` field. The URL is minted
            // for an instant old enough to be dead at `MS`.
            const { url, id } = await poser('icone-get-expiree');
            const r = await fetch(`${url}${signee(id, 'vm-1', EMPREINTE, MS - 3_600_000)}`);
            expect(r.status).toBe(403);
            expect(await r.json()).toEqual({ refus: 'url-expiree' });
            // The WITNESS, without which the refusal would prove nothing: the SAME
            // URL minted at the current instant is served.
            expect((await fetch(`${url}${signee(id)}`)).status).toBe(200);
        });

        it('🔴 a URL signed for one APPLICATION is not valid for ANOTHER', async () => {
            // 🔴 SECURITY RED NO. 3, END TO END: two applications
            // of the SAME VM, of the SAME user, with the SAME icon — hence
            // nothing but the identifier separates them.
            const { url, id } = await poser('icone-get-permutee');
            const autre = await poserApp(montage!.base, 'vm-1', 'Autre', 'c3d4', EMPREINTE, {
                pixels: 256,
            });
            expect(autre).not.toBe(id);
            // The URL of `id`, served on the path of `autre`.
            const p = new URL(signee(id), 'http://interne');
            const r = await fetch(`${url}/application/${autre}/icone${p.search}`);
            expect(r.status).toBe(403);
            expect(await r.json()).toEqual({ refus: 'signature-invalide' });
            // BOTH witnesses: each on ITS own URL is served.
            expect((await fetch(`${url}${signee(id)}`)).status).toBe(200);
            expect((await fetch(`${url}${signee(autre)}`)).status).toBe(200);
        });

        it('🔴 a URL signed for ANOTHER VM returns 404 vm-inconnue', async () => {
            // The signature is GOOD — it is minted with `vm-2` — but the
            // database says the application lives on `vm-1`. It is the check that
            // prevents covering the VM from being an ornament.
            const { url, id } = await poser('icone-get-vm-permutee');
            const r = await fetch(`${url}${signee(id, 'vm-2')}`);
            expect(r.status).toBe(404);
            expect(await r.json()).toEqual({ refus: 'vm-inconnue' });
        });

        it('🔴 a STALE `?e=` returns 404, and that is what makes `immutable` HONEST', async () => {
            // 🔴 WITHOUT THIS REFUSAL, an old URL would serve the CURRENT icon
            // under an immutable header — the cache would be poisoned FOR A YEAR
            // with an image that is not the one the URL names.
            const { url, id } = await poser('icone-get-perime');
            magasin.write(AUTRE, Buffer.from('autre'));
            const r = await fetch(`${url}${signee(id, 'vm-1', AUTRE)}`);
            expect(r.status).toBe(404);
            expect(await r.json()).toEqual({ refus: 'icone-inconnue' });
        });

        it('a bearer token is NO LONGER enough — the old path is REMOVED', async () => {
            // 🔴 THE OWNER RULED: A SINGLE PATH. Two surfaces
            // for the same resource are two authorisation guards that
            // diverge silently. This test pins the withdrawal — without it, one
            // could reopen the door without anything saying so.
            const { url, id, u } = await poser('icone-get-porteur-retire');
            const r = await fetch(`${url}/application/${id}/icone?e=${EMPREINTE}`, {
                headers: withIt(jetonDe(u)),
            });
            expect(r.status).toBe(400);
            expect(await r.json()).toEqual({ refus: 'signature-absente' });
        });

        it('without any `?e=`: typed 400, distinct from the signature refusal', async () => {
            const url = await servir('icone-sans-e-signee');
            const r = await fetch(`${url}/application/x/icone`);
            expect(r.status).toBe(400);
            expect(await r.json()).toEqual({ refus: 'empreinte-absente' });
        });

        it('an UNKNOWN application returns 404, even under a valid signature', async () => {
            const url = await servir('icone-get-app-inconnue');
            const r = await fetch(`${url}${signee('not-an-id')}`);
            expect(r.status).toBe(404);
            expect(await r.json()).toEqual({ refus: 'application-inconnue' });
        });

        it('an application WITHOUT an icon returns 404, not an empty image', async () => {
            const url = await servir('icone-get-sans');
            const base = montage!.base;
            await poserVm(base, 'vm-1');
            await attribuer(base, 'vm-1', 'ada@exemple.test');
            const id = await poserApp(base, 'vm-1', 'Sans', 'c3d4');
            const r = await fetch(`${url}${signee(id)}`);
            expect(r.status).toBe(404);
            expect(await r.json()).toEqual({ refus: 'icone-inconnue' });
        });

        it('the database knows the fingerprint, the DISK does not have it: 404, and this heals by itself', async () => {
            const { url, id } = await poser('icone-get-disque-vide');
            rmSync(join(magasin.repertoire, EMPREINTE));
            const r = await fetch(`${url}${signee(id)}`);
            expect(r.status).toBe(404);
            // And the inventory asks for it again: that is the self-rebuild.
            expect(magasin.manquantes([EMPREINTE])).toEqual([EMPREINTE]);
        });

        it('serves the OPTIONS preflight response', async () => {
            // ⚠️ BOTH ROUTES REQUIRE `Authorization`, SO THE REQUEST IS
            // NON-SIMPLE: the browser first sends an `OPTIONS` and
            // GIVES UP without ever sending the real request if the response does not
            // suit it. It is the exact defect P4's browser corroboration
            // found, and which no Node test saw.
            const url = await servir('icone-options');
            for (const chemin of [`/icone/${EMPREINTE}`, '/application/x/icone']) {
                const r = await fetch(`${url}${chemin}`, { method: 'OPTIONS' });
                expect(r.status, chemin).toBe(204);
            }
        });
    });
});
