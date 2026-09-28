import { afterEach, describe, expect, it } from 'vitest';
import { mkdirSync, mkdtempSync, readdirSync, readlinkSync, symlinkSync, writeFileSync } from 'node:fs';
import { request as httpRequest } from 'node:http';
import type { ServerResponse } from 'node:http';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { Readable, Writable } from 'node:stream';
import { baseNeuve } from '../../base/harnais';
import type { Pilote } from '../../base/pilote';
import { startServer, type ServicePlateforme } from '../serveur';
import type { Config } from '../../config';
import { serveWithStream } from './routes-page';

const SECRET = 'un-secret-de-plateforme-de-quarante-octets';

const CONFIG: Config = {
    hote: '127.0.0.1',
    port: 0,
    base: 'sqlite',
    urlBase: ':memory:',
    secretJeton: SECRET,
    proxyDeConfiance: new Set(),
    repertoireIcones: join(mkdtempSync(join(tmpdir(), 'page-icones-')), 'icones'),
    repertoireTeleversements: join(mkdtempSync(join(tmpdir(), 'page-tranches-')), 'televersements'),
    auth: 'pomerium',
};

let base: Pilote | undefined;
let service: ServicePlateforme | undefined;

afterEach(async () => {
    await service?.close();
    service = undefined;
    await base?.fermer();
    base = undefined;
});

// A temporary root, built by hand: the test does not depend on
// `client/dist`, which may not be built.
function racineJetable(): string {
    const racine = mkdtempSync(join(tmpdir(), 'page-'));
    mkdirSync(join(racine, 'assets'));
    // 🔴 `hub.html` CARRIES A BODY DISTINCT FROM `index.html` — deliberately:
    // it is what makes the test "GET / returns the EXACT body of hub.html"
    // discriminating. Since the decision "serve the hub at the root" (30 August
    // 2026), `resolution.ts::PAGE` resolves `hub.html` for `/`; `index.html`
    // (the session page) is still served, but at ITS OWN explicit path —
    // see the dedicated test "GET /index.html still serves the session page".
    writeFileSync(join(racine, 'hub.html'), '<!doctype html><title>hub</title>');
    writeFileSync(join(racine, 'index.html'), '<!doctype html><title>page</title>');
    writeFileSync(join(racine, 'assets', 'index-a1b2c3.js'), 'export const x = 1;\n');
    // ⚠️ A NAME VITE NEVER FINGERPRINTS, at the ROOT: it is the case that
    // the one-year `immutable` made unrevisable. `client/dist` carries a
    // real one (`ls client/dist` -> `hub.webmanifest`), and this file
    // reproduces it without depending on a build.
    writeFileSync(join(racine, 'hub.webmanifest'), '{"name":"hub"}');
    // The TRAP criterion ⑥ tests: a file with the same name as a route.
    writeFileSync(join(racine, 'sante'), 'this must NEVER be served');
    writeFileSync(join(racine, 'secret.env'), 'MOT_DE_PASSE=x');
    return racine;
}

/// A root carrying TWO symbolic links that point OUT of it — the hole of
/// Critical 3 (round 2). `dessus/` lives NEXT TO the root, never inside.
function rootWithSymlinks(): string {
    const parent = mkdtempSync(join(tmpdir(), 'page-parent-'));
    const dessus = join(parent, 'dessus');
    mkdirSync(dessus);
    writeFileSync(join(dessus, 'vole.json'), '"SECRET_DU_DESSUS"');
    writeFileSync(join(dessus, 'vole.html'), '<!doctype html>SECRET_DU_DESSUS_HTML');

    const racine = join(parent, 'racine');
    mkdirSync(racine);
    writeFileSync(join(racine, 'index.html'), '<!doctype html><title>page</title>');
    // A FILE link that goes out through a RELATIVE path.
    symlinkSync(join('..', 'dessus', 'vole.json'), join(racine, 'lien.json'));
    // A DIRECTORY link that goes out — no `..` ever appears in the URL
    // that reaches it (`/lien-rep/vole.html`).
    symlinkSync(join('..', 'dessus'), join(racine, 'lien-rep'));
    return racine;
}

