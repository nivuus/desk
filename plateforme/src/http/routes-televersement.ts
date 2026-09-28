// The FOUR routes of the upload of an installer: `POST /televersement`
// (declare), `PUT /televersement/:id/tranche/:n` (upload),
// `GET /televersement/:id` (read again to resume) and
// `POST /televersement/:id/sceller` (freeze the content).
//
// 🔴 THE CONTRACT IS THAT OF THE NINE EXISTING ROUTERS: `Promise<boolean>`,
// `true` = served, `false` = not my path. The generic 404 of
// `http/serveur.ts` then answers ALONE, and it is not duplicated here.
//
// ⚠️ « THEN ANSWERS ALONE » IS NO LONGER UNCONDITIONALLY TRUE SINCE 22 AUGUST 2026, and
// the sentence is left as is because it stays right in the nginx
// deployment: when `PLATEFORME_PAGE` is armed, a TENTH router — the page
// server — is chained AFTER all the others, and it resolves any
// path. On a `GET`/`HEAD`, it is IT that answers `200 text/html` to the `false`
// returned here; outside `GET`/`HEAD` it steps aside, and the generic 404 takes
// over. See `http/chaine.ts`, which carries the count and the rule.
//
// 🔴 THIS MODULE DECIDES NEITHER THE SPLITTING, NOR THE WRITING, NOR THE BEARER:
// `proto/ts/tranches.ts` (PURE, and ALSO imported by the browser),
// `apps/magasin-tranches.ts`, `http/porteur.ts`. Copying one of them here would make it
// a second source of truth — « a sealing that refuses without anyone knowing
// which of the two ends is wrong ».
//
// 🔴 THE REFUSAL VOCABULARY IS LOCAL, AND DOES NOT JOIN
// `orchestration/refus.ts`, which belongs to VM ORCHESTRATION — its six
// reasons all talk about a VM, an agent or an assignment. `routes-icone.ts`
// settled it the same way.
//
// ✅ THE `PUT` IS REACHABLE FROM A CROSS-ORIGIN BROWSER, AND IT WAS NOT
// WHEN THIS FILE WAS WRITTEN. `http/cors.ts` then only announced
// `Access-Control-Allow-Methods: 'GET, POST, OPTIONS'`: the uploader BEING the
// browser, and a `PUT` carrying `Authorization` being NON SIMPLE, it asked for
// the preflight, did not find `PUT` there, and GAVE UP WITHOUT SENDING THE REAL
// REQUEST. The value now carries `PUT`, and `cors.test.ts` asserts it
// by name — red seen, `2 failed | 6 passed`.
//
// ⚠️ WHAT REMAINS ENTIRELY TRUE, AND MUST NOT BE READ AS CLOSED:
// **no Node test can see this class of defect**, since Node `fetch`
// does not apply the origin policy. It is the class that the browser
// corroboration of P4 found and declared WITHOUT AN AUTOMATIC GUARD; it
// bit twice in P4 and a third time here, on the METHOD. The only
// guard is an assertion on the VALUE, in `cors.test.ts`. No effect with a
// single origin (`deploiement` profile of P5); it bit in development, `vite`
// serving on 5173 and the service on 8080 — so exactly where it gets tuned.
//
// ⚠️ `routes-auth.ts::lireCorps` IS NEITHER RAISED NOR REUSED, AND BOTH
// HALVES MATTER. It accumulates into a UTF-8 STRING: a BINARY body would be
// corrupted there (any invalid byte becomes U+FFFD, and the hash of the
// reassembled file would no longer be its own), and raising its cap would give
// an anonymous peer back the megabyte that this cap takes away. Here the declaration
// has its bounded reader, and the slice goes through AS A STREAM, never through memory.

import { createHash } from 'node:crypto';
import type { IncomingMessage, ServerResponse } from 'node:http';
import type { MagasinTranches } from '../apps/magasin-tranches';
import { identifiantValide, rangValide } from '../apps/magasin-tranches';
import type { Pilote } from '../base/pilote';
import {
    compterEnCours,
    create,
    lireParId,
    sceller,
    type LigneTeleversement,
} from '../depot/televersement';
import { plan, verdict } from '../../../proto/ts/tranches';
import { entetesCors } from './cors';
import { ENTETES_SECURITE } from './entetes';
import { lirePorteur } from './porteur';
import { empreinteValide, METHODE, reconnaitre } from './televersement-regles';

