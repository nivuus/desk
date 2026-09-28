// ⚠️ **`?raw` and not `node:fs`**: `client/` does not have `@types/node`, and a test
// written with `readFileSync` would pass under vitest — which transpiles with esbuild,
// without checking types — while BREAKING `npm run typecheck`. A trap found
// by sub-block P2 of the platform, and paid for here a second time.
//
// `?raw` requires `test.css: true` in `client/vite.config.ts` for
// stylesheets; on a `.rs` file there is no short-circuit to lift, and the
// test below checks anyway that the text read is not empty.
import rustPressePapier from '../../agent/src/presse_papier.rs?raw';
import { describe, it, expect } from 'vitest';
import {
    PressePapierLocal,
    MESSAGE_ECHEC,
    FAILURES_BEFORE_MESSAGE,
    PRESSE_PAPIER_MAX,
} from './presse-papier';

describe('PressePapierLocal', () => {
    it('returns the received text when the window is focused', () => {
        const pp = new PressePapierLocal();
        pp.recevoir({ texte: 'bonjour', octets: 7 });
        expect(pp.toWrite(true)).toBe('bonjour');
    });

    // 🔴 The deferred deposit: writing without focus would return the text on the
    // first call, and this test falls.
    it('returns nothing without focus, then returns the text when focus comes back', () => {
        const pp = new PressePapierLocal();
        pp.recevoir({ texte: 'bonjour', octets: 7 });
        expect(pp.toWrite(false)).toBeUndefined();
        expect(pp.toWrite(true)).toBe('bonjour');
    });

    // 🔴 "A stale write is impossible": stacking in an array
    // would let the FIRST one out, and this test sees it.
    it('two receptions without focus: the LAST text comes out, never a queue', () => {
        const pp = new PressePapierLocal();
        pp.recevoir({ texte: 'ancien', octets: 6 });
        pp.recevoir({ texte: 'recent', octets: 6 });
        expect(pp.toWrite(true)).toBe('recent');
        pp.confirmer('recent');
        expect(pp.toWrite(true)).toBeUndefined();
    });

    it('does not rewrite what was already written', () => {
        const pp = new PressePapierLocal();
        pp.recevoir({ texte: 'bonjour', octets: 7 });
        pp.confirmer('bonjour');
        expect(pp.toWrite(true)).toBeUndefined();
    });

    // 🔴 Shouting at the FIRST failure would make a permanent banner on a product
    // that works: the first failure is the ordinary case of a window without
    // focus.
    it('says nothing on the first failure, and speaks on the second', () => {
        const pp = new PressePapierLocal();
        expect(pp.echouer()).toBeUndefined();
        expect(pp.echouer()).toBe(MESSAGE_ECHEC);
        expect(FAILURES_BEFORE_MESSAGE).toBe(2);
    });

    it('a success resets the failure counter to zero', () => {
        const pp = new PressePapierLocal();
        expect(pp.echouer()).toBeUndefined();
        pp.confirmer('something');
        expect(pp.echouer()).toBeUndefined();
    });

    // 🔴 The message says HOW to restore, not only that something
    // is missing — same rule as the microphone. The assertion is about an
    // INSTRUCTION substring, never about the mere presence of a message.
    it('the failure message says how to recover', () => {
        expect(MESSAGE_ECHEC).toContain('click');
        expect(MESSAGE_ECHEC).toContain('focus');
    });

    // 🔴 Criterion ③: writing anyway, or keeping the refusal quiet, brings this
    // test down.
    it('a refusal writes nothing and says so, naming the size', () => {
        const pp = new PressePapierLocal();
        pp.recevoir({ texte: null, octets: 102400 });
        expect(pp.toWrite(true)).toBeUndefined();
        const message = pp.refusADire();
        expect(message).toBeDefined();
        expect(message).toContain('100');
    });

    it('a refusal does not overwrite the last remembered text', () => {
        const pp = new PressePapierLocal();
        pp.recevoir({ texte: 'valide', octets: 6 });
        pp.recevoir({ texte: null, octets: 102400 });
        expect(pp.toWrite(true)).toBe('valide');
    });

    // 🔴 Without consumption, the banner would show again on every round.
    it('the refusal is consumed: two calls return only one message', () => {
        const pp = new PressePapierLocal();
        pp.recevoir({ texte: null, octets: 102400 });
        expect(pp.refusADire()).toBeDefined();
        expect(pp.refusADire()).toBeUndefined();
    });
});

