/**
 * Parseur des CLASSES CSS — PUR : ni DOM, ni `fs`, ni chemin.
 *
 * Il sert le contrôle §7.9 (`client/outils/classes-employees.mjs`), qui compare
 * les classes DÉCLARÉES par les feuilles aux classes EMPLOYÉES par les surfaces
 * et par le TypeScript. La lecture du disque appartient au `.mjs`, comme pour
 * `tokens.ts` : « un contrôle qui a sa propre copie des valeurs valide sa
 * copie » (spec §7.1).
 *
 * ⚠️ CE MODULE EST IMPORTÉ PAR DU `.mjs` NON TYPECHECKÉ, via le retrait de
 * types natif de Node. Il doit rester du TypeScript « EFFAÇABLE » : aucun
 * `enum`, aucun `namespace`, aucune propriété de constructeur, aucun
 * décorateur. Même contrainte que `tokens.ts`, même raison.
 *
 * 🔴 TOUTES LES FONCTIONS D'ICI BLANCHISSENT LES COMMENTAIRES AVANT DE
 * CHERCHER. Ce dépôt a payé TROIS fois qu'un garde soit satisfait par le
 * commentaire du fichier qu'il analyse — dont une rouge restée VERTE parce que
 * la chaîne mutée apparaissait d'abord dans le commentaire qui la justifiait.
 * Un `/* .bouton--principale *​/` ne doit compter ni comme déclaration ni comme
 * emploi.
 */