export interface DependancesTeleversement {
    base: Pilote;
    secretJeton: string;
    origineClient?: string;
    /// ⚠️ ONLY THIS ROUTER READS IT — same status as `magasin` for `servirIcone`.
    tranches: MagasinTranches;
    maintenant: () => number;
}

/// The slicing step. ⚠️ **NOT CALIBRATED**, and NO relation to
/// the browser's `OCTETS_LECTURE` (4 MiB) — an equal value would suggest a
/// derivation. 🔴 IT IS WRITTEN TO THE DATABASE AT CREATION, AND THAT
/// COPY IS THE ONE THAT HOLDS AFTERWARDS: changing it would re-slice the uploads
/// ALREADY declared, for which `verdict` would call every slice `incoherentes` —
/// a state that is NOT REPAIRED by uploading again.
export const CHUNK_SIZE = 8 * 1024 * 1024;

/// ⚠️ **NOT CALIBRATED**, AN UPPER BOUND BY EYE: no real installer size was
/// measured to set it. Joins the list kept since `BPP_MIN`.
export const TELEVERSEMENT_MAX_OCTETS = 4 * 1024 * 1024 * 1024;

/// How many UNSEALED uploads a user may have at once.
/// 🔴 IT IS A QUOTA, NOT A BRAKE — the distinction already written by
/// `depot/televersement.ts::compterEnCours`: the brake guards the
/// PRE-AUTHENTICATED doors and counts ATTEMPTS; the four routes here require
/// a valid token, and what protects them is a bound on the DISK.
/// ⚠️ **NOT CALIBRATED.**
export const TELEVERSEMENTS_EN_COURS_MAX = 3;

/// ⚠️ Same number as the cap in `routes-auth.ts` by COINCIDENCE of
/// magnitude, never by derivation: the two would be recalibrated separately.
const CORPS_DECLARATION_MAX_OCTETS = 4 * 1024;

/// What the four handlers share. ⚠️ `cors` is COMPUTED ONCE, at
/// the top: per branch, a forgotten path would answer without an origin header.
interface Contexte {
    rep: ServerResponse;
    deps: DependancesTeleversement;
    cors: Record<string, string> | undefined;
}

/// Answers, and returns `true` — THE ROUTER CONTRACT. 🔴 THE RETURN IS NOT A
/// WRITING CONVENIENCE: it makes it impossible to forget a `return` after having
/// written the response. Without it, a branch that answered then fell through to
/// the rest would write a SECOND response — at best `ERR_HTTP_HEADERS_SENT`, at
/// worst the generic 404 concatenated to a refusal already sent.
function repondre(ctx: Contexte, code: number, corps?: unknown): true {
    ctx.rep.writeHead(code, {
        'content-type': 'application/json; charset=utf-8',
        // ⚠️ UNCONDITIONAL, on EVERY response — refusals included. Spread BEFORE
        // `cors`, OPTIONAL, which must never be able to overwrite them. And the
        // CORS goes ON 401s TOO: a response the browser cannot read
        // shows up as a network failure, not as an invitation to sign in again.
        ...ENTETES_SECURITE,
        ...(ctx.cors ?? {}),
    });
    ctx.rep.end(corps === undefined ? undefined : JSON.stringify(corps));
    return true;
}

/* ── OWNERSHIP ─────────────────────────────────────────────────────── */

/// The refusal, as it goes out on the wire — THE SAME in both cases.
const REFUS_INCONNU = { refus: 'televersement-inconnu' } as const;

