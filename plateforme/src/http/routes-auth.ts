// Les deux routes d'authentification : `POST /auth/connexion` et
// `POST /auth/rafraichir`.
//
// 🔴 THE REFUSAL MESSAGE IS IDENTICAL for « unknown email » and « wrong
// password » — `{refus:'identifiants'}`. A message that told them apart
// would be an account ENUMERATION oracle: the attacker would learn which
// addresses exist by reading the response. It is the same rule as criterion ②
// of P3, set here because the first case where it bites is this one.
//
// ⚠️ AND THE COST OF THE PATH IS EQUALISED TOO: on an unknown email, the
// route still hashes a decoy password, so that the response time
// does not betray the existence of the account. **THIS EQUALISATION IS NOT
// MEASURED**, and will not be: a timing test would be flaky, and
// the spec §8 already files timing attacks among what ⑤ does not test.
// What IS tested is the identical message, which is decidable. Writing
// « equalised » without this caveat would be a claim beyond the survey.
//
// 🔴 NO PASSWORD SHOWS UP IN A TRACE OR IN A RESPONSE
// (criterion ④). No line of this file logs a request body,
// and that is deliberate: logging `JSON.stringify(corps)` to diagnose
// would write the password in clear into `agent.log`. The criterion ④ test
// sweeps THE FIELD (`motdepasse`, `mot_de_passe`, `empreinte_mdp`) and not the
// value, precisely to catch that move.

import type { IncomingMessage, ServerResponse } from 'node:http';
import type { Pilote } from '../base/pilote';
import { emettre, tourner } from '../depot/jeton';
import { lireParEmail, remplacerEmpreinte } from '../depot/utilisateur';
import { DUREE_JETON_ACCES_MS, signer } from '../identite/jeton';
import { doitEtreRehache, hacher, verifier } from '../identite/mot-de-passe';
import { entetesCors } from './cors';
import { repondreIntrouvable } from './introuvable';
import { ENTETES_SECURITE } from './entetes';
import { adresseSource } from './adresse-source';
import { ligne } from '../obs/journal';
import {
    BUDGET_ADRESSE,
    BUDGET_COMPTE,
    cleAdresse,
    cleCompte,
    type Budget,
    type Frein,
} from '../securite/frein';

export interface DependancesAuth {
    base: Pilote;
    secretJeton: string;
    origineClient?: string;
    maintenant: () => number;
    /// The brake, SHARED with the `/agent` channel — one single table, never
    /// two (see `securite/frein.ts`).
    frein: Frein;
    /// The proxies whose `X-Forwarded-For` header is trusted. EMPTY by default:
    /// nobody is trusted (`config.ts`).
    proxyDeConfiance: ReadonlySet<string>;
    /// The authentication mode (`config.ts`). In `pomerium`, this router
    /// STEPS ASIDE: see the guard at the top of `servirAuth`.
    auth: 'pomerium' | 'motdepasse';
}

/// The keys to consult for a request, and the one a success clears.
interface ContexteFrein {
    cles: readonly (readonly [string, Budget])[];
    /// The address RETAINED by `adresseSource` — the one the trace names.
    adresse: string;
    /// ⚠️ FILLED IN FOR `/auth/connexion` ONLY. `/auth/rafraichir` has
    /// no email to present — only an opaque token —, and taking
    /// that token as a key would amount to INDEXING A TABLE ON A SECRET.
    cleDuCompte?: string;
}

/// 4 KiB. An honest authentication body weighs a few hundred
/// bytes; with no bound, an ANONYMOUS peer — the route is open, that is its
/// purpose — would grow the memory of the service at will.
/// ⚠️ NOT CALIBRATED: it is a generous bound, not a measurement.
const CORPS_MAX_OCTETS = 4 * 1024;

const CHEMINS = new Set(['/auth/connexion', '/auth/rafraichir']);

/// The decoy password hashed on an unknown email. Computed ONCE and
/// memoised: hashing it on each request would cost the same time, but
/// computing it here keeps the cost of the « no such account » path comparable to
/// that of the « existing account » path.
const LEURRE = 'un-mot-de-passe-leurre-qui-n-est-a-personne';

