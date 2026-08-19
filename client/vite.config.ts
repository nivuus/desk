// ⚠️ `defineConfig` VIENT DE `vitest/config`, PAS DE `vite` — voir le bloc
// `test` en bas de ce fichier. C'est la même fonction, avec le typage de la
// clé `test` en plus, et Vite l'ignore au build. Aucune dépendance neuve :
// Vitest est déjà en dépendance de développement.
import { defineConfig } from 'vitest/config';

export default defineConfig({
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
