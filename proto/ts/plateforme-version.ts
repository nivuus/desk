// La version du protocole du canal `/agent`, et rien d'autre.
//
// 🔴 CE MODULE EXISTE POUR ROMPRE UN CYCLE, PAS PAR GOÛT DU DÉCOUPAGE. Le
// sous-bloc G3 a extrait les types et les encodeurs de l'installation vers
// `plateforme-installation.ts` pour tenir la règle des 500 lignes ; ces
// encodeurs ont besoin de la constante, et la constante vivait dans
// `plateforme.ts`, qui importe déjà ce module-là. Le cycle aurait été un cycle
// de VALEURS — pas de types, que TypeScript efface —, donc un vrai cycle à
// l'exécution, du genre qui rend `undefined` une constante selon l'ordre
// d'évaluation des modules. La sortir ici le supprime, au lieu de parier sur
// cet ordre.
//
// ⚠️ ELLE RESTE RÉEXPORTÉE PAR `plateforme.ts` : aucun des consommateurs, dans
// aucun paquet, n'a eu à bouger.

/**
 * La version du protocole.
 *
 * 🔴 UN BUMP EST UNE RUPTURE, ET IL SE DÉPLOIE AUX DEUX BOUTS AU MÊME COMMIT.
 * Un agent d'une version antérieure LIT le refus qui le lui apprend — le refus
 * est la seule variante hors versionnement — et RENONCE ; il ne boucle plus.
 * La rupture reste une rupture, elle est seulement devenue diagnosticable.
 *
 * v4 est celle du sous-bloc G3 : elle ajoute `installer` (descendante),
 * `progression` et `termine` (montantes), et les deux énumérations `Phase` et
 * `Issue` qu'elles portent.
 */
export const PLATEFORME_VERSION = 4;
