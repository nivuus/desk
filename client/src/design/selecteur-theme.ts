/**
 * THE THREE THEME BUTTONS — the PRODUCT's theme selector.
 *
 * ✅ THIS MODULE HAS BEEN PART OF THE PRODUCT SINCE SUB-BLOCK S3, and it has tests.
 * It was born in S1 as INSTRUMENT code — it then only served the
 * pages of §8's human judgement — and its header declared "this is not the
 * product's theme selector: that one belongs to S3, which will decide whether to
 * reuse it as is or rewrite it". S3 decided: it REUSES it. There
 * is no second copy — it is the rule §4.1 of the spec applies to
 * values, and there is no reason to grant it to modules.
 *
 * ── WHERE IT LIVES, AND WHERE IT DOES NOT ─────────────────────────────────
 * On the HUB — the surface spec §5.2 calls "the shell page", and which
 * absorbed it on August 31st, 2026 — and on the LOGIN SCREEN, which the spec does not
 * name because it did not exist when it was written (its §2.5 says
 * so) — REASONED EXTENSION, declared: `hub/page.ts` redirects every
 * visitor without a token there, so that it is today the first surface, and
 * sometimes the only one, an unauthenticated user sees. A dark default
 * that cannot be changed before logging in is what spec §11
 * files under "reversible by a user in one click from S3".
 *
 * ⚠️ THIS PARAGRAPH SAID "On the SHELL PAGE … `shell-page.ts` redirects there"
 * until the final review of August 31st, 2026. BOTH halves were wrong:
 * the selector is wired from `hub/page.ts` (`#themes`), and it is
 * `hub/page.ts::demarrer` that sends back to `connexion.html`.
 *
 * ⛔ NEVER IN THE SESSION WINDOW. Spec §5.2 forbids it by name:
 * a toolbar on a fullscreen game is a regression. "The
 * session window FOLLOWS, it does not choose."
 *
 * ── DEPENDENCIES ARE INJECTED ─────────────────────────────────────────────
 * ⚠️ NEITHER `window`, NOR `document`, NOR `localStorage` IS READ ON THE TESTED
 * PATH. `client/` has NEITHER jsdom NOR happy-dom (measured: `client/package.json`
 * only carries `typescript`, `vite`, `vitest`, and no `@vitest-environment`
 * exists in `client/src/`). A product module reading a global at the
 * top would be impossible to load under Node, hence impossible to test. It is
 * the pattern of `theme.ts` and `fullscreen.ts`, and `installerSelecteurDeTheme
 * AuDOM` is the seam linking them to the real objects — like
 * `armerPleinEcranAuDOM`.
 *
 * ⚠️ `Coffre` and `Racine` are IMPORTED from `theme.ts`, never redeclared here.
 * Two copies of a contract diverge silently.
 */
import { CLE_THEME, appliquer, choisir, surStockageModifie, themeStocke } from './theme';
import type { Coffre, Racine, Theme } from './theme';

/** What a theme button must be able to do — nothing more. */
export interface BoutonDeTheme {
    dataset: { theme?: string };
    textContent: string | null;
    setAttribute(nom: string, valeur: string): void;
    addEventListener(type: 'click', ecouteur: () => void): void;
}

/** The host that receives the three buttons. */
export interface HoteDeSelecteur {
    append(bouton: BoutonDeTheme): void;
}

/** The source of `storage` events — `window` in production. */
export interface SourceDeStockage {
    addEventListener(
        type: 'storage',
        ecouteur: (evenement: { key: string | null; newValue: string | null }) => void,
    ): void;
}

export interface OptionsSelecteurDeTheme {
    hote: HoteDeSelecteur;
    racine: Racine;
    coffre: Coffre;
    source: SourceDeStockage;
    /** Makes a BLANK button; the caller puts whatever visuals it wants in it. */
    creerBouton(): BoutonDeTheme;
    /** Called back after EACH theme change, whatever its origin. */
    apres: () => void;
}

/**
 * Places the three buttons in `hote`, applies the stored state, and wires
 * listening to `storage`.
 *
 * ⚠️ `hote` IS NOT EMPTIED: two calls on the same element would place six
 * buttons there. It is the caller that decides.
 */