function repondre(
    rep: ServerResponse,
    code: number,
    corps: unknown,
    cors: Record<string, string> | undefined,
): void {
    rep.writeHead(code, {
        'content-type': 'application/json; charset=utf-8',
        // ⚠️ UNCONDITIONAL, and set on EVERY response — including the
        // ERROR responses (401, 405, 413, 429, 500, 503), which
        // often carry more information than a normal response. They are spread
        // BEFORE `cors` so that the origin policy, which is optional,
        // can never overwrite them by mistake.
        ...ENTETES_SECURITE,
        ...(cors ?? {}),
    });
    rep.end(JSON.stringify(corps));
}

/// Reads the body, or yields `undefined` if the bound is crossed — in which case the
/// request is ABANDONED without reading the rest, rather than piling up.
function lireCorps(req: IncomingMessage): Promise<string | undefined> {
    return new Promise((resolve, rejeter) => {
        let recu = '';
        req.on('data', (morceau: Buffer) => {
            recu += morceau.toString('utf8');
            if (recu.length > CORPS_MAX_OCTETS) {
                // We stop reading AT ONCE: carrying on piling up to
                // answer politely would be exactly the denial of service that
                // the bound exists to prevent.
                req.destroy();
                resolve(undefined);
            }
        });
        req.on('end', () => resolve(recu));
        req.on('error', rejeter);
    });
}

function estObjet(v: unknown): v is Record<string, unknown> {
    return typeof v === 'object' && v !== null && !Array.isArray(v);
}

