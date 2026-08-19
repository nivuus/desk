#!/usr/bin/env node
// Contrôle §7.7 de la spec ⑥ — LE POIDS CSS NE DÉRIVE PAS.
//
// À lancer APRÈS `npm run build`. Somme les octets de `dist/assets/*.css` et
// refuse au-dessus d'un plafond. La somme et le plafond sont imprimés
// TOUJOURS, y compris en cas de succès : un contrôle de dérive dont on ne lit
// jamais la valeur ne sert qu'à passer.
//
// ────────────────────────────────────────────────────────────────────────────
// LE PLAFOND EST ARBITRAIRE, ET IL EST DÉCLARÉ TEL — mot pour mot d'après la
// spec §7.7 : « Il n'est adossé à aucune mesure de performance : c'est un
// garde-fou contre une addition massive, pas une cible de budget. » Il rejoint
// la liste des constantes non calibrées du dépôt — `BPP_MIN`, `FACTEUR_FOCUS`,
// `PART_DORMANTE_BPS`, `TAILLE_MAX_SORTIE` — et se révise sans embarras.
//
// Ligne de base relevée le 19 août 2026, au commit `7314171`, `client/` étant
// inchangé depuis `8ad03a2` :
//
//     $ find dist/assets -name '*.css' -printf '%s\t%p\n'
//     1429    dist/assets/main-nAqQO_Wk.css
//
// ⚠️ Ce n'est PAS le 1 055 octets de la spec §2.3 : le chantier microphone a
// ajouté `#micro` et ses états à `style.css` depuis.
//
// ────────────────────────────────────────────────────────────────────────────
// LE NOM DU FICHIER CSS N'EST PAS PRÉVISIBLE, et c'est pourquoi ce script
// balaie `dist/assets/*.css` sans présumer d'aucun nom. Quand plusieurs pages
// lient la même feuille, Vite émet un actif partagé dont le nom vient d'un
// morceau JavaScript voisin — le prototype de S1 l'a vu sortir sous le nom
// `jeton-BtI6TA8C.css`. Un script qui chercherait `socle-*.css` ne trouverait
// rien, et rendrait donc VERT sur une somme de zéro.
//
// ⚠️ CE QUE LA ROUGE DE `--plafond` ÉPROUVE, ET CE QU'ELLE N'ÉPROUVE PAS. Le
// drapeau `--plafond` existe pour rendre le refus rejouable sans rien salir :
// il éprouve la COMPARAISON. Il n'éprouve pas que la SOMME soit la bonne — la
// rouge que la spec nomme pour cela (« ajouter un @font-face avec une police
// en data: URI ») demanderait d'écrire une police dans une feuille du produit.
// Que la somme soit juste s'établit en la comparant à la sortie de `find`
// ci-dessus. Dit plutôt que caché.

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
    console.error(`${actifs} est absent : lancer d'abord « npm run build ».`);
    process.exit(2);
}

const feuilles = readdirSync(actifs)
    .filter((f) => f.endsWith('.css'))
    .sort()
    .map((f) => ({ nom: f, octets: statSync(join(actifs, f)).size }));

const somme = feuilles.reduce((t, f) => t + f.octets, 0);

// ⚠️ ZÉRO FEUILLE N'EST PAS UN SUCCÈS, c'est une mesure qui n'a pas eu lieu.
// Sans ce garde, le contrôle rend VERT sur une somme de zéro — relevé le
// 19 août 2026 sur un `dist/assets/` vide : « somme : 0 / marge : 12288 /
// exit=0 ». C'est exactement le piège que l'en-tête décrit pour un script qui
// chercherait un nom de fichier précis, et il se refermait ici sous une autre
// forme. Sortie 2, comme pour un `dist/` absent : l'INSTRUMENT n'a rien pu
// mesurer, ce qui n'est pas la même chose que le produit qui dépasse (1).
if (feuilles.length === 0) {
    console.error(`aucune feuille dans ${actifs} : rien n'a été mesuré, ce n'est pas un succès.`);
    process.exit(2);
}

for (const f of feuilles) console.log(`${String(f.octets).padStart(7)}  ${f.nom}`);
console.log(`feuilles émises : ${feuilles.length}`);
console.log(`somme : ${somme} octets`);
console.log(`plafond : ${plafond} octets (arbitraire, voir l'en-tête)`);

if (somme > plafond) {
    console.log(`DÉPASSEMENT de ${somme - plafond} octets`);
    process.exit(1);
}
console.log(`marge : ${plafond - somme} octets`);
process.exit(0);
