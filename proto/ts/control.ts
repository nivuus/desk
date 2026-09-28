// Control channel messages. Must stay aligned with proto/src/control.rs.
//
// Reliable, ordered, low-rate channel: versioned JSON, readable in the
// logs (unlike the binary input channel of proto/ts/input.ts).
// The `v` field is mandatory and checked at parse time: a message missing
// `v`, or whose version differs from CONTROL_VERSION, is rejected.

export const CONTROL_VERSION = 3;

/** Values of the CSS `cursor` property the agent knows how to produce. */
export type CursorShape =
    | 'default' | 'text' | 'wait' | 'progress' | 'crosshair' | 'pointer'
    | 'move' | 'not-allowed' | 'help'
    | 'ns-resize' | 'ew-resize' | 'nwse-resize' | 'nesw-resize';

export interface ResizeMessage {
    v: number;
    type: 'resize';
    width: number;
    height: number;
}

export interface VisibilityMessage {
    v: number;
    type: 'visibility';
    visible: boolean;
    focused: boolean;
}

/// The user pasted into the session window (sub-block P2).
///
/// ⚠️ **The name carries `Client` where its downstream twin carries `Agent`**
/// (`ClipboardAgentMessage`), and BOTH carry the same tag `'clipboard'`:
/// the direction tells them apart, not the tag. P1 had reserved this name with
/// its reason, two interfaces below; P2 takes it.
///
/// ⚠️ **`text` is a `string`, never `string | null`, and the asymmetry with
/// `ClipboardAgentMessage` is intended**: over there the `null` CARRIES the
/// size refusal, which the banner must show. Here the client bounds BEFORE
/// emitting — it has the banner at hand —, so it never has a refusal to
/// express in the message.
export interface ClipboardClientMessage {
    v: number;
    type: 'clipboard';
    text: string;
}

export type ClientControl = ResizeMessage | VisibilityMessage | ClipboardClientMessage;

export interface ReadyMessage {
    v: number;
    type: 'ready';
    width: number;
    height: number;
    /**
     * Is the mic available for this session (work item E)?
     *
     * OPTIONAL on purpose, and without a `CONTROL_VERSION` bump: facing an old
     * agent the field is `undefined`, hence falsy, hence no button is
     * offered — the rule of spec §10, obtained for free. Never make it
     * mandatory: that would be a compatibility break the
     * version number would not signal.
     */
    mic?: boolean;
}

export interface SessionEndMessage {
    v: number;
    type: 'session-end';
    reason: string;
}

export interface PointerMessage {
    v: number;
    type: 'pointer';
    visible: boolean;
    shape: CursorShape;
}

export interface RumbleMessage {
    v: number;
    type: 'rumble';
    left: number;
    right: number;
}

export interface CapabilitiesMessage {
    v: number;
    type: 'capabilities';
    gamepad: boolean;
    /// Is browser → VM paste available (sub-block P2)?
    ///
    /// **OPTIONAL, and that is the point**: an agent from before P2 does not carry this
    /// field, and `undefined` then equals `false` for free — the client
    /// arms nothing, exactly like `ReadyMessage.mic`. Making it mandatory
    /// would make `tsc` fail on a perfectly legitimate message.
    clipboard?: boolean;
}

export type LinkQuality = 'bonne' | 'degradee' | 'insuffisante';
export type LinkAdaptation = 'active' | 'indisponible';

export interface LinkMessage {
    v: number;
    type: 'link';
    bitrate: number;
    width: number;
    height: number;
    quality: LinkQuality;
    adaptation: LinkAdaptation;
}

export interface AsleepMessage {
    v: number;
    type: 'asleep';
    asleep: boolean;
    reason: string;
}

export interface FullscreenMessage {
    v: number;
    type: 'fullscreen';
    active: boolean;
}

