/**
 * Parser of `tokens.css` — PURE: no DOM, no `fs`, no path.
 *
 * Three checks share it (§7.1 contrast, §7.4 block equality, §7.6
 * orphans) rather than each having its own copy of the values: "a check
 * that has its own copy of the values validates its copy" (spec §7.1). Reading
 * the disk belongs to the `.mjs` files of `client/outils/`.
 *
 * ⚠️ THIS MODULE IS IMPORTED BY UNTYPECHECKED `.mjs`, through Node's native
 * type stripping (measured on v24.9.0). It must therefore stay
 * "ERASABLE" TypeScript: no `enum`, no `namespace`, no constructor
 * property, no decorator. The cost is measured, not assumed — an
 * `export enum T { A, B }` imported the same way makes Node crash:
 *
 *     $ node runenum.mjs
 *     .../enum.ts:1
 *     export enum T { A, B }
 *
 * ⚠️ It works on TEXT. An `import { readFileSync } from 'node:fs'`
 * here would be rejected by `npm run typecheck`: `client/node_modules/@types/` only
 * carries `estree`, neither `@types/node` nor `jsdom` (P2 measured it on `Buffer`,
 * TS2580).
 */

export interface BlocDeTheme {
    /** `'racine'` | `'media-clair'` | `'attribut-clair'`. */
    nom: string;
    /** Token name (dashes included) → literal value, as written. */
    tokens: Map<string, string>;
    /** The block's raw body, for properties that are not `--*`. */
    corps: string;
}

