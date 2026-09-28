// The file bridge's data channel: a DEDICATED `RTCPeerConnection`, WITHOUT
// MEDIA, carrying a single data channel `fichiers`.
//
// 🔴 WHY A SEPARATE CONNECTION (decision D4 of F1's plan). The framing
// promises that "a failure of the file channel never touches the video stream".
// Sharing a window's `PeerConnection` would make a reconnection of one
// take the other with it, and would tie the bridge — which is a service of the SHELL, not
// of a window — to the lifecycle of some application window. Yet
// no window is special, and that is the invariant the shell page exists
// to hold.
//
// ⚠️ `connectSession` IS NOT REUSABLE, and it is not an oversight.
// `SessionOptions` requires a `video: HTMLVideoElement`, and `connectSession`
// UNCONDITIONALLY adds three transceivers (`video` recvonly, `audio`
// recvonly, `audio` sendonly for the mic) then the `input` and
// `control` channels. The plan forbids refactoring it to make video
// optional: the cost would be a possible regression on the critical path
// of ALL windows, against thirty or so lines duplicated here. What
// is reused WITHOUT BEING COPIED: `waitForAnswer`, `waitForIceGathering` and
// `attendreConfigIce`, the three exported from `webrtc.ts` (the last two
// were exported BY this sub-block, a change declared in their documentation).

import {
    attendreConfigIce,
    waitForAnswer,
    waitForIceGathering,
} from '../webrtc';
import { composer, lirePrefixe } from '../prefixe';
import type { Racine } from './adaptateur';
import type { RacineInscriptible } from './ecriture';
import { contrePression } from './flux';
import type { RacineMutable } from './mutation';
import { TYPE_ECHEC, decoder, encoderTexte } from '../../../proto/ts/fichiers';
import { encodeEchec } from '../../../proto/ts/fichiers-entetes';

/**
 * Reserved name of the file bridge's signaling session.
 *
 * ⚠️ MIRROR of `NOM_SESSION_DU_PONT` (`agent/src/superviseur/protocole.rs`):
 * both ends compose the SAME identifier, and a divergence would only show
 * in a real session. It is exactly the regime of `SEPARATEUR`
 * (`client/src/prefixe.ts`) and of the control session's name — `'bureau'`,
 * composed by `client/src/bureau/porteur-dom.ts` (`NOM_SESSION_DE_CONTROLE`
 * in `shell-page.ts` before task 9 inlined it, August 31st, 2026) —, and
 * the same debt: this repository has no single source for session names,
 * only commented mirrors.
 *
 * ⚠️ IT IS NOT AN IDENTIFIER ON ITS OWN: it is composed with the VM's
 * prefix (sub-block P3), otherwise two VMs would fight over the same session on
 * the platform and the second would be refused.
 */
export const NOM_SESSION_DU_PONT = 'fichiers';

export interface OptionsCanal {
    signalingUrl: string;
    /// The FULL identifier, prefix included. `sessionDuPont()` composes it.
    sessionId: string;
    /// 🔴 **RECEIVED, NEVER READ FROM THE VAULT — FIX FROM THE FINAL REVIEW OF
    /// AUGUST 31ST, 2026 (critical ①).** This module called `jetonAcces()`,
    /// that is, the **RAW** content of `localStorage`, without going through
    /// `jeton.ts::assurerAccesFrais`. An access token lives **ten minutes**
    /// (`plateforme/src/identite/jeton.ts`) and "Choose my folder" is a
    /// gesture that can come at any time: the bridge thus presented an
    /// expired token, and was refused its session without anything linking
    /// the failure to the expiry. **The caller asks for a fresh token again and
    /// passes it here** (`bureau/fichiers-dom.ts`).
    jeton: string;
    onStatus?: (message: string) => void;
    /// Called for each frame received. Returns the frame to send back, or `null`.
    traiter(octets: ArrayBuffer): Promise<ArrayBuffer | null>;
}

export interface CanalFichiers {
    pc: RTCPeerConnection;
    canal: RTCDataChannel;
    close(): void;
}

