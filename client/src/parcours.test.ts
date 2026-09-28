// THE JOURNEY: a user opens the product, launches an application, and
// SEES its window.
//
// 🔴 IT IS THE TEST THAT WAS MISSING AT EVERY STEP OF THE EVENING OF AUGUST 30TH, 2026.
// Batch 14 checked that `/` serves the hub. Batch 17 checked that a late
// peer receives the announcements. Batch 18 checked that the hub gets its
// token. Each one checked ITS HALF, and the product was broken end to
// end: the page served at the root — the hub — opens no signaling
// connection and leads to NO page that opens one, so that the
// supervisor announced its windows to nobody and refused them thirty
// seconds later. A defect FOUND IN PRODUCTION, on the owner's
// machine, and a direct consequence of moving the hub to the root
// (`plateforme/src/http/page/resolution.ts`, batch 14): as long as the root
// served `index.html`, nobody had seen a visitor land on a
// surface without a desktop.
//
// 🔴 WHAT THIS TEST CANNOT DO, AND IT MUST BE SAID: open a
// browser. Pomerium's OAuth flow requires a human (blocker ② of
// criterion ⑦ of `auth-pomerium`, still not lifted). It therefore models the
// journey through the TWO links the code really carries:
//   ① does the page served at the root lead, by a path a user
//      can follow, to the surface that handles `fenetre-ouverte`?
//   ② does that surface, receiving the announcement, open the window?
// ② was GREEN all evening. It is ① that was red, and no test
// looked at it.
//
// 🔴 IT COPIES NO VALUE: the page served at the root is READ from
// `resolution.ts`, the entry of each page is READ from its `.html`, and the
// import graph is walked over the real files. A test that carried
// its own copy of "the root serves the hub" would validate its copy.
import { describe, expect, it } from 'vitest';
// ⚠️ **`?raw` AND `import.meta.glob`, NEVER `node:fs`**: `client/` does not have
// `@types/node`, and a test that used it would make `tsc --noEmit` fail
// while staying green under Vitest, which checks no types. It is the
// convention of `src/presse-papier.test.ts`, which reads this way even a Rust
// file outside `client/`.
import resolutionTs from '../../plateforme/src/http/page/resolution.ts?raw';
import { createDesktop } from './shell';

/// All the built pages, READ. The scope is DERIVED from the directory, never
/// enumerated: a new surface enters this guard without a line here
/// changing.
const PAGES = import.meta.glob<string>('../*.html', {
    query: '?raw',
    import: 'default',
    eager: true,
});
/// All the modules of `client/src/`, READ. Same reason.
const MODULES = import.meta.glob<string>('./**/*.ts', {
    query: '?raw',
    import: 'default',
    eager: true,
});

/// The page the platform serves at `/`, READ from the rule itself.
///
/// ⚠️ `PAGE` is not exported there, and importing it would bring a file outside
/// the `include` of `client/tsconfig.json` into `tsc --noEmit`. We therefore read the
/// text — and REQUIRE a single occurrence, so that the day the constant
/// is doubled the test says so instead of choosing.
function pageServieALaRacine(): string {
    const trouvees = [...resolutionTs.matchAll(/^const PAGE = '([^']+)';$/gm)];
    expect(trouvees).toHaveLength(1);
    return trouvees[0][1];
}

/// The glob key of a page: `hub.html` → `../hub.html`.
const clePage = (page: string) => `../${page}`;

/// The entry module of a built page, READ from its `<script type="module">`,
/// returned as a `MODULES` key (`/src/hub/page.ts` →
/// `./hub/page.ts`).
function entreeDeLaPage(page: string): string {
    const html = PAGES[clePage(page)];
    expect(html, `page not found: ${page}`).toBeTypeOf('string');
    const trouvees = [...html.matchAll(/<script type="module" src="\/src\/([^"]+)"/g)];
    expect(trouvees, `single entry expected in ${page}`).toHaveLength(1);
    return `./${trouvees[0][1]}`;
}

/// All the modules of `client/src/` reached from `depart` by following the
/// imports — the closure, computed over the real files.
function fermetureDImports(depart: string): string[] {
    const vus = new Set<string>();
    const aVoir = [depart];
    while (aVoir.length > 0) {
        const cle = aVoir.pop()!;
        if (vus.has(cle) || MODULES[cle] === undefined) continue;
        vus.add(cle);
        for (const m of MODULES[cle].matchAll(/from '(\.[^']+)'/g)) {
            // `moduleResolution: bundler`: the extension is omitted when written.
            const segments = `${cle.slice(0, cle.lastIndexOf('/'))}/${m[1]}`.split('/');
            const pile: string[] = [];
            for (const segment of segments) {
                if (segment === '.' || segment === '') continue;
                if (segment === '..') pile.pop();
                else pile.push(segment);
            }
            const brut = `./${pile.join('/')}`;
            for (const candidat of [brut, `${brut}.ts`, `${brut}/index.ts`]) {
                if (MODULES[candidat] !== undefined) aVoir.push(candidat);
            }
        }
    }
    return [...vus];
}

