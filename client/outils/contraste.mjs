#!/usr/bin/env node
// Check §7.1 of spec ⑥ — THE CONTRASTS MEET THE WCAG THRESHOLDS.
//
// It knows NO colour: it reads `tokens/couleurs.css`, has it parsed
// by `client/src/design/tokens.ts` and evaluated by
// `client/src/design/contraste.ts`, both typechecked and tested. It is THE
// design point of §7.1: a twin table is exactly what
// §4.1 refuses elsewhere, and a check that has its own copy of the values
// validates its copy.
//
// 🔴 `tokens/couleurs.css` ALONE, NEVER `tokens.css`: since the extraction of
// task 6 (August 25th, 2026), it is the only file where colours live —
// `tokens/echelles.css`, its neighbour, carries none, and the contrast
// pairs this check evaluates therefore have nothing to read there.
//
// ⚠️ WHAT IT DOES NOT CHECK, according to spec §7.1: that the RIGHT token was
// used in the right place (it is a review rule, §8), nor colours
// composed at run time, nor what is laid over the video — whose
// background cannot be known. The six out-of-theme tokens (`--voile-*`,
// `--video-letterbox`) are OUTSIDE the 52 pairs for this reason: their
// readability on an arbitrary video is guaranteed by nothing, and spec §11
// already declares it.

import { readFileSync, existsSync } from 'node:fs';
import { lireBlocsDeTheme } from '../src/design/tokens.ts';
import { evaluer } from '../src/design/contraste.ts';

const args = process.argv.slice(2);
const iFichier = args.indexOf('--fichier');
const file = iFichier === -1 ? 'client/src/design/tokens/couleurs.css' : args[iFichier + 1];

if (!existsSync(file)) {
    console.error(`${file} is missing: nothing was measured, this is not a success.`);
    process.exit(2);
}

const blocs = lireBlocsDeTheme(readFileSync(file, 'utf8'));
const { verifiees, echecs, minimum } = evaluer(blocs);

for (const { paire, rapport } of echecs) {
    const nom = (t) => t.replace(/^--/, '');
    console.log(
        `FAILURE ${paire.theme} ${nom(paire.encre)}/${nom(paire.fond)} = ` +
            `${rapport.toFixed(2)} < ${paire.seuil}`,
    );
}
console.log(`pairs checked: ${verifiees}`);
console.log(`failures: ${echecs.length}`);
console.log(`minimum global : ${minimum.toFixed(2)}`);
process.exit(echecs.length > 0 ? 1 : 0);
