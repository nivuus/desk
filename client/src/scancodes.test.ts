import { describe, expect, it } from 'vitest';
import { SCANCODES } from './scancodes';

describe('table de scancodes', () => {
    it('conserve les touches déjà présentes', () => {
        expect(SCANCODES.KeyA).toEqual({ scancode: 0x1e, extended: false });
        expect(SCANCODES.ArrowUp).toEqual({ scancode: 0x48, extended: true });
        expect(SCANCODES.F11).toEqual({ scancode: 0x57, extended: false });
    });

    it('couvre le pavé numérique, absent jusqu\'ici', () => {
        expect(SCANCODES.Numpad0).toEqual({ scancode: 0x52, extended: false });
        expect(SCANCODES.Numpad5).toEqual({ scancode: 0x4c, extended: false });
        expect(SCANCODES.Numpad9).toEqual({ scancode: 0x49, extended: false });
        expect(SCANCODES.NumpadAdd).toEqual({ scancode: 0x4e, extended: false });
        expect(SCANCODES.NumpadSubtract).toEqual({ scancode: 0x4a, extended: false });
        expect(SCANCODES.NumpadMultiply).toEqual({ scancode: 0x37, extended: false });
        expect(SCANCODES.NumLock).toEqual({ scancode: 0x45, extended: false });
    });

    it('distingue le point du pavé numérique de la touche Suppr', () => {
        // Même scancode, seul le préfixe étendu les sépare — c'est
        // exactement ce que porte le drapeau `extended`.
        expect(SCANCODES.NumpadDecimal).toEqual({ scancode: 0x53, extended: false });
        expect(SCANCODES.Delete).toEqual({ scancode: 0x53, extended: true });
    });

    it('n\'attribue jamais deux fois la même paire scancode/étendu', () => {
        const vues = new Set<string>();
        for (const [code, entree] of Object.entries(SCANCODES)) {
            const cle = `${entree.scancode}/${entree.extended}`;
            expect(vues.has(cle), `doublon sur ${code} (${cle})`).toBe(false);
            vues.add(cle);
        }
    });
});
