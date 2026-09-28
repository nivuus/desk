// The fake objects of workstream E, **shared** by the mic's two test
// files.
//
// ⚠️ **EXTRACTION, not a rework.** `client/src/micro.test.ts` was at
// **411** lines; the test family of block E3 brought it to **515**, hence
// BEYOND the 500 gate. The repository's doctrine is to extract, never to
// compress — this repository crossed that ceiling five times and caught it twice
// through a compression it forbids itself.
//
// 🔴 **The crossing happened BEFORE being seen.** The plan budgeted "~50"
// lines for this file; the family cost **104**, and its gate table
// said "⚠️ to remeasure before writing". The lesson is not "estimate
// better": it is **remeasure after writing**, which no estimate
// replaces.
//
// ⚠️ **The content is VERBATIM.** Only visibility changed — the five
// functions became `export` —, which is the only thing an
// extraction is allowed to change.

/// Fake track: what this module does with a `MediaStreamTrack` comes down to
/// `stop()`, and it is precisely the call spec §9 makes mandatory.
/// `arretee` is the only instrument able to distinguish a REAL switch-off
/// from an `enabled = false` — the "visual lie" the spec rules out.
export function faussePiste(): MediaStreamTrack & { arretee: boolean; enabled: boolean } {
    return {
        kind: 'audio',
        enabled: true,
        arretee: false,
        stop(this: { arretee: boolean }) {
            this.arretee = true;
        },
    } as unknown as MediaStreamTrack & { arretee: boolean; enabled: boolean };
}

/// Fake stream carrying a single audio track, as `getUserMedia` returns one.
export function fauxFlux(piste: MediaStreamTrack): MediaStream {
    return { getAudioTracks: () => [piste] } as unknown as MediaStream;
}

/// Fake sender: remembers EVERYTHING passed to it, in order. A
/// "received a track" boolean would not distinguish a switch-off from an
/// absence of switch-on.
export function fauxSender() {
    const recus: Array<MediaStreamTrack | null> = [];
    return {
        recus,
        replaceTrack(piste: MediaStreamTrack | null): Promise<void> {
            recus.push(piste);
            return Promise.resolve();
        },
    };
}

/// Error as `getUserMedia` throws it: it is the `name` that carries the meaning,
/// never the message.
export function domError(name: string): Error {
    const e = new Error(name);
    e.name = name;
    return e;
}

/// Fake button: what the module writes on it, and nothing else. A real
/// `HTMLButtonElement` would require a DOM, which `client/src` avoids everywhere through
/// dependency injection (`audio.ts`, `fullscreen.ts`, `visibilite.ts`).
export function fauxBouton() {
    const ecouteurs = new Map<string, Set<EventListener>>();
    return {
        hidden: true,
        disabled: false,
        title: '',
        dataset: {} as { etat?: string },
        addEventListener(type: string, e: EventListener) {
            if (!ecouteurs.has(type)) ecouteurs.set(type, new Set());
            ecouteurs.get(type)!.add(e);
        },
        removeEventListener(type: string, e: EventListener) {
            ecouteurs.get(type)?.delete(e);
        },
        cliquer() {
            for (const e of [...(ecouteurs.get('click') ?? [])]) e(new Event('click'));
        },
        listenerCount: () => (ecouteurs.get('click')?.size ?? 0),
    };
}
