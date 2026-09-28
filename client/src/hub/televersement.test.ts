// The tests of the upload orchestration — ON THE HOST, WITHOUT A DOM.
//
// 🔴 THIS FILE ONLY EXISTS BECAUSE THE THREE DEPENDENCIES ARE INJECTED.
// `fetch`, the clock and the `AbortSignal` being parameters, the whole
// sequence is exercised without a browser or a server — and it is the same property that
// will let the acceptance driver (D14) run THE PRODUCT'S CODE under Node.
//
// ⚠️ `Buffer` IS FORBIDDEN HERE: `client/` does not have `@types/node`, and a test that
// used it would pass under Vitest while BREAKING `npm run typecheck`
// (`TS2580`). A trap this repository already paid for — hence `Uint8Array` everywhere.
import { describe, expect, it } from 'vitest';
import { condenserHex } from '../../../proto/ts/sha256';
import { televerser, type DepsTeleversement, type Fetch, type Issue } from './televersement';

/// Deterministic content — a congruential generator, never `Math.random`:
/// a test whose bytes change from one run to the next cannot be replayed.
function octetsDe(size: number, graine = 7) {
    const sortie = new Uint8Array(size);
    let x = graine;
    for (let i = 0; i < size; i += 1) {
        x = (x * 1103515245 + 12345) & 0x7fffffff;
        sortie[i] = (x >>> 16) & 0xff;
    }
    return sortie;
}

interface Appel {
    methode: string;
    url: string;
    octets: number;
    entetes: Record<string, string>;
    corps?: string;
}

interface Reponse {
    status: number;
    corps?: unknown;
}

type Route = 'creation' | 'etat' | 'tranche' | 'sceller';

function routeDe(url: string, methode: string): Route {
    if (url.endsWith('/sceller')) return 'sceller';
    if (url.includes('/tranche/')) return 'tranche';
    return methode === 'POST' ? 'creation' : 'etat';
}

/// A fake service that RECORDS what it is sent. It is the recording
/// that counts: criterion ③ of the spec is judged on the bytes ACTUALLY EMITTED, not
/// on the final verdict — "starting over from zero would pass a test that only looks at
/// the result".
function serveur(reponses: Partial<Record<Route, Reponse>>) {
    const appels: Appel[] = [];
    const fetch: Fetch = async (url, init) => {
        const methode = init?.method ?? 'GET';
        const corpsEmis = init?.body;
        appels.push({
            methode,
            url,
            octets: corpsEmis instanceof Uint8Array ? corpsEmis.byteLength : 0,
            entetes: init?.headers ?? {},
            corps: typeof corpsEmis === 'string' ? corpsEmis : undefined,
        });
        const r = reponses[routeDe(url, methode)] ?? { status: 200, corps: {} };
        return { ok: r.status < 400, status: r.status, json: async () => r.corps };
    };
    const puts = (): Appel[] => appels.filter((a) => a.methode === 'PUT');
    return {
        fetch,
        appels,
        puts,
        octetsEmis: (): number => puts().reduce((s, a) => s + a.octets, 0),
        rangs: (): number[] => puts().map((a) => Number(a.url.split('/tranche/')[1])),
    };
}

function deps(fetch: Fetch, extra: Partial<DepsTeleversement> = {}): DepsTeleversement {
    return { base: 'https://p', jeton: 'j-1', fetch, maintenant: () => 0, ...extra };
}

/// A 16-byte file cut by 4: four chunks, the case of red no. 1.
const OCTETS = octetsDe(16);
const SHA = condenserHex(OCTETS);
const file = (o: BlobPart = OCTETS, nom = 'setup.exe'): File => new File([o], nom);
const creation = { status: 200, corps: { id: 't-1', taille_tranche: 4, tranches_presentes: [] } };
/// 🔴 THE SHAPE THE ROUTE RETURNS, AND THE ONLY ONE: `{n, octets}`.
///
/// ⚠️ THESE CASES CARRIED BARE RANKS while `routes-televersement.ts`
/// was being written in parallel, and the module tolerated them. The route being settled —
/// it returns the store's LISTING —, the tolerance is removed: it made
/// **the service be believed about a size it had never announced**, so
/// that a rank present at the WRONG size would have passed as compliant and
/// sealing would have refused later, elsewhere, with nothing linking the two.
const presente = (n: number, octets = 4) => ({ n, octets });

const etatDe = (presentes: unknown, extra: Record<string, unknown> = {}): Reponse => ({
    status: 200,
    corps: { taille: 16, sha256: SHA, taille_tranche: 4, tranches_presentes: presentes, ...extra },
});

