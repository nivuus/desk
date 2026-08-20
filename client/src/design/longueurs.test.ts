import { describe, expect, it } from 'vitest';
import { declarationsDe, sansCommentaires } from './css';

/**
 * ═══════════════════════════════════════════════════════════════════════════
 * CONTRÔLE §7.10 — AUCUNE LONGUEUR HORS TOKEN DANS UNE FEUILLE DE SURFACE.
 *
 * 🔴 C'EST LE PREMIER CONTRÔLE DU SOUS-PROJET ⑥ QUI MESURE UNE LONGUEUR, et
 * c'est ce qui fait sa raison d'être. Jusqu'ici, « aucun des huit contrôles ne
 * mesure une longueur » était écrit à cinq endroits du dépôt sous cette formule
 * exacte, et davantage sous d'autres tournures : la clause « aucune longueur
 * hors échelle » du §8 de la spec était une DETTE D'ÉNONCÉ que rien ne pouvait
 * ni tenir ni réfuter. Elle devient une commande.
 *
 * 🔴 IL EST NÉ ROUGE SUR L'ARBRE INTACT — HUIT OCCURRENCES POUR SIX VALEURS —,
 * ET C'EST SA PREUVE D'ATTEIGNABILITÉ, comme l'assertion ② A de §7.9 en S3.
 * Les six : `6px` ×2 (les deux bandeaux), `18px` ×2 (les deux boutons de coin),
 * `0.02em` (le crénage de `#stats`), `72rem`, `18rem` et `26rem` (les trois
 * mesures de contenant). Les tâches 5 à 8 de S4 les font toutes tomber.
 *
 * ⚠️ DEUX NOMBRES, JAMAIS UN SEUL, ET ILS SONT VRAIS DE CHOSES DIFFÉRENTES.
 * Le sous-bloc S3 a publié SIX en comptant des VALEURS DISTINCTES ; un contrôle
 * compte des OCCURRENCES, parce qu'il ne peut pas dédupliquer sans décider que
 * deux `6px` écrits à deux endroits sont le même. C'est la divergence D5 de S2
 * rejouée sur une autre grandeur — « ni dix ni dix-sept ne se suffit sans dire
 * lequel on compte ». Les deux sont donc imprimés, succès compris : « un
 * contrôle de dérive dont on ne lit jamais la valeur ne sert qu'à passer »
 * (`poids-css.mjs`).
 *
 * ── LA FRONTIÈRE AVEC G4, ÉCRITE DES DEUX CÔTÉS ───────────────────────────
 * G4 (`primitives.test.ts`) garde `client/src/design/primitives/` et ses quatre
 * familles. §7.10 garde les FEUILLES DE SURFACE. Aucun des deux ne double
 * l'autre, et la portée ci-dessous est DÉRIVÉE — jamais énumérée : toutes les
 * `*.css` de `client/src/` HORS `client/src/design/`. Une feuille de surface
 * neuve — `client/src/session/*.css`, que la tâche 9 crée — entre donc dans ce
 * contrôle sans qu'une ligne de ce fichier ne change, et une liste recopiée ne
 * peut pas diverger de ce qu'elle décrit.
 *
 * ⚠️ CE QU'IL NE DIT PAS, ET C'EST EXACTEMENT LA LIMITE DE G4 : *que le BON
 * token a été choisi.* `padding: var(--e-8)` sur un bandeau serait vert et
 * absurde. Le bon emploi reste une règle de revue, comme celui de `--bord`
 * contre `--bord-fort` (spec §4.5, §8).
 *
 * ⚠️ IL NE VOIT PAS NON PLUS UNE LONGUEUR CALCULÉE À L'EXÉCUTION —
 * `el.style.padding = …` dans un `.ts`. C'est le même angle mort que §7.9
 * déclare pour les classes, et la même parade : la convention est d'écrire les
 * longueurs dans le CSS.
 *
 * 🔴 CE FICHIER NE LIT UN TEXTE NON VIDE QUE GRÂCE À `test: { css: true }` de
 * `client/vite.config.ts`. Sans cette ligne, Vitest court-circuite les fichiers
 * CSS — la requête `?raw` comprise — et les feuilles vaudraient la chaîne VIDE :
 * l'assertion d'absence passerait au vert EN NE MESURANT RIEN. C'est aussi
 * pourquoi il n'y a délibérément pas de `client/vitest.config.ts`, qui prendrait
 * le pas sur la configuration Vite sans rien dire.
 * ═══════════════════════════════════════════════════════════════════════════
 */

/**
 * Les feuilles de SURFACE, DÉRIVÉES et non énumérées — voir l'en-tête.
 * `client/src/design/` est exclu : G4 s'en occupe, et `tokens.css` y vit, dont
 * les littéraux SONT l'échelle.
 */
const FEUILLES = import.meta.glob<string>(['../**/*.css', '!../design/**'], {
    query: '?raw',
    import: 'default',
    eager: true,
});

/**
 * Les unités qui font une longueur. C'est la liste de G4, mot pour mot, et le
 * partage est délibéré : deux gardes qui mesurent la même chose avec deux
 * listes divergeraient sans qu'aucune commande ne le dise.
 * ⚠️ `ms` et `s` sont des DURÉES, pas des longueurs, et elles sont ici pour la
 * même raison — les durées ont leur échelle (`--duree-1`, `--duree-2`), et une
 * durée littérale est la même dérive sous un autre nom. Le titre du contrôle
 * dit « longueur » parce que c'est le mot de la spec §8 ; la portée réelle est
 * « toute valeur dimensionnée ».
 */
