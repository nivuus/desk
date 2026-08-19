// La page-shell, SCRIPTÉE — le pair humain de la corroboration sur VM réelle.
//
//     tsx shell-scripte.ts <url> <prefixe> <secret-jeton> <duree-secondes>
//
// ⚠️ CE N'EST PAS UN NAVIGATEUR, ET IL FAUT LE DIRE. La corroboration de la
// tâche 23 porte sur l'identité de l'AGENT — enrôlement, préfixe, jeton dans
// les deux poignées de main —, pas sur le chemin humain, que la recette du
// sous-bloc P2 a déjà éprouvé de bout en bout avec ses trois pilotes CDP. Ce
// pair-ci se contente donc de :
//   - signer LUI-MÊME un jeton d'utilisateur avec le secret de signature du
//     service, au lieu de passer par la route d'authentification ;
//   - rejoindre `<préfixe>:bureau` en rôle `client` ;
//   - répondre `viewport` à chaque `fenetre-ouverte`, faute de quoi le
//     superviseur ne lance AUCUN enfant (garde-fou `DELAI_ATTENTE_VIEWPORT_MAX`
//     du sous-bloc D3) ;
//   - rejoindre chaque session de fenêtre annoncée, pour que l'appariement s'y
//     produise réellement et que la plateforme en écrive la ligne.
//
// 🔴 AUCUNE NÉGOCIATION WebRTC N'EST FAITE. L'offre que l'enfant émet reste
// sans réponse : ce pair ne décode aucun média, et rien ici ne doit être lu
// comme une mesure de flux. Ce qui est établi est la POIGNÉE DE MAIN et
// l'appariement, ce qui est exactement l'objet de la tâche.

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

/// Le jeton HUMAIN, de type `utilisateur` — c'est ce que la garde exige du
/// rôle `client`, et un jeton d'agent y serait refusé (claim `sty`, tâche 9).
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
            // Une offre SDP fait des kilo-octets : on ne journalise que son
            // type et sa taille, sinon le journal devient illisible.
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
                    // ⚠️ Un temps de battement avant de rejoindre : le
                    // superviseur doit d'abord lancer l'enfant, qui se déclare
                    // `agent` sur cette session. Rejoindre AVANT lui n'est pas
                    // une erreur (le relais retient l'offre), mais l'ordre du
                    // produit est celui-ci.
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