describe('the nominal path', () => {
    it('fingerprints, creates, deposits the four chunks and seals', async () => {
        const s = serveur({ creation });
        const issue = await televerser(file(), deps(s.fetch));
        expect(issue).toEqual({ etat: 'scelle', id: 't-1', size: 16, sha256: SHA, deposees: [0, 1, 2, 3] });
        expect(s.rangs()).toEqual([0, 1, 2, 3]);
        expect(s.octetsEmis()).toBe(16);
        // The protocol order: create, deposit, seal — and a single creation.
        expect(s.appels.map((a) => routeDe(a.url, a.methode))).toEqual([
            'creation', 'tranche', 'tranche', 'tranche', 'tranche', 'sceller',
        ]);
    });

    it("announces the name, the size and the fingerprint from the CREATION on (D5)", async () => {
        // 🔴 It is this announcement, and it alone, that will make a resumption
        // verifiable: without `{size, sha256}` set beforehand, nothing will say that
        // the file chosen again after a tab reload is THE SAME.
        const s = serveur({ creation });
        await televerser(file(OCTETS, 'programme.msi'), deps(s.fetch));
        expect(s.appels[0].url).toBe('https://p/televersement');
        expect(s.appels[0].entetes.authorization).toBe('Bearer j-1');
        expect(JSON.parse(s.appels[0].corps ?? '{}')).toEqual({ nom: 'programme.msi', taille: 16, sha256: SHA });
    });

    it("the runtime platform's `fetch` satisfies the `Fetch` type", () => {
        // 🔵 THE CHECK THAT KEEPS `Fetch` FROM DRIFTING. It is declared by hand
        // — a complete `Response` could not be built in a test —, so nothing
        // would guarantee a REAL `fetch` still satisfies it after an
        // edit. This assignment is checked by `tsc --noEmit`, and it
        // would go red at COMPILE TIME the day the two shapes diverged.
        const preuve: Fetch = globalThis.fetch;
        expect(typeof preuve).toBe('function');
    });

    it('carries the token and the binary type on each deposited chunk', async () => {
        const s = serveur({ creation });
        await televerser(file(), deps(s.fetch));
        for (const p of s.puts()) {
            expect(p.entetes.authorization).toBe('Bearer j-1');
            expect(p.entetes['content-type']).toBe('application/octet-stream');
        }
    });

    it("on an EMPTY file, deposits no chunk and seals anyway", async () => {
        const s = serveur({ creation: { status: 200, corps: { id: 't-0', taille_tranche: 4, tranches_presentes: [] } } });
        const issue = await televerser(file(new Uint8Array(0)), deps(s.fetch));
        expect(issue).toMatchObject({ etat: 'scelle', size: 0, sha256: condenserHex(new Uint8Array(0)) });
        expect(s.puts()).toHaveLength(0);
        expect(s.appels.at(-1)?.url).toBe('https://p/televersement/t-0/sceller');
    });
});

describe('🔴 resuming deposits again ONLY the missing chunks', () => {
    it('on four chunks of which two are present, emits TWO — not four', async () => {
        // 🔴 It is the red of criterion ③: a product that started over from zero
        // would return the SAME `scelle` verdict. Only the BYTE COUNT separates it
        // from a correct product — hence the assertion on `octetsEmis`, and not on
        // the outcome alone.
        const s = serveur({ etat: etatDe([presente(0), presente(1)]) });
        const issue = await televerser(file(), deps(s.fetch, { reprise: 't-1' }));
        // ⚠️ THE BYTE COUNT COMES FIRST, AND IT IS NOT COSMETIC:
        // `deposees` is a field the PRODUCT declares, hence falsifiable by a
        // product that deposited everything again while announcing the opposite. The bytes
        // actually emitted, on the other hand, are not declared.
        expect(s.octetsEmis()).toBe(8);
        expect(s.rangs()).toEqual([2, 3]);
        expect(s.puts()).toHaveLength(2);
        expect(issue).toMatchObject({ etat: 'scelle', id: 't-1', deposees: [2, 3] });
    });

    it('ALSO accepts the rich `{n, octets}` form of the listing', async () => {
        const s = serveur({ etat: etatDe([{ n: 0, octets: 4 }, { n: 2, octets: 4 }]) });
        const issue = await televerser(file(), deps(s.fetch, { reprise: 't-1' }));
        expect(s.octetsEmis()).toBe(8);
        expect(issue).toMatchObject({ etat: 'scelle', deposees: [1, 3] });
    });

    it("on a COMPLETE deposit, deposits nothing again and goes straight to sealing", async () => {
        const s = serveur({ etat: etatDe([presente(0), presente(1), presente(2), presente(3)]) });
        const issue = await televerser(file(), deps(s.fetch, { reprise: 't-1' }));
        expect(s.puts()).toHaveLength(0);
        expect(issue).toMatchObject({ etat: 'scelle', deposees: [] });
        expect(s.appels.map((a) => a.methode)).toEqual(['GET', 'POST']);
    });

    it('refuses a badly SIZED chunk instead of requesting it again endlessly', async () => {
        // `incoherentes` is not `manquantes`: depositing again would fix nothing,
        // and looping would be the real defect.
        const s = serveur({ etat: etatDe([{ n: 0, octets: 3 }]) });
        const issue = await televerser(file(), deps(s.fetch, { reprise: 't-1' }));
        expect(issue).toEqual({
            etat: 'refus',
            id: 't-1',
            refus: { source: 'client', motif: 'tranches-incoherentes', detail: 'rangs 0' },
        });
        expect(s.puts()).toHaveLength(0);
    });
});

