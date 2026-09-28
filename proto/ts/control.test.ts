import { describe, expect, it } from 'vitest';
import type { CapabilitiesMessage, MicStateMessage, ReadyMessage } from './control';
import {
    CONTROL_VERSION,
    TYPES_AGENT,
    encodeClipboard,
    encodeResize,
    encodeVisibility,
    parseAgentControl,
} from './control';

describe('control protocol', () => {
    it('encodes a resize', () => {
        expect(JSON.parse(encodeResize(1280, 720))).toEqual({
            v: CONTROL_VERSION,
            type: 'resize',
            width: 1280,
            height: 720,
        });
    });

    it('rounds and bounds the dimensions', () => {
        expect(JSON.parse(encodeResize(0, 719.6))).toEqual({
            v: CONTROL_VERSION,
            type: 'resize',
            width: 1,
            height: 720,
        });
    });

    // Workstream E: `mic` is OPTIONAL, and its absence means "no microphone".
    it('parses a ready message WITHOUT mic: the field stays undefined', () => {
        const msg = parseAgentControl(
            `{"v":${CONTROL_VERSION},"type":"ready","width":800,"height":600}`,
        ) as ReadyMessage;
        expect(msg.mic).toBeUndefined();
        // …and `undefined` is falsy: it is what makes a recent client
        // facing an old agent display NO button, without a line of
        // code to decide it.
        expect(Boolean(msg.mic)).toBe(false);
    });

    it('parses a ready message WITH mic and keeps it', () => {
        const msg = parseAgentControl(
            `{"v":${CONTROL_VERSION},"type":"ready","width":800,"height":600,"mic":true}`,
        ) as ReadyMessage;
        expect(msg.mic).toBe(true);
    });

    it('parses a ready message', () => {
        const msg = parseAgentControl(`{"v":${CONTROL_VERSION},"type":"ready","width":800,"height":600}`);
        expect(msg).toEqual({ v: CONTROL_VERSION, type: 'ready', width: 800, height: 600 });
    });

    it('parses a session end', () => {
        const msg = parseAgentControl(`{"v":${CONTROL_VERSION},"type":"session-end","reason":"closed"}`);
        expect(msg.type).toBe('session-end');
    });

    it('rejects an unknown version', () => {
        expect(() => parseAgentControl('{"v":9,"type":"ready","width":1,"height":1}')).toThrow(
            /control version/,
        );
    });

    it('rejects an absent version', () => {
        expect(() => parseAgentControl('{"type":"ready","width":1,"height":1}')).toThrow(
            /control version/,
        );
    });

    it('rejects an unknown type', () => {
        expect(() => parseAgentControl(`{"v":${CONTROL_VERSION},"type":"autre"}`)).toThrow(/control type/);
    });

    it('parses a pointer message', () => {
        const message = parseAgentControl(
            `{"type":"pointer","v":${CONTROL_VERSION},"visible":false,"shape":"ns-resize"}`,
        );
        expect(message).toEqual({ type: 'pointer', v: CONTROL_VERSION, visible: false, shape: 'ns-resize' });
    });

    it('parses a rumble message', () => {
        const message = parseAgentControl(`{"type":"rumble","v":${CONTROL_VERSION},"left":255,"right":0}`);
        expect(message).toEqual({ type: 'rumble', v: CONTROL_VERSION, left: 255, right: 0 });
    });

    it('parses a capabilities message', () => {
        const message = parseAgentControl(`{"type":"capabilities","v":${CONTROL_VERSION},"gamepad":false}`);
        expect(message).toEqual({ type: 'capabilities', v: CONTROL_VERSION, gamepad: false });
    });

    it('rejects control version 1, now obsolete', () => {
        expect(() => parseAgentControl('{"type":"ready","v":1,"width":1,"height":1}')).toThrow();
    });

    it("parses the state of the link", () => {
        const message = parseAgentControl(
            JSON.stringify({
                type: 'link',
                v: CONTROL_VERSION,
                bitrate: 4_000_000,
                width: 1280,
                height: 720,
                quality: 'degradee',
                adaptation: 'active',
            }),
        );
        expect(message).toEqual({
            type: 'link',
            v: CONTROL_VERSION,
            bitrate: 4_000_000,
            width: 1280,
            height: 720,
            quality: 'degradee',
            adaptation: 'active',
        });
    });

    it('encodes a visibility at the current version', () => {
        const json = JSON.parse(encodeVisibility(false, true));
        expect(json).toEqual({ v: CONTROL_VERSION, type: 'visibility', visible: false, focused: true });
    });

    it('accepts an asleep message coming from the agent', () => {
        const raw = JSON.stringify({ v: CONTROL_VERSION, type: 'asleep', asleep: true, reason: 'evincee' });
        expect(parseAgentControl(raw)).toEqual({
            v: CONTROL_VERSION, type: 'asleep', asleep: true, reason: 'evincee',
        });
    });

    it('parses a fullscreen message', () => {
        const message = parseAgentControl(
            JSON.stringify({ v: CONTROL_VERSION, type: 'fullscreen', active: true }),
        );
        expect(message).toEqual({ v: CONTROL_VERSION, type: 'fullscreen', active: true });
    });

    it('parses a clipboard message carrying text', () => {
        const message = parseAgentControl(
            JSON.stringify({ v: CONTROL_VERSION, type: 'clipboard', text: 'bonjour', bytes: 7 }),
        );
        expect(message).toEqual({ v: CONTROL_VERSION, type: 'clipboard', text: 'bonjour', bytes: 7 });
    });

    it('parses a clipboard refusal, text at null', () => {
        const message = parseAgentControl(
            JSON.stringify({ v: CONTROL_VERSION, type: 'clipboard', text: null, bytes: 102400 }),
        );
        expect(message).toEqual({ v: CONTROL_VERSION, type: 'clipboard', text: null, bytes: 102400 });
    });

    // 🔴 The check of `v` precedes that of the type: this test pins it for
    // the new variant. Without it, an agent of a future version would make
    // anything be written into the local clipboard.
    it('rejects a clipboard at version 2', () => {
        const raw = JSON.stringify({ v: 2, type: 'clipboard', text: 'bonjour', bytes: 7 });
        expect(() => parseAgentControl(raw)).toThrow(/unsupported control version/);
    });

    // 🔴 The RUNTIME witness of the derivation of `TYPES_AGENT`. The TEN
    // values are written BY HAND here, precisely so that the test is
    // independent of the table it judges: one key too many in `ALL_AGENT`
    // makes it fail, and a missing key first makes `tsc` fail.
    //
    // ⚠️ **Nine until sub-block A1, TEN since** — and the `tsc` guard was
    // SEEN failing before the key was set:
    //   "Property 'accent' is missing in type { ready: true; … } but required
    //     in type Record<… | "accent", true>"
    // It is the only compile-time guard on the TypeScript side, and RA1-4 required
    // that it be seen, not assumed.
    it('TYPES_AGENT contains exactly the types of the union', () => {
        expect(TYPES_AGENT.slice().sort()).toEqual(
            [
                'ready', 'session-end', 'pointer', 'rumble', 'capabilities',
                'link', 'asleep', 'fullscreen', 'clipboard', 'accent',
                'mic-state',
            ].sort(),
        );
    });

    // Sub-block A1: the accent variant goes through `parseAgentControl`.
    // RED if the interface, the union or `ALL_AGENT` were missing — all three
    // are tested at once here, at RUNTIME.
    it('parses an accent', () => {
        const raw = JSON.stringify({ v: CONTROL_VERSION, type: 'accent', couleur: '#7aa2f7' });
        expect(parseAgentControl(raw)).toEqual({
            v: CONTROL_VERSION,
            type: 'accent',
            couleur: '#7aa2f7',
        });
    });

    // 🔴 The check of `v` precedes that of the type, as for the
    // clipboard: without it, an agent of a future version would make
    // anything be set on `--accent-fenetre`.
    it('rejects an accent at version 2', () => {
        const raw = JSON.stringify({ v: 2, type: 'accent', couleur: '#7aa2f7' });
        expect(() => parseAgentControl(raw)).toThrow(/unsupported control version/);
    });

    // ── Bloc E3 : la variante `mic-state` ───────────────────────────────────
    //
    // ⚠️ **Divergence V1, HANDED DOWN and not closed:** there is NO shared
    // vectors file for `AgentControl`. These assertions pin the wire
    // shape **on the TypeScript side**; `proto/src/control/tests.rs` pins **its
    // own**. The two agree because two hands wrote the same
    // string, and **nothing checks it**: a key rename applied on only
    // one side would stay green on both sides.

    it('parses a refused microphone state', () => {
        const msg = parseAgentControl(
            `{"v":${CONTROL_VERSION},"type":"mic-state","granted":false}`,
        ) as MicStateMessage;
        expect(msg.type).toBe('mic-state');
        expect(msg.granted).toBe(false);
    });

    it('parses a granted microphone state', () => {
        const msg = parseAgentControl(
            `{"v":${CONTROL_VERSION},"type":"mic-state","granted":true}`,
        ) as MicStateMessage;
        expect(msg.granted).toBe(true);
    });

    it("mic-state is in TYPES_AGENT, hence in the derivation of the union", () => {
        // The RUNTIME witness of `ALL_AGENT`: `tsc` already guards the list,
        // but a test cannot observe a compile error.
        expect(TYPES_AGENT).toContain('mic-state');
    });

    it('rejects a microphone state at the wrong version', () => {
        expect(() => parseAgentControl('{"v":2,"type":"mic-state","granted":true}')).toThrow(
            /unsupported control version/,
        );
    });
});