/// Yields `true` if the request was served, `false` if it is not about
/// authentication — the server then answers 404, as today.
///
/// ⚠️ « AS TODAY » IS NO LONGER UNCONDITIONALLY TRUE SINCE 22 AUGUST
/// 2026: when `PLATEFORME_PAGE` is armed, a TENTH router — the page
/// server — is chained AFTER all the others, and it resolves any
/// path. On a `GET`/`HEAD`, it is IT that answers `200 text/html` to the
/// `false` returned here; outside `GET`/`HEAD` it steps aside, and the generic 404
/// takes over. See `http/chaine.ts`, which carries the count and the rule.
export async function servirAuth(
    req: IncomingMessage,
    rep: ServerResponse,
    deps: DependancesAuth,
): Promise<boolean> {
    const chemin = new URL(req.url ?? '/', 'http://placeholder').pathname;
    if (!CHEMINS.has(chemin)) return false;

    // 🔴 IN `pomerium` MODE, THESE TWO ROUTES DO NOT EXIST — `404`, returned
    // HERE. Leaving them alive behind the proxy would be a SECOND authentication
    // door, with passwords nobody rotates any more
    // and a brake nobody watches any more.
    //
    // 🔴 THIS GUARD RETURNED `false` UNTIL 22 AUGUST 2026, TO LET THE GENERIC
    // 404 ANSWER — AND IT WAS THE SYMMETRIC TWIN OF THE DEFECT OF
    // `routes-identite.ts`. The page server, chained last, folds every
    // path without an extension onto the page (`hub.html` since 30 August 2026,
    // `index.html` before — see `page/resolution.ts::PAGE`): `GET
    // /auth/connexion` in `pomerium` mode with `PLATEFORME_PAGE` armed
    // returned `200 text/html`. The two guards having OPPOSITE polarities,
    // they had the SAME defect,
    // each in the other mode — and a per-task review could not
    // see it, each half being right. The `404` comes from
    // `http/introuvable.ts`, the server one, never a second text.
    //
    // ⚠️ THE GUARD IS AFTER THE PATH COMPARISON AND NOT BEFORE, on purpose:
    // a router that returned `false` for EVERY path in pomerium mode would be
    // indistinguishable from an unplugged router, and the red of the chaining
    // could no longer say anything.
    // 🔴 THE INVARIANT OF THE TWO MODE GUARDS, WRITTEN HERE AND IN
    // `routes-identite.ts` BECAUSE IT BELONGS TO NEITHER ONE NOR THE OTHER:
    // **the two guards have OPPOSITE POLARITIES, and that is what makes them
    // PARTITION the modes.** This one steps aside if the mode is NOT
    // `motdepasse`; the one of `routes-identite.ts` steps aside if the mode is
    // NOT `pomerium`. With TWO modes, every mode therefore activates exactly one of the
    // two doors.
    //
    // 🔴 WHAT THIS INVARIANT COSTS THE DAY A THIRD MODE SHOWS UP:
    // **both guards return `404` at the same time, and the service has
    // NO authentication route left — SILENTLY.** No 500, no `warn!`;
    // each of the two 404s is right taken alone, and the login page reads
    // the one of `/auth/moi` as « this deployment authenticates by password »
    // before POSTing to a route that does not exist either.
    // **Adding a value to `AUTHS` (`config.ts`) therefore FORCES coming back here**
    // to decide which of the two doors the new mode opens —
    // TypeScript will not ask, as these guards compare strings and not
    // an exhaustive `switch`.
    if (deps.auth !== 'motdepasse') {
        repondreIntrouvable(rep);
        return true;
    }

    const cors = entetesCors(req.headers.origin, deps.origineClient);

    if (req.method === 'OPTIONS') {
        // 204 even without a CORS header: the preflight request is served, but
        // without permission the browser will refuse the real request — a LOUD
        // refusal, which the operator sees (see `config.ts`).
        rep.writeHead(204, { ...ENTETES_SECURITE, ...(cors ?? {}) });
        rep.end();
        return true;
    }

    if (req.method !== 'POST') {
        repondre(rep, 405, { refus: 'methode' }, cors);
        return true;
    }

    const brut = await lireCorps(req);
    if (brut === undefined) {
        repondre(rep, 413, { refus: 'corps-trop-grand' }, cors);
        return true;
    }

    let corps: unknown;
    try {
        corps = JSON.parse(brut);
    } catch {
        repondre(rep, 400, { refus: 'forme' }, cors);
        return true;
    }
    if (!estObjet(corps)) {
        repondre(rep, 400, { refus: 'forme' }, cors);
        return true;
    }

    // 🔴 THE BRAKE IS CONSULTED HERE, AND THE POSITION IS WHAT MATTERS: BEFORE
    // `lireParEmail`, so BEFORE the slightest database access, AND BEFORE
    // `verifier`/`hacher`, so BEFORE THE `scrypt` DERIVATION. `scrypt` is
    // memory-hard and deliberately costly (68 ms measured on 20 August 2026
    // on this machine): an attacker who triggers it at will exhausts the
    // service without ever guessing a secret. A BRAKE PLACED AFTER THE
    // CHECK PROTECTS NOTHING — it counts failures it has already paid for.
    //
    // The body is read first, because the account key depends on it; it is
    // bounded to `CORPS_MAX_OCTETS` and therefore costs nothing comparable.
    const contexte = clesDe(chemin, corps, req, deps);
    const verdict = deps.frein.consulter(contexte.cles, deps.maintenant());
    if (verdict.freine) {
        // ⚠️ THE 429 CARRIES THE CORS HEADERS LIKE ALL THE OTHER RESPONSES.
        // Without them, the BROWSER cannot read the refusal: the user
        // sees an opaque failure instead of « try again in n minutes ».
        rep.setHeader('Retry-After', String(verdict.retryApresS));
        repondre(rep, 429, { refus: 'trop-de-tentatives' }, cors);
        return true;
    }

    if (chemin === '/auth/connexion') {
        await connexion(corps, rep, deps, cors, contexte);
    } else {
        await rafraichir(corps, rep, deps, cors, contexte);
    }
    return true;
}

