// Orchestrating an installer's upload, browser side:
// fingerprint, create, upload the MISSING slices, seal.
//
// 🔴 NO DOM, AND NO IMPLICIT DEPENDENCY. `fetch`, the clock and
// the `AbortSignal` are PARAMETERS — none is read here, neither at load nor
// at call time. That is what makes the sequence testable on the host without a
// browser, and above all what lets the acceptance driver (D14) execute THE
// PRODUCT'S CODE rather than a `curl` reimplementation that would only test
// itself.
//
// 🔴 THE SLICE ARITHMETIC AND THE DIGEST COME FROM `proto/ts/`, because
// the platform uses them TOO: two divergent splittings would produce
// a sealing that refuses without anyone knowing which of the two ends is wrong (D6).
//
// ⚠️ AN EXPECTED REFUSAL IS AN `Issue`, NEVER AN EXCEPTION — the arbitration of
// `plateforme/src/orchestration/refus.ts`. An ENVIRONMENT failure (the `fetch`
// that rejects outside an interruption) propagates as is: disguising it as a refusal
// would pass it off as a protocol decision.
import { plan, verdict, type Tranche } from '../../../proto/ts/tranches';
import { Sha256 } from '../../../proto/ts/sha256';
/* ── THE DEPENDENCIES, ALL INJECTED ────────────────────────────────── */

/// The response shape this module needs, and nothing more. DECLARED
/// rather than borrowed from `Response`: a fake `fetch` has no chance of
/// satisfying its thirty members, and requiring it would make this file untestable.
/// That the REAL `fetch` satisfies `Fetch` is checked by typing, in the test.
export interface ReponseHttp {
    ok: boolean;
    status: number;
    json(): Promise<unknown>;
}

/// What this module passes to `fetch` — a strict subset of `RequestInit`.
export interface InitHttp {
    method?: string;
    headers?: Record<string, string>;
    /// 🔵 `BufferSource` AND NOT `Uint8Array`: the latter now means
    /// `Uint8Array<ArrayBufferLike>` — possibly backed by a `SharedArrayBuffer` —
    /// and is NOT a `BodyInit`, so that the real `fetch` stopped satisfying
    /// `Fetch`. Found by the test's shape check, AT COMPILE TIME.
    body?: string | BufferSource;
    signal?: AbortSignal;
}
export type Fetch = (url: string, init?: InitHttp) => Promise<ReponseHttp>;
export type Phase = 'empreinte' | 'transfert' | 'scellement';
export interface Progression {
    phase: Phase;
    /// ⚠️ IN THE `scellement` PHASE, `octets` EQUALS `total`: the platform rereads everything as a
    /// stream and announces nothing along the way — a frozen progress is not a blockage.
    octets: number;
    total: number;
}
export interface DepsTeleversement {
    /// The platform's origin, WITHOUT a trailing slash.
    base: string;
    /// The bearer token, as `client/src/jeton.ts` returns it.
    jeton: string;
    fetch: Fetch;
    /// The clock, in milliseconds. It PACES the progress, and that is its only
    /// use: without it, fingerprinting 800 MB would emit thousands of events.
    maintenant: () => number;
    signal?: AbortSignal;
    progression?: (p: Progression) => void;
    /// The identifier of an upload to RESUME. Absent, we create.
    reprise?: string;
}
/* ── LES ISSUES ───────────────────────────────────────────────────────── */

export type MotifLocal = 'fichier-different' | 'tranches-incoherentes' | 'etat-illisible' | 'interrompu';
export type Etape = 'creation' | 'etat' | 'tranche' | 'scellement';
/// 🔴 THE SERVICE'S REASON IS A `string`, NOT A UNION, AND IT IS DELIBERATE.
/// Its vocabulary belongs to it and lives in `plateforme/`, which `client/`
/// cannot import. Copying it into a union would be EXACTLY the defect
/// `client/src/connexion.ts` declares about `aucune-vm`: a copy no type (policy: allow-fr, wire refusal code)
/// confronts with its source, silently wrong on renaming.
export type Refus =
    | { source: 'client'; motif: MotifLocal; detail: string }
    | { source: 'service'; etape: Etape; statut: number; motif: string };

