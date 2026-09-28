// The PURE RULES of the upload HTTP surface: recognising a path,
// saying which method each one expects, judging the shape of a hash.
//
// 🔴 EXTRACTED BEFORE THE ADDITION, AND IT IS THE REPOSITORY RULE, NOT A TASTE.
// `routes-televersement.ts` was at 497 lines for a cap of 500: the
// slightest comment line from a cross-cutting review would have made it CROSS,
// and the repository paid TWICE in D9 for having caught it up with a COMPRESSION
// it explicitly forbids. The extraction therefore happens BEFORE, never after —
// that is the strong form, the one of D9 task 6 and of the three tasks of D10.
// Precedents of the shape: `http/routes-harnais.ts`, `apps/magasin-tranches.ts`.
//
// 🔴 THIS MODULE IS PURE: no database, no socket, no clock, no
// `node:` — the property that makes it testable without standing up a server.

export type Cible =
    | { quoi: 'creer' }
    | { quoi: 'etat'; id: string }
    | { quoi: 'tranche'; id: string; rang: string }
    | { quoi: 'sceller'; id: string };

/// Recognises the FOUR paths, and NOTHING else.
///
/// 🔴 SPLIT BY SEGMENTS, NEVER BY `startsWith`, AND IT IS NOT STYLE:
/// G1 MEASURED that a `startsWith('/application')` left SEVENTEEN tests GREEN
/// — the route ate the whole family and returned ITS OWN typed 404,
/// indistinguishable from the generic one as long as only the status was read. The check
/// that counts compares the BODY. Each pattern is anchored at BOTH ends: EXACT
/// segment count, constant segments compared by equality.
///
/// ⚠️ `URL.pathname` DOES NOT PERCENT-DECODE, and that is intended:
/// `/televersement/..%2F..%2Fetc/tranche/0` arrives as ONE single segment, which
/// `identifiantValide` will refuse — decoding first would make separators
/// appear that the splitting would take for legitimate segments.
export function reconnaitre(chemin: string): Cible | undefined {
    const s = chemin.split('/');
    if (s[1] !== 'televersement') return undefined;
    // ['', 'televersement'] — exactement deux.
    if (s.length === 2) return { quoi: 'creer' };
    // Three: `/televersement/` has three as well, but its identifier is
    // empty — that is one slash too many, not a path.
    if (s.length === 3) return s[2] === '' ? undefined : { quoi: 'etat', id: s[2] };
    if (s.length === 4) {
        return s[2] === '' || s[3] !== 'sceller' ? undefined : { quoi: 'sceller', id: s[2] };
    }
    if (s.length === 5) {
        if (s[2] === '' || s[3] !== 'tranche' || s[4] === '') return undefined;
        return { quoi: 'tranche', id: s[2], rang: s[4] };
    }
    return undefined;
}

/// ⚠️ A table rather than four `if`s: the mapping is exhaustive BY
/// TYPING, so that a fifth target would not compile without its method.
export const METHODE: Readonly<Record<Cible['quoi'], string>> = Object.freeze({
    creer: 'POST',
    etat: 'GET',
    tranche: 'PUT',
    sceller: 'POST',
});

/// ⚠️ LOWERCASE ONLY, like `apps/icones.ts::empreinteValide`: two
/// spellings of the same hash would compare UNEQUAL at sealing, and the
/// uploader would endlessly upload a correct file again.
export function empreinteValide(s: string): boolean {
    return /^[0-9a-f]{64}$/.test(s);
}
