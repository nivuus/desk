// Ronde de correction 1 : un message `null` envoyé par un pair faisait planter
// tout le process Node (TypeError non interceptée sur `null.role`), tuant du
// même coup toutes les sessions actives — un déni de service en une trame,
// sans authentification requise.
//
// Ce fichier ne teste PAS `createSignalingServer` en mémoire : vitest installe
// son propre gestionnaire d'exceptions non interceptées, qui peut faire échouer
// un test sans que le process qui l'exécute ne s'arrête réellement. Un test
// exécuté « dans » vitest ne peut donc pas prouver qu'un process Node réel,
// démarré via `index.ts` (qui n'installe aucun `process.on('uncaughtException')`),
// survivrait à la même attaque.
//
// On lance ici le véritable point d'entrée (`src/index.ts`) comme process enfant
// indépendant, on lui envoie le message malveillant par un vrai socket WebSocket,
// puis on vérifie deux choses distinctes :
//   1. le pair fautif reçoit une erreur JSON propre (pas de coupure de connexion) ;
//   2. le process est toujours vivant ensuite, et une session tierce ouverte en
//      parallèle continue de relayer normalement — la preuve qu'il n'y a pas eu
//      de déni de service.

import { type ChildProcessWithoutNullStreams, spawn } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { afterAll, beforeAll, describe, expect, it } from 'vitest';
import { WebSocket } from 'ws';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const signalingRoot = path.join(__dirname, '..', '..');
const tsxBin = path.join(signalingRoot, 'node_modules', '.bin', 'tsx');

let child: ChildProcessWithoutNullStreams;
let port: number;

// Démarre `index.ts` comme un vrai process Node et attend qu'il annonce son
// port d'écoute (SIGNALING_PORT=0 : le système en attribue un libre).
function startRealServer(): Promise<{ child: ChildProcessWithoutNullStreams; port: number }> {
    return new Promise((resolve, reject) => {
        const proc = spawn(tsxBin, [path.join(signalingRoot, 'src', 'index.ts')], {
            cwd: signalingRoot,
            env: { ...process.env, PLATEFORME_HOTE: '127.0.0.1', PLATEFORME_PORT: '0' },
        });

        let output = '';
        const onStdout = (chunk: Buffer) => {
            output += chunk.toString();
            const match = output.match(/le port (\d+)/);
            if (match) {
                proc.stdout.off('data', onStdout);
                clearTimeout(timer);
                resolve({ child: proc, port: Number(match[1]) });
            }
        };
        proc.stdout.on('data', onStdout);

        let stderr = '';
        proc.stderr.on('data', (chunk: Buffer) => {
            stderr += chunk.toString();
        });

        const timer = setTimeout(() => {
            reject(new Error(`démarrage du process signaling expiré. stderr: ${stderr}`));
        }, 10000);

        proc.once('error', reject);
        proc.once('exit', (code) => {
            clearTimeout(timer);
            reject(new Error(`process signaling terminé prématurément (code ${code}). stderr: ${stderr}`));
        });
    });
}

function connectTo(targetPort: number, role: 'agent' | 'client', session: string): Promise<WebSocket> {
    return new Promise((resolve, reject) => {
        const ws = new WebSocket(`ws://127.0.0.1:${targetPort}`);
        ws.on('error', reject);
        ws.on('open', () => {
            ws.send(JSON.stringify({ role, session }));
            resolve(ws);
        });
    });
}

function nextMessage(ws: WebSocket): Promise<any> {
    return new Promise((resolve, reject) => {
        const timer = setTimeout(() => reject(new Error('aucun message reçu')), 2000);
        ws.once('message', (raw) => {
            clearTimeout(timer);
            resolve(JSON.parse(raw.toString()));
        });
    });
}

beforeAll(async () => {
    ({ child, port } = await startRealServer());
}, 15000);

afterAll(() => {
    child.kill();
});

describe('résilience du process réel (index.ts) face à un message `null`', () => {
    it('survit à un `null` en premier message : le fautif reçoit une erreur, une session tierce continue de fonctionner', async () => {
        const faulty = new WebSocket(`ws://127.0.0.1:${port}`);
        await new Promise((resolve, reject) => {
            faulty.on('open', resolve);
            faulty.on('error', reject);
        });

        const errorReceived = nextMessage(faulty);
        faulty.send('null');
        expect(await errorReceived).toEqual({
            type: 'error',
            reason: 'premier message invalide : {role, session} attendu',
        });

        // Preuve n°1 : le process n'est pas mort.
        expect(child.exitCode).toBeNull();
        expect(child.killed).toBe(false);

        // Preuve n°2 : une session indépendante, ouverte après l'incident,
        // relaie normalement — le serveur répond toujours au réseau.
        const agent = await connectTo(port, 'agent', 'preuve-null-premier');
        const client = await connectTo(port, 'client', 'preuve-null-premier');
        client.send(JSON.stringify({ type: 'offer', sdp: 'toujours vivant (premier message)' }));
        expect(await nextMessage(agent)).toEqual({
            type: 'offer',
            sdp: 'toujours vivant (premier message)',
        });

        faulty.close();
        agent.close();
        client.close();
    });

    it('survit à un `null` en message suivant : le fautif reçoit une erreur, une session tierce continue de fonctionner', async () => {
        const agent = await connectTo(port, 'agent', 'preuve-null-suivant');
        const client = await connectTo(port, 'client', 'preuve-null-suivant');

        // Session témoin ouverte avant l'incident, pour prouver qu'elle n'est
        // pas affectée par ce qui va arriver à la session précédente.
        const agentTemoin = await connectTo(port, 'agent', 'temoin-null-suivant');
        const clientTemoin = await connectTo(port, 'client', 'temoin-null-suivant');

        const errorReceived = nextMessage(client);
        client.send('null');
        expect(await errorReceived).toEqual({
            type: 'error',
            reason: 'message invalide : objet JSON attendu',
        });

        // Preuve n°1 : le process n'est pas mort.
        expect(child.exitCode).toBeNull();
        expect(child.killed).toBe(false);

        // Preuve n°2 : la session témoin, ouverte avant l'incident, fonctionne
        // toujours normalement après.
        clientTemoin.send(JSON.stringify({ type: 'offer', sdp: 'temoin toujours vivant' }));
        expect(await nextMessage(agentTemoin)).toEqual({
            type: 'offer',
            sdp: 'temoin toujours vivant',
        });

        agent.close();
        client.close();
        agentTemoin.close();
        clientTemoin.close();
    });
});
