// Client of the host control socket (`console`, `nivuus-vm-control.socket`).
//
// The protocol is one line: send a verb, receive `ok` or `err <cause>`.
// No verb takes an argument — the VM is the host's.
//
// 🔴 THE PROMISE NEVER REJECTS. A missing, refused or silent socket is a
// value (`{ ok:false, cause }`) that the caller transforms into a typed refusal:
// an exception would return 500 where the service must admit "the host is not
// responding".

import { connect } from 'node:net';

export const VERBES_HOTE = ['wake', 'busy'] as const;
export type VerbeHote = (typeof VERBES_HOTE)[number];

export type ReponseHote = { ok: true } | { ok: false; cause: string };

/// Bound of a request. ⚠️ NOT TUNED: `wake` only requests startup
/// (`--no-block` on the host side), so the response arrives in a few
/// milliseconds; five seconds cover a loaded host.
export const DELAI_CANAL_MS = 5_000;

export function envoyerAuCanal(
    chemin: string,
    verbe: VerbeHote,
    delaiMs: number = DELAI_CANAL_MS,
): Promise<ReponseHote> {
    return new Promise((resoudre) => {
        const socket = connect(chemin);
        let recu = '';
        let fini = false;

        const terminer = (reponse: ReponseHote): void => {
            if (fini) return;
            fini = true;
            socket.destroy();
            resoudre(reponse);
        };

        socket.setEncoding('utf8');
        socket.setTimeout(delaiMs, () => terminer({ ok: false, cause: 'delai' }));
        socket.on('connect', () => socket.write(`${verbe}\n`));
        socket.on('data', (morceau: string) => {
            recu += morceau;
            const fin = recu.indexOf('\n');
            if (fin === -1) return;
            const ligne = recu.slice(0, fin).trim();
            if (ligne === 'ok') return terminer({ ok: true });
            terminer({
                ok: false,
                cause: ligne.startsWith('err ') ? ligne.slice(4) : 'reponse-inattendue',
            });
        });
        socket.on('error', (e: NodeJS.ErrnoException) =>
            terminer({ ok: false, cause: e.code ?? 'erreur-socket' }),
        );
        socket.on('close', () => terminer({ ok: false, cause: 'ferme-sans-reponse' }));
    });
}