/** Blanchit les commentaires CSS et HTML en gardant les sauts de ligne. */
function sansCommentairesCss(texte: string): string {
    return texte.replace(/\/\*[\s\S]*?\*\//g, (bloc) => bloc.replace(/[^\n]/g, ' '));
}

/** Blanchit les commentaires HTML `<!-- … -->` en gardant les sauts de ligne. */
export function sansCommentairesHtml(html: string): string {
    return html.replace(/<!--[\s\S]*?-->/g, (bloc) => bloc.replace(/[^\n]/g, ' '));
}

/**
 * Blanchit les commentaires TypeScript — `//` et `/* … *​/` — SANS toucher à ce
 * qui vit dans une chaîne.
 *
 * ⚠️ L'ÉTAT DE CHAÎNE EST SUIVI, et ce n'est pas du zèle : un
 * `const u = 'https://exemple'` blanchi naïvement perdrait la fin de sa ligne,
 * et une classe écrite après lui deviendrait invisible au contrôle — un faux
 * NÉGATIF, c'est-à-dire exactement la faute de frappe que §7.9 existe pour
 * attraper, mais silencieuse.
 */
/**
 * Les caractères après lesquels un `/` ouvre une LITTÉRALE D'EXPRESSION
 * RÉGULIÈRE plutôt qu'une division. `''` couvre le début du fichier.
 *
 * 🔴 CE CAS N'EST PAS THÉORIQUE, ET IL A ÉTÉ TROUVÉ EN LANÇANT LE CONTRÔLE SUR
 * CE FICHIER-CI. `classesEmployeesTs` contient `/['"]([^'"]*)['"]/g` : six
 * guillemets dans une regex. Sans cette reconnaissance, le suivi d'état de
 * chaîne les prend pour des ouvertures, la parité se rompt, et TOUT LE RESTE
 * DU FICHIER est lu comme une chaîne — donc plus aucun commentaire n'y est
 * blanchi. Le contrôle rendait alors `NON DÉCLARÉE …` sur la prose d'un
 * commentaire qui décrit `className = '…'`, ce qui est exactement le patron
 * « un garde satisfait par le commentaire du fichier qu'il analyse » que ce
 * dépôt a déjà payé trois fois.
 *
 * ⚠️ C'EST UNE HEURISTIQUE, PAS UN LEXEUR JAVASCRIPT. Elle ne distingue pas
 * `a /b/ c` (deux divisions) d'une regex ; le cas ne se présente pas dans ce
 * dépôt, et le prix d'un vrai lexeur serait sans commune mesure avec ce que ce
 * contrôle mesure. La conséquence d'une erreur est bornée : un ensemble
 * « employé » légèrement faux, jamais un plantage.
 */
const OUVRE_UNE_REGEX = ['', '(', ',', '=', ':', '[', '!', '&', '|', '?', '{', '}', ';', '+'];

/** Index juste après la littérale de regex qui commence à `debut`. */
function finDeRegex(ts: string, debut: number): number {
    let i = debut + 1;
    let dansUneClasse = false;
    while (i < ts.length) {
        const c = ts[i];
        if (c === '\\') {
            i += 2;
            continue;
        }
        if (c === '[') dansUneClasse = true;
        else if (c === ']') dansUneClasse = false;
        else if (c === '/' && !dansUneClasse) return i + 1;
        else if (c === '\n') return i;
        i += 1;
    }
    return i;
}

export function sansCommentairesTs(ts: string): string {
    let sortie = '';
    let i = 0;
    let delimiteur: string | null = null;
    /** Le dernier caractère SIGNIFICATIF émis — il décide si `/` ouvre une regex. */
    let precedent = '';
    while (i < ts.length) {
        const c = ts[i];
        if (delimiteur !== null) {
            sortie += c;
            if (c === '\\' && i + 1 < ts.length) {
                sortie += ts[i + 1];
                i += 2;
                continue;
            }
            if (c === delimiteur) delimiteur = null;
            i += 1;
            continue;
        }
        if (c === '/' && ts[i + 1] === '/') {
            while (i < ts.length && ts[i] !== '\n') {
                sortie += ' ';
                i += 1;
            }
            continue;
        }
        if (c === '/' && ts[i + 1] === '*') {
            const fin = ts.indexOf('*/', i + 2);
            const borne = fin === -1 ? ts.length : fin + 2;
            for (; i < borne; i += 1) sortie += ts[i] === '\n' ? '\n' : ' ';
            continue;
        }
        if (c === '/' && OUVRE_UNE_REGEX.includes(precedent)) {
            const fin = finDeRegex(ts, i);
            sortie += ts.slice(i, fin);
            i = fin;
            precedent = '/';
            continue;
        }
        if (c === '"' || c === "'" || c === '`') {
            delimiteur = c;
            sortie += c;
            i += 1;
            continue;
        }
        sortie += c;
        if (!/\s/.test(c)) precedent = c;
        i += 1;
    }
    return sortie;
}

/**
 * Les classes des SÉLECTEURS d'un texte CSS.
 *
 * 🔴 LES CORPS DE DÉCLARATION SONT ÉCARTÉS, et c'est nécessaire : un
 * `margin: .5rem` ou un `content: ".x"` porte un point suivi de caractères, et
 * les compter comme des classes déclarées rendrait le contrôle §7.9 permissif
 * — n'importe quelle faute de frappe finirait par se trouver « déclarée »
 * quelque part. Un bloc dont le prélude commence par `@` (`@media`,
 * `@supports`) contient des RÈGLES et non des déclarations : on y descend.
 */
export function classesDeclarees(css: string): Set<string> {
    const classes = new Set<string>();
    const propre = sansCommentairesCss(css);

    const parcourir = (texte: string): void => {
        let i = 0;
        let debutPrelude = 0;
        while (i < texte.length) {
            const c = texte[i];
            if (c === '{') {
                const prelude = texte.slice(debutPrelude, i);
                let profondeur = 1;
                let j = i + 1;
                for (; j < texte.length && profondeur > 0; j += 1) {
                    if (texte[j] === '{') profondeur += 1;
                    else if (texte[j] === '}') profondeur -= 1;
                }
                const corps = texte.slice(i + 1, j - 1);
                for (const m of prelude.matchAll(/\.(-?[A-Za-z_][\w-]*)/g)) classes.add(m[1]);
                if (prelude.trim().startsWith('@')) parcourir(corps);
                i = j;
                debutPrelude = i;
                continue;
            }
            if (c === '}') {
                i += 1;
                debutPrelude = i;
                continue;
            }
            i += 1;
        }
    };

    parcourir(propre);
    return classes;
}

/** Les classes déclarées par les blocs `<style>` en ligne d'une page. */
export function classesDeclareesEnLigne(html: string): Set<string> {
    const classes = new Set<string>();
    for (const m of sansCommentairesHtml(html).matchAll(/<style[^>]*>([\s\S]*?)<\/style>/gi)) {
        for (const nom of classesDeclarees(m[1])) classes.add(nom);
    }
    return classes;
}

/** Les classes employées par les attributs `class="…"` d'une page. */
export function classesEmployeesHtml(html: string): Set<string> {
    const classes = new Set<string>();
    const propre = sansCommentairesHtml(html);
    for (const m of propre.matchAll(/\sclass\s*=\s*(?:"([^"]*)"|'([^']*)')/g)) {
        for (const nom of (m[1] ?? m[2]).split(/\s+/)) {
            if (nom !== '') classes.add(nom);
        }
    }
    return classes;
}

/**
 * Les classes employées en LITTÉRAL par du TypeScript — `classList.add('…')`
 * et `className = '…'`.
 *
 * ⚠️ UNE CLASSE CALCULÉE À L'EXÉCUTION EST INVISIBLE ICI, par construction :
 * `el.className = variable`, une concaténation, un `classList.toggle(nom)`.
 * C'est le prix de l'analyse statique, et la contrepartie est la convention
 * §6.4 du plan S3 — les classes s'écrivent en littéral, dans le HTML de
 * préférence. Le contrôle ne peut pas forcer cette convention ; il la
 * récompense.
 */
export function classesEmployeesTs(ts: string): Set<string> {
    const classes = new Set<string>();
    const propre = sansCommentairesTs(ts);
    for (const m of propre.matchAll(/classList\.add\(([^)]*)\)/g)) {
        for (const s of m[1].matchAll(/['"]([^'"]*)['"]/g)) {
            for (const nom of s[1].split(/\s+/)) if (nom !== '') classes.add(nom);
        }
    }
    for (const m of propre.matchAll(/className\s*=\s*['"]([^'"]*)['"]/g)) {
        for (const nom of m[1].split(/\s+/)) if (nom !== '') classes.add(nom);
    }
    return classes;
}
