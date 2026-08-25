// Les critères ①, ② et ③ AU NIVEAU DU SOCKET, sur de vrais `WebSocket`.
//
// 🔴 `TURN_URL` et `TURN_SECRET` sont posées ici, et c'est indispensable :
// sans elles `configurationIce` rend `undefined` et le relais n'envoie
// JAMAIS d'`ice-config` (`ice.ts`). L'assertion « aucun `ice-config` avant la
// fermeture » serait alors VRAIE quoi qu'il arrive — un contrôle incapable
// d'échouer, le patron que ce dépôt a payé quatre fois. Le TÉMOIN du premier
// test le prouve dans la même exécution : un client AUTHENTIFIÉ, lui, en
// reçoit un.
//
// L'horloge du service de test est INJECTÉE : le critère ② exige qu'elle
// avance entre deux poignées de main, et `demarrerServeur` ne prend pas
// d'horloge. Ce fichier construit donc sa garde lui-même et appelle
// `createSignalingServer(port, garde)` — choix d'implémentation assumé, la
// forme `port` étant celle qu'éprouve `server.test.ts` depuis le jalon 1.

import { afterAll, afterEach, beforeAll, describe, expect, it, vi } from 'vitest';
import { WebSocket } from 'ws';
import { garde as fabriquerGarde, type Garde } from '../identite/garde';
import { signer, DUREE_JETON_ACCES_MS } from '../identite/jeton';
import { Frein } from '../securite/frein';
import { ProprieteDeSession } from './propriete';
import { createSignalingServer } from './relais';
import { poserTurnAmbiant } from './turn-harnais';

const SECRET = 'un-secret-de-plateforme-de-quarante-octets';
const T0 = 1_787_000_000_000;
/// Le préfixe d'une VM simulée, de la VRAIE longueur que
/// `agents/prefixe.ts` produit (22 caractères base64url). La garde exige
/// que le sujet d'un jeton d'agent PRÉFIXE la session demandée.
const P = 'RhH1x2QmTz9kLpVbNc7dAw';

let serveur: ReturnType<typeof createSignalingServer> | undefined;
let maintenant = T0;
let proprietes: ProprieteDeSession;
let garde: Garde;
/// ⚠️ LA RESTAURATION PASSE PAR LE HARNAIS, ET CE N'EST PAS DU CONFORT.
/// Écrite à la main, elle réaffectait `process.env.TURN_URL = turnAvant.url`,
/// où `turnAvant.url` vaut `undefined` sur une machine sans TURN — et
/// `process.env` coerce en chaîne : la variable ressortait à `"undefined"`,
/// TRUTHY, donc `configurationIce` délivrait ensuite une configuration ICE
/// dont l'URL était le mot `undefined`. Mesuré, jamais observé mordant :
/// l'ordre des fichiers place aujourd'hui `server.test.ts` AVANT celui-ci, et
/// il aurait suffi que l'un des deux change de taille pour l'inverser.
let restaurerTurn: () => void;

beforeAll(() => {
    restaurerTurn = poserTurnAmbiant({
        url: 'turn:127.0.0.1:3478',
        secret: 'un-secret-turn-de-test',
    });
});

afterAll(() => {
    restaurerTurn();
});

function demarrer(): number {
    maintenant = T0;
    proprietes = new ProprieteDeSession();
    garde = fabriquerGarde(SECRET, () => maintenant, proprietes);
    // Un frein NEUF par appel, comme `garde` et `proprietes` juste au-dessus :
    // ce fichier fait plusieurs poignées de main par test, très en dessous du
    // budget « toute requête » (`securite/frein.ts::BUDGET_REQUETES`), mais
    // un frein partagé entre tests ferait dériver un compte d'un test à
    // l'autre.
    serveur = createSignalingServer(0, garde, new Frein(), new Set());
    return serveur.port;
}

afterEach(async () => {
    await serveur?.close();
    serveur = undefined;
    vi.restoreAllMocks();
});

interface Suivi {
    messages: any[];
    /// L'ordre RÉEL des évènements : `message` et `close` s'y suivent tels
    /// qu'ils sont arrivés. C'est ce qui permet d'asserter que le refus est
    /// parvenu AVANT la fermeture, et non l'inverse.
    ordre: string[];
    ferme: Promise<void>;
    socket: WebSocket;
}