// ---------------------------------------------------------------------------
// Sub-block P2 — D5's guard no. 3: the page NEVER re-emits towards the agent a
// content it has just received from it.
// ---------------------------------------------------------------------------

describe('PressePapierLocal.aEmettre — guard no. 3', () => {
    // 🔴 IT IS GUARD NO. 3. RED if `aEmettre` always returns its argument:
    // the agent writes T into the Windows clipboard, the Sondeur reads it back,
    // pushes it to the page, the page writes it locally — and if the user pastes
    // then, the page sends it back to the agent. One round trip per paste.
    it('silences a text that was just received', () => {
        const etat = new PressePapierLocal();
        etat.recevoir({ texte: 'x', octets: 1 });
        expect(etat.aEmettre('x')).toBeUndefined();
    });

    // RED if the guard blocked EVERYTHING after a reception. Without this test, an
    // `aEmettre` that always returned `undefined` would pass the previous one — and
    // pasting would no longer work at all.
    it('lets a different text through', () => {
        const etat = new PressePapierLocal();
        etat.recevoir({ texte: 'x', octets: 1 });
        expect(etat.aEmettre('y')).toBe('y');
    });

    // RED if the initial state compared to the empty string: pasting an empty
    // string would then be mute from the first gesture.
    it('lets through before any reception', () => {
        const etat = new PressePapierLocal();
        expect(etat.aEmettre('x')).toBe('x');
        expect(etat.aEmettre('')).toBe('');
    });

    // 🔴 The guard only holds for the FIRST send-back. A user who pastes
    // the same text twice wants it twice — and the agent will not rewrite
    // for nothing: it is its guard no. 2 that absorbs the duplicate, on the VM side.
    // RED if the marker is permanent instead of being consumable.
    it('silences only the FIRST echo', () => {
        const etat = new PressePapierLocal();
        etat.recevoir({ texte: 'x', octets: 1 });
        expect(etat.aEmettre('x')).toBeUndefined();
        expect(etat.aEmettre('x')).toBe('x');
    });

    // RED if the guard took a REFUSAL for received content: nothing was
    // written locally, so nothing can be an echo.
    it('a refusal does not arm the guard', () => {
        const etat = new PressePapierLocal();
        etat.recevoir({ texte: null, octets: 100_000 });
        expect(etat.aEmettre('x')).toBe('x');
    });

    // RED if the marker were set by `recevoir` of a text THAT WAS NOT
    // WRITTEN: a text received without focus stays pending, and the user can
    // very well paste an identical text from elsewhere in the meantime. The case
    // is indistinguishable and the choice is to stay silent — but then the marker
    // must come from `recevoir`, and this test freezes that choice rather than
    // letting it depend on focus.
    it('arms the guard even when the local write has not happened yet', () => {
        const etat = new PressePapierLocal();
        etat.recevoir({ texte: 'x', octets: 1 });
        expect(etat.toWrite(false)).toBeUndefined();
        expect(etat.aEmettre('x')).toBeUndefined();
    });
});

/// 🔴 **THE BOUND IS WRITTEN IN TWO LANGUAGES THAT NO `import` LINKS**, and
/// this test is the only thing that keeps them from diverging silently.
///
/// The repository paid for this class in sub-block P2 of the platform, and the remedy
/// used is the same: **re-read the source file of the other language** rather
/// than hoping someone will think of both.
///
/// RED if one of the two values moves without the other.
describe('PRESSE_PAPIER_MAX', () => {
    it("is worth what the Rust agent declares", () => {
        const trouve = /pub const PRESSE_PAPIER_MAX: usize = ([^;]+);/.exec(rustPressePapier);
        // ⚠️ Without this assertion, a rename on the Rust side would make `trouve`
        // null and the test would pass while measuring NOTHING — the vacuous check
        // this repository has been paying for since D7.
        expect(trouve, "the Rust constant was not found").not.toBeNull();
        // eslint-disable-next-line no-eval
        const rustValue = Number(new Function(`return ${trouve![1].replace(/_/g, '')}`)());
        expect(PRESSE_PAPIER_MAX).toBe(rustValue);
    });
});