/// Reads the row, and returns one ONLY if it belongs to the requester.
///
/// 🔴 SOMEONE ELSE'S UPLOAD IS INDISTINGUISHABLE FROM AN UNKNOWN UPLOAD,
/// AND IT IS A DECISION ALREADY TAKEN, NOT A TRADE-OFF REOPENED HERE: telling
/// them apart would be an ENUMERATION ORACLE. The repository owner
/// settled it for G1 (the `403 vm-etrangere` gave way to the `404 vm-inconnue`);
/// G3 APPLIES it — fourth time, after `routes-auth.ts`, `agents/enrolement.ts`
/// and `routes-applications.ts`.
///
/// 🔴 BOTH BRANCHES RETURN `undefined`, and the refusal has ONE SINGLE emission
/// site: two expressions, even returning the same value, would leave the
/// door open for one of them to change some day. ⚠️ THE LOG LINE
/// IS THE COUNTERPART, and it NEVER REACHES THE RESPONSE; 🔴 placed HERE and
/// not at the three call sites, where it would be FORGETTABLE — and forgetting it
/// would break nothing visible.
async function lireSienne(
    ctx: Contexte,
    id: string,
    userId: string,
): Promise<LigneTeleversement | undefined> {
    const ligne = await lireParId(ctx.deps.base, id);
    if (ligne === undefined) return journaliserLeRefus('inconnu', id, userId);
    if (ligne.utilisateur_id !== userId) {
        return journaliserLeRefus('etranger', id, userId);
    }
    return ligne;
}

/// ⚠️ `cas=` IS A FIELD, NOT A SENTENCE: it is what the operator `grep`s.
/// A test pins it, and ALSO pins that the two cases differ. ⚠️ It returns
/// `undefined` so that tracing and refusing are the SAME gesture.
function journaliserLeRefus(cas: 'inconnu' | 'etranger', id: string, u: string): undefined {
    console.warn(
        `refus d'accès au téléversement ${id} pour l'utilisateur ${u} : cas=${cas} — `
            + `la réponse HTTP, elle, est le même 404 « televersement-inconnu » dans les `
            + `deux cas (décision du propriétaire du dépôt : pas d'oracle d'énumération).`,
    );
    return undefined;
}

/* ── LE ROUTEUR ───────────────────────────────────────────────────────── */

export async function servirTeleversement(
    req: IncomingMessage,
    rep: ServerResponse,
    deps: DependancesTeleversement,
): Promise<boolean> {
    const cible = reconnaitre(new URL(req.url ?? '/', 'http://placeholder').pathname);
    if (cible === undefined) return false;

    const ctx: Contexte = { rep, deps, cors: entetesCors(req.headers.origin, deps.origineClient) };

    // 🔴 THE PREFLIGHT IS SERVED, AND WITHOUT IT NOTHING IS REACHABLE from a
    // browser: the four routes require `Authorization: Bearer`, so the
    // request is NON SIMPLE, and a 404 on the `OPTIONS` would make the
    // browser give up BEFORE the real request. ⚠️ See the header for `PUT`.
    if (req.method === 'OPTIONS') return repondre(ctx, 204);

    // The path EXISTS, it is the method that does not fit: a 404 would send
    // people looking for a missing route.
    if (req.method !== METHODE[cible.quoi]) return repondre(ctx, 405, { refus: 'methode' });

    // 🔴 AUTHENTICATION COMES BEFORE ANY DATABASE, DISK OR
    // BODY READ: refusing afterwards would offer free work to an anonymous peer —
    // and, on the `PUT`, the chance to write megabytes before the refusal.
    const porteur = lirePorteur(req.headers, deps.secretJeton, deps.maintenant());
    if (!porteur.ok) return repondre(ctx, porteur.code, { refus: porteur.motif });
    const user = porteur.userId;

    if (cible.quoi === 'create') return declarer(req, ctx, user);

    // 🔴 THE SHAPE GUARD COMES BEFORE ANY READ. `:id` comes from the NETWORK and
    // becomes a DIRECTORY NAME, where `..` is meaningful; the store THROWS
    // on a malformed identifier, so without it a twisted URL would return 500.
    if (!identifiantValide(cible.id)) return repondre(ctx, 400, { refus: 'identifiant-invalide' });

    const ligne = await lireSienne(ctx, cible.id, user);
    if (ligne === undefined) return repondre(ctx, 404, REFUS_INCONNU);

    if (cible.quoi === 'etat') return repondre(ctx, 200, etatDe(ctx, ligne));
    if (cible.quoi === 'tranche') return deposer(req, ctx, ligne, cible.rang);
    return arreter(ctx, ligne);
}

/* ── ① DECLARE ───────────────────────────────────────────────────────── */

