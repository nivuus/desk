import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it } from 'vitest';
import { WebSocket } from 'ws';
import type { Garde } from '../identite/garde';
import { Frein, REQUETES_MAX_ADRESSE } from '../securite/frein';
import { createSignalingServer } from './relais';
import { poserTurnAmbiant } from './turn-harnais';

/// Une garde qui accepte tout, LOCALE À CE FICHIER DE TEST et jamais exportée
/// par du code de production.
///
/// Ces douze tests éprouvent le RELAIS — appariement, relais d'offre, isolation
/// des sessions —, jamais l'authentification, qui a son propre fichier
/// (`garde-fil.test.ts`). Leur donner une vraie garde y ajouterait un jeton
/// signé sans rien mesurer de plus.
///
/// ⚠️ Rien de tel n'existe côté production : la seule fabrique de garde exige
/// un secret, et `PLATEFORME_SECRET_JETON` n'a AUCUN défaut (`config.ts`).
const GARDE_OUVERTE: Garde = {
    verifier: () => ({ ok: true }),
    revendiquer: () => {},
    liberer: () => {},
};

/// 🔴 CES DOUZE TESTS LISENT « LE MESSAGE SUIVANT », DONC ILS EXIGENT UN
/// RELAIS SANS TURN — et ils le POSENT, au lieu de l'espérer.
///
/// `relais.ts` envoie un `ice-config` à chaque pair dès qu'il se déclare, dès
/// lors que `TURN_URL` et `TURN_SECRET` sont posées. `nextMessage` rendrait
/// alors cet `ice-config` à la place de l'offre attendue. Ce fichier a échoué
/// exactement ainsi — six tests sur douze — quand `scripts/verify-all.sh`
/// était lancé depuis un shell ayant fait `source .env` : la mesure portait
/// sur l'environnement de celui qui lançait, pas sur le service.
///
/// ⚠️ Neutraliser N'EST PAS abandonner la configuration de production, où TURN
/// est bel et bien posé : le `describe` « avec un serveur TURN configuré », en
/// bas de ce fichier, la mesure explicitement. Sans lui, retirer TURN d'ici
/// retirerait ce cas de la couverture au lieu de le nommer.
let restaurerTurn: () => void;

beforeAll(() => {
    restaurerTurn = poserTurnAmbiant();
});

afterAll(() => {
    restaurerTurn();
});

let server: ReturnType<typeof createSignalingServer>;

function connect(role: 'agent' | 'client', session: string): Promise<WebSocket> {
    return new Promise((resolve, reject) => {
        const ws = new WebSocket(`ws://127.0.0.1:${server.port}`);
        ws.on('error', reject);
        ws.on('open', () => {
            ws.send(JSON.stringify({ role, session }));
            resolve(ws);
        });
    });
}

/// Le prochain message qui n'est pas un message de SERVICE du relais.
///
/// 🔴 `pair-present` EST FILTRÉ ICI, ET LE FILTRE N'EST PAS UNE COMMODITÉ.
/// Depuis le 30 août 2026 le relais prévient un pair `agent` déjà en place
/// qu'un `client` vient de le rejoindre (`signaling/pair-present.ts`) : un
/// test qui attend « le message suivant » sur le socket de l'agent recevrait
/// donc cette nouvelle-là et non la réponse qu'il a provoquée.
///
/// ⚠️ **IL RÉPARE AUSSI UNE FRAGILITÉ QUI PRÉEXISTAIT À CE LOT.** L'ancienne
/// forme employait `ws.once`, qui n'écoute qu'à partir de son attachement :
/// un message arrivé plus tôt était perdu, et l'assertion suivante passait ou
/// non selon l'ordonnancement. Ici, l'écouteur est posé pour la durée de
/// l'attente et retiré à la sortie, quelle que soit l'issue.
///
/// **Ce que ce filtre ne fait PAS** : établir que `pair-present` est bien
/// émis. C'est `pair-present.test.ts` qui le mesure — sans lui, ce filtre
/// serait indiscernable d'une mise sous le tapis.
function nextMessage(ws: WebSocket): Promise<any> {
    return new Promise((resolve, reject) => {
        const finir = (action: () => void) => {
            clearTimeout(timer);
            ws.off('message', surMessage);
            action();
        };
        const surMessage = (raw: any) => {
            const message = JSON.parse(raw.toString());
            if (message?.type === 'pair-present') return;
            finir(() => resolve(message));
        };
        const timer = setTimeout(() => finir(() => reject(new Error('aucun message reçu'))), 2000);
        ws.on('message', surMessage);
    });
}

