// THE CHANNEL'S BACKPRESSURE — the other half of F3's flow control.
// **PURE**: the channel is INJECTED into it, described by what we use of it, and this
// module knows neither `RTCDataChannel`, nor the DOM, nor the frame.
//
// ════════════════════════════════════════════════════════════════════════════
// 🔴 WHY IT IS HERE AND NOT IN THE BRIDGE
// ════════════════════════════════════════════════════════════════════════════
//
// Spec §7.3 states: "the bridge does not request chunk n+1 as long as the channel
// has more than `SEUIL_TAMPON` bytes pending". But **it is the BROWSER that
// emits the large messages** — the bytes of a read file —, and
// `bufferedAmount` is a property of ITS channel. **The bridge does not see it and
// cannot see it.**
//
// The two halves are inseparable, and F3 delivers **both or neither**:
// the bridge's window (`agent/src/pont/lecture.rs`) without backpressure
// would fill the SCTP queue; backpressure without the window would have **nothing
// to hold back**, since the bridge would never request chunk n+1 before
// receiving n.
//
// ⚠️ **FOUND IN F1/F2'S CODE**: `canal.ts` set **no**
// `bufferedAmountLowThreshold` — it only passed `{ ordered: true }` — and
// sent without looking at anything, whereas spec §3.4 requires it ("set").
//
// ⚠️ **F3 CLAIMS NO THROUGHPUT GAIN.** The repository's only throughput measurement
// varied by a factor of ~120 without explanation (F1 §11).
//
// ✅ **F4 HAS JUDGED.** The channel sustains ~30 to 33 KiB/s, linearly, and a
// REREAD does not cross the bridge at all (ProjFS hydration serves alone).
// 🔴 **And the backpressure set up here has never served in operation**: the
// only read that would reach `MORCEAUX_EN_VOL = 4` (256 KiB) fails on the
// `DELAI_LIRE` budget, its four chunks sharing those 33 KiB/s. See
// `docs/…/2026-08-21-pont-fichiers-f4-resultats.md`.

/**
 * The subset of an `RTCDataChannel` that backpressure uses.
 *
 * ⚠️ **A STRUCTURAL SUBSET**, like the handles of `adaptateur.ts`: the
 * real class satisfies it without conversion (`canal.ts` checks it at
 * compile time), and an in-memory fake does too. That is what makes this module
 * testable under Vitest's Node, where `RTCDataChannel` does not exist.
 */
export interface CanalSortant {
    readonly bufferedAmount: number;
    bufferedAmountLowThreshold: number;
    readonly readyState: 'connecting' | 'open' | 'closing' | 'closed';
    addEventListener(type: 'bufferedamountlow' | 'close', ecouteur: () => void): void;
    removeEventListener(type: 'bufferedamountlow' | 'close', ecouteur: () => void): void;
}

/**
 * How many bytes may wait in the channel's buffer before we stop
 * emitting.
 *
 * ⚠️ **NOT CALIBRATED.** It joins `MORCEAUX_EN_VOL`, `DELAI_MUTATION`,
 * `PERIODE_RECENSEMENT`, the four of F1, those of F2 and the eight of workstream
 * D in the list of constants no measurement has judged.
 *
 * **Why this order of magnitude, and it is a REASONING, not a measurement**:
 * `MORCEAUX_EN_VOL` (4) × `TAILLE_TRAME_MAX` (64 KiB) = 256 KiB of answers in
 * flight at most. The threshold is set at a quarter, so that the buffer drains
 * before the window is full — otherwise backpressure would
 * **never** bite, and would be a mechanism unable to trigger.
 */
export const SEUIL_TAMPON = 64 * 1024;

export interface ContrePression {
    /**
     * Waits for the buffer to have dropped back below the threshold.
     *
     * 🔴 **RETURNS IMMEDIATELY if the channel is CLOSED**, and never suspends:
     * a closed channel will never again emit `bufferedamountlow`, and the wait would
     * therefore **never** end — a blockage WORSE than the one being fixed,
     * since it would freeze the page instead of slowing a transfer.
     */
    avantEnvoi(): Promise<void>;
}

export function contrePression(canal: CanalSortant, seuil = SEUIL_TAMPON): ContrePression {
    // ⚠️ **The threshold is set ONCE, at construction.** Setting it at each
    // send would be one write per frame on a browser object, and
    // changing it along the way would make an already armed `bufferedamountlow`
    // fire on the old value.
    canal.bufferedAmountLowThreshold = seuil;
    return {
        async avantEnvoi(): Promise<void> {
            if (canal.readyState !== 'open') return;
            if (canal.bufferedAmount <= seuil) return;
            await new Promise<void>((resolve) => {
                // ⚠️ **BOTH LISTENERS ARE REMOVED, whichever one
                // wins.** Leaving them would be exactly the defect found in
                // the old bridge: a `message` listener set PER REQUEST and
                // never removed (`src/file.js:155`), whose cost grew
                // with the number of past operations, indefinitely.
                const finir = (): void => {
                    canal.removeEventListener('bufferedamountlow', finir);
                    canal.removeEventListener('close', finir);
                    resolve();
                };
                canal.addEventListener('bufferedamountlow', finir);
                // 🔴 **`close` RELEASES TOO**, otherwise an ongoing read
                // would freeze the page when the remote tab closes.
                canal.addEventListener('close', finir);
                // ⚠️ **CHECK AGAIN AFTER SUBSCRIBING.** The buffer may have
                // dropped between the test above and the subscription:
                // the event would then already have passed, and the wait would
                // never end. It is the classic race of any
                // "test then wait" mechanism.
                if (canal.bufferedAmount <= seuil || canal.readyState !== 'open') finir();
            });
        },
    };
}
