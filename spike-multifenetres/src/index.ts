import { createSpikeServer } from './server.js';

const port = Number(process.env.SPIKE_PORT ?? 3445);
const serveur = await createSpikeServer(port);
console.log(`spike à l'écoute sur le port ${serveur.port}`);
console.log(`déclenchement externe : curl -X POST http://127.0.0.1:${serveur.port}/fire \\`);
console.log(`  -H 'content-type: application/json' -d '{"variant":2}'`);
