// The shell page, SCRIPTED — the human peer of the corroboration on a real VM.
//
//     tsx shell-scripte.ts <url> <prefixe> <secret-jeton> <duree-secondes>
//
// ⚠️ IT IS NOT A BROWSER, AND IT MUST BE SAID. The corroboration of
// task 23 bears on the AGENT's identity — enrolment, prefix, token in
// both handshakes —, not on the human path, which the acceptance run of
// sub-block P2 already tested end to end with its three CDP drivers. This
// peer therefore merely:
//   - signs a user token ITSELF with the service's signing
//     secret, instead of going through the authentication route;
//   - joins `<prefix>:bureau` in the `client` role;
//   - answers `viewport` to each `fenetre-ouverte`, otherwise the
//     supervisor launches NO child (the `DELAI_ATTENTE_VIEWPORT_MAX` safeguard
//     of sub-block D3);
//   - joins each announced window session, so that pairing really
//     happens there and the platform writes its line.
//
// 🔴 NO WebRTC NEGOTIATION IS DONE. The offer the child emits stays
// unanswered: this peer decodes no media, and nothing here must be read
// as a stream measurement. What is established is the HANDSHAKE and
// pairing, which is exactly the task's subject.

import { signer, DUREE_JETON_ACCES_MS } from '../../../../../plateforme/src/identite/jeton';

const [url, prefixe, secret, dureeBrute] = process.argv.slice(2);
if (!url || !prefixe || !secret) {
    throw new Error('usage : shell-scripte.ts <url> <prefixe> <secret-jeton> <duree-secondes>');
}
const dureeMs = Number(dureeBrute ?? '60') * 1000;
const debut = Date.now();

const journal: string[] = [];
function noter(texte: string): void {
    journal.push(`+${String(Date.now() - debut).padStart(6)} ms  ${texte}`);
}

/// The HUMAN token, of type `user` — it is what the guard requires of the
/// `client` role, and an agent token would be refused there (claim `sty`, task 9).
const jeton = signer('recette-p3', secret, Date.now(), DUREE_JETON_ACCES_MS, 'utilisateur');

const sessionsRejointes = new Set<string>();

function rejoindre(session: string, role: 'client'): Promise<WebSocket> {
    return new Promise((resolve, reject) => {
        const ws = new WebSocket(url);
        ws.addEventListener('open', () => {
            ws.send(JSON.stringify({ role, session, jeton }));
            noter(`-> poignée de main ${role} sur ${session}`);
            resolve(ws);
        });
        ws.addEventListener('error', () => reject(new Error(`connexion refusée : ${url}`)));
        ws.addEventListener('close', (e) => {
            const f = e as CloseEvent;
            noter(`<- socket fermé sur ${session} (code ${f.code})`);
        });
        ws.addEventListener('message', (e) => {
            const brut = String((e as MessageEvent).data);
            const message = JSON.parse(brut) as Record<string, unknown>;
            // An SDP offer is kilobytes long: we only log its
            // type and its size, otherwise the log becomes unreadable.
            const resume =
                message.type === 'offer' || message.type === 'answer'
                    ? `{"type":"${String(message.type)}","sdp":<${String(message.sdp).length} octets>}`
                    : brut;
            noter(`<- [${session}] ${resume}`);

            if (message.type === 'fenetre-ouverte') {
                const fenetre = String(message.session);
                ws.send(
                    JSON.stringify({
                        type: 'viewport',
                        session: fenetre,
                        largeur: 1280,
                        hauteur: 720,
                    }),
                );
                noter(`-> [${session}] viewport ${fenetre} 1280x720`);
                if (!sessionsRejointes.has(fenetre)) {
                    sessionsRejointes.add(fenetre);
                    // ⚠️ A beat before joining: the
                    // supervisor must first launch the child, which declares itself
                    // `agent` on this session. Joining BEFORE it is not
                    // an error (the relay holds the offer), but the product's order
                    // is this one.
                    setTimeout(() => {
                        void rejoindre(fenetre, 'client').catch((cause) =>
                            noter(`!! ${fenetre} : ${String(cause)}`),
                        );
                    }, 2000);
                }
            }
        });
    });
}

const controle = `${prefixe}:bureau`;
await rejoindre(controle, 'client');
await new Promise((r) => setTimeout(r, dureeMs));

process.stdout.write(
    [
        '# LA PAGE-SHELL SCRIPTÉE — journal des trames, côté humain',
        `# Session de contrôle : ${controle}`,
        `# Durée : ${dureeMs / 1000} s`,
        '#',
        `sessions de fenêtre rejointes : ${[...sessionsRejointes].join(', ') || '<aucune>'}`,
        '',
        ...journal,
        '',
    ].join('\n'),
);
process.exit(0);