/// 🔴 A DISCRIMINATED UNION, NEITHER A BOOLEAN NOR AN EXCEPTION. The compiler
/// forbids reading `id` without having looked at `etat`: a caller cannot
/// take a refusal for a success by forgetting an `if`, which an ignored boolean
/// would allow — the argument of `prefixe.ts::poserPrefixe`, held here by typing.
export type Issue =
    | { etat: 'scelle'; id: string; size: number; sha256: string; deposees: number[] }
    | { etat: 'refus'; refus: Refus; id?: string };

/// The size of a chunk read to fingerprint. NOT CALIBRATED. ⚠️ NO RELATION
/// TO `taille_tranche`, which comes from the service and is not yet known when (policy: allow-fr, wire key of the upload API)
/// we fingerprint: giving them the same value would suggest a derivation,
/// whereas they would be recalibrated separately. It trades tab
/// memory for the time the thread is blocked hashing
/// (≈ 54 ms per chunk, at the measured throughput of 74.7 MB/s of `proto/ts/sha256.ts`).
const OCTETS_LECTURE = 4 * 1024 * 1024;
/// The progress cadence. NOT CALIBRATED: it is a display comfort.
const PERIODE_PROGRESSION_MS = 250;
/* ── THE SEQUENCE ──────────────────────────────────────────────────────── */

function entetes(deps: DepsTeleversement, type?: string): Record<string, string> {
    const en: Record<string, string> = { authorization: `Bearer ${deps.jeton}` };
    if (type !== undefined) en['content-type'] = type;
    return en;
}

/// Calls the service. Returns `null` — and only `null` — WHEN THE SIGNAL SAYS
/// WE WERE INTERRUPTED: an interruption is a user gesture, hence
/// a refusal, never a failure. The condition bears on `signal.aborted`, not on
/// the shape of the exception: otherwise a network failure would disguise itself as a cancellation.
async function appeler(deps: DepsTeleversement, url: string, init: InitHttp): Promise<ReponseHttp | null> {
    try {
        return await deps.fetch(url, { ...init, signal: deps.signal });
    } catch (cause) {
        if (deps.signal?.aborted === true) return null;
        throw cause;
    }
}

async function motifDuService(reponse: ReponseHttp): Promise<string> {
    const corps = (await reponse.json().catch(() => undefined)) as { refus?: unknown } | undefined;
    return typeof corps?.refus === 'string' ? corps.refus : `http-${reponse.status}`;
}
type Emettre = (p: Phase, octets: number, total: number, force: boolean) => void;
/// The progress pacer — the clock's only reader.
function cadenceur(deps: DepsTeleversement): Emettre {
    let last = Number.NEGATIVE_INFINITY;
    return (phase, octets, total, force) => {
        if (deps.progression === undefined) return;
        const instant = deps.maintenant();
        // A phase change ALWAYS goes through: otherwise a small file
        // would only display one phase out of three, and the complete read pass
        // would stay invisible.
        if (!force && instant - last < PERIODE_PROGRESSION_MS) return;
        last = instant;
        deps.progression({ phase, octets, total });
    };
}

/// The fingerprint of the whole file. Returns `null` if the signal cut it.
///
/// 🔴 `File.slice` RATHER THAN THE WHOLE FILE: we never hold more
/// than `OCTETS_LECTURE` in memory, whereas `crypto.subtle.digest` would require
/// holding the 800 MB — the API gap `proto/ts/sha256.ts` works around.
/// ⚠️ THE `await` OF `arrayBuffer()` IS THE YIELD, and one must be exact about
/// what it buys: the tab regains control BETWEEN two chunks, it stays blocked
/// WHILE hashing each one — a series of short pauses, not a silent
/// freeze, and the phase is displayed.
async function empreindre(file: File, deps: DepsTeleversement, emettre: Emettre): Promise<string | null> {
    const condensat = new Sha256();
    for (let debut = 0; debut < file.size; debut += OCTETS_LECTURE) {
        if (deps.signal?.aborted === true) return null;
        const fin = Math.min(debut + OCTETS_LECTURE, file.size);
        condensat.absorber(new Uint8Array(await file.slice(debut, fin).arrayBuffer()));
        emettre('empreinte', fin, file.size, false);
    }
    // An empty file does not enter the loop and returns the fingerprint of the empty message:
    // the right value, not a special case added by hand.
    return condensat.terminer();
}