/// Does this module REALLY dispatch on `fenetre-ouverte`?
///
/// 🔴 **ANCHORED ON SYNTAX, AND IT WAS NOT** (Minor ⑦ of the final
/// review of August 31st, 2026). Guard ① looked for the SUBSTRING
/// `fenetre-ouverte` in the source text, **comments included**. The
/// weakness was declared as pre-existing — it became **ARMED**
/// by this project: `bureau/porteur.ts` now carries this word **in prose**
/// and lives in the import closure of the root, so that deleting the
/// real branch of `porteur-dom.ts` would have left this test **GREEN**.
///
/// The comparison `… .type === 'fenetre-ouverte'` is syntax: it cannot
/// arise from a sentence in prose. **A comment that QUOTED it
/// between backticks would still satisfy it** — that is the limit, and it is
/// stated: this guard excludes ORDINARY prose, never a comment that
/// copied the code. The price of a stronger anchoring (parsing
/// TypeScript) cannot be paid here.
const AIGUILLAGE_FENETRE_OUVERTE = /\.type === 'fenetre-ouverte'/;

function aiguilleSurFenetreOuverte(source: string): boolean {
    return AIGUILLAGE_FENETRE_OUVERTE.test(source);
}

describe('guard ① itself: what it accepts and what it REFUSES', () => {
    // 🔴 A CHECK NEVER SEEN RED IS NOT A CHECK, and the
    // witness does NOT live in the repository: test prose is stable, product
    // prose is not. We therefore exercise the predicate on two
    // strings made up here, one of which is exactly the case the old
    // guard let through.
    it('REFUSES a mere mention in prose', () => {
        expect(
            aiguilleSurFenetreOuverte(
                '// no fenetre-ouverte message reaches its desktop, which opens no socket',
            ),
        ).toBe(false);
    });

    it('ACCEPTS the real comparison', () => {
        expect(
            aiguilleSurFenetreOuverte("if (message.type === 'fenetre-ouverte') bureau.fenetreOuverte(s, t);"),
        ).toBe(true);
    });
});

describe('the journey: open the product, launch an application, see its window', () => {
    it("① the page served at the root handles « fenetre-ouverte » itself", () => {
        // 🔴 REWRITTEN BY TASK 8 (August 31st, 2026): the hub stops LEADING to
        // a second surface — it holds the control session itself
        // (`bureau/porteur-dom.ts`, wired from `hub/page.ts`). Looking for a
        // NAMED PAGE distinct from the root would fall back on the premise
        // this task removes; the right check is now that the
        // import closure of the root itself carries the handling.
        const racine = pageServieALaRacine();
        const modules = fermetureDImports(entreeDeLaPage(racine));
        expect(modules.length, 'the closure cannot be empty').toBeGreaterThan(1);

        // 🔴 ON SYNTAX, NOT ON A SUBSTRING OF THE SOURCE TEXT — see
        // `AIGUILLAGE_FENETRE_OUVERTE` above, and the guard that exercises it.
        const traite = modules.some((cle) => aiguilleSurFenetreOuverte(MODULES[cle]));
        expect(
            traite,
            `no module of the closure of ${racine} DISPATCHES on « fenetre-ouverte »: ${modules.join(', ')}`,
        ).toBe(true);
    });

    it('③ the root RETAINS the VM prefix BEFORE installing the desktop', () => {
        // 🔴 CRITICAL ② OF THE FINAL REVIEW: the hub set NO prefix.
        // `poserPrefixe` had only one production caller — `connexion.ts`,
        // on the SIGN-IN page —, yet this project makes a visitor
        // behind Pomerium get their token ON THE HUB and never go through
        // that screen. `lirePrefixe()` returned `''`, `composer('', 'bureau')`
        // returned `bureau`, and the agent announced on `<prefixe>:bureau`:
        // **no `fenetre-ouverte` ever arrived**. The election lock,
        // not prefixed either, made the protection
        // `nomDuVerrou` claims to offer vacuous.
        //
        // 🔴 **ORDER IS EVERYTHING**: the desktop composes its session and
        // lock names from `lirePrefixe()`, so it must be installed AFTERWARDS.
        //
        // ⚠️ **WHAT THIS GUARD IS WORTH, AND WHAT IT IS NOT.** It reads the
        // SOURCE TEXT, like all of this file — it cannot run
        // `hub/page.ts`, which touches the DOM on load and which `client/` cannot
        // mount (neither jsdom nor happy-dom, by convention). It therefore requires
        // **ONE occurrence of each**: a second one, even in prose,
        // makes it FAIL loudly instead of letting it choose — it is the
        // pattern of `pageServieALaRacine` above.
        const source = MODULES[entreeDeLaPage(pageServieALaRacine())];
        const prefixe = [...source.matchAll(/\bretenirLePrefixe\(/g)];
        const bureau = [...source.matchAll(/\binstallerLeBureau\(/g)];
        expect(prefixe, 'a single call to retenirLePrefixe expected').toHaveLength(1);
        expect(bureau, 'a single call to installerLeBureau expected').toHaveLength(1);
        expect(
            prefixe[0].index,
            'the prefix must be retained BEFORE the desktop composes its session names',
        ).toBeLessThan(bureau[0].index);
    });

    it('② the surface that receives the announcement opens the application window', () => {
        const ouvertes: string[] = [];
        const bureau = createDesktop({
            ouvrirFenetre(session) {
                ouvertes.push(session);
                return { closed: false } as unknown as Window;
            },
            envoyer() {},
            show() {},
            showFilesState() {},
            showPendingWrites() {},
            showRetained() {},
        });
        bureau.fenetreOuverte('vm:w-1', 'Untitled - Notepad');
        expect(ouvertes).toEqual(['vm:w-1']);
        expect(bureau.list()).toEqual([
            { session: 'vm:w-1', titre: 'Untitled - Notepad', ouverte: true },
        ]);
    });
});
