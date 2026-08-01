import { defineConfig } from 'vite';

export default defineConfig({
    server: {
        host: '0.0.0.0',
        port: 5173,
    },
    build: {
        rollupOptions: {
            input: {
                main: 'index.html',
                shell: 'shell.html',
            },
        },
    },
});