// Ferme le socket et attend que le serveur ait traité l'évènement `close`
// (le handler `close` du serveur est synchrone mais s'exécute après un aller-
// retour réseau local ; le petit délai laisse cet aller-retour se terminer
// avant que le test n'interroge l'état côté serveur via une nouvelle connexion).
function closeAndWait(ws: WebSocket): Promise<void> {
    return new Promise((resolve) => {
        ws.once('close', () => setTimeout(resolve, 50));
        ws.close();
    });
}

// 🔴 UN FREIN NEUF PAR TEST, jamais partagé : sans quoi le nouveau test du
// budget « toute requête », plus bas, épuiserait le budget de TOUS les
// tests qui le suivent dans ce fichier, sur la même adresse `127.0.0.1`.
beforeEach(() => {
    server = createSignalingServer(0, GARDE_OUVERTE, new Frein(), new Set());
});

afterEach(async () => {
    await server.close();
});

describe('serveur de signaling', () => {
    it('relaie une offre du client vers l\'agent', async () => {
        const agent = await connect('agent', 's1');
        const client = await connect('client', 's1');

        client.send(JSON.stringify({ type: 'offer', sdp: 'v=0 offre' }));
        expect(await nextMessage(agent)).toEqual({ type: 'offer', sdp: 'v=0 offre' });

        agent.close();
        client.close();
    });

    it('relaie une réponse de l\'agent vers le client', async () => {
        const agent = await connect('agent', 's2');
        const client = await connect('client', 's2');

        agent.send(JSON.stringify({ type: 'answer', sdp: 'v=0 reponse' }));
        expect(await nextMessage(client)).toEqual({ type: 'answer', sdp: 'v=0 reponse' });

        agent.close();
        client.close();
    });

    it('isole les sessions entre elles', async () => {
        const agentA = await connect('agent', 'sa');
        const clientB = await connect('client', 'sb');

        clientB.send(JSON.stringify({ type: 'offer', sdp: 'pour sb' }));
        await expect(nextMessage(agentA)).rejects.toThrow(/aucun message/);

        agentA.close();
        clientB.close();
    });

    it('signale la disparition du pair', async () => {
        const agent = await connect('agent', 's3');
        const client = await connect('client', 's3');

        agent.close();
        expect(await nextMessage(client)).toEqual({ type: 'peer-gone' });

        client.close();
    });

    it('rejette un premier message invalide', async () => {
        const ws = new WebSocket(`ws://127.0.0.1:${server.port}`);
        await new Promise((resolve) => ws.on('open', resolve));
        ws.send(JSON.stringify({ bonjour: true }));
        expect(await nextMessage(ws)).toEqual({
            type: 'error',
            reason: 'premier message invalide : {role, session} attendu',
        });
        ws.close();
    });

    it('rejette un second agent sur la même session', async () => {
        const first = await connect('agent', 's4');
        const second = await connect('agent', 's4');
        // 🔴 `motif` EST TYPÉ, `reason` EST UNE PHRASE. `toEqual` est STRICT :
        // c'est lui qui garantit qu'aucun champ n'est parti en douce, et c'est
        // pourquoi cette assertion est étendue plutôt que doublée.
        expect(await nextMessage(second)).toEqual({
            type: 'error',
            reason: 'un agent est déjà connecté à la session s4',
            motif: 'role-occupe',
        });
        first.close();
        second.close();
    });

    it('le refus porte un motif que le client peut trancher SANS lire la phrase', async () => {
        // 🔴 C'EST LA RAISON D'ÊTRE DU CHAMP. Le hub élit un onglet porteur
        // par Web Locks ; son REPLI (navigateur sans cette API) doit
        // distinguer « la place est prise » — à avaler en silence — d'un refus
        // d'une autre cause, qu'il faut afficher. Trancher sur `reason`
        // obligerait le client à comparer une phrase FRANÇAISE, qui se
        // reformule : le piège de F1.
        const premier = await connect('client', 's-motif');
        const second = await connect('client', 's-motif');
        const message = await nextMessage(second);
        expect(message.motif).toBe('role-occupe');
        premier.close();
        second.close();
    });

    // Ronde de correction 1 : un premier message JSON valide mais non-objet
    // (`null`) faisait planter le handler (`null.role` lève une TypeError non
    // interceptée). Ces deux tests verrouillent le correctif : le pair fautif
    // reçoit une erreur et reste connecté, ET une session indépendante ouverte
    // en parallèle continue de fonctionner normalement après l'incident — ce
    // qui prouve que le serveur (au sens du process qui l'héberge dans ce même
    // test, voir la mise en garde ci-dessous) n'a pas été affecté globalement.
    //
    // Attention : vitest installe son propre gestionnaire d'exceptions non
    // interceptées, qui peut faire échouer le test sans pour autant faire
    // planter le process. Ces deux tests prouvent donc le comportement de la
    // fonction exportée `createSignalingServer`, mais pas celui du process
    // réel lancé via `index.ts` : c'est `resilience.test.ts` (processus enfant
    // séparé, hors du runtime vitest) qui apporte cette preuve-là.
    it('rejette un message racine `null` en premier message sans planter, et permet de retenter', async () => {
        const faulty = new WebSocket(`ws://127.0.0.1:${server.port}`);
        await new Promise((resolve) => faulty.on('open', resolve));

        const errorReceived = nextMessage(faulty);
        faulty.send('null');
        expect(await errorReceived).toEqual({
            type: 'error',
            reason: 'premier message invalide : {role, session} attendu',
        });

        // La connexion fautive reste utilisable : un second essai, valide cette
        // fois, s'enregistre normalement et relaie comme n'importe quelle session.
        faulty.send(JSON.stringify({ role: 'agent', session: 'retry' }));
        const client = await connect('client', 'retry');
        client.send(JSON.stringify({ type: 'offer', sdp: 'apres retry' }));
        expect(await nextMessage(faulty)).toEqual({ type: 'offer', sdp: 'apres retry' });

        // Une session totalement indépendante, déjà ouverte pendant l'incident,
        // continue elle aussi de relayer normalement.
        const agent = await connect('agent', 'temoin');
        const clientTemoin = await connect('client', 'temoin');
        clientTemoin.send(JSON.stringify({ type: 'offer', sdp: 'session temoin' }));
        expect(await nextMessage(agent)).toEqual({ type: 'offer', sdp: 'session temoin' });

        faulty.close();
        client.close();
        agent.close();
        clientTemoin.close();
    });

    it('rejette un message racine `null` en message suivant sans planter, et la session continue de fonctionner', async () => {
        const agent = await connect('agent', 's5');
        const client = await connect('client', 's5');

        // Session témoin ouverte en parallèle, avant l'envoi du message
        // malveillant, pour prouver qu'elle n'est pas affectée par l'incident.
        const agentTemoin = await connect('agent', 'temoin2');
        const clientTemoin = await connect('client', 'temoin2');

        const errorReceived = nextMessage(client);
        client.send('null');
        expect(await errorReceived).toEqual({
            type: 'error',
            reason: 'message invalide : objet JSON attendu',
        });

        // La session s5 reste fonctionnelle après l'incident.
        client.send(JSON.stringify({ type: 'offer', sdp: 'apres null' }));
        expect(await nextMessage(agent)).toEqual({ type: 'offer', sdp: 'apres null' });

        // La session témoin, indépendante, fonctionne toujours normalement.
        clientTemoin.send(JSON.stringify({ type: 'offer', sdp: 'temoin toujours vivant' }));
        expect(await nextMessage(agentTemoin)).toEqual({ type: 'offer', sdp: 'temoin toujours vivant' });

        agent.close();
        client.close();
        agentTemoin.close();
        clientTemoin.close();
    });

    // Sous-bloc D1 : la session de contrôle réutilise les rôles agent/client
    // existants, sur un session_id réservé, pour dialoguer entre le
    // superviseur et la page « bureau » du navigateur.
    it('relaie les messages de la session de contrôle entre superviseur et shell', async () => {
        const agent = await connect('agent', 'bureau');
        const client = await connect('client', 'bureau');

        agent.send(JSON.stringify({ type: 'fenetre-ouverte', session: 'w-1', titre: 'Bloc-notes' }));
        expect(await nextMessage(client)).toEqual({
            type: 'fenetre-ouverte',
            session: 'w-1',
            titre: 'Bloc-notes',
        });

        client.send(JSON.stringify({ type: 'viewport', session: 'w-1', largeur: 1600, hauteur: 900 }));
        expect(await nextMessage(agent)).toEqual({
            type: 'viewport',
            session: 'w-1',
            largeur: 1600,
            hauteur: 900,
        });

        agent.close();
        client.close();
    });

    // Le cas de D1 : la page ouvre sa connexion et envoie son offre AVANT que
    // le superviseur n'ait lancé son enfant (c'est le viewport de cette page
    // qui décide de la taille de la sortie virtuelle, donc rien ne peut être
    // lancé plus tôt). Sans mémorisation, l'offre tombait dans le vide et la
    // session ne s'établissait jamais.
    it("délivre à l'agent l'offre arrivée avant lui", async () => {
        const client = await connect('client', 'w-tardive');
        client.send(JSON.stringify({ type: 'offer', sdp: 'v=0 offre-du-client' }));

        // L'agent arrive après coup.
        const agent = await connect('agent', 'w-tardive');
        expect(await nextMessage(agent)).toEqual({ type: 'offer', sdp: 'v=0 offre-du-client' });

        agent.close();
        client.close();
    });

    it('ne délivre que la dernière offre, pas toutes celles reçues', async () => {
        const client = await connect('client', 'w-rejeu');
        client.send(JSON.stringify({ type: 'offer', sdp: 'v=0 premiere' }));
        client.send(JSON.stringify({ type: 'offer', sdp: 'v=0 seconde' }));

        const agent = await connect('agent', 'w-rejeu');
        expect(await nextMessage(agent)).toEqual({ type: 'offer', sdp: 'v=0 seconde' });

        agent.close();
        client.close();
    });

    // Sans cet oubli, un agent qui se reconnecterait sur un identifiant
    // réutilisé recevrait l'offre d'une session morte.
    it("oublie l'offre mémorisée quand la session se vide", async () => {
        const client = await connect('client', 'w-videe');
        client.send(JSON.stringify({ type: 'offer', sdp: 'v=0 perimee' }));
        await closeAndWait(client);

        const agent = await connect('agent', 'w-videe');
        await expect(nextMessage(agent)).rejects.toThrow(/aucun message/);

        agent.close();
    });
});

