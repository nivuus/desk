/**
 * Parseur de `tokens.css` — PUR : ni DOM, ni `fs`, ni chemin.
 *
 * Trois contrôles le partagent (§7.1 contraste, §7.4 égalité des blocs, §7.6
 * orphelins) plutôt que d'avoir chacun sa copie des valeurs : « un contrôle
 * qui a sa propre copie des valeurs valide sa copie » (spec §7.1). La lecture
 * du disque appartient aux `.mjs` de `client/outils/`.
 *
 * ⚠️ CE MODULE EST IMPORTÉ PAR DU `.mjs` NON TYPECHECKÉ, via le retrait de
 * types natif de Node (mesuré sur v24.9.0). Il doit donc rester du TypeScript
 * « EFFAÇABLE » : aucun `enum`, aucun `namespace`, aucune propriété de
 * constructeur, aucun décorateur. Le coût est mesuré, pas supposé — un
 * `export enum T { A, B }` importé de la même façon fait planter Node :
 *
 *     $ node runenum.mjs
 *     .../enum.ts:1
 *     export enum T { A, B }
 *
 * ⚠️ Il travaille sur du TEXTE. Un `import { readFileSync } from 'node:fs'`
 * ici serait rejeté par `npm run typecheck` : `client/node_modules/@types/` ne
 * porte que `estree`, ni `@types/node` ni `jsdom` (P2 l'a mesuré sur `Buffer`,
 * TS2580).
 */

export interface BlocDeTheme {
    /** `'racine'` | `'media-clair'` | `'attribut-clair'`. */
    nom: string;
    /** Nom de token (tirets compris) → valeur littérale, telle qu'écrite. */
    tokens: Map<string, string>;
    /** Le corps brut du bloc, pour les propriétés qui ne sont pas des `--*`. */
    corps: string;
}