/// Puts the slices announced by the service into the shape `verdict` expects.
///
/// 🔴 ONLY ONE SHAPE IS ACCEPTED: `{n, octets}`, the one the route returns.
///
/// ⚠️ IT TOLERATED TWO while being written — a BARE rank was accepted, and
/// its size was then **borrowed from the local plan**. The route being settled
/// (`routes-televersement.ts` returns the store's LISTING, hence
/// `{n, octets}`), the tolerance is removed, and not only because it had
/// become dead: **it made the service believed about a size it had
/// never announced.** A present rank whose size had drifted would have passed
/// as compliant, and sealing would have refused later, elsewhere, without
/// anything linking the two — whereas `verdict` can say `incoherentes`.
///
/// ⚠️ ANY OTHER SHAPE IS A TYPED REFUSAL, NEVER A SILENCE: it is what
/// distinguishes "the service speaks another version" from "there is nothing to
/// resume", and the two call for opposite gestures.
function normaliserPresentes(brut: unknown): Tranche[] | null {
    if (!Array.isArray(brut)) return null;
    const sortie: Tranche[] = [];
    for (const entree of brut) {
        const t = entree as { n?: unknown; octets?: unknown };
        if (typeof t?.n !== 'number' || typeof t?.octets !== 'number') return null;
        sortie.push({ n: t.n, octets: t.octets });
    }
    return sortie;
}