// 🔴 LE BUDGET « TOUTE REQUÊTE » DU RELAIS — legs des freins manquants
// (25 août 2026). Avant ce lot, RIEN ne bornait le nombre de connexions
// qu'une même adresse pouvait ouvrir sur `/signal` : `TRAME_MAX_OCTETS`
// (`http/serveur.ts`) borne la taille d'un message, pas le nombre de
// sockets. Ce `describe` est SÉPARÉ du premier : il a besoin de compter des
// CONNEXIONS brutes, sans jamais envoyer de `{role, session}` — le frein
// mord AVANT le premier message, voir `relais.ts`.
describe('le budget « toute requête » du relais', () => {
    it('🔴 refuse la connexion après trop de connexions de la même adresse', async () => {
        // 🔴 La rouge : ne jamais consulter `BUDGET_REQUETES` à la connexion.
        // Sans jeton ni message, chaque connexion resterait ouverte
        // indéfiniment — ce relais n'a aucune notion d'échec à ce stade.
        const sockets: WebSocket[] = [];
        try {
            let dernierMessage: { type: string; motif?: string; retryApresS?: number } | undefined;
            for (let i = 0; i <= REQUETES_MAX_ADRESSE; i++) {
                const ws = new WebSocket(`ws://127.0.0.1:${server.port}`);
                sockets.push(ws);
                if (i < REQUETES_MAX_ADRESSE) {
                    // Sous le budget : la connexion s'ouvre normalement, et ne
                    // reçoit RIEN tant qu'elle n'envoie pas de message.
                    await new Promise<void>((resolve, reject) => {
                        ws.once('open', () => resolve());
                        ws.once('error', reject);
                    });
                } else {
                    // La (N+1)ᵉ : refusée avant tout message, sur le seul
                    // évènement `connection`.
                    dernierMessage = await new Promise((resolve, reject) => {
                        const minuteur = setTimeout(
                            () => reject(new Error('aucun message reçu')),
                            2000,
                        );
                        ws.once('message', (raw) => {
                            clearTimeout(minuteur);
                            resolve(JSON.parse(raw.toString()));
                        });
                        ws.once('error', reject);
                    });
                }
            }
            expect(dernierMessage?.type).toBe('error');
            expect(dernierMessage?.motif).toBe('trop-de-requetes');
            // 🔴 round de correction 1, critique ② : sans `retryApresS`,
            // l'agent qui se fait refuser ici ne peut pas savoir combien de
            // temps attendre avant de retenter — c'est la moitié la moins
            // chère du remède au verrouillage documenté par
            // `agent/src/superviseur/boucle/surveillance_pont.rs`.
            expect(dernierMessage?.retryApresS).toBeGreaterThan(0);
        } finally {
            for (const s of sockets) s.close();
        }
    });
});

