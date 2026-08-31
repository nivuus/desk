// LE PARCOURS : un utilisateur ouvre le produit, lance une application, et
// VOIT sa fenêtre.
//
// 🔴 C'EST LE TEST QUI MANQUAIT À CHAQUE ÉTAPE DE LA SOIRÉE DU 30 AOÛT 2026.
// Le lot 14 a vérifié que `/` sert le hub. Le lot 17 a vérifié qu'un pair
// tardif reçoit les annonces. Le lot 18 a vérifié que le hub obtient son
// jeton. Chacun a vérifié SA MOITIÉ, et le produit était cassé de bout en
// bout : la page servie à la racine — le hub — n'ouvre aucune connexion de
// signaling et ne mène à AUCUNE page qui en ouvre une, si bien que le
// superviseur annonçait ses fenêtres à personne et les refusait trente
// secondes plus tard. Défaut TROUVÉ EN PRODUCTION, sur la machine du
// propriétaire, et conséquence directe du passage du hub à la racine
// (`plateforme/src/http/page/resolution.ts`, lot 14) : tant que la racine
// servait `index.html`, personne n'avait vu un visiteur atterrir sur une
// surface sans bureau.
//
// 🔴 CE QUE CE TEST NE PEUT PAS FAIRE, ET IL FAUT LE DIRE : ouvrir un
// navigateur. Le flux OAuth de Pomerium exige un humain (blocage ② du
// critère ⑦ d'`auth-pomerium`, toujours non levé). Il modélise donc le
// parcours par les DEUX maillons que le code porte réellement :
//   ① la page servie à la racine mène-t-elle, par un chemin qu'un utilisateur
//      peut suivre, à la surface qui traite `fenetre-ouverte` ?
//   ② cette surface, recevant l'annonce, ouvre-t-elle la fenêtre ?
// Le ② était VERT toute la soirée. C'est le ① qui était rouge, et aucun test
// ne le regardait.
//
// 🔴 IL NE RECOPIE AUCUNE VALEUR : la page servie à la racine est LUE dans
// `resolution.ts`, l'entrée de chaque page est LUE dans son `.html`, et le
// graphe d'imports est parcouru sur les fichiers réels. Un test qui porterait
// sa propre copie de « la racine sert le hub » validerait sa copie.
import { describe, expect, it } from 'vitest';
// ⚠️ **`?raw` ET `import.meta.glob`, JAMAIS `node:fs`** : `client/` n'a pas
// `@types/node`, et un test qui l'emploierait ferait échouer `tsc --noEmit`
// tout en restant vert sous Vitest, qui ne vérifie aucun type. C'est la
// convention de `src/presse-papier.test.ts`, qui lit ainsi jusqu'à un fichier
// Rust hors de `client/`.
import resolutionTs from '../../plateforme/src/http/page/resolution.ts?raw';
import { creerBureau } from './shell';

/// Toutes les pages bâties, LUES. La portée est DÉRIVÉE du répertoire, jamais
/// énumérée : une surface neuve entre dans ce garde sans qu'une ligne d'ici ne
/// change.
const PAGES = import.meta.glob<string>('../*.html', {
    query: '?raw',
    import: 'default',
    eager: true,
});
/// Tous les modules de `client/src/`, LUS. Même raison.
const MODULES = import.meta.glob<string>('./**/*.ts', {
    query: '?raw',
    import: 'default',
    eager: true,
});

/// La page que la plateforme sert à `/`, LUE dans la règle elle-même.
///
/// ⚠️ `PAGE` n'y est pas exportée, et l'importer ferait entrer un fichier hors
/// du `include` de `client/tsconfig.json` dans `tsc --noEmit`. On lit donc le
/// texte — et on EXIGE une occurrence unique, pour qu'un jour où la constante
/// se dédouble le test le dise au lieu de choisir.
function pageServieALaRacine(): string {
    const trouvees = [...resolutionTs.matchAll(/^const PAGE = '([^']+)';$/gm)];
    expect(trouvees).toHaveLength(1);
    return trouvees[0][1];
}

/// La clé de glob d'une page : `hub.html` → `../hub.html`.
const clePage = (page: string) => `../${page}`;

/// Le module d'entrée d'une page bâtie, LU dans son `<script type="module">`,
/// rendu sous la forme d'une clé de `MODULES` (`/src/hub/page.ts` →
/// `./hub/page.ts`).
function entreeDeLaPage(page: string): string {
    const html = PAGES[clePage(page)];
    expect(html, `page introuvable : ${page}`).toBeTypeOf('string');
    const trouvees = [...html.matchAll(/<script type="module" src="\/src\/([^"]+)"/g)];
    expect(trouvees, `entrée unique attendue dans ${page}`).toHaveLength(1);
    return `./${trouvees[0][1]}`;
}

/// Tous les modules de `client/src/` atteints depuis `depart` en suivant les
/// imports — la fermeture, calculée sur les fichiers réels.
function fermetureDImports(depart: string): string[] {
    const vus = new Set<string>();
    const aVoir = [depart];
    while (aVoir.length > 0) {
        const cle = aVoir.pop()!;
        if (vus.has(cle) || MODULES[cle] === undefined) continue;
        vus.add(cle);
        for (const m of MODULES[cle].matchAll(/from '(\.[^']+)'/g)) {
            // `moduleResolution: bundler` : l'extension est omise à l'écrit.
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

describe('le parcours : ouvrir le produit, lancer une application, voir sa fenêtre', () => {
    it("① la page servie à la racine traite elle-même « fenetre-ouverte »", () => {
        // 🔴 RÉÉCRIT PAR LA TÂCHE 8 (31 août 2026) : le hub cesse de MENER à
        // une seconde surface — il tient lui-même la session de contrôle
        // (`bureau/porteur-dom.ts`, câblé depuis `hub/page.ts`). Chercher une
        // PAGE NOMMÉE distincte de la racine serait retomber sur la prémisse
        // que cette tâche retire ; le bon contrôle est désormais que la
        // fermeture d'imports de la racine elle-même porte le traitement.
        const racine = pageServieALaRacine();
        const modules = fermetureDImports(entreeDeLaPage(racine));
        expect(modules.length, 'la fermeture ne peut pas être vide').toBeGreaterThan(1);

        const traite = modules.some((cle) => MODULES[cle].includes('fenetre-ouverte'));
        expect(
            traite,
            `aucun module de la fermeture de ${racine} ne traite « fenetre-ouverte » : ${modules.join(', ')}`,
        ).toBe(true);
    });

    it('② la surface qui reçoit l’annonce ouvre la fenêtre de l’application', () => {
        const ouvertes: string[] = [];
        const bureau = creerBureau({
            ouvrirFenetre(session) {
                ouvertes.push(session);
                return { closed: false } as unknown as Window;
            },
            envoyer() {},
            afficher() {},
            afficherEtatFichiers() {},
            afficherEcrituresDues() {},
            afficherRetenues() {},
        });
        bureau.fenetreOuverte('vm:w-1', 'Untitled - Notepad');
        expect(ouvertes).toEqual(['vm:w-1']);
        expect(bureau.liste()).toEqual([
            { session: 'vm:w-1', titre: 'Untitled - Notepad', ouverte: true },
        ]);
    });
});