/// Counts, AMONG the open descriptors of the TEST PROCESS (the server
/// runs in this same process — `startServer` forks nothing), those that
/// point EXACTLY to `chemin`.
///
/// 🔴 TARGETED, NOT A TOTAL WITH TOLERANCE (round 3, Minor): a first draft
/// counted the process's total descriptors with a margin of `N/2` —
/// a check a REAL leak can leave GREEN, drowned in the noise of
/// client sockets opening and closing. `readlink` on the
/// precise file — the one the review used — returns `0` versus `210`: exactly
/// discriminating.
function comptesFdPour(chemin: string): number {
    let n = 0;
    for (const entree of readdirSync('/proc/self/fd')) {
        try {
            if (readlinkSync(`/proc/self/fd/${entree}`) === chemin) n++;
        } catch {
            // The descriptor may have closed between the listing and the read —
            // it is not a leak, we ignore it.
        }
    }
    return n;
}

/// Every request of the file goes through it, never through a bare `fetch()`.
///
/// ⚠️ NEW 4 (round 3): this file went from 221 ms to 4–10 s, with
/// REPRODUCIBLE 3.0 s stalls on 1 to 3 tests per run — absent
/// before this batch. EIGHTEEN successive EPHEMERAL servers, one per test, and
/// `fetch()` KEEPS the connection open by default (HTTP/1.1 keep-alive):
/// `service.close()`, in `afterEach`, waits for a connection kept
/// open to close — it does not as long as nothing asks it to.
/// It is the TEST BODY that stalls (connection establishment/close
/// on the client side), never the product: a bench with a fresh `http.request` never
/// saw it. `Connection: close` makes the socket close immediately after
/// the response — checked that Node honours it (the server receives it and closes).
function requeteFermee(url: string, options: RequestInit = {}): Promise<Response> {
    return fetch(url, { ...options, headers: { ...options.headers, Connection: 'close' } });
}