/// Builds the brake keys of a request.
///
/// ⚠️ `/auth/connexion` CARRIES TWO KEYS, `/auth/rafraichir` ONLY ONE (D1):
///   - per ACCOUNT, the only brake that closes TARGETED brute force — an attacker
///     with a thousand source addresses is not slowed down otherwise;
///   - per ADDRESS, the only brake that closes account SWEEPING — a thousand
///     emails tried once each consume no account budget.
function clesDe(
    chemin: string,
    corps: Record<string, unknown>,
    req: IncomingMessage,
    deps: DependancesAuth,
): ContexteFrein {
    const adresse = adresseSource(
        req.socket.remoteAddress,
        // Node yields `string[]` if the header is repeated. Joining it with
        // commas brings it back to the shape of a single header, which
        // `adresseSource` can read — and of which it takes the LAST element,
        // that is, the one the closest proxy wrote.
        Array.isArray(req.headers['x-forwarded-for'])
            ? req.headers['x-forwarded-for'].join(',')
            : req.headers['x-forwarded-for'],
        deps.proxyDeConfiance,
    );
    const parAdresse: readonly [string, Budget] = [cleAdresse(adresse), BUDGET_ADRESSE];

    // The email is a key only if it is a string: a malformed body
    // will be refused with 400 further down, and has no business consuming account budget.
    const email = corps.email;
    if (chemin !== '/auth/connexion' || typeof email !== 'string') {
        return { cles: [parAdresse], adresse };
    }
    const cle = cleCompte(email);
    return { cles: [[cle, BUDGET_COMPTE], parAdresse], adresse, cleDuCompte: cle };
}

/// Records the failure, and logs IF AND ONLY IF the brake has just
/// bitten.
///
/// 🔴 WHY NOT ONE LINE PER REFUSAL. A trace emitted on each 429 would make
/// the service an AMPLIFIER on the very path being closed: an attacker at
/// ten thousand requests per second would get ten thousand lines written per second,
/// for requests that, for their part, no longer cost anything. `CLAUDE.md` has carried the
/// rule since the TURN work — « count or sample, never trace
/// per packet », after a per-packet trace wrote 18 619 lines in
/// a few seconds and destroyed the measurement it served.
///
/// The transition is detected by consulting again AFTER the failure: the next
/// request being refused at the very top of `servirAuth`, it will
/// never reach this function. There is therefore EXACTLY ONE line per key and per
/// window, a bound that `ENTREES_MAX` closes.
///
/// ⚠️ THE LINE CARRIES THE TARGETED EMAIL, and that is a trade-off: knowing WHICH
/// account is attacked is precisely what an operator needs. No
/// password appears in it — criterion ④ —, and the key is already normalised.
function compterLEchec(deps: DependancesAuth, contexte: ContexteFrein, chemin: string): void {
    const instant = deps.maintenant();
    deps.frein.echec(contexte.cles, instant);
    const apres = deps.frein.consulter(contexte.cles, instant);
    if (!apres.freine) return;
    // ⚠️ THE ADDRESS IS NAMED, AND IT IS THE ONLY REMEDY for the failure mode
    // of `http/adresse-source.ts`: an operator who set up a proxy without
    // declaring trust in it will see here the address of their proxy on every
    // line, and will understand that their per-address brake has become GLOBAL.
    console.warn(
        ligne('frein', {
            route: chemin,
            adresse: contexte.adresse,
            cles: contexte.cles.map(([cle]) => cle).join(' '),
            retry_apres_s: apres.retryApresS,
            // ⚠️ `entrees` AND `evictions` ARE THERE SO THAT SATURATION OF THE
            // BRAKE STOPS BEING INVISIBLE. Under saturation, an eviction gives
            // its budget back to a targeted account (see `ENTREES_MAX`): an operator
            // who sees `evictions` climb knows the brake is overwhelmed, and
            // that its budgets are no longer worth what they claim.
            entrees: deps.frein.taille(),
            evictions: deps.frein.evictions(),
        }),
    );
}