/** Blanks out comments while keeping line breaks. */
function sansCommentaires(css: string): string {
    return css.replace(/\/\*[\s\S]*?\*\//g, (bloc) => bloc.replace(/[^\n]/g, ' '));
}

/** Index of the `}` closing the `{` located at `ouvrante`. -1 if the CSS is truncated. */
function fermetureDe(css: string, ouvrante: number): number {
    let profondeur = 0;
    for (let i = ouvrante; i < css.length; i += 1) {
        if (css[i] === '{') profondeur += 1;
        else if (css[i] === '}') {
            profondeur -= 1;
            if (profondeur === 0) return i;
        }
    }
    return -1;
}

function tokensDuCorps(corps: string): Map<string, string> {
    const tokens = new Map<string, string>();
    for (const m of corps.matchAll(/(--[\w-]+)\s*:\s*([^;]+);/g)) {
        tokens.set(m[1], m[2].trim());
    }
    return tokens;
}

/**
 * Splits the text into ONE BLOCK PER PHYSICAL OCCURRENCE of `:root[…]{…}`, WITHOUT
 * MERGING — the count it returns is therefore ABLE to exceed three, and
 * that is precisely why it exists separately from `lireBlocsDeTheme`
 * below.
 *
 * 🔴 FIX FROM THE REVIEW OF TASK 6 (August 25th, 2026, round 1): the merging
 * of `lireBlocsDeTheme` had been added inside the only splitting
 * loop, without any PRE-merge count remaining accessible. Measured
 * result: a surplus `:root { --e-4: 999rem; }` added to
 * `tokens/echelles.css` (a real regression, every `--e-4` going from 1rem
 * to 999rem) melted into the existing `racine` block WITHOUT CHANGING THE
 * LOGICAL BLOCK COUNT, which stays bounded at 3 by construction — the only
 * guard able to see it (`expect(blocs).toHaveLength(3)`, on the merged
 * count) COULD NO LONGER turn red. This function restores a count the
 * merging cannot mask: see `tokensDeclares`… no, see the dedicated
 * test in `tokens.test.ts`, which exercises it on the REAL concatenated content and
 * replays the exact regression above.
 */
export function lireBlocsBruts(css: string): BlocDeTheme[] {
    const propre = sansCommentaires(css);

    const plagesMedia: Array<[number, number]> = [];
    for (const m of propre.matchAll(/@media[^{]*\{/g)) {
        const fin = fermetureDe(propre, m.index + m[0].length - 1);
        if (fin !== -1) plagesMedia.push([m.index, fin]);
    }
    const inMedia = (i: number) => plagesMedia.some(([d, f]) => i > d && i < f);

    const blocs: BlocDeTheme[] = [];
    for (const m of propre.matchAll(/(:root[^{}]*)\{/g)) {
        const selecteur = m[1];
        const ouvrante = m.index + m[0].length - 1;
        const fin = fermetureDe(propre, ouvrante);
        if (fin === -1) continue;
        const corps = propre.slice(ouvrante + 1, fin);

        let nom = 'racine';
        if (/\[data-theme\s*=\s*["']clair["']\]/.test(selecteur)) nom = 'attribut-clair';
        else if (inMedia(m.index)) nom = 'media-clair';

        blocs.push({ nom, tokens: tokensDuCorps(corps), corps });
    }
    return blocs;
}

/**
 * Splits the text of `tokens.css` into its three LOGICAL theme blocks, in
 * document order. The `racine` block is the one WITHOUT a condition — it is the one
 * carrying the dark palette, the out-of-theme tokens and the scales
 * (spec §4.2, §4.4).
 *
 * 🔴 MERGES OCCURRENCES OF THE SAME NAME, SINCE THE EXTRACTION OF TASK 6
 * (August 25th, 2026): `tokens/couleurs.css` and `tokens/echelles.css` each
 * declare their own UNCONDITIONAL `:root {}`, and the text passed here
 * is their CONCATENATION — two physical occurrences of the same logical block
 * `racine`. The browser already unites them through the cascade; without this merge,
 * this parser would return TWO blocks named `racine`, and any caller that
 * looks for ONE (`Array.find`, or a `Map` keyed by name, which only keeps
 * the LAST) would silently lose the other's tokens — exactly the
 * defect `tokens.test.ts` and `reprise.test.ts` exist never to
 * let through. Before the extraction, a single file could only produce
 * ONE occurrence per name: this merge therefore changes NOTHING to the reading
 * of a text that has only one — it only makes the two-occurrence case correct.
 *
 * 🔴 BACKWARD COMPATIBLE ON VALUES, REGRESSIVE ON THE GUARD — and that is
 * why `lireBlocsBruts` exists: the merge, by bounding the count of
 * LOGICAL blocks at 3 by construction, takes away from the ONLY guard that compared this
 * count (`tokens.test.ts`) the ability to denounce a surplus `:root`. Any
 * caller wanting to detect a duplication must compare the count of
 * `lireBlocsBruts` (variable, able to exceed 3), never this one.
 */
export function lireBlocsDeTheme(css: string): BlocDeTheme[] {
    const fusionnes = new Map<string, BlocDeTheme>();
    for (const bloc of lireBlocsBruts(css)) {
        const existant = fusionnes.get(bloc.nom);
        if (!existant) {
            fusionnes.set(bloc.nom, bloc);
            continue;
        }
        for (const [cle, value] of bloc.tokens) existant.tokens.set(cle, value);
        existant.corps += `\n${bloc.corps}`;
    }
    return [...fusionnes.values()];
}

/**
 * The value of a property that is NOT a token — `color-scheme` in
 * particular (divergence D9). It counts neither in the equality of §7.4 nor
 * in the orphans of §7.6, so nothing would guard it without this accessor.
 */
export function propertyValue(bloc: BlocDeTheme, propriete: string): string | null {
    const m = bloc.corps.match(new RegExp(`(?:^|[;{\\s])${propriete}\\s*:\\s*([^;]+);`));
    return m ? m[1].trim() : null;
}

/** All declared tokens, whatever the block. */
export function tokensDeclares(css: string): Set<string> {
    const noms = new Set<string>();
    for (const bloc of lireBlocsDeTheme(css)) {
        for (const nom of bloc.tokens.keys()) noms.add(nom);
    }
    return noms;
}

/** All `var(--…)` referenced by a CSS text. Comments are excluded. */
export function tokensReferences(css: string): Set<string> {
    const references = new Set<string>();
    for (const m of sansCommentaires(css).matchAll(/var\(\s*(--[\w-]+)/g)) {
        references.add(m[1]);
    }
    return references;
}

/**
 * The set differences between blocks. Empty = compliant.
 *
 * 🔴 DELIBERATE DIVERGENCE FROM THE LETTER OF §7.4, and it is substantive. The spec
 * writes "the THREE blocks declare the same set of names […] set
 * equality, both ways". Taken literally, this check is
 * RED FOREVER on a correct `tokens.css`: §4.5 requires the six
 * out-of-theme tokens to be "declared only once and never redefined"
 * — hence in `:root` alone — and §4.4 also puts there the seven typographic
 * steps, the eight spacing ones, the radii, the durations and the font
 * stacks, which no light block redeclares. A check red on correct
 * code is a check that gets loosened: it is by name the risk of §11.
 *
 * ⚠️ The rule retained is the one §7.4 ITSELF NAMES as its real failure
 * mode — "the light palette is written TWICE there, and nothing
 * other than this check prevents the two copies from diverging":
 *
 *   ① `media-clair` ≡ `attribut-clair`, strict equality BOTH WAYS —
 *      it is the duplication §4.2 creates and nothing else guards;
 *   ② (`media-clair` ∪ `attribut-clair`) ⊆ `racine` — a light theme that
 *      overrides a token without a dark counterpart is a typo,
 *      not an intention.
 *
 *   ③ every COLOUR of `racine`, except the six out-of-theme ones NAMED below,
 *      is redeclared in the light blocks — the inclusion `racine` ⊆ light,
 *      restricted to colours (sub-block S3).
 *
 * 🔴 ③ IS THE BLIND SPOT S3 CLOSED, AND THE HOLE WAS MEASURED. Sub-block
 * S2 filed it (`docs/superpowers/plans/journaux-design-s2/trou-7-4.log`):
 * `--accent-survol` removed from BOTH light blocks and left at the root alone
 * returned `bloc racine : 48 / media-clair : 13 / attribut-clair : 13`,
 * `écarts : 0`, `exit=0`. A colour forgotten in the light theme was thus only
 * discovered by eye, on a light page.
 *
 * ⚠️ THE SCOPE OF CHECK §7.4 CHANGED WITH ③, and it is no longer "the three
 * blocks declare the same set of names": it is "the two light blocks
 * are identical, and every colour of the root is redeclared there except the
 * named out-of-theme ones".
 *
 * ⚠️ ③ ONLY BITES ON THE ABSENCE FROM BOTH BLOCKS AT ONCE. A colour
 * present in only one is already caught by ①, and counting it twice would
 * say nothing more.
 *
 * 🔵 THE CLOSURE IS ARITHMETICALLY CLEAN, and it was measured on August 20th, 2026
 * by `lireBlocsDeTheme` on `tokens.css`: root **48** tokens of which **20**
 * colours; light blocks **14**; the **6** colours of the root absent from the
 * light block are EXACTLY the six out-of-theme ones listed below. 20 − 6 = 14,
 * hence ZERO differences from the day ③ was born — there was no doubtful case to
 * arbitrate.
 *
 * ⚠️ "IS A COLOUR" IS DECIDED ON THE VALUE, NEVER ON THE NAME. A
 * prefix (`--voile-*`) is a convention a typo gets around;
 * a value starting with `#`, `rgb(`/`rgba(` or `hsl(`/`hsla(` cannot be
 * got around.
 */

/**
 * The seven COLOUR tokens that ③ does NOT require in the light blocks — NAMED
 * one by one, never derived from a prefix.
 *
 * They are the six out-of-theme veils of `tokens.css` ("declared once,
 * never redefined"): they are laid OVER THE VIDEO, whose content follows
 * no theme, and a light frame around a video image reads as
 * a display defect.
 *
 * ⚠️ THE SEVENTH IS AN INK, NOT A VEIL, and it is here for a SYMMETRIC
 * reason, not an identical one: `--sur-voile` is laid OVER these veils, which follow
 * no theme. An ink that followed the theme on a background that does not
 * follow it is exactly the defect task 4 of S4 fixes — in light
 * theme, near-black on a near-black veil. ⚠️ IT, for its part, is in the contrast
 * pairs (the 53rd): that is what distinguishes it from the six others, and the
 * reason is written next to the pair (`contraste.ts`).
 *
 * ⚠️ It is a SECOND COPY of a fact already written in the comment of
 * `tokens.css`, and the cost is accepted. What it buys: an out-of-theme
 * colour added without being listed here turns the check RED, which forces the
 * question "out of theme, or forgotten light blocks?" instead of letting it
 * through. It is the shape of the waiting list of §7.6, smaller.
 */
export const COULEURS_HORS_THEME: readonly string[] = [
    '--video-letterbox',
    '--voile-flottant',
    '--voile-bouton',
    '--voile-bouton-survol',
    '--voile-micro-actif',
    '--voile-micro-refuse',
    '--sur-voile',
];

/** Is a token value a colour? Decided on the VALUE alone. */
function estUneCouleur(value: string): boolean {
    return /^(#|rgba?\(|hsla?\()/.test(value.trim());
}
export function ecartsEntreBlocs(blocs: BlocDeTheme[]): string[] {
    const parNom = new Map(blocs.map((b) => [b.nom, b]));
    const ecarts: string[] = [];

    const media = parNom.get('media-clair');
    const attribut = parNom.get('attribut-clair');
    const racine = parNom.get('racine');

    for (const nom of ['racine', 'media-clair', 'attribut-clair']) {
        if (!parNom.has(nom)) ecarts.push(`bloc ${nom} absent`);
    }
    if (!media || !attribut || !racine) return ecarts;

    // ① equality of the two copies of the light palette, both ways.
    for (const token of attribut.tokens.keys()) {
        if (!media.tokens.has(token)) ecarts.push(`media-clair: ${token} missing`);
    }
    for (const token of media.tokens.keys()) {
        if (!attribut.tokens.has(token)) ecarts.push(`attribut-clair: ${token} missing`);
    }
    // ② every light token has its counterpart in the unconditional block.
    for (const bloc of [media, attribut]) {
        for (const token of bloc.tokens.keys()) {
            if (!racine.tokens.has(token)) {
                ecarts.push(`root: ${token} overridden by ${bloc.nom} without being declared there`);
            }
        }
    }
    // ③ every colour of the root, except the named out-of-theme ones, has a
    //   light counterpart. Absent from BOTH blocks only: ① already holds
    //   the case where it is missing from just one.
    for (const [token, value] of racine.tokens) {
        if (!estUneCouleur(value)) continue;
        if (COULEURS_HORS_THEME.includes(token)) continue;
        if (media.tokens.has(token) || attribut.tokens.has(token)) continue;
        ecarts.push(
            `light blocks: ${token} is a root colour without a light counterpart`,
        );
    }
    return [...new Set(ecarts)].sort();
}
