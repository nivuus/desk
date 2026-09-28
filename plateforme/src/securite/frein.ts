// The brake: how many recent failures has a key accumulated, and should
// the next one be refused without paying for it.
//
// This module is PURE — a `Map`, no clock read, no socket, no database
// —, on the exact pattern of `signaling/propriete.ts`, and for the same reason:
// that is what makes it testable without opening a single connection. `maintenant`
// is a PARAMETER everywhere, as in `agents/fraicheur.ts`,
// `identite/jeton.ts` and `depot/session.ts`.
//
// 🔴 IT REFUSES, IT DOES NOT DELAY. A brake by delay keeps the socket
// open during the wait: that is a SECOND denial of service offered to
// the attacker, the very one it claimed to close. This module only returns a
// verdict; the caller answers 429 immediately, with `Retry-After`.
//
// 🔴 WHY THE STATE IS IN MEMORY AND NOT IN THE DATABASE, and the first reason is
// decisive:
//   1. a database brake makes the attacker WRITE. Each attempt would become
//      an `INSERT`/`UPDATE`: the brake would be a load amplifier,
//      exactly what it exists to prevent;
//   2. the `ws` `message` handler is SYNCHRONOUS (`signaling/relais.ts`)
//      and a promise rejected there takes down the whole Node process — the brake of the
//      `/agent` channel must therefore be able to answer WITHOUT `await`;
//   3. a WebSocket lives in one process and one only (spec §3.1), and
//      horizontal scaling is out of v1 scope (spec §9): there is
//      no second instance to share this state with.
//
// ⚠️ THE COST, named and NOT fixed, on the pattern of `ProprieteDeSession`:
// this brake DOES NOT SURVIVE a restart of the service. An attacker who
// managed to make it restart would reset the counters to zero — but
// if they can, they already have better things to do. What stays true without reservation: THE
// BRAKE OF ONE INSTANCE ONLY PROTECTS THAT INSTANCE. Two instances
// would multiply each budget by two, without anything saying so — that is
// why the deployment declares only one.
//
// ⚠️ THE TRADE-OFF THAT TURNS AGAINST THE LEGITIMATE USER, and it must be
// written: an attacker can burn the budget of an account they target and
// deny its owner access for the duration of the window. That is the classic
// account-lockout trade-off. It is accepted because (a) the window
// is short, (b) unlocking by email would require SMTP, which spec §9
// puts out of v1 scope, and (c) the alternative — braking only per address —
// lets TARGETED brute force through, which is the named threat.
//
// ⚠️ THE WINDOW IS ANCHORED AT THE FIRST FAILURE OF A SERIES, AND IS NOT
// SLIDING — a deliberate divergence from the word "sliding" in the plan, and here is
// why. A truly sliding window keeps one timestamp PER FAILURE,
// hence `O(max)` per key: at `ECHECS_MAX_ADRESSE = 50` and `ENTREES_MAX =
// 10 000`, that is half a million timestamps, an order of magnitude
// above the cost D2 computes for the bound ("a few hundred
// kilobytes"). Here each key costs two numbers, and the D2 bound is
// held in the sense in which it was computed.
//   THE PRICE OF THIS CHOICE, and it is real: at the boundary of two windows, an
//   attacker can place `max` failures just before and `max` just after, that is
//   `2 × max` in a burst. It is a factor of TWO, not an order of magnitude, and
//   it makes no secret guessable — it makes the burst twice as
//   long.
//   AND IT HAS A COUNTERPART THAT SERVES THE TRADE-OFF ABOVE: the window
//   NOT being pushed back by later failures, an attacker hammering an
//   account does not extend its owner's lockout indefinitely.
//   The account comes back `FENETRE_MS` after the FIRST failure, whatever happens.

/// The budget of a key. It is passed on each call rather than kept by the
/// brake: that is what lets two keys with DIFFERENT budgets (an account and
/// an address) live in the same table, without a second table
/// diverging the day one of the two is hardened.
export interface Budget {
    readonly max: number;
    readonly fenetreMs: number;
}

