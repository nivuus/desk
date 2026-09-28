// The three INSTALLATION routes, tested THROUGH a real HTTP server
// and a REAL chunk store — in the style of `routes-applications.test.ts`
// and `routes-icone.test.ts`.
//
// 🔴 THE STORE IS NOT A DOUBLE, AND THAT IS THE POINT. The bytes are
// really split into chunks on a temporary disk, and the test compares
// the response body with the original file, BYTE FOR BYTE: it is what
// tests the streamed concatenation rather than the promise that it happens.
//
// 🔴 THIS FILE CARRIES THE FOUR MUTATIONS THE PLAN PRESCRIBES, and the first
// is the most important of the sub-block: **an agent token of VM `B` must
// not obtain the installer meant for `A`**. It has its named `it()`, and it carries
// its WITNESS — `A`'s agent, for its part, gets the bytes —, otherwise the assertion
// would hold for a route that never serves anything.

import { afterEach, describe, expect, it, vi } from 'vitest';
import { createHash, randomBytes } from 'node:crypto';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { Readable } from 'node:stream';
import { MOTEUR } from '../base/harnais';
import type { Pilote } from '../base/pilote';
import { ouvrirMagasinTranches, type MagasinTranches } from '../apps/magasin-tranches';
import { RegistreAgents } from '../agents/registre';
import { enroler, marquerVu } from '../depot/agent';
import { create as createInstallation, terminer, avancer } from '../depot/installation';
import { create as createUpload, sceller } from '../depot/televersement';
import { createUser } from '../depot/utilisateur';
import { plan } from '../../../proto/ts/tranches';
import { withIt, demonter, jetonDe, monterRoute, MS, ORIGINE, poserVm, attribuer, type Montage } from './routes-harnais';
import { servirInstallation } from './routes-installation';

let m: Montage | undefined;
let racine: string | undefined;
/// ⚠️ IT IS CALLED `tranches` IN THE DEPENDENCIES, NOT `magasin`, AND THE NAME
/// MATTERS: `serveur.ts` ALREADY carries a `magasin` — G2's ICON one —
/// in the same object. The harness follows that name so that the wiring tested here
/// is the service's.
let tranches: MagasinTranches | undefined;
let maintenant = MS;

afterEach(async () => {
    await demonter(m);
    m = undefined;
    if (racine !== undefined) rmSync(racine, { recursive: true, force: true });
    racine = undefined;
    tranches = undefined;
    vi.restoreAllMocks();
});

/// Mounts the server that carries ONLY this route, plus the generic 404 of
/// `serveur.ts` reproduced word for word by `monterRoute`: it is what makes
/// a `false` returned by `servirInstallation` observable.
async function servir(nom: string, origineClient?: string): Promise<string> {
    maintenant = MS;
    racine = mkdtempSync(join(tmpdir(), `g3-${nom}-`));
    tranches = ouvrirMagasinTranches(racine, () => {});
    m = await monterRoute(nom, (req, rep, base) =>
        servirInstallation(req, rep, {
            base,
            secretJeton: 'un-secret-de-plateforme-de-quarante-octets',
            origineClient,
            tranches: tranches!,
            // ⚠️ AN EMPTY REGISTRY, AND IT IS THE CASE THAT COUNTS HERE: no socket
            // is registered, so `pousser` returns `false` and the order stays
            // `en_attente` in the database — exactly the net that
            // `reemettreLesInstallations` will deliver at the next enrolment. These
            // tests test the ROUTE, never the delivery, which requires a
            // live socket (acceptance, §5quater).
            registre: new RegistreAgents(),
            maintenant: () => maintenant,
        }));
    return m.url;
}

