/**
 * CSS CLASS parser — PURE: no DOM, no `fs`, no path.
 *
 * It serves check §7.9 (`client/outils/classes-employees.mjs`), which compares
 * the classes DECLARED by the stylesheets with the classes USED by the surfaces
 * and by the TypeScript. Reading the disk belongs to the `.mjs`, as for
 * `tokens.ts`: "a check that has its own copy of the values validates its
 * copy" (spec §7.1).
 *
 * ⚠️ THIS MODULE IS IMPORTED BY UNTYPECHECKED `.mjs`, through Node's native
 * type stripping. It must stay "ERASABLE" TypeScript: no
 * `enum`, no `namespace`, no constructor property, no
 * decorator. Same constraint as `tokens.ts`, same reason.
 *
 * 🔴 ALL THE FUNCTIONS HERE BLANK OUT COMMENTS BEFORE
 * SEARCHING. This repository paid THREE times for a guard being satisfied by the
 * comment of the file it analyses — including a red that stayed GREEN because
 * the mutated string appeared first in the comment justifying it.
 * A `/* .bouton--principale *​/` must count neither as a declaration nor as a
 * use.
 */

/** Blanks out CSS and HTML comments while keeping line breaks. */
function sansCommentairesCss(texte: string): string {
    return texte.replace(/\/\*[\s\S]*?\*\//g, (bloc) => bloc.replace(/[^\n]/g, ' '));
}

/** Blanks out HTML comments `<!-- … -->` while keeping line breaks. */
export function sansCommentairesHtml(html: string): string {
    return html.replace(/<!--[\s\S]*?-->/g, (bloc) => bloc.replace(/[^\n]/g, ' '));
}

/**
 * Blanks out TypeScript comments — `//` and `/* … *​/` — WITHOUT touching what
 * lives inside a string.
 *
 * ⚠️ THE STRING STATE IS TRACKED, and it is not overzealousness: a
 * `const u = 'https://exemple'` blanked naively would lose the end of its line,
 * and a class written after it would become invisible to the check — a false
 * NEGATIVE, that is, exactly the typo §7.9 exists to
 * catch, but silent.
 */
/**
 * The characters after which a `/` opens a REGULAR EXPRESSION
 * LITERAL rather than a division. `''` covers the start of the file.
 *
 * 🔴 THIS CASE IS NOT THEORETICAL, AND IT WAS FOUND BY RUNNING THE CHECK ON
 * THIS VERY FILE. `classesEmployeesTs` contains `/['"]([^'"]*)['"]/g`: six
 * quotes in a regex. Without this recognition, the string state
 * tracking takes them for openings, parity breaks, and THE WHOLE REST
 * OF THE FILE is read as a string — so no comment in it is
 * blanked any more. The check then returned `UNDECLARED …` on the prose of a
 * comment describing `className = '…'`, which is exactly the pattern
 * "a guard satisfied by the comment of the file it analyses" this
 * repository has already paid for three times.
 *
 * ⚠️ IT IS A HEURISTIC, NOT A JAVASCRIPT LEXER. It does not distinguish
 * `a /b/ c` (two divisions) from a regex; the case does not occur in this
 * repository, and the price of a real lexer would be out of all proportion with what this
 * check measures. The consequence of a mistake is bounded: a slightly wrong
 * "used" set, never a crash.
 */
const OUVRE_UNE_REGEX = ['', '(', ',', '=', ':', '[', '!', '&', '|', '?', '{', '}', ';', '+'];

/** Index just after the regex literal starting at `debut`. */
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
    /** The last SIGNIFICANT character emitted — it decides whether `/` opens a regex. */
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
 * The classes of the SELECTORS of a CSS text.
 *
 * 🔴 DECLARATION BODIES ARE DISCARDED, and it is necessary: a
 * `margin: .5rem` or a `content: ".x"` carries a dot followed by characters, and
 * counting them as declared classes would make check §7.9 permissive
 * — any typo would end up being "declared"
 * somewhere. A block whose prelude starts with `@` (`@media`,
 * `@supports`) contains RULES and not declarations: we descend into it.
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

/** The classes declared by a page's inline `<style>` blocks. */
export function classesDeclareesEnLigne(html: string): Set<string> {
    const classes = new Set<string>();
    for (const m of sansCommentairesHtml(html).matchAll(/<style[^>]*>([\s\S]*?)<\/style>/gi)) {
        for (const nom of classesDeclarees(m[1])) classes.add(nom);
    }
    return classes;
}

/** The classes used by a page's `class="…"` attributes. */
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
 * The classes used as a LITERAL by TypeScript — `classList.add('…')`
 * and `className = '…'`.
 *
 * ⚠️ A CLASS COMPUTED AT RUNTIME IS INVISIBLE HERE, by construction:
 * `el.className = variable`, a concatenation, a `classList.toggle(nom)`.
 * It is the price of static analysis, and the counterpart is convention
 * §6.4 of plan S3 — classes are written as literals, preferably in the HTML.
 * The check cannot enforce this convention; it
 * rewards it.
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
