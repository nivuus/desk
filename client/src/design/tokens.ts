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
 *
 * 🔴 FUSIONNE LES OCCURRENCES DE MÊME NOM, DEPUIS L'EXTRACTION DE LA TÂCHE 6
 * (25 août 2026) : `tokens/couleurs.css` et `tokens/echelles.css` déclarent
 * chacun leur propre `:root {}` SANS CONDITION, et le texte qu'on passe ici
 * est leur CONCATÉNATION — deux occurrences physiques du même bloc logique
 * `racine`. Le navigateur les unit déjà par le cascade ; sans cette fusion,
 * ce parseur rendrait DEUX blocs nommés `racine`, et tout appelant qui en
 * cherche UN SEUL (`Array.find`, ou une `Map` clé par nom, qui ne garde que
 * le DERNIER) perdrait silencieusement les tokens de l'autre — exactement le
 * défaut que `tokens.test.ts` et `reprise.test.ts` existent pour ne jamais
 * laisser passer. Avant l'extraction, un seul fichier ne pouvait produire
 * qu'UNE occurrence par nom : cette fusion ne change donc RIEN à la lecture
 * d'un texte qui n'en a qu'une — elle rend seulement le cas à deux correct.
 */
export function lireBlocsDeTheme(css: string): BlocDeTheme[] {
    const propre = sansCommentaires(css);

    const plagesMedia: Array<[number, number]> = [];
    for (const m of propre.matchAll(/@media[^{]*\{/g)) {
        const fin = fermetureDe(propre, m.index + m[0].length - 1);
        if (fin !== -1) plagesMedia.push([m.index, fin]);
    }
    const dansMedia = (i: number) => plagesMedia.some(([d, f]) => i > d && i < f);

    const fusionnes = new Map<string, BlocDeTheme>();
    for (const m of propre.matchAll(/(:root[^{}]*)\{/g)) {
        const selecteur = m[1];
        const ouvrante = m.index + m[0].length - 1;
        const fin = fermetureDe(propre, ouvrante);
        if (fin === -1) continue;
        const corps = propre.slice(ouvrante + 1, fin);

        let nom = 'racine';
        if (/\[data-theme\s*=\s*["']clair["']\]/.test(selecteur)) nom = 'attribut-clair';
        else if (dansMedia(m.index)) nom = 'media-clair';

        const existant = fusionnes.get(nom);
        if (!existant) {
            fusionnes.set(nom, { nom, tokens: tokensDuCorps(corps), corps });
            continue;
        }
        for (const [cle, valeur] of tokensDuCorps(corps)) existant.tokens.set(cle, valeur);
        existant.corps += `\n${corps}`;
    }
    return [...fusionnes.values()];
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
 *   ③ toute COULEUR de `racine`, hors les six hors-thème NOMMÉS ci-dessous,
 *      est redéclarée dans les blocs clairs — l'inclusion `racine` ⊆ clair,
 *      restreinte aux couleurs (sous-bloc S3).
 *
 * 🔴 ③ EST L'ANGLE MORT QUE S3 A FERMÉ, ET LE TROU ÉTAIT MESURÉ. Le sous-bloc
 * S2 l'a versé (`docs/superpowers/plans/journaux-design-s2/trou-7-4.log`) :
 * `--accent-survol` retiré des DEUX blocs clairs et laissé à la racine seule
 * rendait `bloc racine : 48 / media-clair : 13 / attribut-clair : 13`,
 * `écarts : 0`, `exit=0`. Une couleur oubliée dans le thème clair ne se
 * découvrait donc que par l'œil, sur une page claire.
 *
 * ⚠️ LA PORTÉE DU CONTRÔLE §7.4 A CHANGÉ AVEC ③, et ce n'est plus « les trois
 * blocs déclarent le même ensemble de noms » : c'est « les deux blocs clairs
 * sont identiques, et toute couleur de la racine y est redéclarée sauf les
 * hors-thème nommés ».
 *
 * ⚠️ ③ NE MORD QUE SUR L'ABSENCE DES DEUX BLOCS À LA FOIS. Une couleur
 * présente dans un seul est déjà attrapée par ①, et la compter deux fois ne
 * dirait rien de plus.
 *
 * 🔵 LA FERMETURE EST ARITHMÉTIQUEMENT PROPRE, et c'est mesuré le 20 août 2026
 * par `lireBlocsDeTheme` sur `tokens.css` : racine **48** tokens dont **20**
 * couleurs ; blocs clairs **14** ; les **6** couleurs de la racine absentes du
 * bloc clair sont EXACTEMENT les six hors-thème listés ci-dessous. 20 − 6 = 14,
 * donc ZÉRO écart dès le jour où ③ est né — il n'y avait aucun cas douteux à
 * arbitrer.
 *
 * ⚠️ « EST UNE COULEUR » SE DÉCIDE SUR LA VALEUR, JAMAIS SUR LE NOM. Un
 * préfixe (`--voile-*`) est une convention qu'une faute de frappe contourne ;
 * une valeur qui commence par `#`, `rgb(`/`rgba(` ou `hsl(`/`hsla(` ne se
 * contourne pas.
 */

/**
 * Les sept tokens de COULEUR que ③ n'exige PAS dans les blocs clairs — NOMMÉS
 * un par un, jamais dérivés d'un préfixe.
 *
 * Ce sont les six voiles hors thème de `tokens.css` (« déclarés une fois,
 * jamais redéfinis ») : ils sont posés SUR LA VIDÉO, dont le contenu ne suit
 * aucun thème, et un encadrement clair autour d'une image vidéo se lit comme
 * un défaut d'affichage.
 *
 * ⚠️ LE SEPTIÈME EST UNE ENCRE, PAS UN VOILE, et il est ici pour une raison
 * SYMÉTRIQUE, pas identique : `--sur-voile` se pose SUR ces voiles, qui ne
 * suivent aucun thème. Une encre qui suivrait le thème sur un fond qui ne le
 * suit pas est exactement le défaut que la tâche 4 de S4 répare — en thème
 * clair, du quasi-noir sur un voile quasi-noir. ⚠️ Il est, LUI, dans les paires
 * de contraste (la 53ᵉ) : c'est ce qui le distingue des six autres, et la
 * raison est écrite auprès de la paire (`contraste.ts`).
 *
 * ⚠️ C'est une SECONDE COPIE d'un fait déjà écrit dans le commentaire de
 * `tokens.css`, et le coût est assumé. Ce qu'elle achète : une couleur hors
 * thème ajoutée sans être listée ici fait ROUGIR le contrôle, ce qui force la
 * question « hors thème, ou blocs clairs oubliés ? » au lieu de la laisser
 * passer. C'est la forme de la liste d'attente de §7.6, en plus petit.
 */
export const COULEURS_HORS_THEME: readonly string[] = [
    '--video-letterbox',
    '--voile-flottant',
    '--voile-bouton',
    '--voile-bouton-survol',
    '--voile-micro-actif',
    '--voile-micro-refuse',
    '--sur-voile',
];

/** Une valeur de token est-elle une couleur ? Décidé sur la VALEUR seule. */
function estUneCouleur(valeur: string): boolean {
    return /^(#|rgba?\(|hsla?\()/.test(valeur.trim());
}
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
    // ③ toute couleur de la racine, hors les hors-thème nommés, a une
    //   contrepartie claire. Absente des DEUX blocs seulement : ① tient déjà
    //   le cas où elle ne manque qu'à l'un.
    for (const [token, valeur] of racine.tokens) {
        if (!estUneCouleur(valeur)) continue;
        if (COULEURS_HORS_THEME.includes(token)) continue;
        if (media.tokens.has(token) || attribut.tokens.has(token)) continue;
        ecarts.push(
            `blocs clairs : ${token} est une couleur de la racine sans contrepartie claire`,
        );
    }
    return [...new Set(ecarts)].sort();
}
