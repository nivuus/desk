// ⚠️ `defineConfig` VIENT DE `vitest/config`, PAS DE `vite` — voir le bloc
// `test` en bas de ce fichier. C'est la même fonction, avec le typage de la
// clé `test` en plus, et Vite l'ignore au build. Aucune dépendance neuve :
// Vitest est déjà en dépendance de développement.
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { defineConfig } from 'vitest/config';
// 🔴 LE GREFFON DU MANIFESTE DU HUB LIT LES TOKENS PAR LEUR PARSEUR, jamais par
// une expression régulière de son cru : un contrôle qui a sa propre copie des
// valeurs valide sa copie (spec §7.1 de ⑥). Node v24 importe un `.ts`
// nativement, ce que trois outils du socle exploitent déjà — et c'est pourquoi
// tout module de `client/src/design/` importé par un outil doit rester
// « effaçable » : ni `enum`, ni `namespace`, ni décorateur.
// ⚠️ L'EXTENSION `.ts` EST OBLIGATOIRE ICI, ET SON ABSENCE CASSE DEUX
// CONTRÔLES DE ⑥ — pas ce fichier. `outils/tokens-orphelins.mjs` et
// `outils/classes-employees.mjs` importent CE fichier pour en dériver leur
// périmètre, et ils sont chargés par NODE, dont le résolveur exige
// l'extension là où Vite s'en passe. Écrite sans elle, la ligne laissait le
// build VERT et faisait tomber §7.6 et §7.9 en `ERR_MODULE_NOT_FOUND`.
import { lireBlocsDeTheme, valeurDePropriete } from './src/design/tokens.ts';
// 🔴 `NOM_FICHIER_AMORCE` ET `baliseAmorce` VIVENT SOUS `src/`, PAS ICI —
// extraits le 29 août 2026 (lot `csp-amorce`) précisément pour rester
// TYPECHECKÉS, ce que ce fichier n'est jamais (voir plus bas). `AMORCE`, elle,
// RESTE lue ici, par `node:fs` — voir le commentaire de
// `amorce-theme-greffon.ts` sur pourquoi un import `?raw` ne résout PAS
// quand Vite bundle sa PROPRE configuration. Lire ce fichier pour le
// raisonnement complet : pourquoi l'amorce est un fichier externe `'self'`
// (jamais `children:` en ligne) et pourquoi un hash `sha256-…` dans la CSP a
// été écarté.
import { NOM_FICHIER_AMORCE, baliseAmorce } from './src/design/amorce-theme-greffon.ts';

/// Le CONTENU de l'amorce, lu UNE FOIS par `node:fs` — jamais `?raw`, voir le
/// commentaire de `amorce-theme-greffon.ts` : un tel import ne résout pas
/// quand Vite bundle sa PROPRE configuration (mesuré : « No matching export …
/// for import "default" »). `amorce-theme.csp.test.ts`, lui, lit ce même
/// fichier par `?raw` — qui résout très bien sous Vitest — pour comparer,
/// octet pour octet, la source à ce que `dist/amorce-theme.js` porte
/// réellement.
const AMORCE = readFileSync(
    fileURLToPath(new URL('./src/design/amorce-theme.js', import.meta.url)),
    'utf8',
);

/* ═══════════════════════════════════════════════════════════════════════════
   LE MANIFESTE DU HUB — engendré AU BUILD, jamais écrit en dur (sous-bloc G5).

   🔴 POURQUOI IL EST ENGENDRÉ. `client/public/` n'existe pas, et un fichier
   `.webmanifest` posé n'importe où ÉCHAPPERAIT à §7.2, dont le périmètre est
   `client/src/**` en `.css`/`.ts` plus les entrées Vite : une couleur
   littérale y passerait sans qu'aucun contrôle ne la voie, et le dépôt aurait
   **deux sources de vérité pour une couleur**. Le greffon lit donc `--fond-0`
   et `--accent` par `client/src/design/tokens.ts`, exactement comme les
   contrôles §7.1, §7.4 et §7.6 le font — Node importe un `.ts` nativement, ce
   que trois outils de ⑥ exploitent déjà.

   🔴 CE QUE LE HUB DÉCLARE ET QUE LES MANIFESTES PAR APPLICATION NE PEUVENT
   PAS DÉCLARER : les `file_handlers` des types installeur, conformément à
   l'amendement du 28/07/2026 au cadrage produit. **Aucune fonctionnalité n'en
   dépend** — c'est la clause de l'amendement, et le critère ② de G5 existe
   pour la garder : le glisser-déposer fonctionne SEUL.

   ⚠️ LES TYPES MIME SONT UN CHOIX, PAS UN STANDARD — ni `.msi` ni `.bat`
   n'ont d'enregistrement IANA univoque. Ce qui compte pour le critère ③ n'est
   pas leur justesse mais CE QUE CHROMIUM EN RÉPOND, et c'est ce qui est relevé.

   ⚠️ CE FICHIER N'EST PAS TYPECHECKÉ (voir plus haut) : une erreur ici est un
   ÉCHEC DE BUILD, jamais une erreur `tsc`.
   ═══════════════════════════════════════════════════════════════════════════ */