/** Blanchit les commentaires en gardant les sauts de ligne. */
function sansCommentaires(css: string): string {
    return css.replace(/\/\*[\s\S]*?\*\//g, (bloc) => bloc.replace(/[^\n]/g, ' '));
}

/** Index du `}` qui ferme le `{` situé à `ouvrante`. -1 si le CSS est tronqué. */
function fermetureDe(css: string, ouvrante: number): number {
    let profondeur = 0;
    for (let i = ouvrante; i < css.length; i += 1) {
        if (css[i] === '{') profondeur += 1;
        else if (css[i] === '}') {
            profondeur -= 1;
            if (profondeur === 0) return i;
        }
    }
    return -1;
}

function tokensDuCorps(corps: string): Map<string, string> {
    const tokens = new Map<string, string>();
    for (const m of corps.matchAll(/(--[\w-]+)\s*:\s*([^;]+);/g)) {
        tokens.set(m[1], m[2].trim());
    }
    return tokens;
}

/**
 * Découpe le texte de `tokens.css` en ses trois blocs de thème, en ordre de
 * document. Le bloc `racine` est celui SANS condition — c'est lui qui porte la
 * palette sombre, les tokens hors thème et les échelles (spec §4.2, §4.4).
 */
export function lireBlocsDeTheme(css: string): BlocDeTheme[] {
    const propre = sansCommentaires(css);

    const plagesMedia: Array<[number, number]> = [];
    for (const m of propre.matchAll(/@media[^{]*\{/g)) {
        const fin = fermetureDe(propre, m.index + m[0].length - 1);
        if (fin !== -1) plagesMedia.push([m.index, fin]);
    }
    const dansMedia = (i: number) => plagesMedia.some(([d, f]) => i > d && i < f);

    const blocs: BlocDeTheme[] = [];
    for (const m of propre.matchAll(/(:root[^{}]*)\{/g)) {
        const selecteur = m[1];
        const ouvrante = m.index + m[0].length - 1;
        const fin = fermetureDe(propre, ouvrante);
        if (fin === -1) continue;
        const corps = propre.slice(ouvrante + 1, fin);

        let nom = 'racine';
        if (/\[data-theme\s*=\s*["']clair["']\]/.test(selecteur)) nom = 'attribut-clair';
        else if (dansMedia(m.index)) nom = 'media-clair';

        blocs.push({ nom, tokens: tokensDuCorps(corps), corps });
    }
    return blocs;
}

/**
 * La valeur d'une propriété qui n'est PAS un token — `color-scheme` en
 * particulier (divergence D9). Elle ne compte ni dans l'égalité de §7.4 ni
 * dans les orphelins de §7.6, donc rien ne la garderait sans cet accès.
 */
export function valeurDePropriete(bloc: BlocDeTheme, propriete: string): string | null {
    const m = bloc.corps.match(new RegExp(`(?:^|[;{\\s])${propriete}\\s*:\\s*([^;]+);`));
    return m ? m[1].trim() : null;
}

/** Tous les tokens déclarés, quel que soit le bloc. */
export function tokensDeclares(css: string): Set<string> {
    const noms = new Set<string>();
    for (const bloc of lireBlocsDeTheme(css)) {
        for (const nom of bloc.tokens.keys()) noms.add(nom);
    }
    return noms;
}

/** Tous les `var(--…)` référencés par un texte CSS. Les commentaires sont exclus. */
export function tokensReferences(css: string): Set<string> {
    const references = new Set<string>();
    for (const m of sansCommentaires(css).matchAll(/var\(\s*(--[\w-]+)/g)) {
        references.add(m[1]);
    }
    return references;
}

/**
 * Les écarts d'ensembles entre blocs. Vide = conforme.
 *
 * 🔴 DIVERGENCE ASSUMÉE AVEC LA LETTRE DU §7.4, et elle est de fond. La spec
 * écrit « les TROIS blocs déclarent le même ensemble de noms […] égalité
 * d'ensembles, dans les deux sens ». Pris à la lettre, ce contrôle est
 * ROUGE POUR TOUJOURS sur un `tokens.css` correct : le §4.5 exige que les six
 * tokens hors thème soient « déclarés une seule fois et jamais redéfinis »
 * — donc dans `:root` seul — et le §4.4 y met aussi les sept crans
 * typographiques, les huit d'espacement, les rayons, les durées et les piles
 * de polices, qu'aucun bloc clair ne redéclare. Un contrôle rouge sur du code
 * juste est un contrôle qu'on assouplit : c'est nommément le risque §11.
 *
 * ⚠️ La règle retenue est celle que le §7.4 NOMME LUI-MÊME comme son mode de
 * défaillance réel — « la palette claire y est écrite DEUX fois, et rien
 * d'autre que ce contrôle n'empêche les deux copies de diverger » :
 *
 *   ① `media-clair` ≡ `attribut-clair`, égalité stricte DANS LES DEUX SENS —
 *      c'est la duplication que le §4.2 crée et que rien d'autre ne garde ;
 *   ② (`media-clair` ∪ `attribut-clair`) ⊆ `racine` — un thème clair qui
 *      surcharge un token sans contrepartie sombre est une faute de frappe,
 *      pas une intention.
 *
 * L'inclusion inverse — `racine` ⊆ les blocs clairs — est délibérément ABSENTE :
 * c'est elle, et elle seule, que les tokens hors thème et les échelles
 * rendraient fausse.
 */
export function ecartsEntreBlocs(blocs: BlocDeTheme[]): string[] {
    const parNom = new Map(blocs.map((b) => [b.nom, b]));
    const ecarts: string[] = [];

    const media = parNom.get('media-clair');
    const attribut = parNom.get('attribut-clair');
    const racine = parNom.get('racine');

    for (const nom of ['racine', 'media-clair', 'attribut-clair']) {
        if (!parNom.has(nom)) ecarts.push(`bloc ${nom} absent`);
    }
    if (!media || !attribut || !racine) return ecarts;

    // ① égalité des deux copies de la palette claire, dans les deux sens.
    for (const token of attribut.tokens.keys()) {
        if (!media.tokens.has(token)) ecarts.push(`media-clair : ${token} manquant`);
    }
    for (const token of media.tokens.keys()) {
        if (!attribut.tokens.has(token)) ecarts.push(`attribut-clair : ${token} manquant`);
    }
    // ② tout token clair a sa contrepartie dans le bloc sans condition.
    for (const bloc of [media, attribut]) {
        for (const token of bloc.tokens.keys()) {
            if (!racine.tokens.has(token)) {
                ecarts.push(`racine : ${token} surchargé par ${bloc.nom} sans y être déclaré`);
            }
        }
    }
    return [...new Set(ecarts)].sort();
}
