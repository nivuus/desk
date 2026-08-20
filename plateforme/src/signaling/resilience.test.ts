// Ronde de correction 1 : un message `null` envoyé par un pair faisait planter
// tout le process Node (TypeError non interceptée sur `null.role`), tuant du
// même coup toutes les sessions actives — un déni de service en une trame,
// sans authentification requise.
//
// ⚠️ « sans authentification requise » décrit l'état d'ALORS, et il reste vrai
// aujourd'hui pour une raison qu'il faut écrire, sinon on croira la phrase
// périmée depuis la garde du sous-bloc P2 : le contrôle de forme court sur le
// PREMIER message, donc AVANT que la garde ait vu un jeton (voir `relais.ts`,
// `isJsonObject`). **Le déni de service en une trame est donc toujours ouvert
// à quiconque atteint le port**, et c'est bien pourquoi ce fichier existe
// encore.
//
// ❌ CETTE PHRASE PORTAIT UNE SECONDE RAISON, « et le rôle `agent` reste de
// toute façon anonyme jusqu'à P3 », ET LE SOUS-BLOC P3 L'A RENDUE FAUSSE (19
// août 2026, revue transverse de fin de branche). Le rôle `agent` exige
// désormais un jeton de type `agent` dont le sujet préfixe la session
// (`identite/garde.ts`), et un pair qui ne présente rien ne reçoit AUCUN
// `ice-config` (`journaux-plateforme-p3/e2-ferme-{1,2}.log`, deux exécutions).
//
// ⚠️ CE N'EST PAS LA PHRASE PRINCIPALE QUI TOMBE, C'EST L'UNE DE SES DEUX
// RAISONS — et la distinction est le sort que le plan de P3 prescrivait
// d'avance pour cette ligne (E15) : **PRÉCISER, pas corriger**. La première
// raison suffit à elle seule, et c'est bien elle qui porte : le contrôle de
// forme est en amont de toute garde, donc AUCUNE authentification, pas même
// celle de P3, ne peut fermer ce chemin-ci.
//
// ⚠️ **CETTE LIGNE DISAIT « LE FREIN EST P5 ③ », ET C'ÉTAIT LE MAUVAIS REMÈDE**
// (revue transverse, 20 août 2026). Le frein de P5 est posé sur
// `/auth/connexion`, `/auth/rafraichir` et `/agent` ; **il ne couvre PAS le
// relais** — `signaling/relais.ts` n'importe pas `Frein`, et
// `createSignalingServer` n'en reçoit aucun. Ce qui ferme réellement le déni
// de service en une trame est `TRAME_MAX_OCTETS` (`http/serveur.ts`), posé en
// `maxPayload` sur les DEUX serveurs WebSocket, donc appliqué par `ws` AVANT
// que la trame n'atteigne le moindre contrôle de forme — et il est éprouvé
// par le `describe` de ce fichier même, plus bas.
//
// ⚠️ **CE QUI RESTE OUVERT, ET QUE `maxPayload` NE FERME PAS** : un pair peut
// toujours ouvrir BEAUCOUP DE CONNEXIONS, et des connexions muettes ne sont
// comptées par rien — ni par le frein, qui compte des tentatives, ni par
// `deploiement/nginx.conf`, qui ne pose ni `limit_conn` ni `limit_req`.
// `http/serveur.ts` le dit déjà auprès de la constante.
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
import { TRAME_MAX_OCTETS } from '../http/serveur';
import { signer } from '../identite/jeton';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const signalingRoot = path.join(__dirname, '..', '..');
const tsxBin = path.join(signalingRoot, 'node_modules', '.bin', 'tsx');

/// Le secret de signature du processus enfant, nommé UNE fois : il est posé
/// dans son `env` ci-dessous et sert à signer les jetons que `connectTo`
/// envoie. Deux valeurs divergentes feraient refuser toutes les poignées de
/// main `client`, avec un diagnostic obscur.
const SECRET_ENFANT = 'un-secret-de-plateforme-de-quarante-octets';

/// Le préfixe de la VM simulée. La garde exige que le sujet du jeton d'agent
/// préfixe la session demandée (sous-bloc P3) : toutes les sessions de ce
/// fichier le portent donc.
const P = 'RhH1x2QmTz9kLpVbNc7dAw';

let child: ChildProcessWithoutNullStreams;
let port: number;

