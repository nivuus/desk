/// <reference types="vite/client" />
/**
 * THE GALLERY'S RENDERING — and the FIRST caller of `getComputedStyle` in the
 * repository.
 *
 * ⚠️ THIS MODULE IS NOT PART OF THE PRODUCT: it only serves `client/design.html`,
 * the instrument of §8's human judgement. No product surface
 * imports it, and it has no test — what it could have that is testable
 * (parsing tokens) already lives in `tokens.ts`, which is tested.
 *
 * ═══════════════════════════════════════════════════════════════════════════
 * 🔴 THE TOKEN LIST IS NOT WRITTEN HERE: it is PARSED from
 * `tokens/couleurs.css` and `tokens/echelles.css` (extracted from `tokens.css` by
 * task 6, August 25th, 2026 — the gallery renders BOTH families, colours and
 * scales), by the same `lireBlocsDeTheme` as checks §7.1, §7.4 and
 * §7.6. It is the design point of §7.1 — "a check that has its own
 * copy of the values validates its copy" — applied to an instrument rather than to
 * a check: a gallery with its own list would show its list, and a
 * new token would never appear in it. It is also what makes true the sentence
 * of §7.6 "the gallery renders all tokens BY CONSTRUCTION", on which
 * its exclusion from the "used" scope rests.
 *
 * 🔴 AND IT IS THE FIRST CALLER OF `getComputedStyle` — spec §4.1
 * wrote the rule "for the first one who needs it, not for an observed
 * need", and declared that "no caller exists today". This
 * file makes that false, and it complies with it: the value is read by
 * `getComputedStyle(document.documentElement).getPropertyValue('--…')`, a
 * single mechanism, zero duplication. The contract of §4.1 is that it is done
 * ONLY ON A THEME CHANGE, never per frame: that is exactly what
 * `rendre()` does, called at load and at each theme click, and never
 * elsewhere.
 * ═══════════════════════════════════════════════════════════════════════════
 */
import { lireBlocsDeTheme } from './tokens';
import { installThemeSelectorInDOM } from './selecteur-theme';
import couleursCss from './tokens/couleurs.css?raw';
import echellesCss from './tokens/echelles.css?raw';

const racine = document.documentElement;
const tokensCss = `${couleursCss}\n${echellesCss}`;

/** All declared names, in source file order. */
const NOMS: string[] = [];
for (const bloc of lireBlocsDeTheme(tokensCss)) {
    for (const nom of bloc.tokens.keys()) if (!NOMS.includes(nom)) NOMS.push(nom);
}

/** The CURRENT value, hence that of the applied theme — see the header. */
const value = (nom: string) => getComputedStyle(racine).getPropertyValue(nom).trim();

const famille = (prefixe: string) => NOMS.filter((n) => n.startsWith(prefixe));
const within = (noms: string[]) => NOMS.filter((n) => noms.includes(n));

const COULEURS = [
    '--fond-0', '--fond-1', '--fond-2', '--bord', '--bord-fort',
    '--texte-fort', '--texte', '--texte-faible',
    '--accent', '--accent-survol', '--sur-accent', '--succes', '--alerte', '--danger',
];

function vide(id: string): HTMLElement {
    const hote = document.getElementById(id);
    if (!hote) throw new Error(`the gallery expects a #${id} element`);
    hote.textContent = '';
    return hote;
}

/** A swatch: a sample, the name, the value read at the source. */
function pastille(hote: HTMLElement, nom: string, style: Partial<CSSStyleDeclaration>): void {
    const carte = document.createElement('div');
    carte.className = 'pastille';
    const echantillon = document.createElement('div');
    echantillon.className = 'echantillon';
    Object.assign(echantillon.style, style);
    const etiquette = document.createElement('code');
    etiquette.className = 'nom';
    etiquette.textContent = nom;
    const val = document.createElement('span');
    val.className = 'valeur';
    val.textContent = value(nom);
    carte.append(echantillon, etiquette, val);
    hote.append(carte);
}

/** A "token name — demonstration" line. */
function ligne(hote: HTMLElement, nom: string, demo: HTMLElement): void {
    const rangee = document.createElement('div');
    rangee.className = 'ligne';
    const etiquette = document.createElement('code');
    etiquette.textContent = `${nom} ${value(nom)}`;
    rangee.append(etiquette, demo);
    hote.append(rangee);
}

function texte(contenu: string, style: Partial<CSSStyleDeclaration>): HTMLElement {
    const span = document.createElement('span');
    span.textContent = contenu;
    Object.assign(span.style, style);
    return span;
}

function rendre(): void {
    const couleurs = vide('couleurs');
    for (const nom of within(COULEURS)) pastille(couleurs, nom, { background: `var(${nom})` });

    const voiles = vide('voiles');
    for (const nom of [...famille('--video-'), ...famille('--voile-')]) {
        pastille(voiles, nom, { background: `var(${nom})` });
    }

    const typo = vide('typo');
    for (const nom of famille('--t-')) {
        ligne(typo, nom, texte('Remote session — Aa Éé 0123', { fontSize: `var(${nom})` }));
    }

    const interlignes = vide('interlignes');
    for (const nom of famille('--lh-')) {
        const bloc = document.createElement('span');
        bloc.textContent =
            'Two lines are enough to see a line height: this one is written long enough to wrap at least once in the gallery column.';
        bloc.style.lineHeight = `var(${nom})`;
        bloc.style.display = 'block';
        ligne(interlignes, nom, bloc);
    }

    const espacement = vide('espacement');
    for (const nom of famille('--e-')) {
        const barre = document.createElement('span');
        barre.className = 'barre';
        barre.style.width = `var(${nom})`;
        barre.style.display = 'inline-block';
        ligne(espacement, nom, barre);
    }

    const rayons = vide('rayons');
    for (const nom of famille('--r-')) {
        pastille(rayons, nom, { background: 'var(--fond-2)', borderRadius: `var(${nom})` });
    }

    const traits = vide('traits');
    for (const nom of famille('--trait')) {
        const echantillon = document.createElement('span');
        echantillon.style.display = 'inline-block';
        echantillon.style.width = 'var(--e-8)';
        echantillon.style.borderBlockEnd = `var(${nom}) solid var(--accent)`;
        ligne(traits, nom, echantillon);
    }

    const polices = vide('polices');
    for (const nom of famille('--police-')) {
        ligne(polices, nom, texte('Remote session — Aa Éé 0123', { fontFamily: `var(${nom})` }));
    }
}

/*
 * The three theme buttons now live in `selecteur-theme.ts`: the
 * primitives gallery (S2) reuses them without copying fifteen lines.
 * The extraction was done BEFORE that second gallery, not after.
 *
 * 🔴 NONE OF THE NINE CHECKS LOOKS AT THE DOM. `design.html` carries an
 * empty `<p id="themes"></p>`: a call that installed nothing would leave the
 * page without a theme selector and would pass everything, `npm test` included. The only
 * proof is a runtime red, played at extraction — comment out the line
 * below, build, and observe that the three buttons have disappeared.
 */
installThemeSelectorInDOM(vide('themes'), rendre);
rendre();
