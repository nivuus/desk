// The four upload routes, tested THROUGH a real HTTP server.
//
// 🔴 THE PATH CHECK COMPARES THE BODY, NEVER THE STATUS ALONE: G1
// MEASURED that a `startsWith('/application')` left SEVENTEEN tests GREEN — the
// route ate the whole family and returned ITS OWN typed 404, indistinguishable
// from the generic one as long as one only read the status. `monterRoute` reproduces the
// 404 of `serveur.ts` word for word: it is what makes a `false` observable.
//
// ⚠️ THE "DROP" FAMILY LIVES IN THE SIBLING,
// `routes-televersement-tranches.test.ts` — extraction, never compression, this
// file having reached 504 lines for a cap of 500. The fixtures of
// both come from `routes-televersement-harnais.ts`, never from a copy.

import { createHash } from 'node:crypto';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { MOTEUR } from '../base/harnais';
import { lireParId } from '../depot/televersement';
import { withIt, jetonDe, MS } from './routes-harnais';
import { ENTETES_SECURITE } from './entetes';
import {
    CHUNK_SIZE,
    TELEVERSEMENT_MAX_OCTETS,
    TELEVERSEMENTS_EN_COURS_MAX,
} from './routes-televersement';
import {
    CONTENU,
    declarerChez,
    deposer,
    INCONNU,
    magasin,
    monter,
    nettoyer,
    PAS,
    poser,
    sceller,
    SHA,
    TRANCHES,
    user,
} from './routes-televersement-harnais';

afterEach(async () => {
    await nettoyer();
    vi.restoreAllMocks();
});