async function connexion(
    corps: Record<string, unknown>,
    rep: ServerResponse,
    deps: DependancesAuth,
    cors: Record<string, string> | undefined,
    contexte: ContexteFrein,
): Promise<void> {
    const { email, motdepasse } = corps;
    if (typeof email !== 'string' || typeof motdepasse !== 'string') {
        repondre(rep, 400, { refus: 'forme' }, cors);
        return;
    }

    const utilisateur = await lireParEmail(deps.base, email);
    if (!utilisateur) {
        // See the header: the cost of the path is equalised, the equalisation is
        // NOT measured, and the message is the same as for a wrong password.
        await hacher(LEURRE);
        // ⚠️ AN UNKNOWN EMAIL COUNTS AS A FAILURE, exactly like a
        // wrong password. Counting only existing accounts would reopen
        // the ORACLE that this route closes over three paragraphs: sweeping
        // a million addresses would then consume no budget.
        compterLEchec(deps, contexte, '/auth/connexion');
        repondre(rep, 401, { refus: 'identifiants' }, cors);
        return;
    }

    let bon: boolean;
    try {
        bon = await verifier(motdepasse, utilisateur.empreinte_mdp);
    } catch (cause) {
        // `verifier` THROWS on an unknown algorithm — a database written by a
        // future version. It is a data defect, not a faulty input:
        // it is logged WITHOUT the request body, and the response stays
        // that of the credentials, so as not to become an oracle.
        console.error(`empreinte illisible pour un compte existant : ${String(cause)}`);
        compterLEchec(deps, contexte, '/auth/connexion');
        repondre(rep, 401, { refus: 'identifiants' }, cors);
        return;
    }
    if (!bon) {
        compterLEchec(deps, contexte, '/auth/connexion');
        repondre(rep, 401, { refus: 'identifiants' }, cors);
        return;
    }

    // Rehashing on the next login: that is what will make any
    // data migration unnecessary the day the parameters change.
    if (doitEtreRehache(utilisateur.empreinte_mdp)) {
        await remplacerEmpreinte(deps.base, utilisateur.id, await hacher(motdepasse));
    }

    // 🔴 SUCCESS ONLY CLEARS THE ACCOUNT KEY, NEVER THE ADDRESS ONE.
    // Clearing it too would LAUNDER an attacker who owns a valid account:
    // they would only need to sign in to it between two bursts to bring their
    // address budget back to zero, and the per-address brake would close nothing any more.
    if (contexte.cleDuCompte !== undefined) deps.frein.succes(contexte.cleDuCompte);

    // A login opens a NEW family — it is the only move that
    // does so.
    await delivrer(rep, deps, utilisateur.id, await emettre(deps.base, utilisateur.id, deps.maintenant()), cors);
}

async function rafraichir(
    corps: Record<string, unknown>,
    rep: ServerResponse,
    deps: DependancesAuth,
    cors: Record<string, string> | undefined,
    contexte: ContexteFrein,
): Promise<void> {
    const { rafraichissement } = corps;
    if (typeof rafraichissement !== 'string') {
        repondre(rep, 400, { refus: 'forme' }, cors);
        return;
    }

    const issue = await tourner(deps.base, rafraichissement, deps.maintenant());
    if (!issue.ok) {
        // ⚠️ ONLY THE ADDRESS KEY IS CONSUMED HERE — `contexte.cles` only
        // carries one for this route (see `clesDe`). A stolen refresh
        // token therefore cannot be used to lock the
        // account of its victim.
        compterLEchec(deps, contexte, '/auth/rafraichir');
        // The reason is returned to the requester: it is about THEIR own token, and
        // telling them whether they must sign in again or whether they have just been compromised
        // teaches nothing about the accounts of others.
        repondre(rep, 401, { refus: issue.motif }, cors);
        return;
    }

    // 🔴 THE TOKEN RETURNED IS THE ROTATION ONE, never a new token issued
    // on top. Calling `emettre` here would open a NEW family on each
    // refresh: replay detection would then revoke a family
    // that the stolen token no longer belongs to, and would protect NOTHING. This
    // defect was really written, and it is the end-to-end replay test
    // that caught it — the repository tests alone could not.
    await delivrer(rep, deps, issue.utilisateurId, issue.clair, cors);
}

/// The pair issued by both routes, written once so that they
/// cannot diverge.
async function delivrer(
    rep: ServerResponse,
    deps: DependancesAuth,
    utilisateurId: string,
    rafraichissement: string,
    cors: Record<string, string> | undefined,
): Promise<void> {
    const maintenant = deps.maintenant();
    const acces = signer(utilisateurId, deps.secretJeton, maintenant);
    repondre(
        rep,
        200,
        // `expire_a` in MILLISECONDS, like every timestamp of this service —
        // see the divergence declared in `identite/jeton.ts`.
        { acces, rafraichissement, expire_a: maintenant + DUREE_JETON_ACCES_MS },
        cors,
    );
}
