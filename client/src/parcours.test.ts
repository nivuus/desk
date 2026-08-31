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

/// Ce module AIGUILLE-T-IL réellement sur `fenetre-ouverte` ?
///
/// 🔴 **ANCRÉ SUR LA SYNTAXE, ET IL NE L'ÉTAIT PAS** (Minor ⑦ de la revue
/// finale du 31 août 2026). Le garde ① cherchait la SOUS-CHAÎNE
/// `fenetre-ouverte` dans le texte source, **commentaires compris**. La
/// faiblesse était déclarée comme préexistante — elle est devenue **ARMÉE**
/// par ce chantier : `bureau/porteur.ts` porte désormais ce mot **en prose**
/// et vit dans la fermeture d'imports de la racine, si bien que supprimer la
/// branche réelle de `porteur-dom.ts` aurait laissé ce test **VERT**.
///
/// La comparaison `… .type === 'fenetre-ouverte'` est de la syntaxe : elle ne
/// peut pas naître d'une phrase française. **Un commentaire qui la CITERAIT
/// entre backticks la satisferait encore** — c'est la limite, et elle est
/// dite : ce garde exclut la prose ORDINAIRE, jamais un commentaire qui
/// recopierait le code. Le prix d'un ancrage plus fort (analyser du
/// TypeScript) n'est pas payable ici.
const AIGUILLAGE_FENETRE_OUVERTE = /\.type === 'fenetre-ouverte'/;

function aiguilleSurFenetreOuverte(source: string): boolean {
    return AIGUILLAGE_FENETRE_OUVERTE.test(source);
}

describe('le garde ① lui-même : ce qu’il accepte et ce qu’il REFUSE', () => {
    // 🔴 UN CONTRÔLE QU'ON N'A JAMAIS VU ROUGE N'EST PAS UN CONTRÔLE, et le
    // témoin ne vit PAS dans le dépôt : une prose de test est stable, une
    // prose de produit ne l'est pas. On éprouve donc le prédicat sur deux
    // chaînes fabriquées ici, dont l'une est exactement le cas que l'ancien
    // garde laissait passer.
    it('REFUSE une simple mention en prose', () => {
        expect(
            aiguilleSurFenetreOuverte(
                '// aucun message fenetre-ouverte n atteint son bureau, qui n ouvre aucun socket',
            ),
        ).toBe(false);
    });

    it('ACCEPTE la comparaison réelle', () => {
        expect(
            aiguilleSurFenetreOuverte("if (message.type === 'fenetre-ouverte') bureau.fenetreOuverte(s, t);"),
        ).toBe(true);
    });
});

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

        // 🔴 SUR LA SYNTAXE, PAS SUR UNE SOUS-CHAÎNE DU TEXTE SOURCE — voir
        // `AIGUILLAGE_FENETRE_OUVERTE` ci-dessus, et le garde qui l'éprouve.
        const traite = modules.some((cle) => aiguilleSurFenetreOuverte(MODULES[cle]));
        expect(
            traite,
            `aucun module de la fermeture de ${racine} n’AIGUILLE sur « fenetre-ouverte » : ${modules.join(', ')}`,
        ).toBe(true);
    });

    it('③ la racine RETIENT le préfixe de la VM AVANT d’installer le bureau', () => {
        // 🔴 CRITIQUE ② DE LA REVUE FINALE : le hub ne posait AUCUN préfixe.
        // `poserPrefixe` n'avait qu'un appelant de production — `connexion.ts`,
        // sur la page de CONNEXION —, or ce chantier fait qu'un visiteur
        // derrière Pomerium obtient son jeton SUR LE HUB et ne passe jamais par
        // cet écran. `lirePrefixe()` rendait `''`, `composer('', 'bureau')`
        // rendait `bureau`, et l'agent annonçait sur `<prefixe>:bureau` :
        // **aucun `fenetre-ouverte` n'arrivait jamais**. Le verrou d'élection,
        // lui non plus préfixé, rendait vacueuse la protection que
        // `nomDuVerrou` déclare offrir.
        //
        // 🔴 **L'ORDRE EST TOUT** : le bureau compose ses noms de session et de
        // verrou à partir de `lirePrefixe()`, donc il doit être installé APRÈS.
        //
        // ⚠️ **CE QUE CE GARDE VAUT, ET CE QU'IL NE VAUT PAS.** Il lit le
        // TEXTE SOURCE, comme tout ce fichier — il ne peut pas exécuter
        // `hub/page.ts`, qui touche le DOM au chargement et que `client/` ne
        // sait pas monter (ni jsdom ni happy-dom, par convention). Il exige
        // donc **UNE occurrence de chacun** : une seconde, fût-elle en prose,
        // le fait ÉCHOUER bruyamment au lieu de le laisser choisir — c'est le
        // patron de `pageServieALaRacine` ci-dessus.
        const source = MODULES[entreeDeLaPage(pageServieALaRacine())];
        const prefixe = [...source.matchAll(/\bretenirLePrefixe\(/g)];
        const bureau = [...source.matchAll(/\binstallerLeBureau\(/g)];
        expect(prefixe, 'un seul appel à retenirLePrefixe attendu').toHaveLength(1);
        expect(bureau, 'un seul appel à installerLeBureau attendu').toHaveLength(1);
        expect(
            prefixe[0].index,
            'le préfixe doit être retenu AVANT que le bureau ne compose ses noms de session',
        ).toBeLessThan(bureau[0].index);
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