// ---------------------------------------------------------------------------
// Sub-block P2 of the clipboard workstream — the browser → VM direction.
// ---------------------------------------------------------------------------

describe('the browser → VM paste', () => {
    // RED if the encoder is absent, or if it does not emit the current version.
    // The exact shape is the one `proto/src/control.rs` deserialises, with
    // `deny_unknown_fields`: one more field would be refused on the agent side.
    it('encodeClipboard returns the shape serde accepts', () => {
        expect(encodeClipboard('bonjour')).toBe(
            JSON.stringify({ v: CONTROL_VERSION, type: 'clipboard', text: 'bonjour' }),
        );
    });

    // RED if the encoder lost non-ASCII characters or line
    // breaks — it is `JSON.stringify` that carries them, and this test pins it.
    it('encodeClipboard carries line breaks and accents', () => {
        const decode = JSON.parse(encodeClipboard('une\r\ndeux\néàü')) as { text: string };
        expect(decode.text).toBe('une\r\ndeux\néàü');
    });

    // 🔴 `CapabilitiesMessage.clipboard` must be OPTIONAL.
    //
    // RED if made mandatory: an agent from before P2 does not carry it, and
    // `undefined` must mean `false` FOR FREE — exactly like
    // `ReadyMessage.mic`. The witness is a `satisfies`: an object without the field
    // must stay assignable, which `tsc` would refuse if the field were required.
    //
    // ⚠️ **This test is judged by `npm run typecheck`, NOT by `vitest`** — vitest
    // relies on esbuild, which transpiles without checking types. The explicit
    // annotation below is the guard: if `clipboard` became mandatory,
    // `tsc` would refuse this assignment. The two `expect`s only anchor the
    // runtime consequence; it is the type line that carries the property.
    it('CapabilitiesMessage.clipboard is optional', () => {
        const ancien: CapabilitiesMessage = {
            v: CONTROL_VERSION,
            type: 'capabilities',
            gamepad: true,
        };
        expect(ancien.clipboard).toBeUndefined();
        // And reading it as a boolean gives `false` without anything to write.
        expect(Boolean(ancien.clipboard)).toBe(false);
    });
});
