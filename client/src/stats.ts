// Measurement overlay. The values come from getStats(): they are the
// browser's own, not an estimate of ours.
//
// Rigour note on the "≈" column: it is an APPROXIMATION of the
// glass-to-glass latency, not a direct measurement. It equals half of the
// network round trip (RTT/2, assuming a symmetric path) plus the
// time spent in the receiver-side jitter buffer. It OMITS agent-side
// capture, hardware encoding, and browser-side decoding (getStats() does
// not give a timestamp crossing these stages). See the acceptance document
// for the independent measurement (timed visual pattern) that fills this gap.

/// A reading of the UPSTREAM track (the mic, workstream E).
export interface InstantaneMontant {
    octets: number;
    paquets: number;
    horodatage: number;
}

/// The mic line, and the snapshot to remember for the next round.
///
/// ⚠️ **`bytesSent` is a BYTE COUNT, and this repository knows a byte
/// count does not prove one hears anything.** Sub-block D7 recorded
/// a steadily growing `bytesReceived` on a spectrum at −1000 dB: the
/// track was alive, the sound was absent. These two numbers are displayed **to
/// DIAGNOSE** — knowing whether we emit, and at what rate —, **never to
/// judge an acceptance criterion.** The mic's judging figure is the dominant
/// frequency recorded on the agent side (`MICRO_MESURE`), not what is written here.
///
/// **`courant` absent = NO audio `outbound-rtp`**, which is not the same
/// thing as a zero rate: the first says nothing is negotiated or no
/// packet has gone out yet, the second that a track lives and stays silent. Confusing
/// them would pass a session without a mic for a silent mic —
/// exactly the distinction the `audioLine` line already makes for downstream
/// audio, and for the same reason.
export function suivreMontant(
    precedent: InstantaneMontant | undefined,
    courant: InstantaneMontant | undefined,
): { ligne: string; memoire: InstantaneMontant | undefined } {
    // The memory is FORGOTTEN when the track disappears: without that, a
    // renegotiated track (new SSRC, counters restarted from zero) would compute its
    // first rate against the counters of another stream.
    if (!courant) return { ligne: 'micro absent', memoire: undefined };

    let kbps = 0;
    if (precedent) {
        const secondes = (courant.horodatage - precedent.horodatage) / 1000;
        // `> 0` and not `!== 0`: a frozen timestamp would produce a division by
        // zero, a timestamp going backwards a negative rate.
        if (secondes > 0) {
            kbps = ((courant.octets - precedent.octets) * 8) / secondes / 1000;
        }
    }
    return {
        ligne: `micro ${kbps.toFixed(0)} kb/s  ·  paquets ${courant.paquets}`,
        memoire: courant,
    };
}

interface Snapshot {
    framesDecoded: number;
    bytesReceived: number;
    timestamp: number;
}