export function installerSelecteurDeTheme(options: OptionsSelecteurDeTheme): void {
    const { hote, racine, coffre, source, creerBouton, apres } = options;
    const etats: Theme[] = ['systeme', 'clair', 'sombre'];

    // 🔴 THE BUTTONS ARE KEPT HERE, AND NOT REREAD THROUGH `querySelectorAll`.
    // The instrument version queried the host's DOM; besides the fact that no
    // double can do so without jsdom, it also marked the buttons of an
    // EARLIER installation on the same host.
    const boutons: BoutonDeTheme[] = [];
    const marquer = () => {
        const courant = themeStocke(coffre);
        for (const bouton of boutons) {
            bouton.setAttribute('aria-pressed', String(bouton.dataset.theme === courant));
        }
    };

    // The bootstrap has already set the attribute before first paint; this call
    // covers the case where storage changed between the bootstrap and the execution of this
    // module.
    appliquer(racine, themeStocke(coffre));

    // The switch coming from ANOTHER window — it is the half `choisir()`
    // cannot cover, `storage` never firing in the writer.
    //
    // ✅ IT CALLS `marquer()` AGAIN, AND IT IS A FIX OF SUB-BLOCK S3.
    // Until then it did not, and this module DECLARED the defect:
    // "an `aria-pressed` set here stays that of the previous theme as long as
    // the user does not click in THIS window. The defect is real and it
    // is PRE-EXISTING". It was indeed — S1's extraction task
    // was right not to fix it, an extraction that fixes making
    // its own proof that nothing moved false.
    //
    // 🔴 WHAT CHANGED IS NOT THE DEFECT, IT IS WHAT THIS MODULE IS. On
    // an INSTRUMENT, a stale `aria-pressed` is a nuisance. On the PRODUCT,
    // it is an interface that LIES about its own state — and the case of a
    // neighbouring window changing the theme is not an edge case: it is the
    // NOMINAL case of multi-window, which is the very reason for being of the
    // `storage` mechanism (spec §4.2). The shell page opens N session
    // windows on the same origin; all receive this event.
    //
    // ⚠️ `marquer()` READS the vault, it does not write it. Writing here would send a
    // `storage` back to the neighbouring windows, which would send it back in turn, without
    // end — `selecteur-theme.test.ts` guards this property with a test that
    // CAN fail, the closure indeed holding the vault.
    source.addEventListener('storage', (evenement) => {
        if (evenement.key !== CLE_THEME) return;
        surStockageModifie(racine, evenement.key, evenement.newValue);
        marquer();
        apres();
    });

    for (const etat of etats) {
        const bouton = creerBouton();
        bouton.dataset.theme = etat;
        bouton.textContent = etat;
        bouton.addEventListener('click', () => {
            choisir(coffre, racine, etat);
            marquer();
            apres();
        });
        boutons.push(bouton);
        hote.append(bouton);
    }
    marquer();
}

/**
 * THE SEAM TO THE REAL DOM — the only function of this module that touches
 * `document`, `window` and `localStorage`, and hence the only one that is not
 * testable without a browser. It carries NO rule: it only
 * provides the real objects to the function above. Same split as
 * `armerPleinEcran` / `armerPleinEcranAuDOM` in `client/src/fullscreen.ts`.
 *
 * ⚠️ THE TWO CLASSES ARE WRITTEN AS LITERALS, one `classList.add` per class.
 * Check §7.9 only sees literals; a composed class
 * (`` `bouton--${variante}` ``) would be invisible to it, and convention §6.4 of
 * plan S3 is what makes this check useful.
 *
 * ⚠️ THE BUTTONS HAVE NO ACCESSIBLE LABEL OTHER THAN THEIR TEXT
 * (`systeme`, `clair`, `sombre`) and their `aria-pressed`. No primitive
 * carries an ARIA role — primitives are CSS, semantics stay in the
 * markup —, and a `role="group"` with its label would belong to the
 * markup of each page. Declared rather than assumed done.
 */
export function installerSelecteurDeThemeAuDOM(
    hote: HTMLElement,
    apres: () => void = () => {},
): void {
    installerSelecteurDeTheme({
        // ⚠️ THE HOST IS ADAPTED RATHER THAN `HTMLElement` WIDENING THE
        // CONTRACT. `HTMLElement.append` accepts `(...nodes: (string|Node)[])`,
        // which `HoteDeSelecteur.append(bouton: BoutonDeTheme)` does not
        // satisfy — and it is TypeScript that said so, not an assumption. Widening
        // `BoutonDeTheme` up to `Node` to silence the error would have made
        // the test double impossible to write without jsdom, that is, would have
        // made this module untestable to satisfy the compiler.
        hote: { append: (bouton) => hote.append(bouton as unknown as HTMLElement) },
        racine: document.documentElement,
        coffre: localStorage,
        source: window,
        creerBouton: () => {
            const bouton = document.createElement('button');
            bouton.type = 'button';
            bouton.classList.add('bouton');
            bouton.classList.add('bouton--secondaire');
            return bouton;
        },
        apres,
    });
}
