// Tests of the protection against unreadable signaling messages.
//
// Targeted regression: `JSON.parse` succeeds on payloads that are not
// objects (`"null"` → `null`, `"42"` → a number, `'"x"'` → a
// string, `"[1,2]"` → an array). Directly accessing `.type` on `null` throws
// an uncaught `TypeError`, which previously reached `onMessage` without
// going through the `try/catch` of `JSON.parse` (the latter protects parsing, not
// the property read that follows). The same defect had already been fixed
// on the signaling server side (commit 31db9f6): this file checks that it does not
// reappear on the client side.
//
// ⚠️ **This file only keeps what is exercised WITHOUT a browser.** The
// end-to-end tests of `connectSession` — which require simulating
// `RTCPeerConnection`, `WebSocket` and `MediaStream` — live in
// `webrtc.session.test.ts`, extracted by task 10 of project E when this
// file crossed the repository's ceiling of 500 lines.

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

/// Minimal fake socket: `waitForAnswer` only uses
/// `addEventListener`/`removeEventListener` for the `message` and
/// `close` events. No need for a real WebSocket (unavailable under Node without a DOM)
/// to prove the handler does not crash.
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

        // Before the fix, this threw an uncaught TypeError inside
        // the `message` event handler (access to `.type`
        // on `null`): invisible to a test that only awaited the
        // promise (no exception goes up to the calling code from an
        // event listener), but fatal in practice on the browser side, because it
        // interrupts the handler before it can process the next
        // message.
        expect(() => socket.emitMessage('null')).not.toThrow();

        // The valid answer arriving afterwards must still resolve the
        // promise: proof that the `null` message was indeed ignored, not
        // that it silently broke the listener.
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
