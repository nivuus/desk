import { createSignalingServer } from './server';

const port = Number(process.env.SIGNALING_PORT ?? 8080);
const server = createSignalingServer(port);
console.log(`signaling à l'écoute sur le port ${server.port}`);

process.on('SIGINT', async () => {
    await server.close();
    process.exit(0);
});
