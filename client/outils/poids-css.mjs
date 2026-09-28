#!/usr/bin/env node
// Check §7.7 of spec ⑥ — THE CSS WEIGHT DOES NOT DRIFT.
//
// To be run AFTER `npm run build`. Sums the bytes of `dist/assets/*.css` and
// refuses above a ceiling. The sum and the ceiling are printed
// ALWAYS, including on success: a drift check whose value is never
// read only serves to pass.
//
// ────────────────────────────────────────────────────────────────────────────
// THE CEILING IS ARBITRARY, AND IT IS DECLARED AS SUCH — word for word from
// spec §7.7: "It rests on no performance measurement: it is a
// safeguard against a massive addition, not a budget target." It joins
// the list of the repository's uncalibrated constants — `BPP_MIN`, `FACTEUR_FOCUS`,
// `PART_DORMANTE_BPS`, `MAX_OUTPUT_SIZE` — and is revised without embarrassment.
//
// Baseline taken on August 19th, 2026, at commit `7314171`, `client/` being
// unchanged since `8ad03a2`:
//
//     $ find dist/assets -name '*.css' -printf '%s\t%p\n'
//     1429    dist/assets/main-nAqQO_Wk.css
//
// ⚠️ It is NOT the 1,055 bytes of spec §2.3: the microphone project has
// added `#micro` and its states to `style.css` since.
//
// ────────────────────────────────────────────────────────────────────────────
// THE NAME OF THE CSS FILE IS NOT PREDICTABLE, and that is why this script
// scans `dist/assets/*.css` without assuming any name. When several pages
// link the same sheet, Vite emits a shared asset whose name comes from a
// neighbouring JavaScript chunk — the S1 prototype saw it come out as
// `jeton-BtI6TA8C.css`. A script that looked for `socle-*.css` would find
// nothing, and would therefore return GREEN on a sum of zero.
//
// ⚠️ WHAT THE RED OF `--plafond` EXERCISES, AND WHAT IT DOES NOT. The
// `--plafond` flag exists to make the refusal replayable without dirtying anything:
// it exercises the COMPARISON. It does not exercise that the SUM is right — the
// red the spec names for that ("add an @font-face with a font
// as a data: URI") would require writing a font into a product sheet.
// That the sum is right is established by comparing it to the output of `find`
// above. Said rather than hidden.

import { readdirSync, statSync, existsSync } from 'node:fs';
import { join } from 'node:path';

const PLAFOND_PAR_DEFAUT = 12288; // 12 Kio, spec §7.7

const args = process.argv.slice(2);
const valeurDe = (drapeau, defaut) => {
    const i = args.indexOf(drapeau);
    return i === -1 ? defaut : args[i + 1];
};
const dist = valeurDe('--dist', 'client/dist');
const plafond = Number(valeurDe('--plafond', PLAFOND_PAR_DEFAUT));

const actifs = join(dist, 'assets');
if (!existsSync(actifs)) {
    console.error(`${actifs} is missing: run « npm run build » first.`);
    process.exit(2);
}

const feuilles = readdirSync(actifs)
    .filter((f) => f.endsWith('.css'))
    .sort()
    .map((f) => ({ nom: f, octets: statSync(join(actifs, f)).size }));

const somme = feuilles.reduce((t, f) => t + f.octets, 0);

// ⚠️ ZERO SHEETS IS NOT A SUCCESS, it is a measurement that did not take place.
// Without this guard, the check returns GREEN on a sum of zero — observed on
// August 19th, 2026 on an empty `dist/assets/`: « somme : 0 / marge : 12288 /
// exit=0 » (the output was in French then). It is exactly the trap the header describes for a script that
// looked for a precise file name, and it closed in here under another
// form. Exit 2, as for an absent `dist/`: the INSTRUMENT could not
// measure anything, which is not the same as the product exceeding (1).
if (feuilles.length === 0) {
    console.error(`no sheet in ${actifs}: nothing was measured, this is not a success.`);
    process.exit(2);
}

for (const f of feuilles) console.log(`${String(f.octets).padStart(7)}  ${f.nom}`);
console.log(`sheets emitted: ${feuilles.length}`);
console.log(`total: ${somme} bytes`);
console.log(`ceiling: ${plafond} bytes (arbitrary, see the header)`);

if (somme > plafond) {
    console.log(`OVERRUN of ${somme - plafond} bytes`);
    process.exit(1);
}
console.log(`margin: ${plafond - somme} bytes`);
process.exit(0);
