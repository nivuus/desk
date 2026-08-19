// Tests de bout en bout de `connectSession`, sur des implémentations factices
// de `RTCPeerConnection`, `WebSocket` et `MediaStream`.
//
// ⚠️ **EXTRAIT de `webrtc.test.ts` par la tâche 10 du chantier E**, qui l'avait
// porté à 517 lignes — au-dessus du plafond de 500 du dépôt. La règle y est
// explicite : « aucun NOUVEAU fichier ne naît au-dessus de 500 lignes, et un
// fichier déjà au-dessus ne doit pas grossir davantage », et le remède prescrit
// est l'EXTRACTION, jamais la compression des commentaires. La coupure suit
// une frontière réelle et non un compte de lignes : `webrtc.test.ts` garde ce
// qui s'éprouve SANS navigateur (`parseSignalingMessage`, `waitForAnswer`, sur
// un faux socket minimal), ce fichier-ci prend tout ce qui exige de simuler un
// navigateur entier. C'est cette seconde moitié qui a grandi, et qui grandira.
//
// Ce que ces tests portent : ce que `connectSession` DEMANDE au navigateur —
// les transceivers et leur ORDRE (qui décide les `mid`, dont l'agent dépend),
// un seul `MediaStream` porteur des pistes reçues, le jeton dans la poignée de
// main (sous-bloc P2), et l'extinction réelle du micro à la fermeture
// (chantier E). Pas la génération SDP elle-même, hors de portée sans moteur
// WebRTC réel.

import { afterEach, describe, expect, it, vi } from 'vitest';

import { connectSession } from './webrtc';
import { CLE_ACCES } from './jeton';

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
/// Ce qu'un `addTransceiver` rend, réduit à ce dont `connectSession` se sert.
interface FauxTransceiver {
    kind: string;
    direction?: string;
    sender: { track: { stop(): void } | null };
}

class FakeRtcPeerConnection {
    transceivers: FauxTransceiver[] = [];
    iceGatheringState = 'complete';
    connectionState = 'new';
    localDescription: { type: string; sdp: string } | null = null;
    private listeners = new Map<string, Set<(event: any) => void>>();

    constructor(_config: unknown) {
        derniereInstancePc = this;
    }