/** The bridge's session identifier for the current VM. */
export function sessionDuPont(): string {
    return composer(lirePrefixe(), NOM_SESSION_DU_PONT);
}

export async function connecterCanalFichiers(options: OptionsCanal): Promise<CanalFichiers> {
    const statut = options.onStatus ?? (() => {});

    const socket = new WebSocket(options.signalingUrl);
    await new Promise<void>((resolve, reject) => {
        socket.addEventListener('open', () => resolve(), { once: true });
        socket.addEventListener('error', () => reject(new Error('signaling injoignable')), {
            once: true,
        });
    });
    socket.send(
        JSON.stringify({ role: 'client', session: options.sessionId, jeton: options.jeton }),
    );

    const iceServers = await attendreConfigIce(socket, 2000);
    const pc = new RTCPeerConnection({ iceServers });

    // 🔴 NO `addTransceiver`: that is the whole point of D4. An offer without a
    // media m-line is legal; the agent answers it through `pont/transport.rs`, whose
    // `Rtc` is built without a codec.
    //
    // 🔴 RELIABLE AND ORDERED, that is, the DEFAULT values. Reliability
    // is not requested explicitly because it has no flag: it is
    // obtained by setting NEITHER `maxRetransmits` NOR `maxPacketLifeTime`. It is
    // the exact opposite of the `input` channel (`webrtc.ts`, `maxRetransmits: 0`), and the
    // reason is the reverse: a stale mouse position has no value,
    // a lost byte range is a corrupted file.
    const canal = pc.createDataChannel('fichiers', { ordered: true });
    // ⚠️ **BACKPRESSURE IS SET UP HERE, AND ITS LOGIC LIVES ELSEWHERE.**
    // `flux.ts` is PURE and injected; this file has NO test (its header
    // declares it), and housing an asynchronous wait here would make it untestable.
    // It is the same split as `protocole.ts` / `adaptateur.ts` since F1.
    //
    // 🔵 **`bufferedAmountLowThreshold` IS SET BY `contrePression`**, not
    // here: setting it twice would make two truths, and spec §3.4 requires it
    // ("set") without saying by whom. Before F3 it was set **nowhere**.
    const frein = contrePression(canal as unknown as import('./flux').CanalSortant);
    // ⚠️ WITHOUT THIS, `event.data` CAN BE A `Blob`. The default of
    // `RTCDataChannel.binaryType` is `'blob'` in the specification; the
    // browsers that only handle `'arraybuffer'` get away with it, the others
    // would deliver a `Blob` that `decoder()` would refuse — a failure mode
    // that depends on the browser, hence invisible in an acceptance run on just one.
    canal.binaryType = 'arraybuffer';

    canal.addEventListener('open', () => statut('canal fichiers ouvert'));
    canal.addEventListener('close', () => statut('canal fichiers fermé'));
    canal.addEventListener('message', (evenement) => {
        const donnees: unknown = evenement.data;
        if (!(donnees instanceof ArrayBuffer)) {
            // The bridge only emits binary. A string here is not a frame.
            console.warn('trame fichiers non binaire, ignorée');
            return;
        }
        // ════════════════════════════════════════════════════════════════
        // 🔴 **F5 (D10) — THE CORRELATION IS CAPTURED HERE, BEFORE ANY `await`.**
        //
        // It is the INCOMING frame that carries it, and the `catch` must know it
        // even if the backpressure wait lasted. Reading it afterwards would be
        // reading it from an object we no longer have.
        //
        // ⚠️ **A failing decode returns `undefined`, not zero**: zero is
        // a legal correlation, and answering on it would direct a failure to
        // a foreign command.
        // ════════════════════════════════════════════════════════════════
        let correlation: number | undefined;
        try {
            correlation = decoder(donnees).correlation;
        } catch {
            correlation = undefined;
        }
        /**
         * 🔴 **TWENTY SECONDS OF FREEZE BECOME AN IMMEDIATE ERROR.**
         *
         * F4 measured the wall: beyond ~3,150 entries, the `send()` of a
         * listing answer is refused by SCTP, this `catch` logged to
         * the console **and sent NOTHING back** — the application stayed frozen
         * `DELAI_LISTER` (20 s), then received an opaque error.
         *
         * ⚠️ **WHAT THIS DOES NOT DO: THE WALL DOES NOT MOVE.** A listing of
         * more than ~3,150 entries **still fails**; it only fails
         * **fast and saying so**. Splitting an enumeration into several
         * frames remains an increment of `FICHIERS_VERSION`, and it leaves
         * sub-project ③ **without a recipient**.
         */
        const denoncer = (raison: string, e: unknown) => {
            console.warn(`trame fichiers non delivree (${raison})`, e);
            if (correlation === undefined) return;
            try {
                if (canal.readyState === 'open') {
                    canal.send(encoderTexte(TYPE_ECHEC, correlation, encodeEchec('interne')));
                }
            } catch (echec: unknown) {
                // The channel went away while we were reporting. There is no one
                // left to tell, and the bridge will learn it through the
                // closing — never through a twenty-second silence.
                console.warn('denonciation impossible : canal ferme', echec);
            }
        };
        void options
            .traiter(donnees)
            .then(async (reponse) => {
                if (reponse === null) return;
                // 🔴 **BACKPRESSURE COMES BEFORE SENDING, AND AFTER IT WE
                // CHECK THE STATE AGAIN.** The wait can last, and the channel may
                // have closed meanwhile: `send` on a closed channel THROWS.
                await frein.avantEnvoi();
                if (canal.readyState !== 'open') {
                    denoncer('canal ferme pendant l attente', undefined);
                    return;
                }
                try {
                    canal.send(reponse);
                } catch (e: unknown) {
                    // It is HERE that F4's wall shows up: SCTP refuses a
                    // frame that is too large, and `send` THROWS.
                    denoncer('send refuse', e);
                }
            })
            .catch((e: unknown) => {
                // `traiter` itself answers the failures it can name; if it
                // throws, the protocol itself has broken. We say so, and
                // we do not kill the channel for all that.
                denoncer('traitement leve', e);
            });
    });

    pc.addEventListener('connectionstatechange', () => {
        statut(`pont fichiers : ${pc.connectionState}`);
    });

    const offre = await pc.createOffer();
    await pc.setLocalDescription(offre);
    await waitForIceGathering(pc);

    statut('offre du pont envoyée, attente de l’agent…');
    socket.send(JSON.stringify({ type: 'offer', sdp: pc.localDescription!.sdp }));

    const reponse = await waitForAnswer(socket);
    await pc.setRemoteDescription({ type: 'answer', sdp: reponse });
    statut('pont fichiers : réponse reçue');

    return {
        pc,
        canal,
        close() {
            canal.close();
            pc.close();
            socket.close();
        },
    };
}

