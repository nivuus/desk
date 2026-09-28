/**
 * THE STYLESHEET READER — the common tool of ⑥'s shape guards.
 *
 * PURE: no DOM, no `fs`, no path. It knows NO design rule — it
 * blanks, it cuts, it names. Every rule belongs to its caller.
 *
 * 🔴 EXTRACTED FROM `primitives.test.ts` BY TASK 1 OF SUB-BLOCK S4, AND BEFORE
 * ANY ADDITION. That file was at 283 lines for a gate at 300 (spec
 * §10), margin 17 — the tightest of `client/` after `verify-webrtc.mjs` —, and
 * S4 adds two guards to it that read stylesheets. "We extract before
 * adding": it is `CLAUDE.md`'s doctrine (task 6 of D9, tasks 1 to 3
 * of D10), and this repository has twice paid for "a comment addition cancels
 * an extraction".
 *
 * ⚠️ DECLARED DIVERGENCE. The landing point S2 then S3 name for
 * `primitives.test.ts` is "split by object — the shape guards on one side,
 * the family guards on the other, without separating G5 from its source". This
 * extraction is DIFFERENT: it moves out the TOOL, not the guards, and leaves
 * G5 next to its source. It is COMPLEMENTARY, not a substitute — the named
 * landing point stays open, and stays the right one if the file grows again.
 *
 * ═══════════════════════════════════════════════════════════════════════════
 * 🔴 WHY BLANKING LIVES HERE, AND NOT IN EACH GUARD.
 *
 * A guard looking for a substring in the RAW text is satisfied by the
 * comment of the file it analyses — this repository's stylesheets write
 * at length WHY a given value is forbidden there, so they write that
 * value. Three occurrences paid for: S1 on `CLE_THEME`, S2 on G1 and G5, S3
 * on its red no. 16. Copying it into each guard would make it diverge from one
 * guard to another without any command saying so.
 *
 * ⚠️ WHAT IT DOES NOT DO: it understands neither CSS strings (`content: "/*"`),
 * nor unquoted `url()`s, nor native nesting. No stylesheet of
 * `client/src/` carries any — noted, not assumed —, and the day one
 * does, it is here that it must be said rather than in its caller.
 * ═══════════════════════════════════════════════════════════════════════════
 */

/** Removes `/* … *​/` comments — see the header. */
export function sansCommentaires(css: string): string {
    return css.replace(/\/\*[\s\S]*?\*\//g, ' ');
}

/** Everything preceding a `{`. At-rules (`@media …`) start with `@`. */
export function preludes(css: string): string[] {
    return [...css.matchAll(/([^{}]+)\{/g)].map((m) => m[1].trim()).filter(Boolean);
}

export interface Declaration {
    propriete: string;
    valeur: string;
}

/**
 * The declarations of the innermost blocks. `[^{}]*` crosses neither `{`
 * nor `}`: the body of an `@media` is therefore never taken for a declaration.
 */
export function declarationsDe(css: string): Declaration[] {
    const sortie: Declaration[] = [];
    for (const bloc of css.matchAll(/\{([^{}]*)\}/g)) {
        for (const morceau of bloc[1].split(';')) {
            const coupe = morceau.indexOf(':');
            if (coupe === -1) continue;
            sortie.push({
                propriete: morceau.slice(0, coupe).trim(),
                valeur: morceau.slice(coupe + 1).trim(),
            });
        }
    }
    return sortie;
}

/** The body of the block following `index`, braces matched. */
export function blocApres(css: string, index: number): string {
    const debut = css.indexOf('{', index);
    if (debut === -1) return '';
    let profondeur = 0;
    for (let i = debut; i < css.length; i += 1) {
        if (css[i] === '{') profondeur += 1;
        else if (css[i] === '}') {
            profondeur -= 1;
            if (profondeur === 0) return css.slice(debut + 1, i);
        }
    }
    return '';
}

/** `.carte > .carte__titre` rend `['.carte', '.carte__titre']`. */
export const compounds = (selecteur: string): string[] =>
    selecteur.split(/[\s>+~]+/).filter(Boolean);
