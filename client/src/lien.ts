// Translating the link state announced by the agent into displayable text.
//
// Separated from any DOM to be testable, like `status.ts` and `audio.ts`.
// The agent decides; this module only says, in plain words, what it decided.

import type { LinkMessage } from '../../proto/ts/control';

export interface TexteLien {
    resume: string;
    /// True when the user must be warned: degraded image or insufficient
    /// link. An unavailable adaptation is NOT an alert — the
    /// link can be excellent, only the control loop is missing.
    alerte: boolean;
}

export function texteLien(message: LinkMessage): TexteLien {
    const mbps = (message.bitrate / 1_000_000).toFixed(1);
    const taille = `${message.width}×${message.height}`;

    if (message.quality === 'insuffisante') {
        return {
            resume: `Réseau insuffisant pour le jeu nerveux — ${taille}, ${mbps} Mb/s`,
            alerte: true,
        };
    }
    if (message.quality === 'degradee') {
        return {
            resume: `Image réduite par le réseau — ${taille}, ${mbps} Mb/s`,
            alerte: true,
        };
    }
    if (message.adaptation === 'indisponible') {
        return {
            resume: `${taille}, ${mbps} Mb/s — adaptation indisponible`,
            alerte: false,
        };
    }
    return { resume: `${taille}, ${mbps} Mb/s`, alerte: false };
}
