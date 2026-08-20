// Le registre des sockets d'agent vivants, éprouvé SANS SERVEUR.
//
// 🔴 LE SOCKET EST UN DOUBLE, et c'est ce que la frontière structurelle de
// `SocketAgent` achète : ces sept cas se jouent sans ouvrir de port, sans
// poignée de main, et sans attendre le réseau. Ce que le registre fait
// réellement d'un vrai `ws.WebSocket` est éprouvé ailleurs — par
// `canal-apps.test.ts`, qui monte le canal pour de bon.
//
// 🔴 L'EXPIRATION SE JOUE SUR DES MINUTEURS FEINTS QUE LE TEST FAIT AVANCER.
// Une horloge figée rendrait le cas inerte : il lirait un état final au lieu
// de voir la TRANSITION, et c'est la leçon du critère ④ de P3.

import { afterEach, describe, expect, it, vi } from 'vitest';
import { encodeLancer, type IssueLancement } from '../../../proto/ts/plateforme';
import { DELAI_LANCEMENT_MS, RegistreAgents, type SocketAgent } from './registre';

/// Un double de socket : il retient ce qu'on lui écrit, et sait se fermer.
function socketFeint(readyState = 1): SocketAgent & { ecrits: string[]; fermetures: number[] } {
    const ecrits: string[] = [];
    const fermetures: number[] = [];
    return {
        ecrits,
        fermetures,
        get readyState() {
            return readyState;
        },
        send(donnees: string) {
            ecrits.push(donnees);
        },
        close(code?: number) {
            fermetures.push(code ?? 0);
        },
    };
}

afterEach(() => {
    vi.useRealTimers();
    vi.restoreAllMocks();
});

