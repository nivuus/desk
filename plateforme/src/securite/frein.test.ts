import { describe, expect, it } from 'vitest';
import {
    BUDGET_ADRESSE,
    BUDGET_REQUETES,
    ECHECS_MAX_ADRESSE,
    ECHECS_MAX_COMPTE,
    ENTREES_MAX,
    FENETRE_MS,
    FENETRE_REQUETES_MS,
    REQUETES_MAX_ADRESSE,
    Frein,
    cleAdresse as cleAdresseDuModule,
    cleRequetes,
    type Budget,
} from './frein';

/// A real epoch, on the pattern of `base/harnais.ts:32`: small
/// values measure nothing (lesson of P1), and a brake that subtracts
/// timestamps must be tested on plausible timestamps.
const T0 = 1_787_000_000_000;

/// Documentation addresses (RFC 5737), never `a`/`b`/`c`: a brake that
/// normalises addresses cannot be tested on labels.
const ADR = '203.0.113.7';
const AUTRE_ADR = '198.51.100.4';

const COMPTE: Budget = { max: ECHECS_MAX_COMPTE, fenetreMs: FENETRE_MS };
const ADRESSE: Budget = { max: ECHECS_MAX_ADRESSE, fenetreMs: FENETRE_MS };

function cleCompte(email: string): [string, Budget] {
    return [`compte:${email}`, COMPTE];
}
function cleAdresse(adresse: string): [string, Budget] {
    return [`adr:${adresse}`, ADRESSE];
}