export interface Verdict {
    readonly freine: boolean;
    /// Seconds to wait, rounded UP. It is 0 when nothing is
    /// braked. ⚠️ Never 0 when something is: a `Retry-After: 0`
    /// would invite the requester to come back immediately.
    readonly retryApresS: number;
}

/// ⚠️ THE FOUR CONSTANTS ARE NOT CALIBRATED, and they join the list
/// this repository has kept since effort C: `BPP_MIN`, `FACTEUR_FOCUS`,
/// `PART_DORMANTE_BPS`, `HYSTERESIS`, `TAILLE_MAX_SORTIE`,
/// `SEUIL_INJOIGNABLE_MS`, `DUREE_JETON_ACCES_MS`. No usage judgement
/// has been made on any of them.

/// The memory of a failure.
export const FENETRE_MS = 15 * 60_000;

/// Attempts against ONE account. Small: it is the only brake that closes TARGETED
/// brute force.
export const ECHECS_MAX_COMPTE = 5;

/// Attempts from ONE address, all accounts combined. Ten times larger:
/// behind a NAT, several legitimate users share an address, and
/// it is the only brake that closes account SWEEPING.
export const ECHECS_MAX_ADRESSE = 50;

/// 🔴 THE HALF THAT MATTERS. A brake that kept one entry per key seen
/// would be a MEMORY EXHAUSTION vector: an attacker tries a million
/// distinct emails, each only once, and the table grows without
/// any budget ever being exceeded. The brake becomes the attack.
///
/// ⚠️ WHAT EVICTION COSTS, and which cannot be caught up here: under
/// saturation, an attacker can get the entry of an account they target evicted
/// to give it its budget back. That is the price of the bound, and the bound is worth
/// more than the memory.
export const ENTREES_MAX = 10_000;

/// The two budgets of the service, built ONCE.
///
/// ⚠️ THEY ARE HERE, AND NOT WITH THEIR CALLERS, SO THAT `/auth/*` AND
/// `/agent` CANNOT DIVERGE. D4 says so: "no second table —
/// two distinct brakes would diverge the day one was hardened". The same
/// reason holds for the budgets and for the key shape below.
export const BUDGET_COMPTE: Budget = { max: ECHECS_MAX_COMPTE, fenetreMs: FENETRE_MS };
export const BUDGET_ADRESSE: Budget = { max: ECHECS_MAX_ADRESSE, fenetreMs: FENETRE_MS };

/// 🔴 THE KEYS ARE PREFIXED, AND THE PREFIXES ARE DISJOINT. Without them, a
/// VM named `203.0.113.7` would share the budget of the address `203.0.113.7`, and
/// an attacker could exhaust one to close the other. The three
/// constructors live here so that nobody writes a fourth.

/// ⚠️ THE EMAIL IS NORMALISED TO LOWERCASE AND TRIMMED. Otherwise
/// `ADA@exemple.test` would be a second key, and the budget of an account would be
/// multiplied by the number of casings an attacker can write.
export function cleCompte(email: string): string {
    return `compte:${email.trim().toLowerCase()}`;
}

export function cleAdresse(adresse: string): string {
    return `adr:${adresse}`;
}

export function cleVm(vmId: string): string {
    return `agent:${vmId}`;
}