/**
 * Opens the folder picker and returns the root.
 *
 * ⚠️ `showDirectoryPicker()` REQUIRES A TRANSIENT USER ACTIVATION:
 * this function must be called INSIDE a click handler, never from
 * a channel message. It is the same constraint as `window.open()`, already
 * known to the shell page.
 *
 * ⚠️ THE HANDLE IS NOT PERSISTED. It is serialisable into IndexedDB, but
 * on reload the permission must be granted again through `requestPermission()`,
 * which in turn requires a user activation: persisting would only save
 * the walk through the tree in the picker, never the gesture. One
 * click per shell page load, and that is all.
 *
 * ❌ `mode: 'read'` IS NO LONGER TRUE — F2 requests `'readwrite'`, otherwise the
 * File System Access API would refuse `createWritable()` and every write would be
 * lost AFTER the application believed it had saved.
 *
 * ⚠️ AND THIS MODE IS NOT EXERCISED BY THE ACCEPTANCE RUN: its instrument is **OPFS**,
 * whose `navigator.storage.getDirectory()` returns a real
 * `FileSystemDirectoryHandle` **without any permission model** (F1 results
 * §3). `queryPermission` / `requestPermission` and the transient user
 * activation remain NOT COVERED, as in F1. Declared.
 *
 * ⚠️ RETURNS `null` WHEN THE USER CANCELS. The browser signals cancellation
 * through an `AbortError`, that is, through the same channel as a real failure:
 * propagating the exception as is would display "the drive could not
 * be mounted" to someone who has simply clicked "Cancel". A
 * failure message on a deliberate gesture teaches the user to ignore
 * failure messages.
 *
 * ⚠️ THIS MODULE HAS NO TEST: it touches `globalThis`, `WebSocket` and
 * `RTCPeerConnection`, which do not exist under Vitest's Node. That is
 * exactly why `protocole.ts` and `adaptateur.ts` do not depend on
 * them — all the logic lives there, tested; here there is only wiring.
 */
