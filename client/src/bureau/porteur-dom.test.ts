// ⚠️ NO `@vitest-environment` directive: these two functions are PURE,
// and `client/` has neither jsdom nor happy-dom — by convention, not by oversight
// (`accent-dom.test.ts`). That is why they are exported separately
// from the rest of the module, which touches the DOM and is not tested.
import { describe, expect, it } from 'vitest';
import { diffuserSiChange, nomDuVerrou, type DepsBureauPage } from './porteur-dom';

describe('nomDuVerrou', () => {
    it('carries the VM PREFIX', () => {
        // 🔴 WITHOUT IT, TWO VMs OPEN IN TWO TABS WOULD EXCLUDE EACH
        // OTHER -- the defect P3 fixed on the session name,
        // reintroduced through the back door.
        expect(nomDuVerrou('vm-7')).toBe('vm-7:nivuus-bureau');
    });

    it('without a known prefix, returns the bare name', () => {
        expect(nomDuVerrou('')).toBe('nivuus-bureau');
    });
});

describe('diffuserSiChange', () => {
    it('broadcasts NOTHING when the state is identical', () => {
        // ⚠️ The holder redraws at 1 Hz: broadcasting on every round would wake
        // all tabs once per second for nothing.
        const envoyes: unknown[] = [];
        const canal = { postMessage: (m: unknown) => void envoyes.push(m) };
        const list = [{ session: 's', titre: 'x', ouverte: true }];
        let last = '';
        last = diffuserSiChange(canal, list, last);
        last = diffuserSiChange(canal, list, last);
        expect(envoyes.length).toBe(1);
        // ⚠️ MINOR round 1: `last` reassigned and never read again suggested an
        // absent assertion. It carries the fingerprint -- check that it IS
        // the one of the stable list, never a forgotten empty string.
        expect(last).toBe(JSON.stringify(list));
    });

    it('broadcasts when a window changes state', () => {
        const envoyes: unknown[] = [];
        const canal = { postMessage: (m: unknown) => void envoyes.push(m) };
        let last = diffuserSiChange(canal, [{ session: 's', titre: 'x', ouverte: true }], '');
        last = diffuserSiChange(canal, [{ session: 's', titre: 'x', ouverte: false }], last);
        expect(envoyes.length).toBe(2);
        // Same reason as above: the fingerprint returned follows the LAST
        // broadcast, not the first.
        expect(last).toBe(JSON.stringify([{ session: 's', titre: 'x', ouverte: false }]));
    });
});

/* ══ WHAT THE FINAL REVIEW OF AUGUST 31ST 2026 ADDED ═════════════════════ */

describe('DepsBureauPage: the token is a PROVIDER, never a string', () => {
    // 🔴 WHAT IS FROZEN HERE IS NOT THE FRESHNESS RULE -- `jeton.test.ts`
    // has held it since task 1 -- BUT THE JUNCTION. The field carried a
    // STRING, captured when the page loaded; yet `ouvrirLaSession` only
    // runs, for a follower, at the moment of its PROMOTION, potentially
    // hours later, and `DUREE_JETON_ACCES_MS` is TEN MINUTES
    // (`plateforme/src/identite/jeton.ts`). A test of `assurerAccesFrais`
    // would have seen nothing: it is this coupling that was wrong.
    //
    // ⚠️ THESE ELEMENTS ARE NEVER TOUCHED: `installerLeBureau` is NOT
    // called here, and cannot be -- `client/` has neither jsdom nor
    // happy-dom, by convention. This test freezes a TYPE CONTRACT, and its judge
    // is `tsc --noEmit`, not Vitest (which transpiles without checking types).
    const elements = {} as DepsBureauPage['elements'];

    it('exposes `jetonFrais`, a FUNCTION the promotion can call again', async () => {
        const deps: DepsBureauPage = {
            signalingUrl: 'ws://exemple/signal',
            jetonFrais: () => Promise.resolve('frais'),
            prefixe: 'vm-7',
            fautesArmees: false,
            elements,
        };
        expect(typeof deps.jetonFrais).toBe('function');
        await expect(deps.jetonFrais()).resolves.toBe('frais');
    });

    it('REFUSES a scalar token -- assertion held by `tsc --noEmit`', () => {
        const deps: DepsBureauPage = {
            signalingUrl: 'ws://exemple/signal',
            jetonFrais: () => Promise.resolve(undefined),
            prefixe: '',
            fautesArmees: false,
            elements,
            // 🔴 THE DIRECTIVE IS THE ASSERTION. `tsc` FAILS on an
            // "Unused '@ts-expect-error' directive" the day this field
            // became legal again -- that is, the day the defect were
            // reintroduced. Vitest checks no types at all:
            // it is `npm run typecheck` that judges, and it is mandatory.
            // @ts-expect-error a FROZEN token no longer belongs in these deps
            jeton: 'a string captured at load time',
        };
        expect('jeton' in deps).toBe(true);
    });
});