    addTransceiver(kind: string, opts?: { direction?: string }): FauxTransceiver {
        // Le faux RENVOIE désormais un transceiver, comme le vrai : c'est le
        // `sender` de celui du micro que `connectSession` expose (chantier E).
        // Un `void` ici faisait échouer les cinq tests de `connectSession` sur
        // « Cannot read properties of undefined (reading 'sender') ».
        const transceiver: FauxTransceiver = {
            kind,
            direction: opts?.direction,
            // `track: null` est l'état d'un transceiver déclaré SANS PISTE —
            // exactement ce que la spec §5 exige du micro : aucune capture,
            // aucune permission demandée tant qu'on n'a pas cliqué.
            sender: { track: null },
        };
        this.transceivers.push(transceiver);
        return transceiver;
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
        messagesEnvoyes.push(data);
        const parsed = JSON.parse(data) as { type?: string; role?: string };
        // Configuration ICE vide, émise dès la déclaration de rôle, comme le
        // fait le serveur de signaling quand aucun relais n'est déployé.
        // Sans elle, `connectSession` patienterait 2 s (le délai
        // d'`attendreConfigIce`) avant de construire la connexion.
        if (parsed.role === 'client') {
            queueMicrotask(() => {
                this.emit('message', {
                    data: JSON.stringify({ type: 'ice-config', iceServers: [] }),
                });
            });
        }
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
/// Tout ce que le client a poussé sur le socket de signaling. C'est le seul
/// instrument qui puisse dire ce que porte la POIGNÉE DE MAIN.
let messagesEnvoyes: string[] = [];

/// La déclaration de rôle, c'est-à-dire le premier message envoyé.
function poigneeDeMain(): Record<string, unknown> {
    return JSON.parse(messagesEnvoyes[0]) as Record<string, unknown>;
}

function fauxVideo(): HTMLVideoElement {
    return { srcObject: null } as unknown as HTMLVideoElement;
}

describe('connectSession — négociation promise par la spec §10', () => {
    afterEach(() => {
        derniereInstancePc = undefined;
        messagesEnvoyes = [];
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
        // La session est attendue AVANT d'émettre les pistes : depuis que la
        // configuration ICE doit être reçue pour construire la connexion, la
        // `RTCPeerConnection` naît après le premier `await` de
        // `connectSession`, et l'instance factice n'existe donc pas encore au
        // retour de l'appel. Le listener `track` est câblé juste après sa
        // construction, bien avant la résolution — l'émettre ici l'atteint
        // aussi sûrement qu'avant.
        await connectSession({
            signalingUrl: 'ws://signaling.invalid',
            sessionId: 'test',
            video,
        });

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
    });

    // ── Le micro (chantier E, tâche 10) ─────────────────────────────────────
    //
    // ⚠️ LE PLAN DÉCLARE CES TROIS TESTS IMPOSSIBLES, ET IL A TORT. Il écrit
    // que « `connectSession` exige un vrai `RTCPeerConnection` et n'est donc
    // pas testé ici — c'est déjà le parti de `webrtc.test.ts`, qui n'exerce que
    // `parseSignalingMessage` et `waitForAnswer` ». Ce n'est plus vrai depuis
    // les deux tests de négociation ci-dessus, et depuis les trois tests du
    // jeton (sous-bloc P2) : cinq tests traversent `connectSession` de bout en
    // bout sur un faux `pc` dont le `createOffer` TRADUIT FIDÈLEMENT les
    // transceivers demandés en lignes SDP.
    //
    // Le plan écartait ensuite « un test qui vérifie que `addTransceiver` a été
    // appelé », au motif qu'il ne pourrait échouer que par suppression de la
    // ligne. Le reproche vaut pour un test qui compterait les appels ; il ne
    // vaut pas pour ceux-ci, qui portent sur l'ORDRE des m-lines et sur
    // l'extinction — deux propriétés qu'on peut casser sans rien supprimer, et
    // dont chacune a été vue tomber sous mutation (voir le rapport de tâche).
    it("l'offre déclare une TROISIÈME m-line, `audio` en `sendonly`, APRÈS l'audio descendante", async () => {
        vi.stubGlobal('RTCPeerConnection', FakeRtcPeerConnection);
        vi.stubGlobal('WebSocket', FakeSignalingSocket);
        vi.stubGlobal('MediaStream', FakeMediaStream);

        await connectSession({
            signalingUrl: 'ws://signaling.invalid',
            sessionId: 'test',
            video: fauxVideo(),
        });

        // L'ORDRE est le fond du test, pas un détail de forme : c'est lui qui
        // décide les `mid`, et l'agent range la piste audio dans `audio_mid` ou
        // dans `mic_mid` selon sa direction (`transport/evenements.rs`).
        // Intervertir les deux transceivers audio ferait partir le son
        // DESCENDANT sur une piste inémissible, sans une seule erreur.
        expect(derniereInstancePc!.transceivers.map((t) => [t.kind, t.direction])).toEqual([
            ['video', 'recvonly'],
            ['audio', 'recvonly'],
            ['audio', 'sendonly'],
        ]);

        const lignes = derniereInstancePc!.localDescription!.sdp.split('\r\n');
        const audios = lignes.flatMap((l, i) => (l.startsWith('m=audio ') ? [i] : []));
        expect(audios).toHaveLength(2);
        expect(lignes[audios[0] + 1]).toBe('a=recvonly');
        expect(lignes[audios[1] + 1]).toBe('a=sendonly');
    });

    it('le sender du micro est exposé, et il naît SANS PISTE', async () => {
        vi.stubGlobal('RTCPeerConnection', FakeRtcPeerConnection);
        vi.stubGlobal('WebSocket', FakeSignalingSocket);
        vi.stubGlobal('MediaStream', FakeMediaStream);

        const session = await connectSession({
            signalingUrl: 'ws://signaling.invalid',
            sessionId: 'test',
            video: fauxVideo(),
        });

        // C'est le sender du TROISIÈME transceiver, pas d'un autre : sans cette
        // égalité d'identité, `micro.ts` remplirait la piste descendante.
        expect(session.micSender).toBe(derniereInstancePc!.transceivers[2].sender);
        expect(session.micSender.track).toBeNull();
    });

    it("`close()` ARRÊTE la piste du micro, et pas seulement la connexion", async () => {
        vi.stubGlobal('RTCPeerConnection', FakeRtcPeerConnection);
        vi.stubGlobal('WebSocket', FakeSignalingSocket);
        vi.stubGlobal('MediaStream', FakeMediaStream);

        const session = await connectSession({
            signalingUrl: 'ws://signaling.invalid',
            sessionId: 'test',
            video: fauxVideo(),
        });

        // Ce que `micro.ts` aura fait au clic : `replaceTrack` pose la piste
        // sur le sender.
        let arretee = false;
        (session.micSender as unknown as FauxTransceiver['sender']).track = {
            stop() {
                arretee = true;
            },
        };

        session.close();

        // ⚠️ `pc.close()` NE STOPPE PAS les pistes locales : sans le `stop()`
        // explicite, l'indicateur de micro de Chrome resterait allumé après la
        // fin de session et le périphérique resterait pris. C'est le « mensonge
        // visuel » que la spec §9 qualifie d'inacceptable sur cette fonction.
        expect(arretee).toBe(true);
    });
});

describe('la poignée de main porte le jeton (sous-bloc P2)', () => {
    afterEach(() => {
        messagesEnvoyes = [];
        vi.unstubAllGlobals();
    });

    function armerLesFactices(): void {
        vi.stubGlobal('RTCPeerConnection', FakeRtcPeerConnection);
        vi.stubGlobal('WebSocket', FakeSignalingSocket);
        vi.stubGlobal('MediaStream', FakeMediaStream);
    }

    it('envoie le jeton passé en option', async () => {
        armerLesFactices();
        await connectSession({
            signalingUrl: 'ws://signaling.invalid',
            sessionId: 'test',
            video: fauxVideo(),
            jeton: 'jeton-de-l-appelant',
        });
        expect(poigneeDeMain()).toEqual({
            role: 'client',
            session: 'test',
            jeton: 'jeton-de-l-appelant',
        });
    });

    it('retombe sur le coffre du navigateur quand aucun jeton n’est passé', async () => {
        // E7 : le champ est FACULTATIF pour que `main.ts` — modifié par un
        // autre chantier — n'ait pas à bouger. Le repli est donc le chemin
        // NOMINAL, pas un cas de secours, et il doit être éprouvé comme tel.
        armerLesFactices();
        vi.stubGlobal('localStorage', {
            getItem: (c: string) => (c === CLE_ACCES ? 'jeton-du-coffre' : null),
            setItem: () => {},
            removeItem: () => {},
        });
        await connectSession({
            signalingUrl: 'ws://signaling.invalid',
            sessionId: 'test',
            video: fauxVideo(),
        });
        expect(poigneeDeMain().jeton).toBe('jeton-du-coffre');
    });

    it("n'ajoute AUCUN champ hors `jeton`, et n'en retire aucun", async () => {
        // 🔴 Spec §10.2 : le champ est AJOUTÉ, aucun n'est retiré. Un service
        // du sous-bloc P1 ne lit que `role` et `session` et ignore `jeton`,
        // donc ce client reste compatible avec lui. La compatibilité ne va que
        // dans ce sens, et c'est ce test qui garde la première moitié.
        armerLesFactices();
        vi.stubGlobal('localStorage', {
            getItem: () => null,
            setItem: () => {},
            removeItem: () => {},
        });
        await connectSession({
            signalingUrl: 'ws://signaling.invalid',
            sessionId: 'test',
            video: fauxVideo(),
        });
        // Sans jeton : la poignée de main d'avant P2, à l'identique.
        expect(Object.keys(poigneeDeMain()).sort()).toEqual(['role', 'session']);

        messagesEnvoyes = [];
        await connectSession({
            signalingUrl: 'ws://signaling.invalid',
            sessionId: 'test',
            video: fauxVideo(),
            jeton: 'j',
        });
        // Avec jeton : EXACTEMENT un champ de plus.
        expect(Object.keys(poigneeDeMain()).sort()).toEqual(['jeton', 'role', 'session']);
    });
});