/// 🔴 THE "ANY REQUEST" BUDGET, SHARED BY `GET /vm`, `POST /session` AND
/// THE `/signal` RELAY (`http/routes-vm.ts`, `http/routes-session.ts`,
/// `signaling/relais.ts`) — as `BUDGET_ADRESSE` already is between
/// `/auth/*` and `/agent`.
///
/// ⚠️ THIS BUDGET HAS NO NOTION OF FAILURE, AND THAT IS THE POINT: the three
/// surfaces it covers have nothing equivalent to a wrong password —
/// their abuse is a VOLUME of requests which, individually, may all
/// SUCCEED. `Frein.echec` is reused to COUNT, never to signal
/// a failure: it is the STRUCTURE — window ANCHORED at the first failure of a
/// series (never sliding, see the module header), entry cap,
/// eviction — that is reused here, never the semantics of the method
/// name. Nothing calls `succes()` on this budget: an entry only empties
/// BY expiry of its window, never by a gesture of the caller — there
/// is no equivalent here of a successful sign-in clearing an
/// account, precisely because there is no account.
///
/// ⚠️ NO "ACCOUNT" KEY EXISTS FOR THIS BUDGET, UNLIKE
/// `BUDGET_COMPTE`: `/signal` only knows its peer AFTER its
/// handshake, and a key set on the authenticated user of `/vm` or
/// `/session` would let a stolen token consume the budget of ITS VICTIM
/// rather than that of the attacker using it — only the ADDRESS is therefore
/// kept.
///
/// 🔵 **FAVOURABLE PROPERTY, ESTABLISHED BY THE REVIEW (correction round 1,
/// 25 August 2026), AND NOBODY HAD LOOKED FOR IT: CROSS-EVICTION DOES NOT
/// BITE.** `Frein` holds ALL its keys in ONE single `Map`
/// (`entrees`), with a COMMON cap (`ENTREES_MAX`) and a purge that
/// first removes EXPIRED entries before evicting the one closest to
/// expiry (`faireDeLaPlace`). Because `FENETRE_REQUETES_MS` (one
/// minute) is **STRICTLY SHORTER** than `FENETRE_MS` (fifteen
/// minutes) — an EXACT ratio of 15 —, a `req:` key expires and is purged
/// ALWAYS before a `compte:`/`adr:` key of the same age has even
/// reached ONE FIFTEENTH of its own window. Under saturation, the purge
/// therefore very
/// preferentially reclaims `req:` entries — those that a mere traffic VOLUME
/// creates in bulk — and spares the `compte:`/`adr:` entries,
/// which hold the memory of an ACCOUNT or an ADDRESS targeted by an
/// ongoing attack. **WHAT THIS CLOSES**: without this property, an
/// attacker could flood `/vm` from throwaway addresses to saturate the
/// table and get the `adr:`/`compte:` entry of THEIR OWN target evicted,
/// giving it back its whole failure budget — exactly the cost that
/// `ENTREES_MAX` already documents ("an attacker can get the entry
/// of an account they target evicted to give it its budget back"), here closed by a
/// happy accident of magnitude between the two windows.
///
/// 🔴 **THIS PROPERTY IS NOT GUARANTEED BY THE CODE — IT HOLDS BECAUSE
/// `FENETRE_REQUETES_MS < FENETRE_MS`, AND NOTHING CHECKS IT ANYWHERE
/// BUT `frein.test.ts`.** A future tuning of `FENETRE_REQUETES_MS`
/// (calibration, or a need for a longer window) that raised it
/// above `FENETRE_MS` would break this property WITHOUT ANY OTHER
/// SIGNAL: the test `(l) FENETRE_REQUETES_MS stays SHORTER than
/// FENETRE_MS` is the only safeguard. Read it before touching either
/// constant.
///
/// ⚠️ NOT CALIBRATED, like the four constants of `Frein` above. The
/// window is SHORTER than `FENETRE_MS` (one minute against fifteen) because
/// the aim is to throttle a RATE, not to lock an account for the time
/// it takes a human to react — and see the favourable property above for the
/// SECOND reason, discovered afterwards, never to lengthen it without
/// rethinking.
///
/// 🔴 **`REQUETES_MAX_ADRESSE` RECONSIDERED (correction round 1, criticism
/// ⑥) — 60 WAS TOO THIN, AND THE COMPUTATION THAT SHOWS IT:** a session
/// with all its windows opens, on the AGENT side (a single address — that of
/// the VM), 1 `/signal` connection for the control session (`bureau`) + 1
/// for the file bridge + up to `CAPACITE` = 10 for the windows
/// (`agent/src/superviseur/boucle.rs::CAPACITE`) = **12** `/signal`
/// connections, PLUS 2 HTTP requests sharing the SAME bucket (`GET /vm`,
/// `POST /session`) = **14** "hits" to open a single all-windows
/// session. A network cut makes all of them reconnect in a
/// tight burst (each process has its own backoff, but all
/// start at the same minimal delay, ~500 ms): same order of magnitude in a
/// single burst. And ON THE CLIENT SIDE, several users behind the same corporate
/// NAT SHARE THE KEY — each able to open their own all-windows
/// session — same reason, at the same factor as between `ECHECS_MAX_COMPTE`
/// and `ECHECS_MAX_ADRESSE` (an order of magnitude), which already exists
/// precisely for this case. `120` = about 8-9 bursts of 14, or the equivalent
/// of several simultaneous users behind the same NAT each opening
/// their session — **REASONED, STILL NOT MEASURED.**
export const FENETRE_REQUETES_MS = 60_000;
export const REQUETES_MAX_ADRESSE = 120;
export const BUDGET_REQUETES: Budget = {
    max: REQUETES_MAX_ADRESSE,
    fenetreMs: FENETRE_REQUETES_MS,
};

