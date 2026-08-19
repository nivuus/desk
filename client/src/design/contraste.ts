/**
 * Contraste WCAG 2.1 — PUR : ni DOM, ni `fs`, ni chemin.
 *
 * ⚠️ TypeScript EFFAÇABLE, ce module étant importé par un `.mjs` : aucun
 * `enum`, aucun `namespace`. Voir l'en-tête de `tokens.ts` pour la mesure.
 *
 * Les valeurs viennent TOUJOURS de `tokens.css`, parsé par `tokens.ts`. Ce
 * module ne connaît AUCUNE couleur : il connaît des NOMS de token. C'est le
 * point de conception du §7.1 — « un contrôle qui a sa propre copie des
 * valeurs valide sa copie ».
 */

import type { BlocDeTheme } from './tokens';

export interface Paire {
    theme: string;
    encre: string;
    fond: string;
    seuil: number;
}

export interface Echec {
    paire: Paire;
    rapport: number;
}

/** Un canal sRGB linéarisé. C'est ici que vit la correction gamma. */
function canalLineaire(octet: number): number {
    const c = octet / 255;
    return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
}

/**
 * Luminance relative WCAG 2.1.
 *
 * 🔴 LA FAUTE CLASSIQUE EST DE MOYENNER LES CANAUX LINÉAIREMENT. Elle rend des
 * rapports plausibles et faux : `#808080` vaudrait 0,5 au lieu de ≈ 0,2159, et
 * noir/blanc rendrait 21 dans les deux cas — donc le vecteur qui discrimine
 * est une couleur INTERMÉDIAIRE, jamais les extrêmes.
 */
export function luminanceRelative(couleur: string): number {
    const brut = couleur.trim().replace(/^#/, '');
    const hex =
        brut.length === 3 || brut.length === 4
            ? brut.slice(0, 3).split('').map((c) => c + c).join('')
            : brut.slice(0, 6);
    if (!/^[0-9a-fA-F]{6}$/.test(hex)) {
        throw new Error(`couleur non reconnue : « ${couleur} » (attendu #rgb ou #rrggbb)`);
    }
    const [r, v, b] = [0, 2, 4].map((i) => canalLineaire(parseInt(hex.slice(i, i + 2), 16)));
    return 0.2126 * r + 0.7152 * v + 0.0722 * b;
}

/** `(L + 0.05) / (l + 0.05)`, avec `L ≥ l` — donc symétrique. */
export function rapportDeContraste(a: string, b: string): number {
    const [x, y] = [luminanceRelative(a), luminanceRelative(b)];
    const [haut, bas] = x >= y ? [x, y] : [y, x];
    return (haut + 0.05) / (bas + 0.05);
}

const FONDS = ['--fond-0', '--fond-1', '--fond-2'];
/** Sept encres. Ni `--bord` (décoratif, exempté) ni `--sur-accent` (sur `--accent`). */
const ENCRES = [
    '--texte-fort',
    '--texte',
    '--texte-faible',
    '--accent',
    '--succes',
    '--alerte',
    '--danger',
];

const SEUIL_TEXTE = 4.5; // WCAG 1.4.3 AA
const SEUIL_COMPOSANT = 3; // WCAG 1.4.11

function pairesDuTheme(theme: string): Paire[] {
    const paires: Paire[] = [];
    for (const encre of ENCRES) {
        for (const fond of FONDS) paires.push({ theme, encre, fond, seuil: SEUIL_TEXTE });
    }
    for (const fond of FONDS) {
        paires.push({ theme, encre: '--bord-fort', fond, seuil: SEUIL_COMPOSANT });
    }
    paires.push({ theme, encre: '--sur-accent', fond: '--accent', seuil: SEUIL_TEXTE });
    // Le SURVOL du bouton principal (S2) : l'encre ne change pas, le fond si.
    // Sans cette paire, l'état le plus fréquent du produit serait le seul dont
    // le contraste ne serait mesuré par rien — c'est la raison pour laquelle
    // `--accent-survol` est un TOKEN et non un `color-mix()` ou un `filter`.
    paires.push({ theme, encre: '--sur-accent', fond: '--accent-survol', seuil: SEUIL_TEXTE });
    return paires;
}

/**
 * Les 52 paires DÉCLARÉES — jamais un produit cartésien.
 * (50 en S1 ; S2 en ajoute deux, `--sur-accent` sur `--accent-survol`.)
 *
 * 25 par thème : 7 encres × 3 fonds au seuil 4,5 ; `--bord-fort` sur les 3
 * fonds au seuil 3 ; `--sur-accent` sur `--accent` au seuil 4,5.
 *
 * ⚠️ `--bord` EST ABSENT, ET C'EST UNE DÉCISION, pas un oubli : il rend 1,45
 * (sombre) et 1,40 (clair) sur `--fond-0`, et il est réservé aux séparateurs
 * PUREMENT décoratifs, que WCAG 1.4.11 exempte explicitement. Dès qu'une
 * bordure porte une information — contour de champ, état d'un contrôle —
 * c'est `--bord-fort` qui s'applique, et lui est mesuré.
 *
 * ⚠️ LE COROLLAIRE : aucune commande ne peut vérifier qu'on n'a pas employé
 * `--bord` là où il fallait `--bord-fort`. C'est une RÈGLE DE REVUE, et la
 * spec §8 la nomme comme telle.
 */
export const PAIRES: readonly Paire[] = [...pairesDuTheme('sombre'), ...pairesDuTheme('clair')];

/** Le bloc qui porte la palette d'un thème. Les deux blocs clairs sont égaux (§7.4). */
function blocDuTheme(blocs: BlocDeTheme[], theme: string): BlocDeTheme | undefined {
    if (theme === 'sombre') return blocs.find((b) => b.nom === 'racine');
    return (
        blocs.find((b) => b.nom === 'attribut-clair') ?? blocs.find((b) => b.nom === 'media-clair')
    );
}

/**
 * Évalue les 52 paires sur les blocs parsés.
 *
 * ⚠️ UN TOKEN INTROUVABLE EST UN ÉCHEC, jamais une paire silencieusement
 * sautée : sans cela, une faute de frappe dans un nom de token ferait BAISSER
 * le nombre de paires vérifiées sans qu'aucun échec ne remonte, et le contrôle
 * passerait au vert en mesurant moins. Le rapport 0 le rend visible dans le
 * même rapport que les autres échecs.
 */
export function evaluer(blocs: BlocDeTheme[]): {
    verifiees: number;
    echecs: Echec[];
    minimum: number;
} {
    const echecs: Echec[] = [];
    let minimum = Infinity;

    for (const paire of PAIRES) {
        const bloc = blocDuTheme(blocs, paire.theme);
        // `--fond-0` du thème sombre peut n'être déclaré que dans `racine` :
        // on retombe sur ce bloc quand le bloc clair ne surcharge pas le token.
        const racine = blocs.find((b) => b.nom === 'racine');
        const encre = bloc?.tokens.get(paire.encre) ?? racine?.tokens.get(paire.encre);
        const fond = bloc?.tokens.get(paire.fond) ?? racine?.tokens.get(paire.fond);

        if (encre === undefined || fond === undefined) {
            echecs.push({ paire, rapport: 0 });
            minimum = 0;
            continue;
        }
        const rapport = rapportDeContraste(encre, fond);
        if (rapport < minimum) minimum = rapport;
        if (rapport < paire.seuil) echecs.push({ paire, rapport });
    }

    return { verifiees: PAIRES.length, echecs, minimum };
}