describe(`upload routes, engine=${MOTEUR}`, () => {
    /* ── LE CHEMIN ───────────────────────────────────────────────────── */

    it('🔴 eats ONLY its four paths — the body of the 404 is compared', async () => {
        // 🔴 IF THIS MUTATION SURVIVES, THE TEST IS WRONG, NOT THE MUTATION —
        // it is the red G1 saw SURVIVE. We compare the body because a
        // prefix router can return ITS OWN typed 404.
        const { url } = await monter('tel-chemins');
        for (const chemin of [
            '/televersementautre',
            '/televersement/',
            '/televersement/x/tranche',
            '/televersement/x/tranche/1/z',
            '/televersement/x/autre',
            '/televersements',
            '/applications',
        ]) {
            const r = await fetch(`${url}${chemin}`, { headers: withIt(jetonDe('u1')) });
            expect(r.status, chemin).toBe(404);
            expect(await r.text(), chemin).toBe('not found\n');
        }
    });

    it('serves the preflight request and sets the security headers', async () => {
        const { url } = await monter('tel-options');
        const r = await fetch(`${url}/televersement`, { method: 'OPTIONS' });
        expect(r.status).toBe(204);
        for (const [nom, value] of Object.entries(ENTETES_SECURITE)) {
            expect(r.headers.get(nom), nom).toBe(value);
        }
        // On a REFUSAL too — it is where they count the most.
        const refus = await fetch(`${url}/televersement`, { method: 'POST' });
        expect(refus.status).toBe(401);
        expect(refus.headers.get('X-Content-Type-Options')).toBe('nosniff');
    });

    it('returns 405 on the wrong method, never 404', async () => {
        const { url } = await monter('tel-methode');
        const jeton = jetonDe('u1');
        const cas: [string, string][] = [
            ['/televersement', 'GET'],
            ['/televersement/x', 'POST'],
            ['/televersement/x/sceller', 'GET'],
            ['/televersement/x/tranche/0', 'POST'],
        ];
        for (const [chemin, methode] of cas) {
            const r = await fetch(`${url}${chemin}`, { method: methode, headers: withIt(jeton) });
            expect(r.status, chemin).toBe(405);
            expect(await r.json(), chemin).toEqual({ refus: 'methode' });
        }
    });

    it('requires a token, and refuses an agent one', async () => {
        const { url } = await monter('tel-porteur');
        const sans = await fetch(`${url}/televersement`, { method: 'POST' });
        expect(sans.status).toBe(401);
        expect(await sans.json()).toEqual({ refus: 'jeton-absent' });

        const agent = await fetch(`${url}/televersement`, {
            method: 'POST',
            headers: withIt(jetonDe('a1', 'agent')),
        });
        // 403 and not 401: the token is VALID, it is not a human's.
        expect(agent.status).toBe(403);
        expect(await agent.json()).toEqual({ refus: 'jeton-agent' });
    });

    /* ── ① DECLARE ──────────────────────────────────────────────────── */

    it('creates, and returns the step and an EMPTY chunk list', async () => {
        const { url, base } = await monter('tel-create');
        const u = await user(base, 'ada@exemple.test');
        const r = await declarerChez(url, jetonDe(u), {
            nom: 'installeur.exe',
            taille: 42,
            sha256: SHA,
        });
        expect(r.status).toBe(201);
        const corps = (await r.json()) as Record<string, unknown>;
        expect(typeof corps.id).toBe('string');
        expect(corps.taille_tranche).toBe(CHUNK_SIZE);
        expect(corps.tranches_presentes).toEqual([]);
        expect(corps.scelle_a).toBe(null);
        // The row REALLY exists, and it carries the requester.
        const ligne = await lireParId(base, corps.id as string);
        expect(ligne?.utilisateur_id).toBe(u);
        expect(ligne?.taille_tranche).toBe(CHUNK_SIZE);
    });

    it('refuses a malformed declaration, field by field', async () => {
        const { url, base } = await monter('tel-create-shape');
        const jeton = jetonDe(await user(base, 'bob@exemple.test'));
        const cas: [unknown, string][] = [
            [{ nom: '', taille: 1, sha256: SHA }, 'nom-invalide'],
            [{ taille: 1, sha256: SHA }, 'nom-invalide'],
            [{ nom: 'a', taille: -1, sha256: SHA }, 'taille-invalide'],
            [{ nom: 'a', taille: 1.5, sha256: SHA }, 'taille-invalide'],
            [{ nom: 'a', taille: '1', sha256: SHA }, 'taille-invalide'],
            [{ nom: 'a', taille: 1, sha256: 'trop-court' }, 'empreinte-invalide'],
            // ⚠️ UPPER CASE REFUSED: two spellings of the same hash would
            // compare FALSE at sealing.
            [{ nom: 'a', taille: 1, sha256: SHA.toUpperCase() }, 'empreinte-invalide'],
            [{ nom: 'a', taille: TELEVERSEMENT_MAX_OCTETS + 1, sha256: SHA }, 'trop-grand'],
        ];
        for (const [corps, motif] of cas) {
            const r = await declarerChez(url, jeton, corps);
            expect(((await r.json()) as { refus: string }).refus, JSON.stringify(corps)).toBe(motif);
        }
        // Neither a JSON object, nor JSON at all: `forme`, and nothing more.
        for (const brut of ['[1,2]', 'this-is-not-json']) {
            const r = await declarerChez(url, jeton, brut);
            expect(r.status, brut).toBe(400);
            expect(await r.json(), brut).toEqual({ refus: 'forme' });
        }
    });

    it('bounds the body of the declaration, and the upload quota', async () => {
        const { url, base } = await monter('tel-create-quota');
        const jeton = jetonDe(await user(base, 'cle@exemple.test'));

        const enorme = await declarerChez(url, jeton, {
            nom: 'x'.repeat(9000),
            taille: 1,
            sha256: SHA,
        });
        expect(enorme.status).toBe(413);
        expect(await enorme.json()).toEqual({ refus: 'corps-trop-grand' });

        const bon = () => declarerChez(url, jeton, { nom: 'a.exe', taille: 1, sha256: SHA });
        for (let i = 0; i < TELEVERSEMENTS_EN_COURS_MAX; i += 1) {
            expect((await bon()).status).toBe(201);
        }
        const trop = await bon();
        expect(trop.status).toBe(429);
        expect(await trop.json()).toEqual({
            refus: 'trop-de-televersements',
            maximum: TELEVERSEMENTS_EN_COURS_MAX,
        });
    });

    /* ── ② OWNERSHIP ──────────────────────────────────────────────── */

    it('🔴 SOMEONE ELSE\'S upload returns the SAME body as the unknown one', async () => {
        // 🔴 THREE REDS PLAY OUT HERE: removing the owner check
        // (200), returning 403 (status), or returning a 404 with a distinct
        // BODY — only the body comparison catches the third.
        const { url, base } = await monter('tel-propriete');
        const ada = await user(base, 'ada@exemple.test');
        const bob = await user(base, 'bob@exemple.test');
        const id = await poser(base, ada);

        const chez = await fetch(`${url}/televersement/${id}`, { headers: withIt(jetonDe(bob)) });
        const absent = await fetch(`${url}/televersement/${INCONNU}`, {
            headers: withIt(jetonDe(bob)),
        });
        expect(chez.status).toBe(404);
        expect(absent.status).toBe(404);
        const corpsChez = await chez.text();
        expect(corpsChez).toBe(await absent.text());
        expect(corpsChez).toBe(JSON.stringify({ refus: 'televersement-inconnu' }));
        // ⚠️ AND THE BODY CARRIES NO TRACE OF THE REAL CASE — that is the whole point.
        expect(corpsChez).not.toContain('etranger');
        expect(corpsChez).not.toContain(ada);

        // The owner, for their part, sees theirs.
        const sien = await fetch(`${url}/televersement/${id}`, { headers: withIt(jetonDe(ada)) });
        expect(sien.status).toBe(200);
        expect((await sien.json()) as Record<string, unknown>).toMatchObject({
            id,
            taille: CONTENU.length,
            sha256: SHA,
            taille_tranche: PAS,
            tranches_presentes: [],
        });
    });

    it('logs WHICH of the two cases, and the two lines differ', async () => {
        const { url, base } = await monter('tel-journal');
        const ada = await user(base, 'ada@exemple.test');
        const bob = await user(base, 'bob@exemple.test');
        const id = await poser(base, ada);
        const vu = vi.spyOn(console, 'warn').mockImplementation(() => {});

        await fetch(`${url}/televersement/${id}`, { headers: withIt(jetonDe(bob)) });
        await fetch(`${url}/televersement/00000000-0000-4000-8000-000000000000`, {
            headers: withIt(jetonDe(bob)),
        });
        const lignes = vu.mock.calls.map((c) => String(c[0]));
        expect(lignes.some((l) => l.includes('cas=etranger'))).toBe(true);
        expect(lignes.some((l) => l.includes('cas=inconnu'))).toBe(true);
    });

    it('refuses an identifier that is not a UUID, before any disk access', async () => {
        const { url, base } = await monter('tel-id-invalide');
        const jeton = jetonDe(await user(base, 'ada@exemple.test'));
        for (const id of ['..%2F..%2Fetc', 'not-a-uuid', SHA]) {
            const r = await fetch(`${url}/televersement/${id}`, { headers: withIt(jeton) });
            expect(r.status, id).toBe(400);
            expect(await r.json(), id).toEqual({ refus: 'identifiant-invalide' });
        }
    });

    /* ── ④ SCELLER ───────────────────────────────────────────────────── */

    it('refuses to seal while a chunk is missing', async () => {
        const { url, base } = await monter('tel-manquantes');
        const ada = await user(base, 'ada@exemple.test');
        const jeton = jetonDe(ada);
        const id = await poser(base, ada);
        await deposer(url, id, 0, TRANCHES[0], jeton);

        const r = await sceller(url, id, jeton);
        expect(r.status).toBe(409);
        expect(await r.json()).toEqual({ refus: 'tranches-manquantes', n: [1, 2] });
        expect((await lireParId(base, id))?.scelle_a).toBe(null);
    });

    it('🔴 the right NUMBER of chunks is not enough: a wrong SIZE is inconsistent', async () => {
        // 🔴 THE RED: replacing `verdict` with "the count is right" makes
        // this case pass — the three ranks are there, the second is too short.
        // The refusal must be `tranches-incoherentes`, NEVER `-manquantes`:
        // asking again for a badly cut chunk would never repair it.
        const { url, base } = await monter('tel-incoherentes');
        const ada = await user(base, 'ada@exemple.test');
        const jeton = jetonDe(ada);
        const id = await poser(base, ada);
        await deposer(url, id, 0, TRANCHES[0], jeton);
        await deposer(url, id, 1, Buffer.from('ab'), jeton); // two bytes out of four
        await deposer(url, id, 2, TRANCHES[2], jeton);
        expect(magasin.lister(id).length).toBe(3);

        const r = await sceller(url, id, jeton);
        expect(r.status).toBe(409);
        expect(await r.json()).toEqual({ refus: 'tranches-incoherentes', n: [1] });
        expect((await lireParId(base, id))?.scelle_a).toBe(null);
    });

    it('🔴 RECOMPUTES the fingerprint: chunks of the right size with the wrong content are refused', async () => {
        // 🔴 THE RED: trusting the ANNOUNCED `sha256` seals this
        // upload, whose bytes are not its own. The three
        // chunks have the EXACT size of the plan — only a recomputation sees it.
        const { url, base } = await monter('tel-empreinte');
        const ada = await user(base, 'ada@exemple.test');
        const jeton = jetonDe(ada);
        const id = await poser(base, ada);
        await deposer(url, id, 0, Buffer.from('AAAA'), jeton);
        await deposer(url, id, 1, Buffer.from('BBBB'), jeton);
        await deposer(url, id, 2, Buffer.from('CC'), jeton);

        const r = await sceller(url, id, jeton);
        expect(r.status).toBe(409);
        expect(await r.json()).toEqual({ refus: 'empreinte' });
        // ⚠️ NEITHER THE ANNOUNCED HASH NOR THE REREAD ONE GO THROUGH.
        expect((await lireParId(base, id))?.scelle_a).toBe(null);
    });

    it('seals when everything matches, refuses any drop afterwards, and sealing twice succeeds', async () => {
        const { url, base } = await monter('tel-sceller');
        const ada = await user(base, 'ada@exemple.test');
        const jeton = jetonDe(ada);
        const id = await poser(base, ada);
        for (let n = 0; n < TRANCHES.length; n += 1) {
            expect((await deposer(url, id, n, TRANCHES[n], jeton)).status).toBe(200);
        }

        const un = await sceller(url, id, jeton);
        expect(un.status).toBe(200);
        const attendu = { id, taille: CONTENU.length, sha256: SHA, scelle_a: MS };
        expect(await un.json()).toEqual(attendu);
        expect((await lireParId(base, id))?.scelle_a).toBe(MS);

        // 🔴 A LATER DROP WOULD CANCEL THE SEALING WITHOUT SAYING SO.
        const apres = await deposer(url, id, 0, Buffer.from('ZZZZ'), jeton);
        expect(apres.status).toBe(409);
        expect(await apres.json()).toEqual({ refus: 'deja-scelle' });

        // Sealing twice is a success — a client that retries must
        // get THE SAME response back, to the letter.
        const deux = await sceller(url, id, jeton);
        expect(deux.status).toBe(200);
        expect(await deux.text()).toBe(JSON.stringify(attendu));
    });

    it('seals an EMPTY file without requiring any chunk', async () => {
        // Zero chunks (`ceil(0 / pas)`), verdict `complet` on an empty list:
        // sealing a file without content does not require a frame without content.
        const { url, base } = await monter('tel-vide');
        const ada = await user(base, 'ada@exemple.test');
        const vide = createHash('sha256').update('').digest('hex');
        const id = await poser(base, ada, vide, 0);
        const r = await sceller(url, id, jetonDe(ada));
        expect(r.status).toBe(200);
        expect((await lireParId(base, id))?.scelle_a).toBe(MS);
    });

    it('a seal of SOMEONE ELSE returns the same 404 as the unknown one', async () => {
        const { url, base } = await monter('tel-sceller-autrui');
        const ada = await user(base, 'ada@exemple.test');
        const bob = await user(base, 'bob@exemple.test');
        const id = await poser(base, ada);
        const r = await sceller(url, id, jetonDe(bob));
        expect(r.status).toBe(404);
        expect(await r.text()).toBe(JSON.stringify({ refus: 'televersement-inconnu' }));
        expect((await lireParId(base, id))?.scelle_a).toBe(null);
    });

    it('a drop on someone else\'s upload writes NOTHING', async () => {
        const { url, base } = await monter('tel-deposer-autrui');
        const ada = await user(base, 'ada@exemple.test');
        const bob = await user(base, 'bob@exemple.test');
        const id = await poser(base, ada);
        const r = await deposer(url, id, 0, TRANCHES[0], jetonDe(bob));
        expect(r.status).toBe(404);
        expect(await r.text()).toBe(JSON.stringify({ refus: 'televersement-inconnu' }));
        expect(magasin.lister(id)).toEqual([]);
    });
});