/// 🔴 PREFIX DISJOINT FROM `compte:`, `adr:` AND `agent:` — FOR THE SAME
/// REASON AS THEM. Without it, the address `203.0.113.7` on this budget
/// would share its key with the SAME address on `BUDGET_ADRESSE`
/// (`cleAdresse`), and an attacker could exhaust one of the two budgets
/// to drain the other — the exact test this batch adds to `frein.test.ts`.
/// ⚠️ **THIS PREFIX IS ALSO WHAT CARRIES THE FAVOURABLE PROPERTY
/// ABOVE**: without it, there would be only ONE entry per address,
/// whose window would be that of the LAST budget consulted — the distinction
/// between "expires fast" and "expires slowly" would disappear with it.
export function cleRequetes(adresse: string): string {
    return `req:${adresse}`;
}

interface Entree {
    compte: number;
    /// The instant of the FIRST failure of the series — see the header: it is what
    /// anchors the window, and it is not pushed back by later failures.
    premierA: number;
    /// Kept with the entry, and not reread from the caller's budget: the purge
    /// and the eviction must be able to date an entry without knowing which
    /// key it comes from.
    fenetreMs: number;
}

export class Frein {
    private readonly entrees = new Map<string, Entree>();
    private readonly entreesMax: number;
    private evincees = 0;

    /// `entreesMax` is a PARAMETER, and not only for the test: an
    /// instance serving a larger deployment should not have to recompile.
    constructor(entreesMax: number = ENTREES_MAX) {
        this.entreesMax = entreesMax;
    }