describe('GET /', () => {
    // 🔴 THE NEGATIVE WITNESS. Without it, the 200 of the next test does not prove that
    // PLATEFORME_PAGE was of any use.
    it("without PLATEFORME_PAGE, GET / returns yesterday's 404, word for word", async () => {
        base = await baseNeuve('page-temoin-negatif');
        service = await startServer({ ...CONFIG, racinePage: undefined }, base);
        const r = await requeteFermee(`http://127.0.0.1:${service.port}/`);
        expect(r.status).toBe(404);
        expect(await r.text()).toBe('not found\n');
    });

    // ⚠️ MINOR PROMOTED (round 2): `racinePage: ''` must behave like
    // ABSENCE, never like `resolve('')` (the current directory of the
    // process). `process.chdir` to a CONTROLLED directory carrying an
    // `index.html` makes the guard discriminating: without it, this test would see
    // `200` and the trapped body.
    it("an EMPTY root ('') also removes the server, never the current directory", async () => {
        const cwdBefore = process.cwd();
        const piege = mkdtempSync(join(tmpdir(), 'page-cwd-piege-'));
        writeFileSync(join(piege, 'index.html'), 'THIS MUST NEVER BE SERVED');
        process.chdir(piege);
        try {
            base = await baseNeuve('page-racine-vide');
            service = await startServer({ ...CONFIG, racinePage: '' }, base);
            const r = await requeteFermee(`http://127.0.0.1:${service.port}/`);
            expect(r.status).toBe(404);
        } finally {
            process.chdir(cwdBefore);
        }
    });

    it('with PLATEFORME_PAGE, GET / returns 200', async () => {
        base = await baseNeuve('page-index-statut');
        service = await startServer({ ...CONFIG, racinePage: racineJetable() }, base);
        const r = await requeteFermee(`http://127.0.0.1:${service.port}/`);
        expect(r.status).toBe(200);
    });

    // 🔴 THE RED OF "SERVE THE HUB AT THE ROOT" (30 August 2026): before this
    // batch, `GET /` returned the body of `index.html` — the SESSION page,
    // measured in PRODUCTION as the failure (see `resolution.ts::PAGE`).
    it('with PLATEFORME_PAGE, GET / returns the EXACT body of hub.html', async () => {
        base = await baseNeuve('page-index-corps');
        service = await startServer({ ...CONFIG, racinePage: racineJetable() }, base);
        const r = await requeteFermee(`http://127.0.0.1:${service.port}/`);
        // Compare the BYTES, never the 200 code alone.
        expect(await r.text()).toBe('<!doctype html><title>hub</title>');
    });

    // ⚠️ NO EXISTING PATH BREAKS: `index.html` is still served, at ITS
    // OWN explicit path — it is THIS test that guarantees it, distinct from the
    // SPA fallback of `/` above.
    it('GET /index.html still serves the session page, explicitly', async () => {
        base = await baseNeuve('page-index-explicite');
        service = await startServer({ ...CONFIG, racinePage: racineJetable() }, base);
        const r = await requeteFermee(`http://127.0.0.1:${service.port}/index.html`);
        expect(r.status).toBe(200);
        expect(await r.text()).toBe('<!doctype html><title>page</title>');
    });

    it('the document carries the CSP', async () => {
        base = await baseNeuve('page-index-csp');
        service = await startServer({ ...CONFIG, racinePage: racineJetable() }, base);
        const r = await requeteFermee(`http://127.0.0.1:${service.port}/`);
        expect(r.headers.get('content-security-policy')).toContain("default-src 'self'");
    });

    it('the document carries cache-control no-store', async () => {
        base = await baseNeuve('page-index-cache');
        service = await startServer({ ...CONFIG, racinePage: racineJetable() }, base);
        const r = await requeteFermee(`http://127.0.0.1:${service.port}/`);
        expect(r.headers.get('cache-control')).toBe('no-store');
    });

    it('a FINGERPRINTED asset carries immutable, NEVER no-store', async () => {
        base = await baseNeuve('page-actif');
        service = await startServer({ ...CONFIG, racinePage: racineJetable() }, base);
        const r = await requeteFermee(`http://127.0.0.1:${service.port}/assets/index-a1b2c3.js`);
        expect(r.headers.get('cache-control')).toBe('public, max-age=31536000, immutable');
    });

    // 🔴 THE DEFECT MEASURED ON THE REAL `client/dist`: `/hub.webmanifest`
    // returned `public, max-age=31536000, immutable`, hence **the hub's PWA
    // manifest unrevisable for a year** in every browser that had seen it.
    // The classification was done by EXTENSION, and the MIME list admits
    // `webmanifest`, `json`, `ico`, `png` — which Vite never fingerprints at the
    // root. Under nginx, `location /` emits NO `Cache-Control`: it was
    // a regression only the Pomerium setup introduced.
    //
    // 🔴 ANCHORED ON THE ABSENCE OF `immutable`, NOT ON THE EXACT VALUE: it is
    // the property that counts, and it would turn red whatever the form
    // a regression took to go back to a year.
    it("a NON-fingerprinted resource NEVER gets immutable", async () => {
        base = await baseNeuve('page-manifeste-immutable');
        service = await startServer({ ...CONFIG, racinePage: racineJetable() }, base);
        const r = await requeteFermee(`http://127.0.0.1:${service.port}/hub.webmanifest`);
        expect(r.headers.get('cache-control')).not.toContain('immutable');
    });

    // 🔴 SEPARATE, AND IT IS THE OTHER EXTREME: `no-store` would make one pay the
    // whole transfer again at each visit. A single grouped assertion
    // would only test the first.
    it("a NON-fingerprinted resource does not get no-store either", async () => {
        base = await baseNeuve('page-manifeste-nostore');
        service = await startServer({ ...CONFIG, racinePage: racineJetable() }, base);
        const r = await requeteFermee(`http://127.0.0.1:${service.port}/hub.webmanifest`);
        expect(r.headers.get('cache-control')).not.toContain('no-store');
    });

    it('a NON-fingerprinted resource is REVALIDATABLE', async () => {
        base = await baseNeuve('page-manifeste-revalidable');
        service = await startServer({ ...CONFIG, racinePage: racineJetable() }, base);
        const r = await requeteFermee(`http://127.0.0.1:${service.port}/hub.webmanifest`);
        expect(r.headers.get('cache-control')).toContain('must-revalidate');
    });

    // 🔴 THE RED OF THE CHAINING ORDER. A file named `sante` dropped into
    // the root must NEVER supplant the health router — the most
    // discreet failure possible, the service answering 200 with a
    // plausible body. SPLIT INTO TWO TESTS (round 2, Important 1): in the
    // mutation of step 6, `content-type` turns red and stops the test BEFORE
    // the body is ever checked — the second, grouped assertion
    // was therefore never tested.
    it('/sante stays served by ITS router — json content-type, despite a file of the same name', async () => {
        base = await baseNeuve('page-homonyme-type');
        service = await startServer({ ...CONFIG, racinePage: racineJetable() }, base);
        const r = await requeteFermee(`http://127.0.0.1:${service.port}/sante`);
        expect(r.headers.get('content-type')).toContain('application/json');
    });

    it('/sante stays served by ITS router — body, despite a file of the same name', async () => {
        base = await baseNeuve('page-homonyme-corps');
        service = await startServer({ ...CONFIG, racinePage: racineJetable() }, base);
        const r = await requeteFermee(`http://127.0.0.1:${service.port}/sante`);
        expect(await r.text()).not.toContain('JAMAIS');
    });

    // 🔴 SEPARATE (round 3, New 2): grouped, `expect(r.status).toBe(404)`
    // ran FIRST — if the page server had leaked the secret in a
    // `200`, this assertion would have failed and the test would have stopped THERE,
    // never reaching the one that checks the absence of the secret. It is
    // exactly the pattern denounced by round 2's Important 1, reintroduced
    // here while fixing that one — each of the two MUST be able to turn red
    // independently of the other.
    it('refuses a file outside the MIME list', async () => {
        base = await baseNeuve('page-mime-refuse-statut');
        service = await startServer({ ...CONFIG, racinePage: racineJetable() }, base);
        const r = await requeteFermee(`http://127.0.0.1:${service.port}/secret.env`);
        expect(r.status).toBe(404);
    });

    it("a file outside the MIME list never leaks its content, EVEN if the status regressed", async () => {
        // ⚠️ ANCHORED ON THE BODY (round 2, Important 3): a `404` alone does not say
        // WHETHER it is because the MIME list is closed, or because nothing
        // answers — both return the same status. This assertion does
        // NOT depend on the status: it would turn red even if a future defect
        // turned the response into a `200`.
        base = await baseNeuve('page-mime-refuse-corps');
        service = await startServer({ ...CONFIG, racinePage: racineJetable() }, base);
        const r = await requeteFermee(`http://127.0.0.1:${service.port}/secret.env`);
        expect(await r.text()).not.toContain('MOT_DE_PASSE');
    });

    // ⚠️ IMPORTANT 2 (round 2): the hole the previous addendum closed
    // (`resolution.ts`, reason `nom-vide`) was only tested on the PURE rule,
    // never with a REAL bare `.json` file on disk — the only task
    // of the batch that exposes one.
    it('refuses a bare `.json` file (empty name), even one really present on the disk', async () => {
        const racine = racineJetable();
        writeFileSync(join(racine, '.json'), '"must never be served"');
        base = await baseNeuve('page-json-nu-disque');
        service = await startServer({ ...CONFIG, racinePage: racine }, base);
        const r = await requeteFermee(`http://127.0.0.1:${service.port}/.json`);
        expect(r.status).toBe(404);
    });

    // ⚠️ IMPORTANT 2 (round 2): traversal too, at DISK level.
    // 🔴 ENCODED FORM, NOT THE PLAIN FORM: `new URL(req.url, …)`, in
    // `routes-page.ts`, COLLAPSES `/assets/../../etc/passwd` into `/etc/passwd`
    // BEFORE `resoudre()` even sees it — the plain form therefore tests
    // NOTHING of `resolution.ts`'s guard, it is neutralised one step
    // earlier. `%2e%2e%2f` SURVIVES this normalisation (checked:
    // `new URL('/%2e%2e%2f…', base).pathname` returns it verbatim), and
    // it is what `resoudre()` decodes and refuses ITSELF.
    it('refuses an ENCODED traversal escaping the root, through a real HTTP request', async () => {
        base = await baseNeuve('page-traversee-disque');
        service = await startServer({ ...CONFIG, racinePage: racineJetable() }, base);
        const r = await requeteFermee(
            `http://127.0.0.1:${service.port}/assets/..%2f..%2f..%2f..%2f..%2f..%2fetc%2fpasswd`,
        );
        expect(r.status).toBe(404);
    });

    // 🔴 CRITICAL 3 (round 2), MEASURED: a FILE link going out of the
    // root through a relative path returned `200` with the stolen content before
    // the `realpath` of `routes-page.ts`.
    it('refuses a file reached through a FILE symbolic link escaping the root', async () => {
        base = await baseNeuve('page-link-file');
        service = await startServer({ ...CONFIG, racinePage: rootWithSymlinks() }, base);
        const r = await requeteFermee(`http://127.0.0.1:${service.port}/lien.json`);
        expect(r.status).toBe(404);
    });

    // 🔴 CRITICAL 3 (round 2), MEASURED: a DIRECTORY link going out of the
    // root — no `..` ever appears in the URL that reaches it.
    it('refuses a file reached through a DIRECTORY symbolic link escaping the root', async () => {
        base = await baseNeuve('page-lien-repertoire');
        service = await startServer({ ...CONFIG, racinePage: rootWithSymlinks() }, base);
        const r = await requeteFermee(`http://127.0.0.1:${service.port}/lien-rep/vole.html`);
        expect(r.status).toBe(404);
    });

    // 🔴 CRITICAL 1 (round 2), MEASURED: `.pipe()` does not destroy the source
    // when the destination dies. A client that gives up — here, a socket
    // destroyed as soon as the headers are received — left the file
    // descriptor open FOREVER, MONOTONICALLY.
    it("a download ABANDONED by the client does not leak a descriptor", async () => {
        const racine = racineJetable();
        // A file big enough to leave time to give up BEFORE
        // the stream has finished flowing.
        const cheminGros = join(racine, 'assets', 'gros.js');
        writeFileSync(cheminGros, 'x'.repeat(8 * 1024 * 1024));
        base = await baseNeuve('page-fd-abandon');
        service = await startServer({ ...CONFIG, racinePage: racine }, base);

        const before = comptesFdPour(cheminGros);
        const N = 20;
        for (let i = 0; i < N; i++) {
            await new Promise<void>((resolvePromesse, reject) => {
                const req = httpRequest(
                    { hostname: '127.0.0.1', port: service!.port, path: '/assets/gros.js', method: 'GET' },
                    (res) => {
                        // Give up as soon as the headers are received — before the
                        // body has finished flowing.
                        res.destroy();
                        resolvePromesse();
                    },
                );
                req.on('error', () => resolvePromesse());
                req.end();
            });
        }
        // Leaves time for the asynchronous 'close' handlers to run —
        // closing a file descriptor is not synchronous.
        await new Promise((r) => setTimeout(r, 300));
        const apres = comptesFdPour(cheminGros);
        // 🔴 EXACTLY DISCRIMINATING (round 3, Minor): targeted at THE precise
        // file, a leak would make `apres` close to `before + N` (one fd per
        // abandonment, monotonic, measured). No tolerance: `before` and `apres`
        // MUST be equal.
        expect(apres).toBe(before);
    });

    // 🔴 CRITICAL 2 (round 2), MEASURED: a read that breaks AFTER the
    // headers have gone has NO error handler under `.pipe()` —
    // an `EventEmitter` that emits `error` without a listener THROWS, and the review
    // reproduced the WHOLE service dying (`EMFILE`). The stream is HERE
    // INJECTED to reproduce a MID-STREAM read error WITHOUT causing
    // a real exhaustion of descriptors — which would run the exact risk
    // this test exists to test, on the test process
    // itself.
    //
    // ⚠️ UNIT SETUP, WITHOUT A REAL HTTP SERVER (round 3, New 3): this
    // calls `serveWithStream` directly, with FABRICATED `req`/`rep` —
    // there is no socket, no port, no server process. The previous title
    // ("… does NOT kill THE SERVICE") promised a property this
    // setup does NOT establish; the property remains true (see the comment
    // of `serveWithStream` on what this choice costs and what it does not
    // establish), but THIS test only tests the behaviour of the FUNCTION.
    //
    // Builds a NEW `(racine, rep, req, flux)` set at each call:
    // the fake stream is SINGLE-USE (`envoyee`), and `rep` carries its
    // own destruction state — the two tests below can NOT
    // share a single build without one of the two becoming
    // vacuous.
    function unitCallWithFaultyStream(): { result: Promise<boolean>; rep: Writable } {
        const racine = racineJetable();
        // 🔴 A REAL `Writable`, NOT A FAKE OBJECT WITH SILENT METHODS:
        // `pipeline()` relies on REAL events (`'close'`,
        // `'finish'`, `'error'`) to know when the destination has finished or
        // is dead. A first attempt with an object whose `on()`/`once()` just
        // ignored the callback left `pipeline()` waiting for a
        // signal that would never come — the test stayed stuck until the
        // timeout. `writeHead` is simply added on top,
        // the only method a bare `Writable` lacks.
        const rep = new Writable({
            write(_chunk, _enc, cb) {
                cb();
            },
        });
        (rep as unknown as { writeHead: (...a: unknown[]) => unknown }).writeHead = () => rep;

        // A stream that emits a first chunk THEN fails — reproducing a
        // read that breaks while the response is already in progress.
        //
        // ⚠️ `envoyee` IS MANDATORY: without it, `_read()` — called back in a
        // SYNCHRONOUS loop as long as the consumer signals it is ready —
        // would push forever before the error's `process.nextTick`
        // even got its chance to run, filling the internal buffer
        // without bound. Paid once while writing this very test:
        // `JavaScript heap out of memory` on the Vitest worker.
        let envoyee = false;
        const fluxFautif = new Readable({
            read() {
                if (envoyee) return;
                envoyee = true;
                this.push('debut ');
                process.nextTick(() => this.emit('error', new Error('EMFILE (simulated)')));
            },
        });

        const req = { url: '/assets/index-a1b2c3.js', method: 'GET', headers: {} };
        const result = serveWithStream(
            req as never,
            rep as unknown as ServerResponse,
            { racinePage: racine },
            () => fluxFautif,
        );
        return { result, rep };
    }

    // 🔴 SEPARATE (round 3, New 2), same reason as the MIME test above:
    // grouped, `expect(result).toBe(true)` ran first and would have
    // masked a failure of the second assertion had it turned red.
    it('a stream error injected AFTER the headers is handled (route not fallen back to 404)', async () => {
        const { result } = unitCallWithFaultyStream();
        // The route IS handled (the headers have gone): returning
        // `false` would make the chain fall back to a generic 404 on top of a
        // response already started.
        expect(await result).toBe(true);
    });

    it('a stream error injected AFTER the headers destroys the response, rather than leaving it hanging', async () => {
        const { result, rep } = unitCallWithFaultyStream();
        await result;
        expect(rep.destroyed).toBe(true);
    });

    // 🔴 OUTSIDE GET/HEAD, THE BEHAVIOUR IS YESTERDAY'S TO THE BYTE. Returning
    // 405 would mean a POST on a misspelled path — the SPA fallback resolves
    // anything — would get "method" instead of the 404 that points to it.
    it('a POST on an unknown path always returns 404, never 405', async () => {
        base = await baseNeuve('page-post-inconnu');
        service = await startServer({ ...CONFIG, racinePage: racineJetable() }, base);
        const r = await requeteFermee(`http://127.0.0.1:${service.port}/aplication/x`, { method: 'POST' });
        expect(r.status).toBe(404);
    });

    it('serves a HEAD without a body', async () => {
        base = await baseNeuve('page-head');
        service = await startServer({ ...CONFIG, racinePage: racineJetable() }, base);
        const r = await requeteFermee(`http://127.0.0.1:${service.port}/`, { method: 'HEAD' });
        expect(r.status).toBe(200);
        expect(await r.text()).toBe('');
    });
});
