// Parsing the messages the signalling relay sends to a client.
//
// Moved out of `webrtc.ts` so that the handshake guard (`poignee-de-main.ts`)
// can read them without importing the session module that uses the guard.

/// Subset of the signaling messages expected in answer to the offer,
/// discriminated by `type`.
export type SignalingMessage =
    | { type: 'answer'; sdp: string }
    | { type: 'error'; reason?: string; motif?: string }
    | { type: 'peer-gone' }
    | { type: 'ice-config'; iceServers: RTCIceServer[] };

/// Parses and validates a raw signaling message.
///
/// `JSON.parse` succeeds on payloads that are not objects:
/// `"null"` gives `null`, but also `"42"` gives a number, `'"x"'` a
/// string, `"[1,2]"` an array. Accessing `.type` on one of these values
/// does not always throw (an array or a string do have an `undefined`
/// `.type`, no exception), but `null.type` throws an uncaught
/// `TypeError` — that is the defect fixed here. We therefore explicitly validate
/// that the result is a non-null, non-array object before any property
/// read, whatever the shape of the payload.
///
/// Returns `undefined` if the message is unreadable or of an unexpected shape:
/// the caller then silently ignores it, without crashing the wait
/// (the awaited message — the SDP answer — may still arrive afterwards).
// Used by `webrtc.ts` (`waitForAnswer`, `attendreConfigIce`) and by the
// handshake guard (`poignee-de-main.ts`); tested in `webrtc.test.ts`.
export function parseSignalingMessage(raw: string): SignalingMessage | undefined {
    let parsed: unknown;
    try {
        parsed = JSON.parse(raw);
    } catch {
        return undefined;
    }
    if (typeof parsed !== 'object' || parsed === null || Array.isArray(parsed)) {
        return undefined;
    }
    const { type } = parsed as Record<string, unknown>;
    if (type === 'answer' || type === 'error' || type === 'peer-gone' || type === 'ice-config') {
        return parsed as SignalingMessage;
    }
    return undefined;
}
