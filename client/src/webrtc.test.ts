// Tests de la protection contre les messages de signaling illisibles.
//
// Régression visée : `JSON.parse` réussit sur des charges utiles qui ne sont
// pas des objets (`"null"` → `null`, `"42"` → un nombre, `'"x"'` → une
// chaîne, `"[1,2]"` → un tableau). Un accès direct à `.type` sur `null` lève
// une `TypeError` non interceptée, qui atteignait auparavant `onMessage` sans
// passer par le `try/catch` de `JSON.parse` (celui-ci protège l'analyse, pas
// la lecture de propriété qui suit). Le même défaut avait déjà été corrigé
// côté serveur de signaling (commit 31db9f6) : ce fichier vérifie qu'il ne
// réapparaît pas côté client.
//
// ⚠️ **Ce fichier ne garde que ce qui s'éprouve SANS navigateur.** Les tests
// de bout en bout de `connectSession` — qui exigent de simuler
// `RTCPeerConnection`, `WebSocket` et `MediaStream` — vivent dans
// `webrtc.session.test.ts`, extraits par la tâche 10 du chantier E quand ce
// fichier a franchi le plafond de 500 lignes du dépôt.

import { describe, expect, it } from 'vitest';

import { parseSignalingMessage, waitForAnswer } from './webrtc';

describe('parseSignalingMessage', () => {
    it('ignores a `null` message rather than throwing an exception', () => {
        expect(parseSignalingMessage('null')).toBeUndefined();
    });

    it('ignores any non-object payload (number, string, array, boolean)', () => {
        expect(parseSignalingMessage('42')).toBeUndefined();
        expect(parseSignalingMessage('"a string"')).toBeUndefined();
        expect(parseSignalingMessage('[1, 2, 3]')).toBeUndefined();
        expect(parseSignalingMessage('true')).toBeUndefined();
    });

    it('ignores unreadable JSON', () => {
        expect(parseSignalingMessage('{this is not json')).toBeUndefined();
    });

    it("ignores an object whose `type` is not recognised", () => {
        expect(parseSignalingMessage('{"foo": "bar"}')).toBeUndefined();
        expect(parseSignalingMessage('{"type": "inconnu"}')).toBeUndefined();
    });

    it('accepts valid messages as is', () => {
        expect(parseSignalingMessage('{"type": "answer", "sdp": "v=0..."}')).toEqual({
            type: 'answer',
            sdp: 'v=0...',
        });
        expect(parseSignalingMessage('{"type": "error", "reason": "boom"}')).toEqual({
            type: 'error',
            reason: 'boom',
        });
        expect(parseSignalingMessage('{"type": "peer-gone"}')).toEqual({ type: 'peer-gone' });
    });

    it('recognises the ICE configuration', () => {
        const message = parseSignalingMessage(
            JSON.stringify({
                type: 'ice-config',
                iceServers: [{ urls: 'turn:x:3478', username: 'u', credential: 'c' }],
            }),
        );
        expect(message).toEqual({
            type: 'ice-config',
            iceServers: [{ urls: 'turn:x:3478', username: 'u', credential: 'c' }],
        });
    });
});

/// Faux socket minimal : `waitForAnswer` n'utilise que
/// `addEventListener`/`removeEventListener` pour les événements `message` et
/// `close`. Pas besoin d'un vrai WebSocket (indisponible sous Node sans DOM)
/// pour prouver que le gestionnaire ne plante pas.
class FakeSocket {
    private listeners = new Map<string, Set<(event: unknown) => void>>();

    addEventListener(type: string, listener: (event: unknown) => void): void {
        if (!this.listeners.has(type)) this.listeners.set(type, new Set());
        this.listeners.get(type)!.add(listener);
    }

    removeEventListener(type: string, listener: (event: unknown) => void): void {
        this.listeners.get(type)?.delete(listener);
    }

    emitMessage(data: string): void {
        for (const listener of this.listeners.get('message') ?? []) {
            listener({ data });
        }
    }
}

describe('waitForAnswer facing malformed messages', () => {
    it("a raw `null` message does not blow up the handler and the next valid answer is still accepted", async () => {
        const socket = new FakeSocket();
        const pending = waitForAnswer(socket as unknown as WebSocket);

        // Avant le correctif, ceci levait une TypeError non interceptée à
        // l'intérieur du gestionnaire d'événement `message` (accès à `.type`
        // sur `null`) : invisible pour un test qui ne ferait qu'attendre la
        // promesse (aucune exception ne remonte au code appelant depuis un
        // event listener), mais fatale en pratique côté navigateur, car elle
        // interrompt le gestionnaire avant qu'il puisse traiter le message
        // suivant.
        expect(() => socket.emitMessage('null')).not.toThrow();

        // La réponse valide arrivée ensuite doit toujours résoudre la
        // promesse : preuve que le message `null` a bien été ignoré, pas
        // qu'il a cassé silencieusement l'écouteur.
        socket.emitMessage(JSON.stringify({ type: 'answer', sdp: 'v=0...' }));

        await expect(pending).resolves.toBe('v=0...');
    });

    it('successive non-object payloads (number, string, array) are all ignored', async () => {
        const socket = new FakeSocket();
        const pending = waitForAnswer(socket as unknown as WebSocket);

        expect(() => {
            socket.emitMessage('42');
            socket.emitMessage('"a string"');
            socket.emitMessage('[1, 2, 3]');
        }).not.toThrow();

        socket.emitMessage(JSON.stringify({ type: 'answer', sdp: 'ok' }));
        await expect(pending).resolves.toBe('ok');
    });
});
