#!/usr/bin/env node
// Contrôle §7.1 de la spec ⑥ — LES CONTRASTES TIENNENT LES SEUILS WCAG.
//
// Il ne connaît AUCUNE couleur : il lit `tokens.css`, le fait parser par
// `client/src/design/tokens.ts` et évaluer par `client/src/design/contraste.ts`,
// tous deux typecheckés et testés. C'est LE point de conception du §7.1 : une
// table jumelle est exactement ce que le §4.1 refuse ailleurs, et un contrôle
// qui a sa propre copie des valeurs valide sa copie.
//
// ⚠️ CE QU'IL NE VÉRIFIE PAS, d'après la spec §7.1 : que le BON token ait été
// employé au bon endroit (c'est une règle de revue, §8), ni les couleurs
// composées à l'exécution, ni ce qui est posé au-dessus de la vidéo — dont le
// fond n'est pas connaissable. Les six tokens hors thème (`--voile-*`,
// `--video-letterbox`) sont HORS des 52 paires pour cette raison : leur
// lisibilité sur une vidéo quelconque n'est garantie par rien, et la spec §11
// le déclare déjà.

import { readFileSync, existsSync } from 'node:fs';
import { lireBlocsDeTheme } from '../src/design/tokens.ts';
import { evaluer } from '../src/design/contraste.ts';

const args = process.argv.slice(2);
const iFichier = args.indexOf('--fichier');
const fichier = iFichier === -1 ? 'client/src/design/tokens.css' : args[iFichier + 1];

if (!existsSync(fichier)) {
    console.error(`${fichier} est absent : rien n'a été mesuré, ce n'est pas un succès.`);
    process.exit(2);
}

const blocs = lireBlocsDeTheme(readFileSync(fichier, 'utf8'));
const { verifiees, echecs, minimum } = evaluer(blocs);

for (const { paire, rapport } of echecs) {
    const nom = (t) => t.replace(/^--/, '');
    console.log(
        `ÉCHEC ${paire.theme} ${nom(paire.encre)}/${nom(paire.fond)} = ` +
            `${rapport.toFixed(2)} < ${paire.seuil}`,
    );
}
console.log(`paires vérifiées : ${verifiees}`);
console.log(`échecs : ${echecs.length}`);
console.log(`minimum global : ${minimum.toFixed(2)}`);
process.exit(echecs.length > 0 ? 1 : 0);