/// 🔴 THE CAP IS CHECKED DURING THE READ, NOT AFTER: accumulating first
/// would let a peer fill memory before the refusal. ⚠️ BYTES ARE
/// COUNTED, NOT CHARACTERS — `routes-auth.ts` measures an ALREADY
/// DECODED string, which undercounts everything that is not ASCII.
async function lireDeclaration(req: IncomingMessage): Promise<string | 'trop-gros'> {
    const morceaux: Buffer[] = [];
    let total = 0;
    for await (const morceau of req) {
        const b = morceau as Buffer;
        total += b.length;
        // 🔴 WE LEAVE THE LOOP, AND WE DO NOT CALL `req.destroy()`. Leaving
        // is enough to stop reading — the async iterator destroys the READABLE
        // side as it closes —, whereas `destroy()` kills the SOCKET, and
        // MEASURED: the client then gets `UND_ERR_SOCKET` instead of the 413 that
        // was just decided. That is the behaviour of `routes-icone.ts`;
        // `routes-auth.ts`, for its part, calls `destroy()`, and its own test admits
        // in so many words that "the connection may be cut before the
        // response". A refusal that cannot be read is not a refusal.
        if (total > CORPS_DECLARATION_MAX_OCTETS) return 'trop-gros';
        morceaux.push(b);
    }
    return Buffer.concat(morceaux).toString('utf8');
}

/// The refusal reason, or nothing. ⚠️ THREE DISTINCT REASONS RATHER THAN A SINGLE
/// `forme`, and no oracle is opened there: they talk about the content the
/// requester HAS JUST SENT. 🔴 `isSafeInteger` AND NOT `isInteger`: beyond
/// 2^53 the plan arithmetic stops being exact and the two ends
/// would diverge SILENTLY — `proto/ts/tranches.ts` declares that bound
/// "named, not guarded", it is guarded HERE, the only place fed by the WIRE.
function motifDeDeclaration(c: Record<string, unknown>): string | undefined {
    if (typeof c.nom !== 'string' || c.nom === '') return 'nom-invalide';
    if (!Number.isSafeInteger(c.taille) || (c.taille as number) < 0) return 'taille-invalide';
    if (typeof c.sha256 !== 'string' || !empreinteValide(c.sha256)) return 'empreinte-invalide';
    return undefined;
}

async function declarer(req: IncomingMessage, ctx: Contexte, user: string): Promise<boolean> {
    const brut = await lireDeclaration(req);
    if (brut === 'trop-gros') return repondre(ctx, 413, { refus: 'corps-trop-grand' });

    let corps: unknown;
    try {
        corps = JSON.parse(brut);
    } catch {
        return repondre(ctx, 400, { refus: 'forme' });
    }
    if (typeof corps !== 'object' || corps === null || Array.isArray(corps)) {
        return repondre(ctx, 400, { refus: 'forme' });
    }

    const champs = corps as Record<string, unknown>;
    const motif = motifDeDeclaration(champs);
    if (motif !== undefined) return repondre(ctx, 400, { refus: motif });

    const size = champs.taille as number;
    if (size > TELEVERSEMENT_MAX_OCTETS) {
        return repondre(ctx, 413, { refus: 'trop-grand', maximum: TELEVERSEMENT_MAX_OCTETS });
    }

    // 🔴 THE QUOTA IS COUNTED BEFORE CREATION: checking afterwards would create the
    // row then remove it, and a failure between the two would leave
    // precisely the one upload too many.
    if ((await compterEnCours(ctx.deps.base, user)) >= TELEVERSEMENTS_EN_COURS_MAX) {
        const refus = { refus: 'trop-de-televersements', maximum: TELEVERSEMENTS_EN_COURS_MAX };
        return repondre(ctx, 429, refus);
    }

    const entree = {
        userId: user,
        nom: champs.nom as string,
        taille: size,
        sha256: champs.sha256 as string,
        chunkSize: CHUNK_SIZE,
    };
    const ligne = await create(ctx.deps.base, entree, ctx.deps.maintenant());

    // 201: the resource IS created, its identifier is in the body. The
    // browser only looks at `r.ok`, which 200 and 201 both satisfy.
    return repondre(ctx, 201, etatDe(ctx, ligne));
}

/* ── ② RELIRE ─────────────────────────────────────────────────────────── */

