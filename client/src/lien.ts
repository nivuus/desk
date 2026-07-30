// Traduction de l'état du lien annoncé par l'agent en texte affichable.
//
// Séparé de tout DOM pour être testable, comme `status.ts` et `audio.ts`.
// L'agent décide ; ce module ne fait que dire, en français, ce qu'il a décidé.

import type { LinkMessage } from '../../proto/ts/control';

export interface TexteLien {
    resume: string;
    /// Vrai quand l'utilisateur doit être averti : image dégradée ou lien
    /// insuffisant. Une adaptation indisponible n'est PAS une alerte — le
    /// lien peut être excellent, seul l'asservissement manque.
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