/// A REAL upload: the row in the database, and its chunks on disk.
///
/// ⚠️ THE CHUNKING GOES THROUGH `proto/ts/tranches.ts::plan`, never through a local
/// arithmetic: it is the rule the route uses to rebuild
/// the stream, and two independent arithmetics would one day diverge.
async function poserTeleversement(
    base: Pilote,
    userId: string,
    nom: string,
    octets: Buffer,
    { scelle = true, chunkSize = 7 } = {},
): Promise<{ id: string; octets: Buffer }> {
    const ligne = await createUpload(
        base,
        {
            userId,
            nom,
            taille: octets.length,
            sha256: createHash('sha256').update(octets).digest('hex'),
            chunkSize,
        },
        MS,
    );
    for (const t of plan(octets.length, chunkSize)) {
        const debut = t.n * chunkSize;
        await tranches!.write(ligne.id, t.n, Readable.from([octets.subarray(debut, debut + t.octets)]), 1 << 20);
    }
    if (scelle) await sceller(base, ligne.id, MS);
    return { id: ligne.id, octets };
}

/// An enrolled VM, alive, assigned to a user — and its prefix, which is
/// the SUBJECT of the agent token.
async function poserVmVivante(base: Pilote, vmId: string, email: string, prefixe: string): Promise<string> {
    await poserVm(base, vmId);
    const u = await attribuer(base, vmId, email);
    await enroler(base, vmId, 'empreinte-opaque', prefixe);
    await marquerVu(base, vmId, MS);
    return u;
}

/// ⚠️ THE TOKEN CARRIES THE REAL IDENTIFIER of the user, never a convenient name:
/// it is what the route compares with `vm.utilisateur_id`, and an invented subject
/// would make cases that have nothing to do with it return `404 vm-inconnue` — that is
/// what a first draft of this file measured.
const CORPS = (u: string, vm: string, televersement: string) => ({
    method: 'POST',
    headers: withIt(jetonDe(u)),
    body: JSON.stringify({ vm, televersement }),
});

