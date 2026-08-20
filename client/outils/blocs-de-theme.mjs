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
//
// 🔴 LA PORTÉE DE CE CONTRÔLE A ÉTÉ ÉLARGIE PAR LE SOUS-BLOC S3, et son énoncé
// n'est plus « les trois blocs déclarent le même ensemble de noms ». Il est :
//
//     les deux blocs clairs sont IDENTIQUES, et toute COULEUR de la racine y
//     est redéclarée, SAUF les hors-thème nommés.
//
// L'inclusion `racine` ⊆ clair, restreinte aux couleurs, est l'angle mort que
// S2 avait mesuré et versé (`journaux-design-s2/trou-7-4.log`) : une couleur
// retirée des DEUX blocs clairs et laissée à la racine seule rendait
// `écarts : 0`, `exit=0`. La liste des six hors-thème vit dans `tokens.ts`,
// auprès de la règle, sous le nom `COULEURS_HORS_THEME`.
//
// ⚠️ CE SCRIPT NE PORTE TOUJOURS AUCUNE RÈGLE : ni le prédicat « est une
// couleur », ni la liste des hors-thème ne sont ici. Ils sont dans `tokens.ts`,
// qui est typechecké et testé.

import { readFileSync, existsSync } from 'node:fs';
import { COULEURS_HORS_THEME, ecartsEntreBlocs, lireBlocsDeTheme } from '../src/design/tokens.ts';

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
console.log(
    `portée : ① clair \u2261 clair, ② clair \u2286 racine, ` +
        `③ couleurs de racine \u2286 clair sauf ${COULEURS_HORS_THEME.length} hors-thème nommés`,
);
console.log(`écarts : ${ecarts.length}`);
for (const ecart of ecarts) console.log(`  ÉCART  ${ecart}`);
process.exit(ecarts.length > 0 ? 1 : 0);