// Démarre `index.ts` comme un vrai process Node et attend qu'il annonce son
// port d'écoute (SIGNALING_PORT=0 : le système en attribue un libre).
function startRealServer(): Promise<{ child: ChildProcessWithoutNullStreams; port: number }> {
    return new Promise((resolve, reject) => {
        const proc = spawn(tsxBin, [path.join(signalingRoot, 'src', 'index.ts')], {
            cwd: signalingRoot,
            // `PLATEFORME_BASE` est FIXÉ, et n'hérite pas de l'environnement :
            // ce fichier éprouve la résilience du RELAIS, jamais le choix du
            // moteur. Sans ce garde, `npm run test:postgres` transmettrait
            // `PLATEFORME_BASE=postgres` à l'enfant sans lui transmettre
            // l'URL (que le harnais tient en dur), l'enfant retomberait sur
            // `:memory:` que `pg` prend pour un hôte, et mourrait sur
            // ECONNREFUSED — le service ayant RAISON de refuser de démarrer.
            env: {
                ...process.env,
                PLATEFORME_HOTE: '127.0.0.1',
                PLATEFORME_PORT: '0',
                PLATEFORME_BASE: 'sqlite',
                PLATEFORME_BASE_URL: ':memory:',
                // `lireConfig` refuse désormais de démarrer sans secret de
                // signature, et n'en invente aucun : sans cette ligne
                // l'enfant meurt avant d'annoncer son port.
                PLATEFORME_SECRET_JETON: SECRET_ENFANT,
            },
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
            // 🔴 LES DEUX RÔLES exigent un jeton, signé avec le MÊME secret
            // que celui posé dans l'`env` de l'enfant ci-dessus : le rôle
            // `client` depuis P2, le rôle `agent` depuis P3, qui a fermé la
            // fenêtre anonyme de E2. Le jeton d'agent est de TYPE `agent`, et
            // son sujet est le PRÉFIXE que sa session doit porter.
            const jeton = role === 'client'
                ? signer('u-resilience', SECRET_ENFANT, Date.now())
                : signer(P, SECRET_ENFANT, Date.now(), undefined, 'agent');
            ws.send(JSON.stringify({ role, session, jeton }));
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
        const agent = await connectTo(port, 'agent', `${P}:preuve-null-premier`);
        const client = await connectTo(port, 'client', `${P}:preuve-null-premier`);
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
        const agent = await connectTo(port, 'agent', `${P}:preuve-null-suivant`);
        const client = await connectTo(port, 'client', `${P}:preuve-null-suivant`);

        // Session témoin ouverte avant l'incident, pour prouver qu'elle n'est
        // pas affectée par ce qui va arriver à la session précédente.
        const agentTemoin = await connectTo(port, 'agent', `${P}:temoin-null-suivant`);
        const clientTemoin = await connectTo(port, 'client', `${P}:temoin-null-suivant`);

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

describe('résilience du process réel face à une trame TROP GRANDE (P5)', () => {
    /// 🔴 CE TEST VIT ICI, ET NON DANS `http/serveur.test.ts`, POUR LA RAISON
    /// EXACTE QUE L'EN-TÊTE DE CE FICHIER DONNE : « vitest installe son propre
    /// gestionnaire d'exceptions non interceptées », si bien qu'un test
    /// exécuté DANS vitest ne peut pas prouver qu'un process Node réel
    /// survivrait. `serveur.test.ts` éprouve que la trame est REFUSÉE ; seul
    /// ce fichier-ci peut éprouver que le service y SURVIT.
    ///
    /// 🔴 ET LE DANGER EST NEUF, INTRODUIT PAR LE CORRECTIF LUI-MÊME. Poser
    /// `maxPayload` fait émettre `error` par `ws` sur le socket SERVEUR ; or
    /// aucun socket serveur de ce service n'avait d'écouteur `error` — relevé
    /// le 20 août 2026, `grep -n "on('error'" relais.ts canal.ts serveur.ts`
    /// ne rendait que le `http.once('error', reject)` du démarrage. Un
    /// `EventEmitter` qui émet `error` sans écouteur LÈVE, et une exception
    /// non attrapée dans un gestionnaire d'évènement Node abat tout le
    /// process. Sans l'écouteur, LE CORRECTIF ANTI-DÉNI-DE-SERVICE AURAIT
    /// DONNÉ UN DÉNI DE SERVICE PIRE : une trame anonyme unique tuant le
    /// service au lieu de le ralentir.
    it('🔴 survit à une trame au-delà de `maxPayload`, sur `/` comme sur `/agent`', async () => {
        for (const chemin of ['/', '/agent']) {
            const gros = new WebSocket(`ws://127.0.0.1:${port}${chemin}`);
            await new Promise((resolve, reject) => {
                gros.on('open', resolve);
                gros.on('error', reject);
            });
            const ferme = new Promise<number>((resolve) => gros.once('close', resolve));
            // Un écouteur `error` CÔTÉ CLIENT : c'est le pair fautif, et son
            // socket lève quand le serveur le coupe en cours d'écriture.
            gros.on('error', () => {});
            gros.send('x'.repeat(TRAME_MAX_OCTETS + 1));
            // 1009 = « message trop grand » (RFC 6455).
            expect(await ferme).toBe(1009);
        }

        // Preuve n°1 : le process n'est pas mort.
        expect(child.exitCode).toBeNull();
        expect(child.killed).toBe(false);

        // Preuve n°2 : une session ouverte APRÈS l'incident relaie
        // normalement. Sans elle, un process abattu se lirait exactement
        // comme un process sain — `exitCode` ne bascule pas instantanément.
        const agent = await connectTo(port, 'agent', `${P}:preuve-trame-geante`);
        const client = await connectTo(port, 'client', `${P}:preuve-trame-geante`);
        client.send(JSON.stringify({ type: 'offer', sdp: 'vivant apres la trame geante' }));
        expect(await nextMessage(agent)).toEqual({
            type: 'offer',
            sdp: 'vivant apres la trame geante',
        });
        agent.close();
        client.close();
    }, 20000);
});
