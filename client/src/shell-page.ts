// Câblage de la page-shell : WebSocket du signaling d'un côté, DOM de
// l'autre. Aucune règle ici — elles sont dans `shell.ts`, qui est testé.

import { creerBureau } from './shell';

const params = new URLSearchParams(window.location.search);
const signalingUrl = params.get('signaling') ?? `ws://${window.location.hostname}:8080`;
// Identifiant réservé de la session de contrôle : le superviseur s'y déclare
// en `agent`, cette page en `client`.
const SESSION_DE_CONTROLE = 'bureau';

const statut = document.querySelector<HTMLDivElement>('#statut')!;
const liste = document.querySelector<HTMLUListElement>('#fenetres')!;

const socket = new WebSocket(signalingUrl);

const bureau = creerBureau({
    ouvrirFenetre(session) {
        return window.open(`/?session=${encodeURIComponent(session)}`, `guac-${session}`);
    },
    envoyer(message) {
        if (socket.readyState === WebSocket.OPEN) socket.send(JSON.stringify(message));
    },
    afficher(message) {
        statut.textContent = message;
    },
});

function redessiner(): void {
    liste.replaceChildren();
    for (const f of bureau.liste()) {
        const item = document.createElement('li');
        item.textContent = `${f.titre} — ${f.ouverte ? 'ouverte' : 'fermée'} `;
        if (!f.ouverte) {
            const bouton = document.createElement('button');
            bouton.textContent = 'Rouvrir';
            bouton.addEventListener('click', () => { bureau.rouvrir(f.session); redessiner(); });
            item.append(bouton);
        }
        liste.append(item);
    }
}

socket.addEventListener('open', () => {
    socket.send(JSON.stringify({ role: 'client', session: SESSION_DE_CONTROLE }));
    statut.textContent = 'bureau connecté';
});

socket.addEventListener('message', (evenement) => {
    const message = JSON.parse(evenement.data);
    if (message.type === 'fenetre-ouverte') bureau.fenetreOuverte(message.session, message.titre);
    else if (message.type === 'fenetre-fermee') bureau.fenetreFermee(message.session);
    else if (message.type === 'refus') bureau.refus(message.titre, message.motif);
    redessiner();
});

// Les pages d'application annoncent leur viewport par `postMessage` sur leur
// ouvreuse — c'est-à-dire ici.
window.addEventListener('message', (evenement) => {
    // Même origine seulement : cette page ouvre des fenêtres, elle ne doit
    // pas relayer ce que n'importe quel site lui enverrait.
    if (evenement.origin !== window.location.origin) return;
    const message = evenement.data;
    if (message?.type === 'viewport') {
        bureau.viewportRecu(message.session, message.largeur, message.hauteur);
    }
});

// La fermeture d'une page par l'utilisateur ne prévient personne : on relit
// l'état périodiquement plutôt que d'attendre un événement qui n'existe pas.
setInterval(redessiner, 1000);