// 🔴 `tokens/couleurs.css` SEUL, JAMAIS `tokens.css` : depuis l'extraction de
// la tâche 6 (25 août 2026), `tokenRacine` ci-dessous n'est appelée qu'avec
// des noms de COULEUR (`--fond-0`, `--accent`, pour le manifeste web) —
// `tokens/echelles.css`, son voisin, n'en a aucune à offrir.
const TOKENS_CSS = readFileSync(
    fileURLToPath(new URL('./src/design/tokens/couleurs.css', import.meta.url)),
    'utf8',
);

/** La valeur d'un token du bloc RACINE (thème sombre), ou une erreur de build. */
function tokenRacine(nom: string): string {
    const blocs = lireBlocsDeTheme(TOKENS_CSS);
    const racine = blocs.find((b) => b.nom === 'racine');
    if (racine === undefined) throw new Error('tokens/couleurs.css ne porte plus de bloc racine');
    const valeur = valeurDePropriete(racine, nom);
    // 🔴 ON LÈVE PLUTÔT QUE DE REPLIER SUR UNE COULEUR PAR DÉFAUT : un repli
    //    poserait une couleur qui n'est celle d'aucun token, c'est-à-dire la
    //    seconde source de vérité que ce greffon existe pour éviter — et il le
    //    ferait EN SILENCE.
    if (valeur === null) throw new Error(`tokens/couleurs.css ne déclare plus ${nom}`);
    return valeur;
}

const MANIFESTE_HUB = {
    name: 'Applications',
    short_name: 'Applications',
    id: '/hub.html',
    start_url: '/hub.html',
    scope: '/',
    display: 'standalone',
    display_override: ['window-controls-overlay', 'standalone'],
    background_color: tokenRacine('--fond-0'),
    theme_color: tokenRacine('--accent'),
    file_handlers: [
        {
            action: '/hub.html',
            accept: {
                'application/x-msi': ['.msi'],
                'application/vnd.microsoft.portable-executable': ['.exe'],
                'application/x-bat': ['.bat'],
            },
        },
    ],
};

const greffonManifesteHub = {
    name: 'guac-manifeste-hub',
    generateBundle(_options: unknown, _bundle: unknown) {
        // @ts-expect-error — `this.emitFile` est l'API de Rollup, et ce
        // fichier n'est pas typechecké : l'annotation dit l'intention.
        this.emitFile({
            type: 'asset',
            fileName: 'hub.webmanifest',
            source: JSON.stringify(MANIFESTE_HUB, null, 2),
        });
    },
    transformIndexHtml: {
        order: 'post' as const,
        handler(_html: string, ctx: { path: string }) {
            // 🔴 LE FILTRE EST ICI DÉLIBÉRÉ, À L'INVERSE DU GREFFON D'AMORCE :
            //    le manifeste du hub ne concerne QUE le hub. Le poser sur les
            //    cinq autres pages en ferait des PWA qu'on n'a pas voulues, et
            //    §7.3 ne le dirait pas — il ne juge que les feuilles de style.
            if (!ctx.path.endsWith('/hub.html')) return [];
            return [
                {
                    tag: 'link',
                    attrs: { rel: 'manifest', href: '/hub.webmanifest' },
                    injectTo: 'head' as const,
                },
            ];
        },
    },
};

