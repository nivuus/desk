// ⚠️ `defineConfig` VIENT DE `vitest/config`, PAS DE `vite` — voir le bloc
// `test` en bas de ce fichier. C'est la même fonction, avec le typage de la
// clé `test` en plus, et Vite l'ignore au build. Aucune dépendance neuve :
// Vitest est déjà en dépendance de développement.
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { defineConfig } from 'vitest/config';

/* ── L'AMORCE ANTI-FOUC, INJECTÉE DEPUIS UNE SOURCE UNIQUE ─────────────────
   Le texte de `src/design/amorce-theme.js` est lu UNE FOIS, à la construction
   du greffon, et rendu tel quel dans le `<head>` de CHAQUE entrée. Il n'y a
   donc aucune copie du script dans ce fichier, et aucune page ne peut recevoir
   une version différente d'une autre.

   ⚠️ LE CHEMIN EST RÉSOLU DEPUIS `import.meta.url`, PAS DEPUIS `process.cwd()`.
   `verifier-design.mjs` bâtit depuis la racine du dépôt aussi bien que depuis
   `client/` ; un chemin relatif au répertoire courant ferait échouer le build
   d'un côté et pas de l'autre.

   🔴 `injectTo: 'head'` ET NON `'head-prepend'`, ET C'EST UNE MESURE.
   `'head-prepend'` est le DÉFAUT de Vite (`HtmlTagDescriptor.injectTo`,
   documenté « default: 'head-prepend' » dans
   `client/node_modules/vite/dist/node/index.d.ts`) : le script sortirait AVANT
   `<meta charset>`. Or ce fichier porte des commentaires accentués, qui
   seraient donc décodés avant que l'encodage du document ne soit connu, et la
   déclaration de charge utile serait repoussée plus loin dans les 1 024
   premiers octets que la spécification HTML lui accorde.
   Avec `'head'`, le script sort APRÈS `<meta charset>` et `<title>`, AVANT le
   module et la feuille de style — et cela SUFFIT à l'anti-FOUC : un script en
   ligne synchrone dans `<head>` s'exécute avant que `<body>` ne soit analysé,
   donc avant la première peinture. Poser l'attribut plus tôt que cela n'achète
   rien et coûte le charset.

   ⚠️ CE FICHIER N'EST PAS TYPECHECKÉ : `client/tsconfig.json:12` n'inclut que
   les fichiers `.ts` sous `src/` et sous `../proto/ts/` — deux motifs que
   `vite.config.ts`, à la racine du paquet, ne satisfait ni l'un ni l'autre. Une erreur de type ici se manifeste comme un ÉCHEC DE BUILD,
   jamais comme une erreur `tsc`. Et un greffon qui « marcherait » sans rien
   injecter ne serait attrapé par aucun des deux : c'est le contrôle §7.3 qui
   le rattrape, plus le relevé de la tâche 10 (`grep -c 'guac.theme'` sur
   chaque `dist/*.html`).

   ⚠️ IL N'Y A DÉLIBÉRÉMENT AUCUN FILTRE SUR L'ENTRÉE. Le handler ignore son
   contexte et rend le même script pour toutes les pages : c'est ce qui rend
   impossible le défaut que la spec §11 nomme — « le greffon Vite d'injection
   casse une entrée ». La rouge correspondante a été jouée en filtrant sur
   `index.html` (1, 0, 0) ; voir le document de résultats de S1. */
const AMORCE = readFileSync(
    fileURLToPath(new URL('./src/design/amorce-theme.js', import.meta.url)),
    'utf8',
);

const greffonAmorce = {
    name: 'guac-amorce-theme',
    transformIndexHtml: {
        order: 'pre' as const,
        handler() {
            return [{ tag: 'script', children: AMORCE, injectTo: 'head' as const }];
        },
    },
};

export default defineConfig({
    plugins: [greffonAmorce],
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