export function attachStats(pc: RTCPeerConnection, element: HTMLElement): () => void {
    let previous: Snapshot | undefined;
    let previousAudio: Snapshot | undefined;
    let precedentMontant: InstantaneMontant | undefined;

    const timer = window.setInterval(async () => {
        const report = await pc.getStats();
        let inbound: RTCInboundRtpStreamStats | undefined;
        let inboundAudio: RTCInboundRtpStreamStats | undefined;
        let outboundAudio: RTCOutboundRtpStreamStats | undefined;
        let pair: RTCIceCandidatePairStats | undefined;

        report.forEach((stat) => {
            if (stat.type === 'inbound-rtp' && (stat as any).kind === 'video') {
                inbound = stat as RTCInboundRtpStreamStats;
            }
            if (stat.type === 'inbound-rtp' && (stat as any).kind === 'audio') {
                inboundAudio = stat as RTCInboundRtpStreamStats;
            }
            if (stat.type === 'candidate-pair' && (stat as any).nominated) {
                pair = stat as RTCIceCandidatePairStats;
            }
            // The UPSTREAM track (workstream E). `kind === 'audio'` discriminates:
            // there is also a video `outbound-rtp` as soon as the agent emits, and
            // without this filter we would display its rate under the mic's name.
            if (stat.type === 'outbound-rtp' && (stat as any).kind === 'audio') {
                outboundAudio = stat as RTCOutboundRtpStreamStats;
            }
        });
        if (!inbound) return;

        const current: Snapshot = {
            framesDecoded: (inbound as any).framesDecoded ?? 0,
            bytesReceived: (inbound as any).bytesReceived ?? 0,
            timestamp: inbound.timestamp,
        };

        let fps = 0;
        let mbps = 0;
        if (previous) {
            const seconds = (current.timestamp - previous.timestamp) / 1000;
            if (seconds > 0) {
                fps = (current.framesDecoded - previous.framesDecoded) / seconds;
                mbps =
                    ((current.bytesReceived - previous.bytesReceived) * 8) / seconds / 1_000_000;
            }
        }
        previous = current;

        let audioKbps = 0;
        if (inboundAudio) {
            const currentAudio: Snapshot = {
                framesDecoded: 0,
                bytesReceived: (inboundAudio as any).bytesReceived ?? 0,
                timestamp: inboundAudio.timestamp,
            };
            if (previousAudio) {
                const seconds = (currentAudio.timestamp - previousAudio.timestamp) / 1000;
                if (seconds > 0) {
                    audioKbps =
                        ((currentAudio.bytesReceived - previousAudio.bytesReceived) * 8) /
                        seconds /
                        1000;
                }
            }
            previousAudio = currentAudio;
        } else {
            // Without this, an audio entry that disappears then comes back (renegotiated
            // track, SSRC changed) would compute its first rate after
            // the return against a stale `previousAudio`: counters of another
            // stream, a delta underestimated or outright negative if the
            // new `bytesReceived` restarts from zero.
            previousAudio = undefined;
        }

        const rttMs = (pair?.currentRoundTripTime ?? 0) * 1000;
        // Approximate end-to-end latency: half of the network round trip,
        // plus the wait in the jitter buffer and browser-side decoding.
        // Cf. the file header: this ignores agent capture and encoding.
        const jitterBufferMs = averageDelay(inbound);
        const glassToGlassMs = rttMs / 2 + jitterBufferMs;

        const width = (inbound as any).frameWidth ?? 0;
        const height = (inbound as any).frameHeight ?? 0;

        // `verify-webrtc.mjs` carefully distinguishes an absent audio `inbound-rtp`
        // entry (no negotiated track, or no first
        // packet yet) from a track present with zero loss/zero jitter: the two must
        // not produce the same text, otherwise a
        // session without audio would pass for a session whose audio is simply
        // perfect.
        const audioLine = inboundAudio
            ? `audio ${audioKbps.toFixed(0)} kb/s  ·  perdus ${(inboundAudio as any).packetsLost ?? 0}  ·  gigue ${(((inboundAudio as any).jitter ?? 0) * 1000).toFixed(1)} ms`
            : 'audio absente';

        const montant = suivreMontant(
            precedentMontant,
            outboundAudio
                ? {
                      octets: (outboundAudio as any).bytesSent ?? 0,
                      paquets: (outboundAudio as any).packetsSent ?? 0,
                      horodatage: outboundAudio.timestamp,
                  }
                : undefined,
        );
        precedentMontant = montant.memoire;

        element.textContent = [
            `${fps.toFixed(1)} i/s`,
            `${width}×${height}`,
            `${mbps.toFixed(2)} Mb/s`,
            `RTT ${rttMs.toFixed(1)} ms`,
            `tampon ${jitterBufferMs.toFixed(1)} ms`,
            `≈ ${glassToGlassMs.toFixed(1)} ms`,
            `perdues ${(inbound as any).framesDropped ?? 0}`,
            audioLine,
            montant.ligne,
        ].join('  ·  ');
    }, 1000);

    return () => window.clearInterval(timer);
}

/// Average delay spent in the jitter buffer, per emitted frame.
function averageDelay(inbound: RTCInboundRtpStreamStats): number {
    const total = (inbound as any).jitterBufferDelay ?? 0;
    const count = (inbound as any).jitterBufferEmittedCount ?? 0;
    return count > 0 ? (total / count) * 1000 : 0;
}