export async function televerser(file: File, deps: DepsTeleversement): Promise<Issue> {
    const size = file.size;
    const emettre = cadenceur(deps);
    // 🔴 THE IDENTIFIER IS CAPTURED, NOT PASSED AT EACH REFUSAL: that is what guarantees
    // no refusal forgets it — a caller losing the `id` on a refused sealing
    // could no longer resume, and would upload everything again. `''` = not created yet.
    let id = deps.reprise ?? '';
    const refuse = (refus: Refus): Issue => ({ etat: 'refus', refus, id: id === '' ? undefined : id });
    const nonLocal = (motif: MotifLocal, detail: string): Issue => refuse({ source: 'client', motif, detail });
    const nonService = async (etape: Etape, r: ReponseHttp): Promise<Issue> =>
        refuse({ source: 'service', etape, statut: r.status, motif: await motifDuService(r) });

    // ① REREAD THE STATE, AND COMPARE THE SIZE BEFORE FINGERPRINTING (D5). The order is
    // a real gain: a manifestly different file is refused without paying the
    // complete read pass — eleven seconds for 800 MB — nor uploading a byte.
    let etat: Record<string, unknown> | undefined;
    if (deps.reprise !== undefined) {
        const url = `${deps.base}/televersement/${encodeURIComponent(id)}`;
        const r = await appeler(deps, url, { method: 'GET', headers: entetes(deps) });
        if (r === null) return nonLocal('interrompu', 'during the re-read');
        if (!r.ok) return await nonService('etat', r);
        etat = (await r.json().catch(() => undefined)) as Record<string, unknown> | undefined;
        // 🔴 A STATE THAT DOES NOT ANNOUNCE `{size, sha256}` MAKES THE RESUMPTION REFUSED, it
        // does not make it resume blindly: without these two values, nothing says
        // the re-chosen file is THE SAME, the slices of two files would
        // mix, and sealing would fail without anything saying why.
        if (typeof etat?.taille !== 'number' || typeof etat.sha256 !== 'string') { // policy: allow-fr - wire key of the upload API
            return nonLocal('etat-illisible', 'state without size or fingerprint: resuming cannot be checked');
        }
        if (etat.taille !== size) { // policy: allow-fr - wire key of the upload API
            return nonLocal('fichier-different', `size ${size} versus ${etat.taille} at upload time`);
        }
    }

    // ② THE FINGERPRINT, AT CREATION AND NOT AT SEALING (D5): it is what
    // makes resumption safe, and the only value the three stages compare.
    emettre('empreinte', 0, size, true);
    const sha256 = await empreindre(file, deps, emettre);
    if (sha256 === null) return nonLocal('interrompu', "during fingerprinting");
    emettre('empreinte', size, size, true);
    if (etat !== undefined && etat.sha256 !== sha256) {
        return nonLocal('fichier-different', `fingerprint ${sha256} versus ${String(etat.sha256)} retained`);
    }

    // ③ CREATE, IF WE ARE NOT RESUMING.
    let chunkSize: unknown;
    let presentesBrut: unknown;
    if (etat !== undefined) {
        chunkSize = etat.taille_tranche; // policy: allow-fr - wire key of the upload API
        presentesBrut = etat.tranches_presentes;
    } else {
        const r = await appeler(deps, `${deps.base}/televersement`, {
            method: 'POST',
            headers: entetes(deps, 'application/json'),
            body: JSON.stringify({ nom: file.name, taille: size, sha256 }), // policy: allow-fr - wire key of the upload API
        });
        if (r === null) return nonLocal('interrompu', 'during the creation');
        if (!r.ok) return await nonService('creation', r);
        const c = (await r.json().catch(() => undefined)) as Record<string, unknown> | undefined;
        if (typeof c?.id !== 'string') return nonLocal('etat-illisible', 'creation without an identifier');
        id = c.id;
        chunkSize = c.taille_tranche; // policy: allow-fr - wire key of the upload API
        presentesBrut = c.tranches_presentes;
    }

    // ④ THE SPLITTING. `plan` and `verdict` THROW on an absurd contract — their guard
    // targets a programming defect. Yet `taille_tranche` comes from the WIRE: validating it here (policy: allow-fr, wire key of the upload API)
    // prevents a deranged answer from bringing everything down through an exception.
    if (!Number.isInteger(chunkSize) || (chunkSize as number) <= 0) {
        return nonLocal('etat-illisible', `taille_tranche ${String(chunkSize)}`);
    }
    const pas = chunkSize as number;
    const attendu = plan(size, pas);
    const presentes = normaliserPresentes(presentesBrut);
    if (presentes === null) return nonLocal('etat-illisible', 'tranches_presentes of unknown shape');

    // 🔴 `incoherentes` IS NOT FILLED IN AGAIN: the two ends no longer agree
    // on the splitting, and uploading again would return the same thing indefinitely. We
    // refuse, we name the ranks, the caller decides.
    const v = verdict(size, pas, presentes);
    if (v.etat === 'incoherentes') return nonLocal('tranches-incoherentes', `rangs ${v.n.join(', ')}`);
    const aDeposer = v.etat === 'manquantes' ? v.n : [];

    // ⑤ UPLOADING ONLY THE MISSING ONES. Starting from zero would pass a test that
    // only looks at the result: it is the spec's criterion ③, and that is
    // why the test COUNTS THE BYTES SENT.
    let envoyes = size - aDeposer.reduce((s, n) => s + attendu[n].octets, 0);
    emettre('transfert', envoyes, size, true);
    for (const n of aDeposer) {
        if (deps.signal?.aborted === true) return nonLocal('interrompu', `before chunk ${n}`);
        const debut = n * pas;
        const corps = new Uint8Array(await file.slice(debut, debut + attendu[n].octets).arrayBuffer());
        const url = `${deps.base}/televersement/${encodeURIComponent(id)}/tranche/${n}`;
        const r = await appeler(deps, url, {
            method: 'PUT',
            headers: entetes(deps, 'application/octet-stream'),
            body: corps,
        });
        if (r === null) return nonLocal('interrompu', `during chunk ${n}`);
        if (!r.ok) return await nonService('tranche', r);
        envoyes += attendu[n].octets;
        emettre('transfert', envoyes, size, false);
    }

    // ⑥ THE SEALING. The platform RECOMPUTES the fingerprint and refuses if it differs
    // (D4). This refusal propagates as is: neither swallowed nor retried — a fingerprint that
    // diverges does not converge, and a retry loop would upload endlessly.
    emettre('scellement', size, size, true);
    const url = `${deps.base}/televersement/${encodeURIComponent(id)}/sceller`;
    const r = await appeler(deps, url, { method: 'POST', headers: entetes(deps) });
    if (r === null) return nonLocal('interrompu', 'during sealing');
    if (!r.ok) return await nonService('scellement', r);
    return { etat: 'scelle', id, size, sha256, deposees: aDeposer };
}