export async function choisirDossier(): Promise<{ racine: Racine; nom: string } | null> {
    const global = globalThis as {
        showDirectoryPicker?: (o?: { mode?: 'read' | 'readwrite' }) => Promise<
            FileSystemDirectoryHandle
        >;
    };
    if (typeof global.showDirectoryPicker !== 'function') {
        throw new Error(
            'ce navigateur n’expose pas la File System Access API ' +
                '(Chromium 86+ requis, hors navigation privée)',
        );
    }
    let poignee: FileSystemDirectoryHandle;
    try {
        poignee = await global.showDirectoryPicker({ mode: 'readwrite' });
    } catch (e) {
        if (e instanceof DOMException && e.name === 'AbortError') return null;
        throw e;
    }
    // 🔴 THE ASSIGNMENT BELOW IS THE STRUCTURAL COMPATIBILITY CHECK
    // between `FileSystemDirectoryHandle` and the interfaces of `adaptateur.ts`.
    // They describe a SUBSET of the real handle, precisely so that
    // an in-memory fake can satisfy them under Node; if the real one no longer
    // satisfied them, `tsc --noEmit` would say so HERE, at compile time, and not
    // in a real session.
    //
    // ⚠️ IT DEPENDS ON `"DOM.AsyncIterable"` IN `client/tsconfig.json`:
    // without that library, `FileSystemDirectoryHandle` has no `values()`
    // at all and the assignment fails. It was added there by this sub-block.
    const racine: Racine = poignee;
    // ── 🔴 THE STRUCTURAL COMPATIBILITY CHECK, IN FULL ─────────────
    //
    // ⚠️ **F2'S ONE WAS VACUOUS, AND IT IS MEASURED.**
    // `shell-page.ts` carried `choix.racine as RacineInscriptible` while
    // declaring it "F2's STRUCTURAL COMPATIBILITY CHECK: if the real
    // handle stopped satisfying it, `tsc --noEmit` would say so HERE".
    // **`choix.racine` is typed `Racine` there, and `RacineInscriptible` is a
    // SUBtype of it**: an `as` towards a subtype is an assertion, never a
    // check. Adding to `RacineInscriptible` a method
    // `FileSystemDirectoryHandle` does not have only turned the test fake
    // red — never that line.
    // Log:
    // `docs/superpowers/plans/journaux-pont-fichiers-f3/t9-controle-structurel-de-f2-vacueux.txt`
    //
    // **The three ASSIGNMENTS below, for their part, do check**: they bear
    // on the REAL `FileSystemDirectoryHandle`, before any widening. If
    // it stopped satisfying one of the three interfaces, `tsc --noEmit` would
    // say so here, at compile time, and not in a real session.
    //
    // ⚠️ **They depend on `"DOM.AsyncIterable"` in `client/tsconfig.json`**
    // (otherwise `values()` does not exist) and, for `RacineMutable`, on
    // `removeEntry`, which the DOM library declares with an `options` we
    // do not use — see `mutation.ts`.
    const _inscriptible: RacineInscriptible = poignee;
    const _mutable: RacineMutable = poignee;
    void _inscriptible;
    void _mutable;
    return { racine, nom: poignee.name };
}