/// The VM clipboard has changed.
///
/// The interface is called `ClipboardAgentMessage` and not `ClipboardMessage`
/// because it has a twin in the other direction, `ClipboardClientMessage`,
/// carrying the SAME tag `'clipboard'`. No real collision —
/// `parseAgentControl` only parses `AgentControl`, and a client message never
/// goes through there — but the two interfaces cannot carry the
/// same name.
///
/// ✅ **This paragraph said "while it is ALONE today: the
/// sub-block P2 WILL ADD a `ClipboardClientMessage` […] Naming this one
/// now spares P2 from renaming shipped code". P2 happened, and the bet
/// held: nothing was renamed.** Fixed by the cross-cutting review of 21 August
/// 2026 — a prediction that comes true stops being a prediction.
///
/// ⚠️ **`text` is `string | null`, never optional.** The agent always
/// emits it; a `?` would pass a message truncated in transit off as a refusal.
/// `null` IS the refusal, and `bytes` then carries its size.
export interface ClipboardAgentMessage {
    v: number;
    type: 'clipboard';
    text: string | null;
    bytes: number;
}

/// The accent colour of the Windows window — the dominant hue of its
/// icon (sub-block A1). Emitted **on change only**, and **its FIRST
/// reading included**: without it, `--accent-fenetre` would never be set for
/// the session.
///
/// ⚠️ **`couleur` is `#rrggbb`, lowercase, and the client DOES NOT
/// TRUST it**: `client/src/accent.ts::conformer` checks the shape **before**
/// calling `rapportDeContraste`, which **THROWS** on anything that is not
/// `#rgb`, `#rgba`, `#rrggbb` or `#rrggbbaa`. An exception in a
/// data channel message handler kills a session without a word.
///
/// ⚠️ **No `hwnd`, no PID, no window title** — see the doc of the
/// twin Rust variant: that is the lesson of the P2 clipboard leak,
/// and it applies **to the TYPE, not to the logging site**.
export interface AccentAgentMessage {
    v: number;
    type: 'accent';
    couleur: string;
}

/// Is the mic of THIS window heard by the VM? (block E3)
///
/// **Emitted ON TRANSITION, never on every write** — the mic writes a frame
/// every 20 ms, and the control channel is the *reliable, ordered,
/// low-rate* channel.
///
/// 🔴 **This message exists because `ReadyMessage.mic` CANNOT express it**
/// : `mic` is decided at setup, whereas exclusive ownership of the VM
/// cable is acquired at the first upstream packet. Without this variant, with two
/// windows **the losing one's button lights up and nothing comes out** — measured by
/// block E2, confirmed by reading the code in E3.
///
/// ⚠️ **`granted: true` DOES NOT MEAN "the mic is open"** — it is
/// `mic` and the button state that say so. Confusing the two would switch off
/// the button of a window whose browser is really emitting, which spec
/// §9 "Privacy" forbids: the Chrome indicator, for its part, stays lit.
///
/// 🔴 **AND IT DOES NOT MEAN "THIS MIC IS HEARD" EITHER.** This
/// sentence said "it means: what this mic picks up reaches the VM", and the
/// block E3 acceptance REFUTED it by arming `MICRO_FAUTE_ECRITURE`: the WASAPI
/// render thread dies (`micro : ecriture sur le cable echouee, fil de rendu
/// arrete`), the judge on CABLE Output records an amplitude of **0.000000**, and
/// the window still receives `granted: true` — the mutex lives in
/// `PuitsCable::deposer`, the render thread is elsewhere.
///
/// **What this field says exactly: "no OTHER window holds the VM
/// cable".** It is a verdict of EXCLUSIVITY, never an acknowledgement of
/// receipt. A `false` is therefore conclusive — someone else has it — whereas a
/// `true` is not: it rules out one cause of silence, it does not rule out
/// two others (the WASAPI failure, which `mic` does not see either once the
/// session is set up, and the mic simply closed).
export interface MicStateMessage {
    v: number;
    type: 'mic-state';
    granted: boolean;
}

