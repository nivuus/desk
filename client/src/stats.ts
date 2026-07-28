// Overlay de mesure. Les valeurs proviennent de getStats() : ce sont celles du
// navigateur lui-même, pas une estimation de notre part.
//
// Note de rigueur sur la colonne « ≈ » : c'est une APPROXIMATION de la
// latence glass-to-glass, pas une mesure directe. Elle vaut la moitié de
// l'aller-retour réseau (RTT/2, en supposant un chemin symétrique) plus le
// temps passé en tampon de gigue côté récepteur. Elle OMET la capture côté
// agent, l'encodage matériel, et le décodage côté navigateur (getStats() ne
// donne pas d'horodatage traversant ces étages). Voir le document de recette
// pour la mesure indépendante (motif visuel chronométré) qui comble ce trou.

interface Snapshot {
    framesDecoded: number;
    bytesReceived: number;
    timestamp: number;
}

export function attachStats(pc: RTCPeerConnection, element: HTMLElement): () => void {
    let previous: Snapshot | undefined;
    let previousAudio: Snapshot | undefined;

    const timer = window.setInterval(async () => {
        const report = await pc.getStats();
        let inbound: RTCInboundRtpStreamStats | undefined;
        let inboundAudio: RTCInboundRtpStreamStats | undefined;
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
            // Sans ceci, une entrée audio qui disparaît puis revient (piste
            // renégociée, SSRC changé) calculerait son premier débit après
            // le retour contre un `previousAudio` périmé : compteurs d'un
            // autre flux, delta sous-estimé ou carrément négatif si le
            // nouveau `bytesReceived` repart de zéro.
            previousAudio = undefined;
        }

        const rttMs = (pair?.currentRoundTripTime ?? 0) * 1000;
        // Latence bout en bout approchée : la moitié de l'aller-retour réseau,
        // plus l'attente en tampon de gigue et le décodage côté navigateur.
        // Cf. l'en-tête du fichier : ceci ignore capture et encodage agent.
        const jitterBufferMs = averageDelay(inbound);
        const glassToGlassMs = rttMs / 2 + jitterBufferMs;

        const width = (inbound as any).frameWidth ?? 0;
        const height = (inbound as any).frameHeight ?? 0;

        // `verify-webrtc.mjs` distingue soigneusement une entrée `inbound-rtp`
        // audio absente (aucune piste négociée, ou pas encore de premier
        // paquet) d'une piste présente à zéro perte/zéro gigue : les deux ne
        // doivent pas produire le même texte, sous peine de faire passer une
        // session sans audio pour une session dont l'audio est simplement
        // parfait.
        const audioLine = inboundAudio
            ? `audio ${audioKbps.toFixed(0)} kb/s  ·  perdus ${(inboundAudio as any).packetsLost ?? 0}  ·  gigue ${(((inboundAudio as any).jitter ?? 0) * 1000).toFixed(1)} ms`
            : 'audio absente';

        element.textContent = [
            `${fps.toFixed(1)} i/s`,
            `${width}×${height}`,
            `${mbps.toFixed(2)} Mb/s`,
            `RTT ${rttMs.toFixed(1)} ms`,
            `tampon ${jitterBufferMs.toFixed(1)} ms`,
            `≈ ${glassToGlassMs.toFixed(1)} ms`,
            `perdues ${(inbound as any).framesDropped ?? 0}`,
            audioLine,
        ].join('  ·  ');
    }, 1000);

    return () => window.clearInterval(timer);
}

/// Délai moyen passé en tampon de gigue, par image émise.
function averageDelay(inbound: RTCInboundRtpStreamStats): number {
    const total = (inbound as any).jitterBufferDelay ?? 0;
    const count = (inbound as any).jitterBufferEmittedCount ?? 0;
    return count > 0 ? (total / count) * 1000 : 0;
}