describe('RegistreAgents', () => {
    it('rend `agent-injoignable` IMMÉDIATEMENT pour une VM absente', async () => {
        // 🔴 Attendre `DELAI_LANCEMENT_MS` pour une VM dont on sait DÉJÀ
        // qu'elle n'a pas de socket ferait payer cinq secondes à un appelant
        // pour une réponse connue d'avance. Les minuteurs sont feints et ne
        // sont PAS avancés : si la promesse attendait quoi que ce soit, elle
        // ne se résoudrait jamais et le cas expirerait.
        vi.useFakeTimers();
        const registre = new RegistreAgents();
        await expect(registre.lancer('v-absente', 'cle-1', 'd-1')).resolves.toBe('agent-injoignable');
    });

    it("envoie un `Lancer` ENCODÉ PAR LE PROTOCOLE, jamais une forme recopiée", async () => {
        // 🔴 Recopier la forme du message ici la ferait diverger EN SILENCE de
        // `proto/ts/plateforme.ts`, et `plateforme-vectors.json` ne
        // comparerait plus rien de ce que la plateforme met sur le fil. C'est
        // la règle que `agents/canal.ts` s'impose depuis P3 ; l'assertion
        // compare à l'encodeur lui-même, caractère pour caractère.
        vi.useFakeTimers();
        const registre = new RegistreAgents();
        const socket = socketFeint();
        registre.inscrire('v-1', socket);

        void registre.lancer('v-1', 'cle-1', 'd-1');
        expect(socket.ecrits).toEqual([encodeLancer('d-1', 'cle-1')]);
    });

    it('apparie la réponse sur la DEMANDE, jamais sur la clé', async () => {
        // 🔴 Apparier sur la clé mélangerait deux lancements concurrents de la
        // MÊME application — cas parfaitement ordinaire, deux onglets du hub
        // suffisent. La demande est le seul identifiant unique d'un ordre.
        vi.useFakeTimers();
        const registre = new RegistreAgents();
        registre.inscrire('v-1', socketFeint());

        const premier = registre.lancer('v-1', 'meme-cle', 'd-1');
        const second = registre.lancer('v-1', 'meme-cle', 'd-2');
        registre.resoudre('d-2', 'cible');
        registre.resoudre('d-1', 'raccourci');

        expect(await premier).toBe('raccourci' satisfies IssueLancement);
        expect(await second).toBe('cible' satisfies IssueLancement);
    });

    it("IGNORE une demande inconnue, avec sa trace, et ne LÈVE jamais", async () => {
        // 🔴 Lever ici serait grave : l'appelant de `resoudre` est le
        // gestionnaire `message` d'un socket, et une exception qui le
        // traverse — ou une promesse rejetée qui en part — abat TOUT LE
        // PROCESS Node. Mode de défaillance que `relais.ts`, `trace.ts` et
        // `canal.ts` documentent tous les trois.
        //
        // Une demande inconnue n'a rien d'anormal : un agent peut répondre à
        // un ordre dont l'attente a déjà expiré.
        const traces: string[] = [];
        vi.spyOn(console, 'warn').mockImplementation((l: string) => void traces.push(l));
        const registre = new RegistreAgents();

        expect(() => registre.resoudre('jamais-emise', 'echec')).not.toThrow();
        expect(traces.join(' | ')).toContain('jamais-emise');
    });

    it('`retirer` REJETTE les demandes en vol de cette VM', async () => {
        // 🔴 Ne rien faire ferait attendre `DELAI_LANCEMENT_MS` à la route
        // après une mort d'agent CONNUE À L'INSTANT MÊME : le socket vient de
        // se fermer, et on ferait patienter cinq secondes pour une réponse qui
        // n'arrivera jamais.
        vi.useFakeTimers();
        const registre = new RegistreAgents();
        registre.inscrire('v-1', socketFeint());
        const enVol = registre.lancer('v-1', 'cle-1', 'd-1');

        registre.retirer('v-1');

        await expect(enVol).resolves.toBe('agent-injoignable');
    });

    it('une SECONDE inscription de la même VM ferme la première et la remplace', async () => {
        // 🔴 Garder les deux poserait la question à laquelle
        // `0003-agents.sql` répond déjà pour la clé primaire d'`agent_enrole` :
        // « laquelle serait la bonne ? ». Le dernier enrôlement gagne, et
        // l'ancien socket est fermé plutôt que laissé à flotter.
        vi.useFakeTimers();
        const registre = new RegistreAgents();
        const ancien = socketFeint();
        const neuf = socketFeint();
        registre.inscrire('v-1', ancien);
        registre.inscrire('v-1', neuf);

        expect(ancien.fermetures).toEqual([1008]);
        void registre.lancer('v-1', 'cle-1', 'd-1');
        // L'ordre part sur le NEUF, et le vieux n'a rien reçu.
        expect(neuf.ecrits).toEqual([encodeLancer('d-1', 'cle-1')]);
        expect(ancien.ecrits).toEqual([]);
    });

    it("`retirer` d'un socket DÉJÀ remplacé ne débranche pas le neuf", async () => {
        // ⚠️ COURSE RÉELLE, ET ELLE EST ORDINAIRE : un agent qui se relance
        // s'inscrit AVANT que la fermeture de son ancien socket ne soit
        // notifiée. Un `retirer(vmId)` nu effacerait alors l'inscription du
        // NEUF, et la VM deviendrait injoignable alors qu'elle vient de se
        // reconnecter — panne muette, jusqu'au prochain enrôlement.
        vi.useFakeTimers();
        const registre = new RegistreAgents();
        const ancien = socketFeint();
        const neuf = socketFeint();
        registre.inscrire('v-1', ancien);
        registre.inscrire('v-1', neuf);

        registre.retirer('v-1', ancien);

        void registre.lancer('v-1', 'cle-1', 'd-1');
        expect(neuf.ecrits).toEqual([encodeLancer('d-1', 'cle-1')]);
    });

    it('rend `delai` à `DELAI_LANCEMENT_MS`, et le test FAIT AVANCER le temps', async () => {
        // 🔴 LE CAS VOIT LA TRANSITION, il ne lit pas un état final : la
        // promesse est encore en vol avant l'échéance, et résolue après. Une
        // horloge figée le rendrait inerte.
        vi.useFakeTimers();
        const registre = new RegistreAgents();
        registre.inscrire('v-1', socketFeint());
        const enVol = registre.lancer('v-1', 'cle-1', 'd-1');

        let resolue: string | undefined;
        void enVol.then((i) => {
            resolue = i;
        });

        // Une milliseconde AVANT l'échéance : rien.
        await vi.advanceTimersByTimeAsync(DELAI_LANCEMENT_MS - 1);
        expect(resolue).toBeUndefined();

        await vi.advanceTimersByTimeAsync(1);
        expect(resolue).toBe('delai');
    });

    it("n'écrit pas sur un socket qui n'est plus OUVERT", async () => {
        // ⚠️ Un `send` sur un socket en cours de fermeture LÈVE, et cette
        // exception traverserait l'appelant. Même garde que `envoyer` dans
        // `agents/canal.ts`. Le lancement rend alors `agent-injoignable` :
        // la VM est inscrite, mais son socket ne porte plus rien.
        vi.useFakeTimers();
        const registre = new RegistreAgents();
        const mourant = socketFeint(2 /* CLOSING */);
        registre.inscrire('v-1', mourant);

        await expect(registre.lancer('v-1', 'cle-1', 'd-1')).resolves.toBe('agent-injoignable');
        expect(mourant.ecrits).toEqual([]);
    });
});