/// The state of an upload, as the browser reads it to resume.
///
/// 🔴 `tranches_presentes` IS DERIVED FROM THE DISK, NEVER FROM A COLUMN —
/// decision D7 (`depot/televersement.ts`). ⚠️ Derived EVEN at creation, where it
/// is necessarily `[]`: a hardcoded `[]` would make the creation response
/// a SECOND expression of the same thing.
///
/// ⚠️ THE SHAPE IS `{n, octets}[]`, AND NOT `number[]`: the browser accepts
/// both (`normaliserPresentes`), but only the first lets it SEE
/// a slice of the wrong size BEFORE uploading again — with bare ranks it
/// would deduce the sizes from the plan, hence assume correct the ones it should
/// check, and the inconsistency would only show up at sealing.
///
/// ⚠️ `scelle_a` is a DATE or `null` — never `0`, which would read as an
/// epoch of 1970 (same reasoning as `application.disparue_a`).
function etatDe(ctx: Contexte, ligne: LigneTeleversement): unknown {
    return {
        id: ligne.id,
        nom: ligne.nom,
        taille: ligne.taille,
        sha256: ligne.sha256,
        taille_tranche: ligne.taille_tranche,
        scelle_a: ligne.scelle_a,
        tranches_presentes: ctx.deps.tranches.lister(ligne.id),
    };
}

/* ── ③ UPLOAD ───────────────────────────────────────────────────────── */

async function deposer(
    req: IncomingMessage,
    ctx: Contexte,
    ligne: LigneTeleversement,
    rangBrut: string,
): Promise<boolean> {
    // 🔴 THE RANK IS COMPARED TO A RUN OF DIGITS BEFORE BEING CONVERTED:
    // `Number` alone accepts `+1`, ` 1`, `0x10`, `1e3` and `Infinity`, which
    // would give an `n` that `String(n)` would not rewrite identically — two
    // distinct URLs would then designate the same slice.
    const n = Number(rangBrut);
    if (!/^\d+$/.test(rangBrut) || !rangValide(n)) {
        return repondre(ctx, 400, { refus: 'rang-invalide' });
    }

    // 🔴 WRITING INTO A SEALED UPLOAD IS REFUSED, AND THE BRIEF DID NOT
    // ASK FOR IT: leaving it out would CANCEL THE SEAL WITHOUT SAYING SO — a
    // later upload would replace the verified bytes, and the agent
    // would install content nobody has seen, under a row that claims
    // the opposite.
    if (ligne.scelle_a !== null) return repondre(ctx, 409, { refus: 'deja-scelle' });

    // 🔴 A RANK OUTSIDE THE PLAN IS REFUSED AT UPLOAD, NOT AT SEALING: it will
    // NEVER become consistent (`verdict` would call it `incoherentes`, the verdict
    // that cannot be repaired). Refusing it at once avoids writing bytes
    // whose only future is to make the whole upload fail.
    const attendu = plan(ligne.taille, ligne.taille_tranche);
    if (n >= attendu.length) {
        return repondre(ctx, 409, { refus: 'rang-hors-plan', tranches: attendu.length });
    }

    // 🔴 THE BOUND IS HARD, READ BACK FROM THE DATABASE, AND IT IS `taille_tranche` — never the
    // EXPECTED size of this particular slice: the last one is shorter than the
    // step, and bounding to its exact size would make this cap a judge of the
    // SLICING, the role of `proto/ts/tranches.ts::verdict` and of it alone. Here we
    // bound the DISK; a slice that is too short goes through and will be `incoherentes`.
    //
    // 🔴 THE BODY IS NEVER HELD IN MEMORY: `req` is an
    // `AsyncIterable<Uint8Array>` handed AS IS to the store. A `Buffer.concat`
    // would turn the service into a memory bomb driven by its clients.
    const issue = await ctx.deps.tranches.write(ligne.id, n, req, ligne.taille_tranche);
    if (!issue.ok) {
        // ⚠️ THE PARTIAL FILE IS ALREADY DELETED BY THE STORE — reread in
        // `magasin-tranches.ts`: `rmSync(provisoire)` in the `catch`, and the
        // `renameSync` never happened. Nothing to ASSUME: a test reads back
        // the state after the refusal.
        return repondre(ctx, 413, { refus: 'tranche-trop-grande', maximum: issue.plafond });
    }

    // ⚠️ THE UPLOAD IS IDEMPOTENT, AND IT IS THE STORE'S `renameSync` THAT
    // MAKES IT SO: renaming onto an existing file replaces it — a slice
    // uploaded halfway then uploaded again in full must win.
    return repondre(ctx, 200, { n, octets: issue.octets });
}