describe(`routes /installation, engine=${MOTEUR}`, () => {
    it("returns `false` on a foreign path: the server's 404 follows", async () => {
        const url = await servir('inst-etranger');
        const r = await fetch(`${url}/nothing-at-all`);
        expect([r.status, await r.text()]).toEqual([404, 'not found\n']);
    });

    it('🔴 BOTH patterns are ANCHORED AT BOTH ENDS, and the BODY discriminates', async () => {
        // 🔴 A `startsWith` WOULD OPEN A WHOLE FAMILY OF PATHS THAT
        // NOBODY DECIDED. And it is the BODY that judges, never the code:
        // G1 measured that a `startsWith('/application')` left SEVENTEEN
        // tests green, the route returning ITS OWN typed 404, indistinguishable from the
        // generic 404 as long as one only reads the status.
        const url = await servir('inst-ancre');
        const declines = [
            `${url}/installation/x/y`,
            `${url}/installations`,
            `${url}/installationsdetournees`,
            `${url}/televersement/x/contenu/y`,
            `${url}/televersement/x`,
            `${url}/televersements/x/contenu`,
        ];
        for (const cible of declines) {
            const r = await fetch(cible);
            expect([cible, r.status, await r.text()]).toEqual([cible, 404, 'not found\n']);
        }
        // The three RIGHT paths are served — without this witness, the
        // assertions above would hold for a route that serves NOTHING.
        expect((await fetch(`${url}/installation/x`)).status).not.toBe(404);
        expect((await fetch(`${url}/televersement/x/contenu`)).status).not.toBe(404);
        expect((await fetch(`${url}/installation`, { method: 'POST' })).status).not.toBe(404);
    });

    it('serves the `OPTIONS` preflight request, without which nothing is reachable', async () => {
        // ⚠️ The three routes require `Authorization`, which makes the request NON-
        // SIMPLE: a 404 on the `OPTIONS` would make the browser give up before
        // the real request. NO Node test sees the origin policy —
        // it is this assertion, and nothing else, that holds the header.
        const url = await servir('inst-options', ORIGINE);
        for (const c of ['/installation', '/installation/x', '/televersement/x/contenu']) {
            const r = await fetch(`${url}${c}`, { method: 'OPTIONS', headers: { origin: ORIGINE } });
            expect([c, r.status, r.headers.get('access-control-allow-origin')]).toEqual([c, 204, ORIGINE]);
        }
    });

    it('refuses the METHOD on a path that exists, rather than a 404', async () => {
        const url = await servir('inst-methode');
        expect((await fetch(`${url}/installation`)).status).toBe(405);
        expect((await fetch(`${url}/installation/x`, { method: 'POST' })).status).toBe(405);
        expect((await fetch(`${url}/televersement/x/contenu`, { method: 'POST' })).status).toBe(405);
    });

    it('WITHOUT an `Authorization` header, the three routes return 401', async () => {
        const url = await servir('inst-sans-jeton');
        for (const [c, o] of [['/installation', { method: 'POST' }], ['/installation/x', {}],
            ['/televersement/x/contenu', {}]] as const) {
            const r = await fetch(`${url}${c}`, o);
            expect([c, r.status, await r.json()]).toEqual([c, 401, { refus: 'jeton-absent' }]);
        }
    });

    it("🔴 `POST /installation` refuses an AGENT token — MUTATION no. 2, first direction", async () => {
        // 🔴 Agent and human are signed by the SAME secret: accepting any valid
        // token would reopen P3's E5, and a compromised agent would order
        // the installation of software on its owner's machine.
        const url = await servir('inst-ordre-jeton-agent');
        const r = await fetch(`${url}/installation`, {
            method: 'POST',
            headers: withIt(jetonDe('prefixe-quelconque', 'agent')),
            body: JSON.stringify({ vm: 'v-1', televersement: 't-1' }),
        });
        expect([r.status, await r.json()]).toEqual([403, { refus: 'jeton-agent' }]);
    });

    it("🔴 `GET …/contenu` refuses a BEARER token — MUTATION no. 2, second direction", async () => {
        // 🔴 The symmetrical half of the guard: a human never takes the
        // agent's path. If either one is relaxed, the two identities
        // become interchangeable again.
        const url = await servir('inst-contenu-jeton-humain');
        const r = await fetch(`${url}/televersement/t-1/contenu`, { headers: withIt(jetonDe('u-1')) });
        expect([r.status, await r.json()]).toEqual([403, { refus: 'jeton-utilisateur' }]);
    });

    it('refuses a malformed body, and says so', async () => {
        const url = await servir('inst-forme');
        const entetes = withIt(jetonDe('u-1'));
        for (const corps of ['not json', '{}', '{"vm":"v-1"}', '{"vm":"","televersement":"t"}', '[]']) {
            const r = await fetch(`${url}/installation`, { method: 'POST', headers: entetes, body: corps });
            expect([corps, r.status, await r.json()]).toEqual([corps, 400, { refus: 'forme' }]);
        }
    });

    it("🔴 a FOREIGN VM answers EXACTLY like an UNKNOWN VM", async () => {
        // 🔴 Both bodies are compared CHARACTER FOR CHARACTER: a test that
        // only read `404` would be satisfied by the GENERIC 404 of
        // `serveur.ts`, which is not this one.
        const url = await servir('inst-vm-etrangere');
        await poserVmVivante(m!.base, 'v-1', 'proprietaire@exemple.test', 'PREFIXE-A');
        const autre = await createUser(m!.base, 'autre@exemple.test', 'x', MS);
        const tel = await poserTeleversement(m!.base, autre, 'setup.exe', Buffer.from('abc'));
        const opts = (vm: string) => ({
            method: 'POST',
            headers: withIt(jetonDe(autre)),
            body: JSON.stringify({ vm, televersement: tel.id }),
        });
        const etrangere = await fetch(`${url}/installation`, opts('v-1'));
        const inconnue = await fetch(`${url}/installation`, opts('jamais-vue'));
        expect([etrangere.status, await etrangere.text()])
            .toEqual([inconnue.status, await inconnue.text()]);
        expect(etrangere.status).toBe(404);
    });

    it("🔴 a foreign UPLOAD answers EXACTLY like an unknown one, and the body says nothing of the real case", async () => {
        const traces: string[] = [];
        vi.spyOn(console, 'warn').mockImplementation((l: string) => void traces.push(l));
        const url = await servir('inst-tel-etranger');
        const u = await poserVmVivante(m!.base, 'v-1', 'moi@exemple.test', 'PREFIXE-A');
        const autre = await createUser(m!.base, 'autre@exemple.test', 'x', MS);
        const aLautre = await poserTeleversement(m!.base, autre, 'setup.exe', Buffer.from('abc'));
        const opts = (tid: string) => ({
            method: 'POST',
            headers: withIt(jetonDe(u)),
            body: JSON.stringify({ vm: 'v-1', televersement: tid }),
        });
        const etranger = await fetch(`${url}/installation`, opts(aLautre.id));
        const corpsEtranger = await etranger.text();
        const inconnu = await fetch(`${url}/installation`, opts('jamais-vu'));
        expect([etranger.status, corpsEtranger]).toEqual([inconnu.status, await inconnu.text()]);
        expect(corpsEtranger).toBe(JSON.stringify({ refus: 'televersement-inconnu' }));
        // 🔴 THE COUNTERPART: the log line, for its part, DISTINGUISHES — otherwise
        // uniformising would cost the operator all their diagnosis.
        const tout = traces.join(' | ');
        expect(tout).toContain('cas=etrangere');
        expect(tout).toContain('cas=inconnue');
        expect(corpsEtranger).not.toContain('etrangere');
    });

    it('🔴 an UNSEALED upload is refused UPSTREAM, and NO row is written — MUTATION no. 3', async () => {
        // 🔴 A REFUSAL IN THE RIGHT PLACE IS WORTH MORE THAN A REFUSAL AT THE RIGHT TIME:
        // without it, the order would leave, the agent would pull a PARTIAL file and
        // would only learn of its wrong hash after writing it in full.
        // ⚠️ The test does not only assert the code: it checks that NO
        // installation was created, which a `409` returned AFTER the write
        // would let through.
        const url = await servir('inst-non-scelle');
        const u = await poserVmVivante(m!.base, 'v-1', 'moi@exemple.test', 'PREFIXE-A');
        const tel = await poserTeleversement(m!.base, u, 'setup.exe', Buffer.from('abcdefghij'), { scelle: false });
        const r = await fetch(`${url}/installation`, {
            method: 'POST',
            headers: withIt(jetonDe(u)),
            body: JSON.stringify({ vm: 'v-1', televersement: tel.id }),
        });
        expect([r.status, await r.json()]).toEqual([409, { refus: 'non-scelle' }]);
        const lignes = await m!.base.interroger('SELECT id FROM installation', []);
        expect(lignes).toEqual([]);
    });

    it("refuses an extension the agent will not run, rather than an 800 MB round trip", async () => {
        const url = await servir('inst-extension');
        const u = await poserVmVivante(m!.base, 'v-1', 'moi@exemple.test', 'PREFIXE-A');
        for (const nom of ['setup.bat', 'setup', 'setup.exe.txt', '.exe']) {
            const tel = await poserTeleversement(m!.base, u, nom, Buffer.from('abc'));
            const r = await fetch(`${url}/installation`, {
                method: 'POST',
                headers: withIt(jetonDe(u)),
                body: JSON.stringify({ vm: 'v-1', televersement: tel.id }),
            });
            expect([nom, r.status, await r.json()]).toEqual([nom, 400, { refus: 'extension' }]);
        }
        // ⚠️ AND THE WITNESS, without which this test would hold for a route that refuses
        // EVERYTHING: `.MSI` in upper case passes, because Windows is case
        // insensitive.
        const bon = await poserTeleversement(m!.base, u, 'SETUP.MSI', Buffer.from('abc'));
        const r = await fetch(`${url}/installation`, {
            method: 'POST',
            headers: withIt(jetonDe(u)),
            body: JSON.stringify({ vm: 'v-1', televersement: bon.id }),
        });
        expect(r.status).toBe(201);
    });

    it('🔴 an UNREACHABLE VM returns 503, never 201 on a row nobody will receive', async () => {
        // 🔴 Writing the row silently would make the hub display an installation
        // "requested" that nobody will receive — nothing anywhere would
        // contradict it. ⚠️ The transition is BESIEGED: the same setup returns 201
        // at `vu_a + SEUIL` and 503 one millisecond later.
        const url = await servir('inst-injoignable');
        const u = await poserVmVivante(m!.base, 'v-1', 'moi@exemple.test', 'PREFIXE-A');
        const tel = await poserTeleversement(m!.base, u, 'setup.exe', Buffer.from('abc'));
        maintenant = MS + 90_000 + 1;
        const mort = await fetch(`${url}/installation`, CORPS(u, 'v-1', tel.id));
        expect([mort.status, await mort.json()]).toEqual([503, { refus: 'agent-injoignable' }]);
        maintenant = MS + 90_000;
        const vif = await fetch(`${url}/installation`, CORPS(u, 'v-1', tel.id));
        expect(vif.status).toBe(201);
    });

    it("an accepted order returns 201 and its identifier, and the row is born `en_attente`", async () => {
        // ⚠️ `en_attente` IS NOT DECORATIVE: it is this state, and it alone, that
        // re-emission at enrolment pushes back to the agent.
        const url = await servir('inst-ordre-ok');
        const u = await poserVmVivante(m!.base, 'v-1', 'moi@exemple.test', 'PREFIXE-A');
        const tel = await poserTeleversement(m!.base, u, 'setup.exe', Buffer.from('abc'));
        const r = await fetch(`${url}/installation`, CORPS(u, 'v-1', tel.id));
        expect(r.status).toBe(201);
        const { id } = (await r.json()) as { id: string };
        const lignes = await m!.base.interroger<{ id: string; etat: string; vm_id: string }>(
            'SELECT id, etat, vm_id FROM installation WHERE id = ?', [id],
        );
        expect(lignes).toEqual([{ id, etat: 'en_attente', vm_id: 'v-1' }]);
    });

    it("`GET /installation/:id` returns the state, the outcome, the reason and the log tail", async () => {
        const url = await servir('inst-etat');
        const u = await poserVmVivante(m!.base, 'v-1', 'moi@exemple.test', 'PREFIXE-A');
        const tel = await poserTeleversement(m!.base, u, 'setup.exe', Buffer.from('abc'));
        const inst = await createInstallation(m!.base, { vmId: 'v-1', televersementId: tel.id }, MS);
        await terminer(m!.base, inst.id, {
            issue: 'refusee', motif: 'elevation-requise', codeSortie: 740,
            journal: 'the tail', journalTronque: true,
        }, MS + 5);
        const r = await fetch(`${url}/installation/${inst.id}`, { headers: withIt(jetonDe(u)) });
        expect(r.status).toBe(200);
        const vue = (await r.json()) as Record<string, unknown>;
        expect(vue).toMatchObject({
            id: inst.id, vm: 'v-1', televersement: tel.id, etat: 'terminee',
            issue: 'refusee', motif: 'elevation-requise', code_sortie: 740, journal: 'the tail',
        });
        // ⚠️ A BOOLEAN ON THE WIRE, never SQLite's `0`/`1`: the hub does not have
        // to know a storage convention.
        expect(vue.journal_tronque).toBe(true);
    });

    it("🔴 a FOREIGN installation answers EXACTLY like an UNKNOWN one", async () => {
        // 🔴 And the reason is `installation-inconnue`, NEVER `vm-inconnue`:
        // returning the VM's reason WOULD SAY that the installation, for its part, exists.
        const url = await servir('inst-etat-etranger');
        const u = await poserVmVivante(m!.base, 'v-1', 'moi@exemple.test', 'PREFIXE-A');
        const autre = await createUser(m!.base, 'autre@exemple.test', 'x', MS);
        const tel = await poserTeleversement(m!.base, u, 'setup.exe', Buffer.from('abc'));
        const inst = await createInstallation(m!.base, { vmId: 'v-1', televersementId: tel.id }, MS);
        const entetes = withIt(jetonDe(autre));
        const etrangere = await fetch(`${url}/installation/${inst.id}`, { headers: entetes });
        const inconnue = await fetch(`${url}/installation/jamais-vue`, { headers: entetes });
        const corps = await etrangere.text();
        expect([etrangere.status, corps]).toEqual([inconnue.status, await inconnue.text()]);
        expect(corps).toBe(JSON.stringify({ refus: 'installation-inconnue' }));
    });

    it('🔴 AN AGENT OF VM `B` DOES NOT GET THE INSTALLER MEANT FOR `A` — MUTATION no. 1', async () => {
        // 🔴 IT IS THE MOST IMPORTANT MUTATION OF THE SUB-BLOCK. Without the
        // VM comparison, any enrolled VM would download
        // the installer of any other — the content a user
        // dropped for THEIR machine and for it alone. The authorisation is not
        // "a valid agent", it is "THAT agent".
        //
        // 🔴 THE WITNESS IS IN THE SAME `it()`, AND IT IS MANDATORY: without it,
        // the refusal assertion would hold for a route that NEVER serves anything.
        const url = await servir('inst-contenu-vm-etrangere');
        const a = await poserVmVivante(m!.base, 'v-a', 'a@exemple.test', 'PREFIXE-A');
        await poserVmVivante(m!.base, 'v-b', 'b@exemple.test', 'PREFIXE-B');
        const tel = await poserTeleversement(m!.base, a, 'setup.exe', randomBytes(33));
        await createInstallation(m!.base, { vmId: 'v-a', televersementId: tel.id }, MS);

        const deB = await fetch(`${url}/televersement/${tel.id}/contenu`, {
            headers: withIt(jetonDe('PREFIXE-B', 'agent')),
        });
        // ⚠️ COMPARED WITH THE REFUSAL OF A TRULY UNKNOWN UPLOAD: the refusal must
        // be INDISTINGUISHABLE, otherwise `B` learns that this content exists.
        const inconnu = await fetch(`${url}/televersement/jamais-vu/contenu`, {
            headers: withIt(jetonDe('PREFIXE-B', 'agent')),
        });
        expect([deB.status, await deB.text()]).toEqual([inconnu.status, await inconnu.text()]);
        expect(deB.status).toBe(404);

        const deA = await fetch(`${url}/televersement/${tel.id}/contenu`, {
            headers: withIt(jetonDe('PREFIXE-A', 'agent')),
        });
        expect(deA.status).toBe(200);
        expect(Buffer.from(await deA.arrayBuffer())).toEqual(tel.octets);
    });

    it("serves the bytes AS A STREAM, identical, with its length and without a file name", async () => {
        // 🔴 THE CHUNKS ARE NEVER ASSEMBLED: the body is the
        // CONCATENATION of five disk files, and byte-for-byte equality
        // is what tests it. ⚠️ NO `Content-Disposition`: the agent
        // knows the name, it received it in the order.
        const url = await servir('inst-contenu-flux');
        const a = await poserVmVivante(m!.base, 'v-a', 'a@exemple.test', 'PREFIXE-A');
        const octets = randomBytes(31);
        const tel = await poserTeleversement(m!.base, a, 'setup.exe', octets, { chunkSize: 7 });
        await createInstallation(m!.base, { vmId: 'v-a', televersementId: tel.id }, MS);
        expect(tranches!.lister(tel.id).map((t) => t.n)).toEqual([0, 1, 2, 3, 4]);

        const r = await fetch(`${url}/televersement/${tel.id}/contenu`, {
            headers: withIt(jetonDe('PREFIXE-A', 'agent')),
        });
        expect(r.status).toBe(200);
        expect(r.headers.get('content-type')).toBe('application/octet-stream');
        expect(r.headers.get('content-length')).toBe(String(octets.length));
        expect(r.headers.get('content-disposition')).toBeNull();
        expect(r.headers.get('x-content-type-options')).toBe('nosniff');
        expect(Buffer.from(await r.arrayBuffer())).toEqual(octets);
    });

    it("🔴 an UNSEALED content is refused 409 — and the FOREIGN agent reads 404", async () => {
        // 🔴 THE ORDER OF THE TWO GUARDS MATTERS: answering `409` to an agent without
        // rights WOULD TELL IT that this upload exists. The authorisation therefore goes
        // BEFORE the state, and this test asserts it by contrasting the two responses.
        const url = await servir('inst-contenu-non-scelle');
        const a = await poserVmVivante(m!.base, 'v-a', 'a@exemple.test', 'PREFIXE-A');
        await poserVmVivante(m!.base, 'v-b', 'b@exemple.test', 'PREFIXE-B');
        const tel = await poserTeleversement(m!.base, a, 'setup.exe', Buffer.from('abcdefghij'), { scelle: false });
        await createInstallation(m!.base, { vmId: 'v-a', televersementId: tel.id }, MS);
        const deA = await fetch(`${url}/televersement/${tel.id}/contenu`, {
            headers: withIt(jetonDe('PREFIXE-A', 'agent')),
        });
        expect([deA.status, await deA.json()]).toEqual([409, { refus: 'non-scelle' }]);
        const deB = await fetch(`${url}/televersement/${tel.id}/contenu`, {
            headers: withIt(jetonDe('PREFIXE-B', 'agent')),
        });
        expect([deB.status, await deB.json()]).toEqual([404, { refus: 'televersement-inconnu' }]);
    });

    it("🔴 `en_cours` stays served — it is the RESUMPTION —, `terminee` no longer is", async () => {
        // 🔴 Accepting only `en_attente` would make any transfer resume
        // impossible without any trace saying so: the agent that reported a
        // progress then lost its connection could no longer pull anything.
        const url = await servir('inst-contenu-etats');
        const a = await poserVmVivante(m!.base, 'v-a', 'a@exemple.test', 'PREFIXE-A');
        const tel = await poserTeleversement(m!.base, a, 'setup.exe', Buffer.from('abc'));
        const inst = await createInstallation(m!.base, { vmId: 'v-a', televersementId: tel.id }, MS);
        const entetes = withIt(jetonDe('PREFIXE-A', 'agent'));
        await avancer(m!.base, inst.id, { phase: 'transfert', octetsFaits: 1, octetsTotal: 3, ecouleMs: 1 }, MS);
        expect((await fetch(`${url}/televersement/${tel.id}/contenu`, { headers: entetes })).status).toBe(200);
        await terminer(m!.base, inst.id, {
            issue: 'reussie', motif: null, codeSortie: 0, journal: '', journalTronque: false,
        }, MS + 1);
        const apres = await fetch(`${url}/televersement/${tel.id}/contenu`, { headers: entetes });
        expect([apres.status, await apres.json()]).toEqual([404, { refus: 'televersement-inconnu' }]);
    });

    it("a prefix WITHOUT enrolment is refused like everything else, and the trace names it", async () => {
        // ⚠️ The token is perfectly valid: it is the VM it designates that
        // no longer exists. The response does not say so; the log does.
        const traces: string[] = [];
        vi.spyOn(console, 'warn').mockImplementation((l: string) => void traces.push(l));
        const url = await servir('inst-contenu-sans-enrolement');
        const a = await poserVmVivante(m!.base, 'v-a', 'a@exemple.test', 'PREFIXE-A');
        const tel = await poserTeleversement(m!.base, a, 'setup.exe', Buffer.from('abc'));
        await createInstallation(m!.base, { vmId: 'v-a', televersementId: tel.id }, MS);
        const r = await fetch(`${url}/televersement/${tel.id}/contenu`, {
            headers: withIt(jetonDe('PREFIXE-JAMAIS-ENROLE', 'agent')),
        });
        expect([r.status, await r.json()]).toEqual([404, { refus: 'televersement-inconnu' }]);
        expect(traces.join(' | ')).toContain('cas=prefixe-sans-enrolement');
    });
});
