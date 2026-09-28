/// <reference types="vite/client" />
/**
 * Tests of the accent DOM wiring.
 *
 * ⚠️ `client/` has **neither jsdom nor happy-dom**: dependencies are injected,
 * and that is what makes this module testable where `main.ts` is not.
 *
 * 🔴 **No colour as a literal** — see the header of `accent.test.ts`: §7.2
 * scans the `.ts` files of `client/src/`, and its exclusion only covers the base layer.
 * Everything is READ from `tokens/couleurs.css` (`tokens.css` before the extraction of
 * task 6, August 25th, 2026).
 */

import { describe, expect, it } from 'vitest';
import { lireBlocsDeTheme } from './design/tokens';
import tokensCss from './design/tokens/couleurs.css?raw';
import { attacherAccentAuDOM, TOKEN_ACCENT, type AccesTokens } from './accent-dom';

const blocs = lireBlocsDeTheme(tokensCss);
const bloc = (nom: string) => blocs.find((b) => b.nom === nom)!.tokens;
const sombre = bloc('racine');
const clairBrut = bloc('attribut-clair');

const NOMS = ['--fond-0', '--fond-1', '--fond-2', '--accent'] as const;
const theme = (t: Map<string, string>, repli: Map<string, string>): Record<string, string> =>
    Object.fromEntries(NOMS.map((n) => [n, t.get(n) ?? repli.get(n)!]));

const SOMBRE = theme(sombre, sombre);
const CLAIR = theme(clairBrut, sombre);

/// `--succes` of the dark block: **8.867 / 8.186 / 7.446** on the DARK
/// backgrounds, and **2.194 / 2.047 / 1.889** on the LIGHT ones — measured on
/// August 21st, 2026. That is what makes it the exact fixture of the switch test:
/// the SAME colour is accepted in one theme and refused in the other.
const READABLE_IN_DARK_ONLY = sombre.get('--succes')!;

/// A fake root that counts its reads: this counter is what tells
/// "the backgrounds are re-read" from "the backgrounds were memorised at mount".
function racine(depart: Record<string, string>) {
    const poses: Array<[string, string]> = [];
    const lus: string[] = [];
    let current = depart;
    const acces: AccesTokens = {
        lireToken: (nom) => {
            lus.push(nom);
            return current[nom] ?? '';
        },
        poserToken: (nom, value) => {
            poses.push([nom, value]);
        },
    };
    return { acces, poses, lus, basculer: (t: Record<string, string>) => (current = t) };
}

describe('attacherAccentAuDOM', () => {
    it('a COMPLIANT colour is set on the root', () => {
        // RED: the untouched tree before the module existed.
        const r = racine(SOMBRE);
        attacherAccentAuDOM(r.acces).recevoir(READABLE_IN_DARK_ONLY);
        expect(r.poses).toEqual([[TOKEN_ACCENT, READABLE_IN_DARK_ONLY]]);
    });

    it('a REFUSED colour makes the theme accent be set, never nothing', () => {
        // RED: setting nothing on refusal ⟹ the token would keep its PREVIOUS
        // value, that is the tint of an icon that is no longer the one
        // of this window — a stale state, more misleading than a visible fallback.
        const r = racine(SOMBRE);
        const a = attacherAccentAuDOM(r.acces);
        a.recevoir(READABLE_IN_DARK_ONLY);
        a.recevoir(sombre.get('--bord')!); // 1,447 / 1,336 / 1,215 : illisible
        expect(r.poses).toEqual([
            [TOKEN_ACCENT, READABLE_IN_DARK_ONLY],
            [TOKEN_ACCENT, SOMBRE['--accent']],
        ]);
    });

    it('the THREE backgrounds and the accent are re-read on EACH message', () => {
        // 🔴 RED: memorising the backgrounds at mount ⟹ a theme switch
        // would leave the accent judged against the OLD theme. The SAME colour is
        // accepted in dark and refused in light: without the re-read, the second
        // message would set the colour instead of the light accent.
        const r = racine(SOMBRE);
        const a = attacherAccentAuDOM(r.acces);
        a.recevoir(READABLE_IN_DARK_ONLY);
        expect(r.lus).toEqual(['--fond-0', '--fond-1', '--fond-2', '--accent']);

        r.basculer(CLAIR);
        a.recevoir(READABLE_IN_DARK_ONLY);
        expect(r.lus).toHaveLength(8);
        expect(r.poses[1]).toEqual([TOKEN_ACCENT, CLAIR['--accent']]);
        expect(CLAIR['--accent']).not.toBe(SOMBRE['--accent']);
    });

    it('the token is set on the ROOT, and the module knows no element', () => {
        // 🔴 THIS IS THE REPLACEMENT RED OF CRITERION ② (E7 of the plan): the red
        // the spec prescribed — "set the token without declaring it in the
        // three blocks, §7.4 fails" — is VACUOUS under D-A1-2, where there is
        // NO declaration: §7.4 would see nothing.
        //
        // The playable red is to set it on `document.body`. Then
        // `getComputedStyle(document.documentElement).getPropertyValue(...)`
        // returns the EMPTY STRING, and criterion ② goes red on the assertion it
        // states. This test is the half of it that can be exercised on the host: the module
        // receives NO means of designating an element — its interface does not
        // carry one —, so it cannot acquire one by accident.
        //
        // ⚠️ A red that sent NO message would be worse: it
        // would also return the empty string, and the empty string is what an
        // entirely dead mechanism returns. That is the lesson of red ① of P3.
        const r = racine(SOMBRE);
        attacherAccentAuDOM(r.acces).recevoir(READABLE_IN_DARK_ONLY);
        expect(r.poses.every(([nom]) => nom === TOKEN_ACCENT)).toBe(true);
        expect(Object.keys(r.acces)).toEqual(['lireToken', 'poserToken']);
    });

    it('a root whose tokens are ABSENT makes the empty string be set, without throwing', () => {
        // RED: not checking the shape of the backgrounds in `conformer` ⟹
        // `rapportDeContraste` THROWS on the empty string, and an exception in a
        // data channel message handler kills the session without a
        // word. The case is REAL: `getComputedStyle` returns the empty string for an
        // absent token — so for any page whose base layer is not linked yet.
        const r = racine({});
        attacherAccentAuDOM(r.acces).recevoir(READABLE_IN_DARK_ONLY);
        expect(r.poses).toEqual([[TOKEN_ACCENT, '']]);
    });
});