function poignee(port: number, corps: unknown): Promise<Suivi> {
    return new Promise((resolve, reject) => {
        const w = new WebSocket(`ws://127.0.0.1:${port}`);
        const suivi: Suivi = {
            messages: [],
            ordre: [],
            socket: w,
            ferme: new Promise((r) => w.once('close', () => r())),
        };
        const minuteur = setTimeout(() => reject(new Error('aucune issue en 3000 ms')), 3000);
        w.on('message', (brut) => {
            suivi.messages.push(JSON.parse(brut.toString()));
            suivi.ordre.push('message');
        });
        w.on('close', () => suivi.ordre.push('close'));
        w.once('open', () => {
            clearTimeout(minuteur);
            w.send(JSON.stringify(corps));
            // Laisse au serveur le temps de répondre et, le cas échéant, de
            // fermer. Une borne, jamais une attente infinie.
            setTimeout(() => resolve(suivi), 250);
        });
        w.once('error', () => {
            clearTimeout(minuteur);
            resolve(suivi);
        });
    });
}

describe('la garde, au niveau du socket', () => {
    it('CRITÈRE ① : un client sans jeton est refusé, et ne voit AUCUN ice-config', async () => {
        const port = demarrer();
        const refuse = await poignee(port, { role: 'client', session: 's-1' });

        // Première assertion : le refus est typé.
        expect(refuse.messages).toContainEqual(
            expect.objectContaining({ type: 'error', motif: 'jeton-absent' }),
        );
        // 🔴 Seconde assertion, EXIGÉE par la spec au même titre que la
        // première : un service qui refuserait APRÈS avoir envoyé
        // `ice-config` passerait la première et laisserait fuir des
        // identifiants TURN de 24 h.
        expect(refuse.messages.map((m) => m.type)).not.toContain('ice-config');

        // TÉMOIN, dans la même exécution : un client AUTHENTIFIÉ en reçoit un.
        // Sans lui, l'assertion ci-dessus serait vraie quoi qu'il arrive.
        const admis = await poignee(port, {
            role: 'client',
            session: 's-temoin',
            jeton: signer('u1', SECRET, T0),
        });
        expect(admis.messages.map((m) => m.type)).toContain('ice-config');
        admis.socket.terminate();
    });

    it('le socket est FERMÉ après le refus, et le message est arrivé AVANT', async () => {
        const port = demarrer();
        const refuse = await poignee(port, { role: 'client', session: 's-1' });
        await refuse.ferme;
        // Fermer avant d'envoyer tronquerait le message : le pair verrait une
        // fermeture sans motif.
        expect(refuse.ordre).toEqual(['message', 'close']);
        expect(refuse.socket.readyState).toBe(WebSocket.CLOSED);
    });

    it('CRITÈRE ② : un jeton dont la durée est écoulée est refusé jeton-expire', async () => {
        const port = demarrer();
        const jeton = signer('u1', SECRET, T0);
        // Le même jeton passe à t0…
        const admis = await poignee(port, { role: 'client', session: 's-2', jeton });
        expect(admis.messages.map((m) => m.type)).not.toContain('error');
        admis.socket.terminate();

        // 🔴 …et l'horloge du service AVANCE. La figer rendrait ce test inerte.
        maintenant = T0 + DUREE_JETON_ACCES_MS;
        const expire = await poignee(port, { role: 'client', session: 's-3', jeton });
        expect(expire.messages).toContainEqual(
            expect.objectContaining({ type: 'error', motif: 'jeton-expire' }),
        );
    });

    it('CRITÈRE ③ : u2 se voit refuser la session de u1, et le JOURNAL la nomme', async () => {
        const port = demarrer();
        const journal = vi.spyOn(console, 'warn').mockImplementation(() => {});

        const un = await poignee(port, {
            role: 'client',
            session: 's-privee',
            jeton: signer('u1', SECRET, T0),
        });
        expect(un.messages.map((m) => m.type)).not.toContain('error');

        const deux = await poignee(port, {
            role: 'client',
            session: 's-privee',
            jeton: signer('u2', SECRET, T0),
        });
        const erreur = deux.messages.find((m) => m.type === 'error');
        expect(erreur).toBeDefined();
        expect(erreur.motif).toBe('session-refusee');
        // Le message SUR LE FIL ne nomme ni la session ni son propriétaire.
        expect(erreur.reason).not.toContain('s-privee');
        expect(erreur.reason).not.toContain('u1');

        // Le JOURNAL, lui, porte le nom de session ET le demandeur.
        const lignes = journal.mock.calls.map((c) => String(c[0])).join('\n');
        expect(lignes).toContain('s-privee');
        expect(lignes).toContain('u2');

        un.socket.terminate();
    });

    it('après le départ des deux pairs, u2 PEUT prendre la session', async () => {
        const port = demarrer();
        const un = await poignee(port, {
            role: 'client',
            session: 's-rendue',
            jeton: signer('u1', SECRET, T0),
        });
        expect(proprietes.proprietaire('s-rendue')).toBe('u1');

        un.socket.close();
        await un.ferme;
        await new Promise((r) => setTimeout(r, 100));
        // Ne jamais libérer perdrait le nom de session à vie.
        expect(proprietes.proprietaire('s-rendue')).toBeUndefined();

        const deux = await poignee(port, {
            role: 'client',
            session: 's-rendue',
            jeton: signer('u2', SECRET, T0),
        });
        expect(deux.messages.map((m) => m.type)).not.toContain('error');
        expect(proprietes.proprietaire('s-rendue')).toBe('u2');
        deux.socket.terminate();
    });

    it('🔴 un pair `agent` SANS jeton est REFUSÉ — la fenêtre de E2 est FERMÉE', async () => {
        // 🔴 CE TEST EST L'INVERSE EXACT DE CELUI QUE P2 LIVRAIT, et P2 l'avait
        // prévu : « Le jour où P3 l'inversera, il faudra le réécrire À DESSEIN,
        // pas par surprise. » C'est fait, à dessein, et la rouge est GRATUITE —
        // le binaire de P2 la porte.
        const port = demarrer();
        const agent = await poignee(port, { role: 'agent', session: 'bureau' });
        expect(agent.messages.map((m) => m.type)).toContain('error');
        agent.socket.terminate();
    });

    it('🔴 …et il ne reçoit AUCUN `ice-config` — LA FUITE d’E12 est fermée', async () => {
        // 🔴 CE TEST EST LA SUBSTANCE D'E12, et le refus ci-dessus n'en était
        // que la moitié. Un service qui refuserait APRÈS avoir envoyé
        // `ice-config` passerait le test précédent mot pour mot, et laisserait
        // pourtant fuir vers un ANONYME des identifiants TURN valables 86 400 s
        // (`ice.ts`) — c'est-à-dire exactement ce que P2 avait nommé et laissé
        // ouvert.
        //
        // 🔴 IL VIT DANS UN TEST DISTINCT, jamais comme seconde assertion du
        // précédent : `expect` interrompt à la première, et cette fuite-ci —
        // la seule que P3 ferme réellement — ne serait alors éprouvée par
        // rien. C'est la leçon ①A-bis de P2, appliquée d'avance.
        const port = demarrer();
        const agent = await poignee(port, { role: 'agent', session: 'bureau' });
        expect(agent.messages.map((m) => m.type)).not.toContain('ice-config');
        agent.socket.terminate();

        // TÉMOIN, DANS LA MÊME EXÉCUTION ET SUR LE MÊME RÔLE : un agent
        // AUTHENTIFIÉ, lui, en reçoit un. Sans lui, l'assertion ci-dessus
        // serait vraie quoi qu'il arrive le jour où `ice-config` cesserait
        // d'être envoyé aux agents — et `TURN_URL`/`TURN_SECRET`, posées en
        // `beforeAll`, ne prouvent que la moitié de cette non-vacuité.
        //
        // Le jeton est de TYPE `agent` et son sujet est le PRÉFIXE de la VM :
        // la garde exige les deux (`identite/garde.ts`).
        const admis = await poignee(port, {
            role: 'agent',
            session: `${P}:bureau`,
            jeton: signer(P, SECRET, T0, undefined, 'agent'),
        });
        expect(admis.messages.map((m) => m.type)).toContain('ice-config');
        admis.socket.terminate();
    });
});