export const greffonAmorce = {
    name: 'guac-amorce-theme',
    // Émet le texte de l'amorce comme un ACTIF du build, au même titre que
    // `hub.webmanifest` plus haut — jamais recopié, toujours la même lecture.
    generateBundle(_options: unknown, _bundle: unknown) {
        // @ts-expect-error — `this.emitFile` est l'API de Rollup, et ce
        // fichier n'est pas typechecké : l'annotation dit l'intention.
        this.emitFile({
            type: 'asset',
            fileName: NOM_FICHIER_AMORCE,
            source: AMORCE,
        });
    },
    transformIndexHtml: {
        order: 'pre' as const,
        // 🔴 `baliseAmorce()` VIENT DE `src/design/amorce-theme-greffon.ts`,
        // JAMAIS RECOPIÉE ICI : c'est la MÊME fonction que
        // `amorce-theme.csp.test.ts` appelle pour vérifier la forme de la
        // balise — un greffon qui aurait sa propre copie validerait sa copie.
        handler: () => [baliseAmorce()],
    },
};

export default defineConfig({
    plugins: [greffonAmorce, greffonManifesteHub],
    server: {
        host: '0.0.0.0',
        port: 5173,
    },
    build: {
        // ⚠️ UNE PAGE ABSENTE DE CETTE LISTE NE SORT PAS DU BUILD, ET RIEN NE
        // LE DIT : `npm run build` rend 0 et la page manque simplement de
        // `dist/`. Relevé le 19 août 2026 en jouant le cas — `connexion.html`
        // existait déjà à la racine, le build a réussi, et `dist/` ne portait
        // que `index.html` et `shell.html`. Toute page neuve s'ajoute ici.
        rollupOptions: {
            input: {
                main: 'index.html',
                shell: 'shell.html',
                connexion: 'connexion.html',
                // La galerie de tokens : une surface bâtie comme les autres,
                // donc soumise aux contrôles §7.2 et §7.3 — mais EXCLUE de la
                // moitié « employé » du §7.6, qu'elle rendrait incapable
                // d'échouer. Voir `client/outils/tokens-orphelins.mjs`.
                design: 'design.html',
                // La galerie des PRIMITIVES (S2). Elle naît à part plutôt que
                // dans `design.html`, qui était à 231 lignes pour une porte de
                // 300 : la scission est décidée AVANT l'addition, jamais après.
                // Elle aussi est EXCLUE de la moitié « employé » du §7.6, et
                // pour la même raison — voir `client/outils/tokens-orphelins.mjs`.
                primitives: 'primitives.html',
                // Le HUB (sous-bloc G5 de ④). Il entre AUTOMATIQUEMENT dans
                // §7.2, §7.3, §7.6 et §7.9 ① du seul fait d'être ici — leurs
                // périmètres sont DÉRIVÉS de cette liste, jamais recopiés.
                // ⚠️ Il n'entre PAS dans `SURFACES_PRODUIT` de §7.9 ② A, qui
                // est la SEULE liste en dur du socle
                // (`client/outils/classes-employees.mjs`). L'y ajouter ne
                // changerait aucun verdict — ② A est un PLANCHER, déjà
                // satisfait par trois surfaces — et ferait franchir à ④ la
                // frontière d'un outil de ⑥, qui est clos. Legs, plutôt
                // qu'une modification cosmétique à risque (décision D6).
                hub: 'hub.html',
            },
        },
    },
    test: {
        // ⚠️ SANS CECI, UN `import css from './x.css?raw'` REND LA CHAÎNE VIDE
        // SOUS VITEST, silencieusement. Vitest court-circuite les fichiers CSS
        // par défaut (`css: false`), et le court-circuit attrape aussi la
        // requête `?raw`. Mesuré le 19 août 2026 : `typeof` rend bien
        // `string`, mais `longueur` rend `0` — donc un test qui parserait ce
        // texte ne verrait AUCUN token et passerait au vert en ne mesurant
        // rien, ce qui est le pire des deux mondes.
        //
        // `client/src/design/reprise.test.ts` en dépend : c'est lui qui prouve
        // que `tokens.css` reprend caractère pour caractère les valeurs
        // d'avant le socle, et il doit lire le VRAI fichier — un miroir des
        // valeurs en TypeScript validerait sa propre copie.
        //
        // ⚠️ Il n'y a délibérément PAS de `client/vitest.config.ts` : un tel
        // fichier prendrait le pas sur celui-ci et Vitest cesserait de lire la
        // configuration Vite, sans rien dire.
        css: true,
    },
});