// La configuration de PRODUCTION : un serveur TURN est posé. C'est le cas que
// le `poserTurnAmbiant()` de ce fichier écarte partout ailleurs, et il serait
// malhonnête de l'écarter sans le mesurer nulle part — on l'écarterait alors
// de la couverture en croyant seulement stabiliser les tests.
//
// ⚠️ C'est aussi le test qui aurait ATTRAPÉ le défaut : il énonce que
// l'`ice-config` arrive, et que le relais continue de relayer APRÈS lui. Les
// douze tests ci-dessus l'énonçaient à l'envers, sans le dire, en supposant
// que le premier message reçu était toujours celui qu'ils attendaient.
describe('avec un serveur TURN configuré', () => {
    let restaurer: () => void;

    beforeAll(() => {
        restaurer = poserTurnAmbiant({
            url: 'turn:127.0.0.1:3478',
            secret: 'un-secret-turn-de-test',
        });
    });

    afterAll(() => {
        restaurer();
    });

    it("délivre l'ice-config à chaque pair, PUIS relaie normalement", async () => {
        const agent = await connect('agent', 'turn-1');
        // Premier message de l'agent : sa configuration ICE, avant tout relais.
        const iceAgent = await nextMessage(agent);
        expect(iceAgent.type).toBe('ice-config');
        expect(iceAgent.iceServers[0].urls).toBe('turn:127.0.0.1:3478');

        const client = await connect('client', 'turn-1');
        // Le client reçoit la sienne : les deux extrémités en ont besoin.
        expect((await nextMessage(client)).type).toBe('ice-config');

        // Et le relais relaie toujours, une fois l'ice-config passé.
        client.send(JSON.stringify({ type: 'offer', sdp: 'v=0 apres ice' }));
        expect(await nextMessage(agent)).toEqual({ type: 'offer', sdp: 'v=0 apres ice' });

        agent.close();
        client.close();
    });
});