const UNITES = /(\d+(?:\.\d+)?)(px|rem|em|ms|s|pt|ch|vw|vh|dvw|dvh|vmin|vmax)\b/g;

/**
 * LES TROIS EXCEPTIONS, CLOSES, CHACUNE AVEC SA RAISON.
 *
 * ① LES REMPLISSAGES DE FENÊTRE — `100vw`, `100vh`, `100dvh`. C'est la règle de
 *    compte que `style.css` applique depuis S1 et que le journal de S3 a
 *    énoncée AVANT de compter : « un remplissage de fenêtre n'est pas une
 *    valeur hors échelle ». Aucune échelle ne prétend couvrir « toute la
 *    fenêtre », et un token qui vaudrait `100vh` ne serait qu'un alias.
 * ② LE ZÉRO — `0`, `0px`. Aucune échelle n'a de cran nul, et le repli des
 *    `env(titlebar-area-*)` du Window Controls Overlay en porte un par
 *    construction (tâche 10).
 * ③ `tokens.css` — la source unique ; ses littéraux SONT l'échelle. Il est de
 *    toute façon hors de la portée dérivée ci-dessus, et le dire ici évite
 *    qu'on l'y ramène « pour être complet ».
 */
function horsExceptions(valeur: string): string {
    return valeur
        .replace(/var\(\s*--[a-z0-9-]+\s*\)/gi, ' ')
        .replace(/\b100(vw|vh|dvw|dvh)\b/g, ' ')
        .replace(/(^|[\s(,])0(px)?(?=$|[\s),;])/g, '$1 ');
}

interface Occurrence {
    fichier: string;
    propriete: string;
    valeur: string;
    fautive: string;
}

/** `../style.css` → `client/src/style.css`, pour que la sortie soit ouvrable. */
const chemin = (cle: string) => cle.replace(/^\.\.\//, 'client/src/');

const occurrences: Occurrence[] = [];
/** Les déclarations dimensionnées qui passent BIEN par un token — l'atteignabilité. */
let parToken = 0;
let declarationsLues = 0;

for (const [cle, texte] of Object.entries(FEUILLES).sort()) {
    for (const d of declarationsDe(sansCommentaires(texte))) {
        declarationsLues += 1;
        if (/var\(\s*--[a-z0-9-]+\s*\)/i.test(d.valeur) && !UNITES.test(d.valeur)) parToken += 1;
        UNITES.lastIndex = 0;
        for (const m of horsExceptions(d.valeur).matchAll(UNITES)) {
            occurrences.push({
                fichier: chemin(cle),
                propriete: d.propriete,
                valeur: d.valeur,
                fautive: m[0],
            });
        }
    }
}
const valeurs = new Set(occurrences.map((o) => o.fautive));

// ── LE RELEVÉ, TOUJOURS IMPRIMÉ, SUCCÈS COMPRIS ───────────────────────────
console.log(`§7.10  feuilles de surface : ${Object.keys(FEUILLES).length}`);
console.log(`       déclarations lues : ${declarationsLues}, dont ${parToken} par un token`);
for (const o of occurrences) {
    console.log(`       ${o.fichier}  ${o.propriete}: ${o.valeur}  → « ${o.fautive} »`);
}
console.log(
    `       hors token : ${occurrences.length} occurrence(s), ` +
        `${valeurs.size} valeur(s) distincte(s)` +
        (valeurs.size ? ` — ${[...valeurs].sort().join(', ')}` : ''),
);

describe('§7.10 — aucune longueur hors token dans une feuille de surface', () => {
    it('atteignabilité : des feuilles sont lues, et des longueurs y passent par un token', () => {
        // 🔴 SANS CETTE ASSERTION, LA SUIVANTE EST VERTE SUR DES FICHIERS VIDES.
        // C'est G5 de `primitives.test.ts`, et c'est le piège que ce sous-projet
        // a payé en S2 : « quatre gardes sur cinq ne prouveraient rien ». Sa
        // rouge se joue en VIDANT `client/src/style.css`, jamais en y ajoutant
        // une valeur.
        expect(
            Object.keys(FEUILLES).length,
            'aucune feuille de surface trouvée : §7.10 est vert en ne mesurant rien',
        ).toBeGreaterThan(0);
        expect(
            declarationsLues,
            'aucune déclaration lue : §7.10 est vert en ne mesurant rien',
        ).toBeGreaterThan(0);
        expect(
            parToken,
            'aucune longueur ne passe par un token : le contrôle ne mesure pas ce qu’il croit',
        ).toBeGreaterThan(0);
    });

    it('toute longueur passe par un token, hors les trois exceptions nommées', () => {
        expect(
            occurrences.map((o) => `${o.fichier}  ${o.propriete}: ${o.valeur}  → « ${o.fautive} »`),
            `longueurs hors token : ${occurrences.length} occurrence(s) pour ${valeurs.size} valeur(s) distincte(s)`,
        ).toEqual([]);
    });
});
