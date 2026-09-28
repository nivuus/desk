import { describe, expect, it } from 'vitest';

import { estUnRaccourciDeCollage, type ToucheObservee } from './raccourcis';

/// Builds an observed key: everything is false by default, only what is
/// named is true. That is what makes each case of the table readable.
function touche(partiel: Partial<ToucheObservee> & { code: string }): ToucheObservee {
    return { ctrlKey: false, shiftKey: false, altKey: false, metaKey: false, ...partiel };
}

describe('estUnRaccourciDeCollage', () => {
    it('recognises Ctrl+V', () => {
        expect(estUnRaccourciDeCollage(touche({ ctrlKey: true, code: 'KeyV' }))).toBe(true);
    });

    // The probe of August 20th, 2026 establishes that `Shift+Insert` produces the SAME
    // trusted `paste`, in the four cells where it is exercised and over the two
    // runs (`journaux-presse-papier-p2/p2-paste-video-{1,2}.json`). D6
    // named it without having measured it.
    it('recognises Shift+Insert', () => {
        expect(estUnRaccourciDeCollage(touche({ shiftKey: true, code: 'Insert' }))).toBe(true);
    });

    // 🔴 THIS IS RISK R5 OF THE SPEC, AND THESE THREE CASES ARE THE WHOLE DEFENCE.
    // RED if the condition tests `e.ctrlKey` alone: `Ctrl+W` would close the
    // session window, `Ctrl+T` would open a tab, `Ctrl+N` a window —
    // because P2 then removes the `preventDefault` that held them back.
    it.each(['KeyW', 'KeyT', 'KeyN'])('refuse Ctrl+%s (risque R5)', (code) => {
        expect(estUnRaccourciDeCollage(touche({ ctrlKey: true, code }))).toBe(false);
    });

    // RED if `!e.shiftKey` is omitted. It is "paste without formatting"
    // in several applications, and it MUST stay with the Windows product.
    it('refuse Ctrl+Shift+V', () => {
        expect(
            estUnRaccourciDeCollage(touche({ ctrlKey: true, shiftKey: true, code: 'KeyV' })),
        ).toBe(false);
    });

    // RED if `!e.altKey` is omitted.
    it('refuse Ctrl+Alt+V', () => {
        expect(estUnRaccourciDeCollage(touche({ ctrlKey: true, altKey: true, code: 'KeyV' }))).toBe(
            false,
        );
    });

    // RED if `!e.metaKey` is omitted. On macOS it would be the native paste;
    // the product does NOT handle it in v1, and this test freezes the decision rather than
    // leaving it to the judgement of the next reader.
    it('refuse Meta+V', () => {
        expect(estUnRaccourciDeCollage(touche({ metaKey: true, code: 'KeyV' }))).toBe(false);
    });

    // RED if only `code` is tested.
    it('refuses V alone', () => {
        expect(estUnRaccourciDeCollage(touche({ code: 'KeyV' }))).toBe(false);
    });

    // RED if the `Insert` branch does not require `!e.ctrlKey`.
    it('refuse Ctrl+Shift+Insert', () => {
        expect(
            estUnRaccourciDeCollage(touche({ ctrlKey: true, shiftKey: true, code: 'Insert' })),
        ).toBe(false);
    });

    // RED if the `Insert` branch does not require `!e.altKey` / `!e.metaKey`.
    it('refuse Shift+Alt+Insert et Shift+Meta+Insert', () => {
        expect(
            estUnRaccourciDeCollage(touche({ shiftKey: true, altKey: true, code: 'Insert' })),
        ).toBe(false);
        expect(
            estUnRaccourciDeCollage(touche({ shiftKey: true, metaKey: true, code: 'Insert' })),
        ).toBe(false);
    });

    // The modifiers themselves are NOT paste shortcuts: their
    // `preventDefault` is kept, and the probe establishes that this does NOT prevent
    // the `paste` from arriving (`ControlLeft` carries `dp: true`, `KeyV` carries
    // `dp: false`, and the `paste` is received — two runs).
    it.each(['ControlLeft', 'ControlRight', 'ShiftLeft', 'ShiftRight'])(
        'refuses the modifier %s alone',
        (code) => {
            expect(estUnRaccourciDeCollage(touche({ ctrlKey: true, code }))).toBe(false);
            expect(estUnRaccourciDeCollage(touche({ shiftKey: true, code }))).toBe(false);
        },
    );
});