describe('Frein', () => {
    it('(a) under the budget, does not brake', () => {
        const f = new Frein();
        const cles = [cleCompte('alice@exemple.test')];
        // `max - 1` failures: exactly one try remains.
        for (let i = 0; i < ECHECS_MAX_COMPTE - 1; i++) f.echec(cles, T0 + i);
        expect(f.consulter(cles, T0 + ECHECS_MAX_COMPTE)).toEqual({
            freine: false,
            retryApresS: 0,
        });
    });

    it('(b) exactly at the budget, brakes — the comparison is `>=`, never `>`', () => {
        const f = new Frein();
        const cles = [cleCompte('alice@exemple.test')];
        for (let i = 0; i < ECHECS_MAX_COMPTE; i++) f.echec(cles, T0 + i);
        expect(f.consulter(cles, T0 + ECHECS_MAX_COMPTE).freine).toBe(true);
    });

    it('(c) after the window, the budget is given back', () => {
        const f = new Frein();
        const cles = [cleCompte('alice@exemple.test')];
        for (let i = 0; i < ECHECS_MAX_COMPTE; i++) f.echec(cles, T0 + i);
        expect(f.consulter(cles, T0 + FENETRE_MS - 1).freine).toBe(true);
        // The clock is a PARAMETER: the test advances, it does not wait.
        expect(f.consulter(cles, T0 + FENETRE_MS + 1).freine).toBe(false);
    });

    it('(d) `succes` erases the given key, and only it', () => {
        const f = new Frein();
        const compte = cleCompte('alice@exemple.test');
        const adresse = cleAdresse(ADR);
        // BOTH keys are exhausted, so that clearing only one is
        // observable on the other.
        for (let i = 0; i < ECHECS_MAX_ADRESSE; i++) f.echec([compte, adresse], T0 + i);
        f.succes(compte[0]);
        expect(f.consulter([compte], T0 + ECHECS_MAX_ADRESSE).freine).toBe(false);
        expect(f.consulter([adresse], T0 + ECHECS_MAX_ADRESSE).freine).toBe(true);
    });

    it("(e) `consulter` records NOTHING", () => {
        const f = new Frein();
        const cles = [cleCompte('alice@exemple.test')];
        // `max + 1` consultations: if `consulter` counted, the last one
        // would brake. A self-sustaining brake would keep an account blocked without
        // any password ever being tried.
        for (let i = 0; i <= ECHECS_MAX_COMPTE; i++) {
            expect(f.consulter(cles, T0 + i).freine).toBe(false);
        }
        expect(f.size()).toBe(0);
    });

    it('(f) 🔴 the table NEVER exceeds its entry ceiling', () => {
        // `entreesMax` is a constructor parameter PRECISELY so that
        // the test can set a small one: inserting ten thousand keys would
        // measure nothing more, and would cost the suite's time.
        const f = new Frein(8);
        for (let i = 0; i < 9; i++) f.echec([cleCompte(`n${i}@exemple.test`)], T0 + i);
        expect(f.size()).toBeLessThanOrEqual(8);
    });

    it('(g) an eviction is COUNTED, never silent', () => {
        const f = new Frein(8);
        expect(f.evictions()).toBe(0);
        for (let i = 0; i < 9; i++) f.echec([cleCompte(`n${i}@exemple.test`)], T0 + i);
        expect(f.evictions()).toBeGreaterThanOrEqual(1);
    });

    it('(g bis) an EXPIRED entry is purged before a live one is evicted', () => {
        const f = new Frein(2);
        f.echec([cleCompte('vieille@exemple.test')], T0);
        f.echec([cleCompte('recente@exemple.test')], T0 + FENETRE_MS + 1);
        // The table is full, and the oldest is EXPIRED: the purge must
        // suffice, without any live entry being evicted.
        f.echec([cleCompte('neuve@exemple.test')], T0 + FENETRE_MS + 2);
        expect(f.size()).toBeLessThanOrEqual(2);
        expect(f.evictions()).toBe(0);
    });

    it('(h) `retryApresS` is the REMAINING time, rounded up', () => {
        const f = new Frein();
        const cles = [cleCompte('alice@exemple.test')];
        for (let i = 0; i < ECHECS_MAX_COMPTE; i++) f.echec(cles, T0);
        // At half the window, half remains — not the whole
        // window, which is what a hardcoded constant would return.
        const moitie = FENETRE_MS / 2;
        expect(f.consulter(cles, T0 + moitie).retryApresS).toBe(moitie / 1000);
        // 1 ms remaining rounds to 1 s, never to 0: a `Retry-After: 0`
        // would invite the requester to come back immediately.
        expect(f.consulter(cles, T0 + FENETRE_MS - 1).retryApresS).toBe(1);
    });

    it('(i) a single braked key is enough, and the verdict carries ITS time', () => {
        const f = new Frein();
        const compte = cleCompte('alice@exemple.test');
        const adresse = cleAdresse(AUTRE_ADR);
        // Only the ACCOUNT key is exhausted; the address key is far from
        // its budget. The pair must still be braked.
        for (let i = 0; i < ECHECS_MAX_COMPTE; i++) f.echec([compte, adresse], T0);
        const v = f.consulter([compte, adresse], T0 + 1000);
        expect(v.freine).toBe(true);
        expect(v.retryApresS).toBe(FENETRE_MS / 1000 - 1);
    });

    it('(j) the constants are those the plan sets, and they are exported', () => {
        // They are NOT CALIBRATED (see the module header): this test
        // pins them so that a change is a deliberate gesture, not a drift.
        expect(FENETRE_MS).toBe(15 * 60_000);
        expect(ECHECS_MAX_COMPTE).toBe(5);
        expect(ECHECS_MAX_ADRESSE).toBe(50);
        expect(ENTREES_MAX).toBe(10_000);
        // ⚠️ NOT CALIBRATED EITHER — see `BUDGET_REQUETES` in the module.
        expect(FENETRE_REQUETES_MS).toBe(60_000);
        expect(REQUETES_MAX_ADRESSE).toBe(120);
    });

    // 🔴 THE TEST THAT COUNTS THE MOST IN THIS BATCH. `Frein` COUNTS FAILURES —
    // `echec()`, `succes()`, `BUDGET_COMPTE`, `BUDGET_ADRESSE` — and
    // `BUDGET_REQUETES` reuses the `echec()` METHOD to count a
    // VOLUME, never a failure. If `cleRequetes` and `cleAdresse` produced
    // the SAME key for the SAME address, an active user would exhaust their
    // own budget of AUTHENTICATION FAILURES by simply making
    // legitimate traffic on `/vm` or `/session` — or the reverse, and an
    // attacker would get free password attempts by generating
    // volume. The two MUST therefore live under disjoint prefixes
    // (`req:` versus `adr:`), and it is what this test tests directly on
    // the functions REALLY exported — not on clones local to the
    // file, like tests (a) to (i) above: those test the generic
    // MECHANICS of `Frein`, this one tests the real NAMESPACING of
    // `securite/frein.ts`.
    it("(k) the request budget does NOT eat into the failure budget of the same address", () => {
        // 🔴 FIX (correction round 1, critical ①): the original brief
        // prescribed `i < 10` against a failure budget of 50 — the assertion
        // COULD NOT fail, key merge or not (10 < 50 in BOTH
        // cases). MEASURED: by merging `cleRequetes` and `cleAdresse` on the
        // same prefix, this test stayed GREEN. `ECHECS_MAX_ADRESSE + 1` is the
        // SMALLEST number that makes the merge detectable: exactly at the budget,
        // `Frein` brakes (comparison `>=`, never `>`).
        const frein = new Frein();
        for (let i = 0; i < ECHECS_MAX_ADRESSE + 1; i++) {
            frein.echec([[cleRequetes(ADR), BUDGET_REQUETES]], T0);
        }
        expect(frein.consulter([[cleAdresseDuModule(ADR), BUDGET_ADRESSE]], T0).freine).toBe(
            false,
        );
    });

    it('(k bis) — and CONVERSELY: failures on an address do NOT eat into its request budget', () => {
        // Symmetrical to (k): an attacker who exhausts the FAILURE budget
        // of an address (by getting the password wrong, for example) must
        // NOT see their VOLUME budget eaten into for all that — the two
        // are distinct resources, protecting against distinct abuses.
        //
        // 🔴 SAME FIX AS (k): `ECHECS_MAX_ADRESSE` (50) failures against
        // a request budget of `REQUETES_MAX_ADRESSE` (120) could NOT
        // fail, merge or not (50 < 120 in BOTH cases). `REQUETES_MAX_
        // ADRESSE + 1` is the smallest number that makes the merge detectable
        // on THIS side.
        const frein = new Frein();
        for (let i = 0; i < REQUETES_MAX_ADRESSE + 1; i++) {
            frein.echec([[cleAdresseDuModule(ADR), BUDGET_ADRESSE]], T0 + i);
        }
        expect(
            frein.consulter(
                [[cleRequetes(ADR), BUDGET_REQUETES]],
                T0 + REQUETES_MAX_ADRESSE + 1,
            ).freine,
        ).toBe(false);
    });

    // 🔵 THE FAVOURABLE PROPERTY THE REVIEW FOUND (correction round 1):
    // cross eviction does not bite because `req:` (one minute) ALWAYS expires
    // before `compte:`/`adr:` (fifteen minutes). It holds ONLY if
    // this inequality holds — see `BUDGET_REQUETES` in the module.
    it('(l) 🔴 FENETRE_REQUETES_MS stays SHORTER than FENETRE_MS — otherwise the favourable purge property falls', () => {
        expect(FENETRE_REQUETES_MS).toBeLessThan(FENETRE_MS);
    });

    // 🔴 THE RED ASKED FOR BY THE REVIEW (correction round 1, critical ③):
    // "a refused peer retries in a loop and does not lock its address".
    // Reproduces here, IN TS, the EXACT schedule of
    // `agent/src/plateforme/repli.rs::delai_de_repli` — the remedy the
    // agent fix reuses for `/signal` — and shows that, unlike
    // the flat 500 ms hammering from BEFORE this fix, a peer that backs off
    // this way between two attempts only ever consumes a tiny fraction of the
    // shared budget: the rest remains open to any other request from the
    // same address (the supervisor's control session, other windows).
    it("(m) 🔴 a refused peer that retries according to delai_de_repli (repli.rs) DOES NOT LOCK its address", () => {
        // Faithful copy of `agent/src/plateforme/repli.rs::delai_de_repli` —
        // same constants (`REPLI_MIN_MS`/`REPLI_MAX_MS`), same formula.
        // If one of the two drifts one day without the other following, this test
        // will not see it — it is a COMPARISON, not a guaranteed mirror.
        const REPLI_MIN_MS = 500;
        const REPLI_MAX_MS = 30_000;
        function delaiDeRepli(tentative: number): number {
            const facteur = tentative >= 63 ? Number.MAX_SAFE_INTEGER : 2 ** tentative;
            return Math.min(REPLI_MIN_MS * facteur, REPLI_MAX_MS);
        }

        const frein = new Frein();
        const cle: readonly [string, Budget] = [cleRequetes(ADR), BUDGET_REQUETES];
        const CINQ_MINUTES_MS = 5 * 60_000;
        let instant = T0;
        let tentative = 0;
        let admises = 0;
        // ⚠️ THE LAST SIMULATED INSTANT, NOT A NEW INSTANT chosen after the fact:
        // a new instant could fall exactly on a window
        // boundary (`restant === 0`, edge case documented in `consulter`) and
        // would make the test depend on the chance of that coincidence rather than
        // on the behaviour it tests.
        let lastInstant = instant;
        while (instant < T0 + CINQ_MINUTES_MS) {
            const verdict = frein.consulter([cle], instant);
            if (!verdict.freine) {
                frein.echec([cle], instant);
                admises++;
            }
            lastInstant = instant;
            instant += delaiDeRepli(tentative);
            tentative++;
        }
        // Far, very far from the cap of `REQUETES_MAX_ADRESSE`: the proof
        // that the agent remedy leaves the budget almost entirely available for
        // the OTHER peers of the same address.
        expect(admises).toBeLessThan(REQUETES_MAX_ADRESSE / 4);
        // 🔴 THIS SECOND ASSERTION IS UNABLE TO TURN RED, AND IT IS SAID HERE
        // RATHER THAN LEFT IMPLICIT (review, correction round 2): under
        // ANY spacing that respects even just the documented floor
        // (≥ 500 ms), `admises` can structurally NOT approach
        // `REQUETES_MAX_ADRESSE` over a window of `FENETRE_REQUETES_MS`
        // (60 s) — the assertion above already establishes it. `freine` can therefore
        // NEVER become true here, whatever the REAL quality of the simulated
        // exponential backoff: this line only rephrases the same
        // property in another form, it does not test it
        // independently. **It is `(m bis)`, alone, that carries the proof that the
        // brake can still lock** — by retrying faster than the
        // documented floor, it does turn this same assertion red
        // (`freine === true` there). Kept here for the READABILITY of the
        // scenario ("and so, a legitimate peer is not blocked"), never
        // as a check in its own right.
        expect(frein.consulter([cle], lastInstant).freine).toBe(false);
    });

    // 🔵 NEGATIVE WITNESS OF (m): without the agent fix, a peer that retries
    // FLAT every 500 ms — the behaviour MEASURED before this round —
    // does LOCK the address. Without this witness, (m) would prove
    // nothing: a check never seen red is not a check.
    it('(m bis) — negative witness: a peer that retries FLAT, faster than the documented floor, LOCKS its address', () => {
        // ⚠️ NOT 500 ms: it is PRECISELY `ESPACEMENT_PLANCHER_MS`, the
        // documented FLOOR of `agent/src/relance_pont.rs` (`PERIODE_
        // RELANCE_PONT_MIN`, the former name quoted here before correction round
        // 2, was REMOVED by that round — this quotation, which crosses
        // into Rust from TypeScript, had become a dead reference
        // that NO compiler could catch; fixed by the review
        // of correction round 3), and it coincides —
        // it is a rounding accident — with `REQUETES_MAX_ADRESSE` (120) over
        // a 60 s window: 60000/500 = 120 exactly, so that a
        // hammering at EXACTLY 500 ms NEVER reaches the `>=` threshold
        // (the 121st request arrives RIGHT when the window expires). This witness
        // retries at 200 ms — faster than the floor, which this batch does
        // NOT allow in practice (see `surveillance_pont.rs`), but which
        // remains the best way to test that THE BRAKE ITSELF can
        // still lock an address in the absence of any backoff.
        const frein = new Frein();
        const cle: readonly [string, Budget] = [cleRequetes(ADR), BUDGET_REQUETES];
        const CINQ_MINUTES_MS = 5 * 60_000;
        let instant = T0;
        let admises = 0;
        let lastInstant = instant;
        while (instant < T0 + CINQ_MINUTES_MS) {
            const verdict = frein.consulter([cle], instant);
            if (!verdict.freine) {
                frein.echec([cle], instant);
                admises++;
            }
            lastInstant = instant;
            instant += 200;
        }
        // The pattern of the MEASURED bug, at a cadence more aggressive than the
        // floor: the whole budget is consumed by THIS SINGLE peer, again
        // and again.
        expect(admises).toBeGreaterThanOrEqual(REQUETES_MAX_ADRESSE);
        // And a LEGITIMATE peer arriving just after this burst stays braked
        // — it is exactly "a new window can no longer attach".
        expect(frein.consulter([cle], lastInstant).freine).toBe(true);
    });
});
