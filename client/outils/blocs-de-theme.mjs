#!/usr/bin/env node
// Contrôle §7.4 de la spec ⑥ — LES BLOCS DE THÈME NE DIVERGENT PAS.
//
// Il ne porte AUCUNE règle : il lit `tokens.css` et délègue à
// `ecartsEntreBlocs` de `client/src/design/tokens.ts`, qui est typechecké et
// testé. C'est le point de conception du §7.1 : « un contrôle qui a sa propre
// copie des valeurs valide sa copie. » Si une condition apparaissait ici,
// c'est qu'elle serait au mauvais endroit.
//
// ⚠️ CE `.mjs` IMPORTE UN `.ts` NATIVEMENT — mesuré sur Node v24.9.0, sans
// `tsx`, sans `ts-node`, sans aucune dépendance neuve. Le coût est le retrait
// de types : il NE TYPECHECKE PAS et refuse le TypeScript non effaçable
// (`enum`, `namespace`, propriétés de constructeur, décorateurs). D'où la
// contrainte portée en tête de `tokens.ts`.
//
// ⚠️ LA RÈGLE APPLIQUÉE DIVERGE DE LA LETTRE DU §7.4, et la raison est écrite
// auprès de `ecartsEntreBlocs` : une égalité littérale entre les TROIS blocs
// serait rouge pour toujours sur un fichier correct, les tokens hors thème et
// les échelles ne vivant que dans `:root`.

import { readFileSync, existsSync } from 'node:fs';
import { ecartsEntreBlocs, lireBlocsDeTheme } from '../src/design/tokens.ts';

const args = process.argv.slice(2);
const iFichier = args.indexOf('--fichier');
const fichier = iFichier === -1 ? 'client/src/design/tokens.css' : args[iFichier + 1];

if (!existsSync(fichier)) {
    console.error(`${fichier} est absent : rien n'a été mesuré, ce n'est pas un succès.`);
    process.exit(2);
}

const css = readFileSync(fichier, 'utf8');
const blocs = lireBlocsDeTheme(css);

console.log(`fichier : ${fichier}`);
for (const bloc of blocs) {
    console.log(`  bloc ${bloc.nom} : ${bloc.tokens.size} token(s)`);
}

const ecarts = ecartsEntreBlocs(blocs);
console.log(`écarts : ${ecarts.length}`);
for (const ecart of ecarts) console.log(`  ÉCART  ${ecart}`);
process.exit(ecarts.length > 0 ? 1 : 0);