describe("🔴 resuming checks the identity of the file BEFORE resuming", () => {
    it("refuses a different SIZE BEFORE fingerprinting, and emits NO PUT", async () => {
        // 🔴 THE FIRST ASSERTION IS THE ONLY ONE THAT TELLS THE SIZE GUARD
        // FROM THE FINGERPRINT GUARD, and it was added AFTER seeing the
        // corresponding red stay GREEN: a 12-byte file has a different
        // fingerprint anyway, so the `fichier-different` verdict came out
        // even without the size guard. What this guard buys is not the
        // verdict — it is NOT paying for the full read pass, eleven
        // seconds for 800 MB. An observed `empreinte` phase proves it was
        // paid.
        const phases: string[] = [];
        const s = serveur({ etat: etatDe([presente(0), presente(1)]) });
        const issue = await televerser(
            file(octetsDe(12)),
            deps(s.fetch, { reprise: 't-1', progression: (p) => void phases.push(p.phase) }),
        );
        expect(phases).toEqual([]);
        expect(issue.etat).toBe('refus');
        expect((issue as Extract<Issue, { etat: 'refus' }>).refus).toMatchObject({
            source: 'client',
            motif: 'fichier-different',
        });
        expect(s.puts()).toHaveLength(0);
    });

    it("refuses a different CONTENT of equal size, and emits NO PUT", async () => {
        const s = serveur({ etat: etatDe([presente(0), presente(1)]) });
        const issue = await televerser(file(octetsDe(16, 99)), deps(s.fetch, { reprise: 't-1' }));
        expect(issue).toMatchObject({ etat: 'refus', id: 't-1', refus: { motif: 'fichier-different' } });
        expect(s.puts()).toHaveLength(0);
    });

    it("refuses to resume a state that announces NEITHER size NOR fingerprint", async () => {
        // Resuming blindly would mix the chunks of two files, and
        // sealing would fail without anything saying why.
        const s = serveur({ etat: { status: 200, corps: { taille_tranche: 4, tranches_presentes: [] } } });
        const issue = await televerser(file(), deps(s.fetch, { reprise: 't-1' }));
        expect(issue).toMatchObject({ etat: 'refus', refus: { motif: 'etat-illisible' } });
        expect(s.puts()).toHaveLength(0);
    });
});

