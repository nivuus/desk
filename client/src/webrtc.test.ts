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

import { afterEach, describe, expect, it, vi } from 'vitest';

import { connectSession, parseSignalingMessage, waitForAnswer } from './webrtc';

describe('parseSignalingMessage', () => {
    it('ignore un message `null` plutôt que de lever une exception', () => {
        expect(parseSignalingMessage('null')).toBeUndefined();
    });

    it('ignore toute charge utile non-objet (nombre, chaîne, tableau, booléen)', () => {
        expect(parseSignalingMessage('42')).toBeUndefined();
        expect(parseSignalingMessage('"une chaine"')).toBeUndefined();
        expect(parseSignalingMessage('[1, 2, 3]')).toBeUndefined();
        expect(parseSignalingMessage('true')).toBeUndefined();
    });

    it('ignore un JSON illisible', () => {
        expect(parseSignalingMessage('{ceci nest pas du json')).toBeUndefined();
    });

    it("ignore un objet dont le `type` n'est pas reconnu", () => {
        expect(parseSignalingMessage('{"foo": "bar"}')).toBeUndefined();
        expect(parseSignalingMessage('{"type": "inconnu"}')).toBeUndefined();
    });

    it('accepte les messages valides tels quels', () => {
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

describe('waitForAnswer face à des messages malformés', () => {
    it("un message brut `null` n'explose pas le gestionnaire et la réponse valide suivante est toujours acceptée", async () => {
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

    it('les charges utiles non-objet successives (nombre, chaîne, tableau) sont toutes ignorées', async () => {
        const socket = new FakeSocket();
        const pending = waitForAnswer(socket as unknown as WebSocket);

        expect(() => {
            socket.emitMessage('42');
            socket.emitMessage('"une chaine"');
            socket.emitMessage('[1, 2, 3]');
        }).not.toThrow();

        socket.emitMessage(JSON.stringify({ type: 'answer', sdp: 'ok' }));
        await expect(pending).resolves.toBe('ok');
    });
});

// Ces deux tests couvrent un comportement promis par la spec (§10) mais
// jamais écrit : « l'offre contient un `m=audio` en `recvonly` » et « deux
// pistes reçues aboutissent dans un seul `MediaStream` ». Le reste du
// fichier isole `parseSignalingMessage`/`waitForAnswer` justement pour
// éviter d'avoir à faire tourner un `RTCPeerConnection`/`WebSocket` réels
// sous Node ; ici, on va jusqu'au bout via des fausses implémentations
// globales plutôt que de laisser le trou ouvert. Le point testé est ce que
// `connectSession` DEMANDE au navigateur (transceiver `audio` en
// `recvonly`, un seul `MediaStream` porteur des deux pistes reçues) — pas la
// génération SDP elle-même, hors de portée sans moteur WebRTC réel.

/// Point d'accès `RTCPeerConnection` factice. `createOffer` traduit
/// fidèlement les transceivers demandés en lignes SDP : c'est cette
/// traduction, fidèle à la demande, que les tests vérifient.
class FakeRtcPeerConnection {
    transceivers: Array<{ kind: string; direction?: string }> = [];
    iceGatheringState = 'complete';
    connectionState = 'new';
    localDescription: { type: string; sdp: string } | null = null;
    private listeners = new Map<string, Set<(event: any) => void>>();

    constructor(_config: unknown) {
        derniereInstancePc = this;
    }

    addTransceiver(kind: string, opts?: { direction?: string }): void {
        this.transceivers.push({ kind, direction: opts?.direction });
    }

    createDataChannel(label: string, _opts?: unknown) {
        return {
            label,
            readyState: 'open',
            addEventListener() {},
            removeEventListener() {},
            send() {},
        };
    }

    addEventListener(type: string, cb: (event: any) => void): void {
        if (!this.listeners.has(type)) this.listeners.set(type, new Set());
        this.listeners.get(type)!.add(cb);
    }

    removeEventListener(type: string, cb: (event: any) => void): void {
        this.listeners.get(type)?.delete(cb);
    }

    emit(type: string, event: unknown): void {
        for (const cb of [...(this.listeners.get(type) ?? [])]) cb(event);
    }

    async createOffer(): Promise<{ type: 'offer'; sdp: string }> {
        const lignes = ['v=0'];
        for (const { kind, direction } of this.transceivers) {
            lignes.push(`m=${kind} 9 UDP/TLS/RTP/SAVPF 0`);
            lignes.push(`a=${direction ?? 'sendrecv'}`);
        }
        return { type: 'offer', sdp: lignes.join('\r\n') };
    }

    async setLocalDescription(desc: { type: string; sdp: string }): Promise<void> {
        this.localDescription = desc;
    }

    async setRemoteDescription(_desc: unknown): Promise<void> {}

    close(): void {}
}

/// `MediaStream` factice : juste assez pour prouver que les pistes reçues
/// s'accumulent dans le même objet plutôt que de se chasser l'une l'autre.
class FakeMediaStream {
    private tracks: unknown[] = [];
    addTrack(track: unknown): void {
        this.tracks.push(track);
    }
    getTracks(): unknown[] {
        return this.tracks;
    }
}

/// `WebSocket` factice qui s'ouvre tout de suite et répond automatiquement
/// « answer » dès qu'il voit passer une offre, pour que `connectSession`
/// puisse aller jusqu'au bout sans jamais toucher un vrai réseau.
class FakeSignalingSocket {
    private listeners = new Map<string, Set<(event: any) => void>>();

    constructor(_url: string) {
        queueMicrotask(() => this.emit('open', {}));
    }

    addEventListener(type: string, cb: (event: any) => void): void {
        if (!this.listeners.has(type)) this.listeners.set(type, new Set());
        this.listeners.get(type)!.add(cb);
    }

    removeEventListener(type: string, cb: (event: any) => void): void {
        this.listeners.get(type)?.delete(cb);
    }

    emit(type: string, event: unknown): void {
        for (const cb of [...(this.listeners.get(type) ?? [])]) cb(event);
    }

    send(data: string): void {
        const parsed = JSON.parse(data) as { type?: string };
        if (parsed.type === 'offer') {
            queueMicrotask(() => {
                this.emit('message', {
                    data: JSON.stringify({ type: 'answer', sdp: 'v=0\r\n' }),
                });
            });
        }
    }

    close(): void {}
}

let derniereInstancePc: FakeRtcPeerConnection | undefined;

function fauxVideo(): HTMLVideoElement {
    return { srcObject: null } as unknown as HTMLVideoElement;
}

describe('connectSession — négociation promise par la spec §10', () => {
    afterEach(() => {
        derniereInstancePc = undefined;
        vi.unstubAllGlobals();
    });

    it("l'offre envoyée contient un `m=audio` en `recvonly`", async () => {
        vi.stubGlobal('RTCPeerConnection', FakeRtcPeerConnection);
        vi.stubGlobal('WebSocket', FakeSignalingSocket);
        vi.stubGlobal('MediaStream', FakeMediaStream);

        await connectSession({
            signalingUrl: 'ws://signaling.invalid',
            sessionId: 'test',
            video: fauxVideo(),
        });

        const sdp = derniereInstancePc!.localDescription!.sdp;
        const lignes = sdp.split('\r\n');
        const indexAudio = lignes.indexOf('m=audio 9 UDP/TLS/RTP/SAVPF 0');
        expect(indexAudio).toBeGreaterThanOrEqual(0);
        expect(lignes[indexAudio + 1]).toBe('a=recvonly');
    });

    it('deux pistes reçues aboutissent dans un seul MediaStream', async () => {
        vi.stubGlobal('RTCPeerConnection', FakeRtcPeerConnection);
        vi.stubGlobal('WebSocket', FakeSignalingSocket);
        vi.stubGlobal('MediaStream', FakeMediaStream);

        const video = fauxVideo();
        const sessionPromise = connectSession({
            signalingUrl: 'ws://signaling.invalid',
            sessionId: 'test',
            video,
        });

        // Le listener `track` est câblé avant le premier `await` de
        // `connectSession` (voir webrtc.ts) : l'instance factice est donc
        // déjà disponible ici, sans attendre la résolution complète.
        // `receiver` est toujours présent sur un vrai `RTCTrackEvent` — un
        // objet nu ici, sans `playoutDelayHint`, imite un navigateur qui ne
        // supporte pas la propriété (voir l'accès défensif dans webrtc.ts).
        const pisteVideo = { kind: 'video' } as unknown as MediaStreamTrack;
        const pisteAudio = { kind: 'audio' } as unknown as MediaStreamTrack;
        derniereInstancePc!.emit('track', { track: pisteVideo, receiver: {} });
        derniereInstancePc!.emit('track', { track: pisteAudio, receiver: {} });

        const flux = video.srcObject as unknown as FakeMediaStream;
        expect(flux).toBeInstanceOf(FakeMediaStream);
        expect(flux.getTracks()).toEqual([pisteVideo, pisteAudio]);

        // La seconde piste ne doit pas avoir chassé la première en
        // réassignant `srcObject` : même objet `flux` avant et après.
        expect(video.srcObject).toBe(flux);

        await sessionPromise;
    });
});