    /// Consults WITHOUT RECORDING ANYTHING. Called BEFORE any costly work —
    /// before `scrypt`, before any database access.
    ///
    /// 🔴 `consulter` AND `echec` ARE TWO FUNCTIONS, NEVER ONE. A single
    /// function that consulted AND counted would charge a failure to a
    /// legitimate request arriving during the window, and would make the brake
    /// SELF-SUSTAINING: an attacker would keep an account locked
    /// indefinitely without ever trying a password.
    consulter(cles: readonly (readonly [string, Budget])[], maintenant: number): Verdict {
        let restantMs = 0;
        for (const [cle, budget] of cles) {
            const entree = this.entrees.get(cle);
            if (entree === undefined) continue;
            // `>=`, never `>`: at exactly the budget, we refuse. A `>`
            // would grant one more attempt than the constant announces.
            if (entree.compte >= budget.max) {
                const restant = entree.premierA + entree.fenetreMs - maintenant;
                // 🔴 A NON-POSITIVE REMAINDER *IS* THE EXPIRY, and that is why
                // there is NO second expiry test here. `restantMs`
                // starts at 0: an entry whose window is closed returns a
                // remainder <= 0, which can therefore never raise it.
                //
                // ⚠️ THIS COMMENT WAS WRITTEN AFTER A MUTATION THAT DISTURBED
                // NOTHING. An `if (this.expiree(entree, maintenant)) continue;`
                // lived two lines higher, and REMOVING it left the eleven
                // tests GREEN: it was EXACTLY redundant with the
                // comparison below, `maintenant - premierA >= fenetreMs`
                // being the same proposition as `premierA + fenetreMs -
                // maintenant <= 0`. A line no test can bring
                // down is not a belt: it is dead code giving
                // the APPEARANCE of a guard, and it would have made people believe next
                // year that expiry is decided there. It is decided
                // HERE, and mutation `T1-c` now targets this line.
                if (restant > restantMs) restantMs = restant;
            }
        }
        if (restantMs <= 0) return { freine: false, retryApresS: 0 };
        // Rounded UP: see `Verdict.retryApresS`.
        return { freine: true, retryApresS: Math.ceil(restantMs / 1000) };
    }

    /// Records a failure on each of the keys.
    echec(cles: readonly (readonly [string, Budget])[], maintenant: number): void {
        for (const [cle, budget] of cles) {
            const entree = this.entrees.get(cle);
            if (entree !== undefined && !this.expiree(entree, maintenant)) {
                entree.compte += 1;
                continue;
            }
            if (entree === undefined) this.faireDeLaPlace(maintenant);
            // An expired entry is REARMED in place, never accumulated: the
            // window restarts from the present, and the table does not grow.
            this.entrees.set(cle, { compte: 1, premierA: maintenant, fenetreMs: budget.fenetreMs });
        }
    }

    /// Clears a key — called on a SUCCESS.
    ///
    /// ⚠️ THE CALLER ONLY PASSES IT THE ACCOUNT KEY. Passing it the
    /// address key too would CLEAR an attacker who owns a valid account: they
    /// would just need to sign in to it between two bursts to reset their
    /// address budget to zero. This module imposes nothing — it clears what it is
    /// told —, and it is the route that holds the rule.
    succes(cle: string): void {
        this.entrees.delete(cle);
    }

    /// For the trace, and for the cap test.
    taille(): number {
        return this.entrees.size;
    }

    /// 🔴 AN EVICTION IS NEVER SILENT FOR THE OPERATOR: this counter
    /// goes up, the trace reads it, and a saturation stops being invisible.
    evictions(): number {
        return this.evincees;
    }

    private expiree(entree: Entree, maintenant: number): boolean {
        return maintenant - entree.premierA >= entree.fenetreMs;
    }

    /// Purge first, evict next — and never the reverse: evicting a LIVE
    /// entry while the table is full of dead entries would give its
    /// budget back to an attacker for nothing.
    private faireDeLaPlace(maintenant: number): void {
        if (this.entrees.size < this.entreesMax) return;

        for (const [cle, entree] of this.entrees) {
            if (this.expiree(entree, maintenant)) this.entrees.delete(cle);
        }
        if (this.entrees.size < this.entreesMax) return;

        // Still full: we evict the one whose window closes the
        // soonest. It is the one whose disappearance costs the least memory of the
        // past — and that is a choice, not a given: see `ENTREES_MAX`.
        let plusTot: string | undefined;
        let plusTotA = Number.POSITIVE_INFINITY;
        for (const [cle, entree] of this.entrees) {
            const finA = entree.premierA + entree.fenetreMs;
            if (finA < plusTotA) {
                plusTotA = finA;
                plusTot = cle;
            }
        }
        if (plusTot !== undefined) {
            this.entrees.delete(plusTot);
            this.evincees += 1;
        }
    }
}