describe('🔴 the service refusals come back TYPED', () => {
    it("on a sealing answered 409 `empreinte`, returns the refusal and does NOT retry", async () => {
        const s = serveur({ creation, sceller: { status: 409, corps: { refus: 'empreinte' } } });
        const issue = await televerser(file(), deps(s.fetch));
        expect(issue).toEqual({
            etat: 'refus',
            id: 't-1',
            refus: { source: 'service', etape: 'scellement', statut: 409, motif: 'empreinte' },
        });
        // ⚠️ THE TWO ASSERTIONS THAT FOLLOW ARE THE HEART OF THIS RED: a
        // retry loop would upload endlessly, and a lost `id` would force
        // the caller to deposit everything again.
        expect(s.appels.filter((a) => a.url.endsWith('/sceller'))).toHaveLength(1);
        expect(s.octetsEmis()).toBe(16);
    });

    it("on a refused chunk deposit, stops at the FIRST one and names the step", async () => {
        const s = serveur({ creation, tranche: { status: 413, corps: { refus: 'tranche-trop-grande' } } });
        const issue = await televerser(file(), deps(s.fetch));
        expect(issue).toMatchObject({
            etat: 'refus',
            refus: { source: 'service', etape: 'tranche', statut: 413, motif: 'tranche-trop-grande' },
        });
        expect(s.puts()).toHaveLength(1);
    });

    it("on a body without `refus`, returns the status rather than making up a reason", async () => {
        const s = serveur({ creation: { status: 503, corps: 'indisponible' } });
        const issue = await televerser(file(), deps(s.fetch));
        expect(issue).toEqual({
            etat: 'refus',
            refus: { source: 'service', etape: 'creation', statut: 503, motif: 'http-503' },
        });
    });

    it("on a creation without an identifier, refuses instead of depositing into the void", async () => {
        const s = serveur({ creation: { status: 200, corps: { taille_tranche: 4 } } });
        const issue = await televerser(file(), deps(s.fetch));
        expect(issue).toMatchObject({ etat: 'refus', refus: { motif: 'etat-illisible' } });
        expect(s.puts()).toHaveLength(0);
    });

    it("on an absurd `taille_tranche`, refuses instead of THROWING", async () => {
        // `plan` throws on a zero step — its guard targets a program defect, not
        // wire data. Validating it here is what makes the refusal named.
        const s = serveur({ creation: { status: 200, corps: { id: 't-1', taille_tranche: 0 } } });
        const issue = await televerser(file(), deps(s.fetch));
        expect(issue).toMatchObject({ etat: 'refus', id: 't-1', refus: { motif: 'etat-illisible' } });
    });

    it("on `tranches_presentes` of unknown shape, refuses", async () => {
        const s = serveur({ creation: { status: 200, corps: { id: 't-1', taille_tranche: 4, tranches_presentes: 'deux' } } });
        const issue = await televerser(file(), deps(s.fetch));
        expect(issue).toMatchObject({ etat: 'refus', refus: { motif: 'etat-illisible' } });
    });
});

describe("interruption is a refusal, never a failure", () => {
    it('stops the deposit between two chunks and returns `interrompu` with its `id`', async () => {
        const s = serveur({ creation });
        const arret = new AbortController();
        // ⚠️ THE CLOCK MUST MOVE FORWARD: frozen, the pacer would only let through
        // the first event of each phase — the forced one — and
        // abandonment would never be triggered. This test was SEEN WRONGLY GREEN
        // for this reason before being fixed.
        let horloge = 0;
        const issue = await televerser(
            file(),
            deps(s.fetch, {
                signal: arret.signal,
                maintenant: () => (horloge += 1000),
                progression: (p) => {
                    if (p.phase === 'transfert' && p.octets > 0) arret.abort();
                },
            }),
        );
        expect(issue).toMatchObject({ etat: 'refus', id: 't-1', refus: { motif: 'interrompu' } });
        expect(s.puts().length).toBeLessThan(4);
    });

    it("turns the rejection of a CUT `fetch` into a refusal, but lets the others through", async () => {
        const arret = new AbortController();
        const casse: Fetch = async () => {
            throw new Error('cut');
        };
        arret.abort();
        const issue = await televerser(file(), deps(casse, { signal: arret.signal }));
        expect(issue).toMatchObject({ etat: 'refus', refus: { motif: 'interrompu' } });
        // Without a raised signal, the same failure GOES UP: this module has nothing useful to
        // say about an environment failure, and disguising it as a refusal would make it
        // pass for a protocol decision.
        await expect(televerser(file(), deps(casse))).rejects.toThrow('cut');
    });
});

describe("progress, paced by the injected clock", () => {
    it('announces the three phases, and depends only on the `maintenant` parameter', async () => {
        const s = serveur({ creation });
        const vues: string[] = [];
        let horloge = 0;
        await televerser(
            file(),
            deps(s.fetch, {
                maintenant: () => (horloge += 1000),
                progression: (p) => void vues.push(`${p.phase}:${p.octets}/${p.total}`),
            }),
        );
        expect(vues[0]).toBe('empreinte:0/16');
        expect(vues.at(-1)).toBe('scellement:16/16');
        expect(new Set(vues.map((v) => v.split(':')[0]))).toEqual(
            new Set(['empreinte', 'transfert', 'scellement']),
        );
    });

    it('holds back NON-forced events when the period has not elapsed', async () => {
        // The frozen clock: only phase changes, which always go through,
        // must come out. Without pacing, fingerprinting 800 MB would emit thousands.
        const s = serveur({ creation });
        const vues: string[] = [];
        await televerser(file(), deps(s.fetch, { progression: (p) => void vues.push(p.phase) }));
        expect(vues.filter((v) => v === 'transfert')).toHaveLength(1);
    });
});