/* ── ④ SCELLER ───────────────────────────────────────────────────────── */

async function arreter(ctx: Contexte, ligne: LigneTeleversement): Promise<boolean> {
    // ⚠️ SEALING TWICE IS A SUCCESS, NOT A CONFLICT, and without recomputing:
    // the hash HAS ALREADY been verified and uploads have been refused since. A 409
    // would make the most ordinary case fail — a lost response, a client that
    // retries.
    if (ligne.scelle_a !== null) return repondre(ctx, 200, scellement(ligne, ligne.scelle_a));

    // ⚠️ `plan` and `verdict` THROW on an absurd contract, and that is intended: the
    // contract comes from OUR database, not from the wire, and `declarer` validated it before
    // writing it. A row with a zero step would be a PROGRAM defect — a boundary
    // set by `proto/ts/tranches.ts` —, and the 500 is the right answer:
    // disguising it as a refusal would make the client refill slices that do not exist.
    const attendu = plan(ligne.taille, ligne.taille_tranche);
    const v = verdict(ligne.taille, ligne.taille_tranche, ctx.deps.tranches.lister(ligne.id));

    // 🔴 TWO VERDICTS, TWO DISTINCT REFUSALS, AND MERGING THEM WOULD BE AN ENDLESS
    // LOOP: a hole is filled by asking for it again, a slice of the wrong
    // size never is — the uploader would send the same thing back, forever. The whole
    // argument is in `proto/ts/tranches.ts`; here we merely refrain from
    // flattening what it distinguished.
    if (v.etat === 'incoherentes') {
        return repondre(ctx, 409, { refus: 'tranches-incoherentes', n: v.n });
    }
    if (v.etat === 'manquantes') {
        return repondre(ctx, 409, { refus: 'tranches-manquantes', n: v.n });
    }

    // 🔴 THE PLATFORM RECOMPUTES, IT DOES NOT TRUST. The row's `sha256`
    // is what the UPLOADER ANNOUNCED; sealing without checking it would make
    // the column a claim that nothing has confronted with the bytes, and
    // the agent would install a file believing it had verified it. It is the only
    // check in the chain that cannot be satisfied by accident.
    //
    // 🔴 THE RANKS COME FROM THE PLAN, NOT FROM THE LISTING, and the order IS the contract:
    // a slice that vanished between the verdict and the read becomes a stream
    // ERROR, whereas a fresh listing would end CLEANLY but shorter — and
    // the hash would be wrong without anyone knowing why. ⚠️ AS A STREAM: the
    // memory does not depend on the file size.
    const condensat = createHash('sha256');
    const flux = ctx.deps.tranches.concatener(ligne.id, attendu.map((t) => t.n));
    for await (const morceau of flux) condensat.update(morceau as Uint8Array);
    const relu = condensat.digest('hex');

    if (relu !== ligne.sha256) {
        // ⚠️ NEITHER THE ANNOUNCED HASH NOR THE REREAD ONE GOES OUT: the reread one
        // would describe the content actually stored to a peer that might have
        // uploaded only part of it. The log, for its part, carries both.
        console.warn(
            `scellement refusé pour le téléversement ${ligne.id} : empreinte annoncée `
                + `${ligne.sha256}, empreinte relue ${relu} — les octets stockés ne sont `
                + `pas ceux que le déposant a annoncés.`,
        );
        return repondre(ctx, 409, { refus: 'empreinte' });
    }

    const instant = ctx.deps.maintenant();
    await sceller(ctx.deps.base, ligne.id, instant);
    return repondre(ctx, 200, scellement(ligne, instant));
}

/// ⚠️ THE SAME EXPRESSION for the sealing that just happened and for the one
/// that had already happened: otherwise a client that retries would read two shapes of the
/// same fact.
function scellement(ligne: LigneTeleversement, scelleA: number): unknown {
    return { id: ligne.id, taille: ligne.taille, sha256: ligne.sha256, scelle_a: scelleA };
}
