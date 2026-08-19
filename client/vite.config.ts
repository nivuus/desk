import { defineConfig } from 'vite';

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
});
