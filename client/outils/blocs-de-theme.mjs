#!/usr/bin/env node
// Check §7.4 of spec ⑥ — THE THEME BLOCKS DO NOT DIVERGE.
//
// It carries NO rule: it reads `tokens/couleurs.css` and delegates to
// `ecartsEntreBlocs` of `client/src/design/tokens.ts`, which is typechecked and
// tested. It is the design point of §7.1: "a check that has its own
// copy of the values validates its copy." If a condition appeared here,
// it would be in the wrong place.
//
// 🔴 `tokens/couleurs.css` ALONE, NEVER `tokens.css`: since the extraction of
// task 6 (August 25th, 2026), the THREE theme blocks this check compares
// live ENTIRELY in `tokens/couleurs.css` — `tokens/echelles.css`, its
// neighbour, only carries a single `racine` block without a theme. Giving it both
// files concatenated would change nothing in the verdict (no colour lives there),
// but would widen for no reason what this check claims to cover.
//
// ⚠️ THIS `.mjs` IMPORTS A `.ts` NATIVELY — measured on Node v24.9.0, without
// `tsx`, without `ts-node`, without any new dependency. The cost is type
// stripping: it does NOT TYPECHECK and refuses non-erasable TypeScript
// (`enum`, `namespace`, constructor properties, decorators). Hence the
// constraint stated at the top of `tokens.ts`.
//
// ⚠️ THE RULE APPLIED DIVERGES FROM THE LETTER OF §7.4, and the reason is written
// next to `ecartsEntreBlocs`: a literal equality between the THREE blocks
// would be red forever on a correct file, the tokens outside the theme and
// the scales only living in `:root`.
//
// 🔴 THE SCOPE OF THIS CHECK WAS WIDENED BY SUB-BLOCK S3, and its statement
// is no longer "the three blocks declare the same set of names". It is:
//
//     the two light blocks are IDENTICAL, and every COLOUR of the root is
//     redeclared in them, EXCEPT the named out-of-theme ones.
//
// The inclusion `racine` ⊆ light, restricted to colours, is the blind spot
// S2 had measured and filed (`journaux-design-s2/trou-7-4.log`): a colour
// removed from BOTH light blocks and left at the root alone returned
// `gaps: 0`, `exit=0`. The list of the six out-of-theme ones lives in `tokens.ts`,
// next to the rule, under the name `COULEURS_HORS_THEME`.
//
// ⚠️ THIS SCRIPT STILL CARRIES NO RULE: neither the "is a
// colour" predicate, nor the list of out-of-theme ones are here. They are in `tokens.ts`,
// which is typechecked and tested.

import { readFileSync, existsSync } from 'node:fs';
import { COULEURS_HORS_THEME, ecartsEntreBlocs, lireBlocsDeTheme } from '../src/design/tokens.ts';

const args = process.argv.slice(2);
const iFichier = args.indexOf('--fichier');
const file = iFichier === -1 ? 'client/src/design/tokens/couleurs.css' : args[iFichier + 1];

if (!existsSync(file)) {
    console.error(`${file} is missing: nothing was measured, this is not a success.`);
    process.exit(2);
}

const css = readFileSync(file, 'utf8');
const blocs = lireBlocsDeTheme(css);

console.log(`file: ${file}`);
for (const bloc of blocs) {
    console.log(`  bloc ${bloc.nom} : ${bloc.tokens.size} token(s)`);
}

const ecarts = ecartsEntreBlocs(blocs);
console.log(
    `scope: ① light ≡ light, ② light ⊆ root, ` +
        `③ root colours ⊆ light except ${COULEURS_HORS_THEME.length} named out-of-theme ones`,
);
console.log(`gaps: ${ecarts.length}`);
for (const ecart of ecarts) console.log(`  GAP  ${ecart}`);
process.exit(ecarts.length > 0 ? 1 : 0);
