/**
 * LE LECTEUR DE FEUILLE — l'outil commun des gardes de forme de ⑥.
 *
 * PUR : ni DOM, ni `fs`, ni chemin. Il ne connaît AUCUNE règle de design — il
 * blanchit, il découpe, il nomme. Toute règle appartient à son appelant.
 *
 * 🔴 EXTRAIT DE `primitives.test.ts` PAR LA TÂCHE 1 DU SOUS-BLOC S4, ET AVANT
 * TOUTE ADDITION. Ce fichier-là était à 283 lignes pour une porte à 300 (spec
 * §10), marge 17 — la plus serrée de `client/` après `verify-webrtc.mjs` —, et
 * S4 lui ajoute deux gardes qui lisent des feuilles. « On extrait avant
 * d'ajouter » : c'est la doctrine de `CLAUDE.md` (tâche 6 de D9, tâches 1 à 3
 * de D10), et ce dépôt a payé deux fois « une addition de commentaire annule
 * une extraction ».
 *
 * ⚠️ DIVERGENCE DÉCLARÉE. Le point de chute que S2 puis S3 nomment pour
 * `primitives.test.ts` est « scinder par objet — les gardes de forme d'un côté,
 * les gardes de famille de l'autre, sans séparer G5 de sa source ». Cette
 * extraction-ci est DIFFÉRENTE : elle sort l'OUTIL, pas les gardes, et laisse
 * G5 auprès de sa source. Elle est COMPLÉMENTAIRE, pas substitutive — le point
 * de chute nommé reste ouvert, et reste le bon si le fichier regrossit.
 *
 * ═══════════════════════════════════════════════════════════════════════════
 * 🔴 POURQUOI LE BLANCHIMENT VIT ICI, ET PAS DANS CHAQUE GARDE.
 *
 * Un garde qui cherche une sous-chaîne dans le texte BRUT est satisfait par le
 * commentaire du fichier qu'il analyse — les feuilles de ce dépôt écrivent
 * longuement POURQUOI telle valeur y est interdite, donc elles écrivent cette
 * valeur. Trois occurrences payées : S1 sur `CLE_THEME`, S2 sur G1 et G5, S3
 * sur sa rouge n°16. Le recopier dans chaque garde le ferait diverger d'un
 * garde à l'autre sans qu'aucune commande ne le dise.
 *
 * ⚠️ CE QU'IL NE FAIT PAS : il ne comprend ni les chaînes CSS (`content: "/*"`),
 * ni les `url()` non citées, ni l'imbrication native. Aucune feuille de
 * `client/src/` n'en porte — relevé, pas supposé —, et le jour où l'une en
 * portera, c'est ici qu'il faudra le dire plutôt que dans son appelant.
 * ═══════════════════════════════════════════════════════════════════════════
 */

/** Retire les commentaires `/* … *​/` — voir l'en-tête. */
export function sansCommentaires(css: string): string {
    return css.replace(/\/\*[\s\S]*?\*\//g, ' ');
}

/** Tout ce qui précède un `{`. Les at-rules (`@media …`) commencent par `@`. */
export function preludes(css: string): string[] {
    return [...css.matchAll(/([^{}]+)\{/g)].map((m) => m[1].trim()).filter(Boolean);
}

export interface Declaration {
    propriete: string;
    valeur: string;
}

/**
 * Les déclarations des blocs les plus intérieurs. `[^{}]*` ne franchit ni `{`
 * ni `}` : le corps d'un `@media` n'est donc jamais pris pour une déclaration.
 */
export function declarationsDe(css: string): Declaration[] {
    const sortie: Declaration[] = [];
    for (const bloc of css.matchAll(/\{([^{}]*)\}/g)) {
        for (const morceau of bloc[1].split(';')) {
            const coupe = morceau.indexOf(':');
            if (coupe === -1) continue;
            sortie.push({
                propriete: morceau.slice(0, coupe).trim(),
                valeur: morceau.slice(coupe + 1).trim(),
            });
        }
    }
    return sortie;
}

/** Le corps du bloc qui suit `index`, accolades appariées. */
export function blocApres(css: string, index: number): string {
    const debut = css.indexOf('{', index);
    if (debut === -1) return '';
    let profondeur = 0;
    for (let i = debut; i < css.length; i += 1) {
        if (css[i] === '{') profondeur += 1;
        else if (css[i] === '}') {
            profondeur -= 1;
            if (profondeur === 0) return css.slice(debut + 1, i);
        }
    }
    return '';
}

/** `.carte > .carte__titre` rend `['.carte', '.carte__titre']`. */
export const compounds = (selecteur: string): string[] =>
    selecteur.split(/[\s>+~]+/).filter(Boolean);