export type AgentControl =
    | ReadyMessage | SessionEndMessage
    | PointerMessage | RumbleMessage | CapabilitiesMessage | LinkMessage
    | AsleepMessage | FullscreenMessage | ClipboardAgentMessage | AccentAgentMessage
    | MicStateMessage;

/// 🔴 Written as an EXHAUSTIVE record typed by the union, never as
/// a literal: adding a variant to `AgentControl` without adding its key
/// here makes `npm run typecheck` fail, because a `Record<K, true>` with
/// a missing key is a `tsc` error.
///
/// **Before this remedy, the omission broke NEITHER the build NOR any test.**
/// `parseAgentControl` threw, `client/src/webrtc.ts` caught it, and the
/// message was simply lost against a `console.warn` — an entirely silent
/// failure mode, in the file that defines the
/// protocol. It is the same remedy as the one from the app management work,
/// applied here at its source.
const ALL_AGENT: Record<AgentControl['type'], true> = {
    ready: true,
    'session-end': true,
    pointer: true,
    rumble: true,
    capabilities: true,
    link: true,
    asleep: true,
    fullscreen: true,
    clipboard: true,
    accent: true,
    'mic-state': true,
};

/// Exported so the derivation has a RUNTIME witness, and not only
/// a compile-time witness. A widening of surface, assumed and declared:
/// without it, `ALL_AGENT` is guarded only by `tsc`, and a test cannot
/// observe that the list and the union match.
export const TYPES_AGENT = Object.keys(ALL_AGENT) as AgentControl['type'][];

export function encodeResize(width: number, height: number): string {
    const message: ResizeMessage = {
        v: CONTROL_VERSION,
        type: 'resize',
        width: Math.max(1, Math.round(width)),
        height: Math.max(1, Math.round(height)),
    };
    return JSON.stringify(message);
}

export function encodeVisibility(visible: boolean, focused: boolean): string {
    const message: VisibilityMessage = {
        v: CONTROL_VERSION,
        type: 'visibility',
        visible,
        focused,
    };
    return JSON.stringify(message);
}

/// Encodes a paste coming from the browser.
///
/// 🔴 **There is deliberately NO `TYPES_CLIENT` to keep this union
/// exhaustive, and it is not an oversight.** `ALL_AGENT` exists because
/// `parseAgentControl` PARSES `AgentControl` on the TypeScript side: a forgotten
/// variant was lost there against a `console.warn`, silently. In this
/// direction there is nothing to parse — the client ENCODES, and it is `serde` that
/// deserialises on the Rust side, with an exhaustive `match` the compiler guards
/// (it in fact demanded it when the variant was born). An
/// exhaustiveness witness written here would be **unable to fail**, that is the
/// failure mode this repository has been paying for since D7.
export function encodeClipboard(text: string): string {
    const message: ClipboardClientMessage = {
        v: CONTROL_VERSION,
        type: 'clipboard',
        text,
    };
    return JSON.stringify(message);
}

/// ⚠️ **This parser validates ONLY `v` and `type`**, then CASTS. It looks at
/// no other field, and a test that claimed to check that it "accepts a
/// `capabilities` without `clipboard`" would thus be decorative: it could
/// not fail. It is the TYPING (`clipboard?: boolean`) that carries this
/// property, and an explicit annotation of `control.test.ts` that measures it —
/// whose red is `tsc`, never vitest, which transpiles through esbuild without
/// checking types.
export function parseAgentControl(raw: string): AgentControl {
    const parsed = JSON.parse(raw) as Partial<AgentControl>;
    if (parsed.v !== CONTROL_VERSION) {
        throw new Error(`unsupported control version: ${parsed.v}`);
    }
    if (!TYPES_AGENT.includes(parsed.type as AgentControl['type'])) {
        throw new Error(`unknown control type: ${parsed.type}`);
    }
    return parsed as AgentControl;
}
