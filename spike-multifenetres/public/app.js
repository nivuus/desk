// Pilotage des quatre variantes du spike.
//
// Invariant central : une variante n'est JAMAIS déclenchée par le clic sur son
// propre bouton. Le bouton arme la variante ; l'ordre d'ouverture arrive ensuite
// par le WebSocket, soit après le compte à rebours, soit via POST /fire depuis un
// autre poste. C'est toute la condition testée.

import { classer, SEUIL_ACTIVATION_MS } from './lib/classify.js';

const DELAI_ARMEMENT_MS = 15000;   // > SEUIL_ACTIVATION_MS, avec marge confortable
const DELAI_SIGNAL_VIE_MS = 3000;  // au-delà, la fenêtre est réputée perdue

const journal = document.getElementById('journal');
const etat = document.getElementById('etat');

let dernierGeste = 0;
let armementVariante3 = null;
const vies = new Map();

// Tout geste utilisateur est horodaté : c'est ce qui permettra d'affirmer, chiffre
// à l'appui, que l'activation transitoire avait bien expiré au moment du open().
for (const evenement of ['pointerdown', 'keydown']) {
    window.addEventListener(evenement, () => {
        dernierGeste = Date.now();
        if (evenement === 'pointerdown' && armementVariante3) {
            const executer = armementVariante3;
            armementVariante3 = null;
            executer();
        }
    }, true);
}

function tracer(texte, niveau = 'info') {
    const ligne = document.createElement('div');
    ligne.className = `ligne ${niveau}`;
    ligne.textContent = `${new Date().toLocaleTimeString('fr-FR')} — ${texte}`;
    journal.prepend(ligne);
    console.log(`[spike] ${texte}`);
}

// Attend le signal de vie de la variante, ou renonce après DELAI_SIGNAL_VIE_MS.
function attendreVie(variante) {
    return new Promise((resolve) => {
        vies.set(variante, resolve);
        setTimeout(() => {
            if (vies.delete(variante)) resolve(false);
        }, DELAI_SIGNAL_VIE_MS);
    });
}

async function conclure(variante, poigneeNulle, gesteAttendu = false) {
    const msDepuisGeste = Date.now() - dernierGeste;
    const vivante = await attendreVie(variante);
    const verdict = classer({ poigneeNulle, vivante, msDepuisGeste, gesteAttendu });

    const niveau = verdict === 'succes' ? 'succes'
        : verdict === 'non-concluant' ? 'alerte' : 'echec';
    tracer(
        `variante ${variante} → ${verdict.toUpperCase()} ` +
        `(poignée ${poigneeNulle === 'sans-objet' ? 'sans objet' : poigneeNulle ? 'nulle' : 'rendue'}, ` +
        `vie ${vivante ? 'reçue' : 'absente'}, ${msDepuisGeste} ms depuis le dernier geste)`,
        niveau,
    );
    document.querySelector(`#verdict-${variante}`).textContent = verdict;
}

function ouvrir(variante) {
    const poignee = window.open(`/opened.html?variant=${variante}`, `spike-${variante}`);
    return poignee === null;
}

async function executerVariante(variante) {
    switch (variante) {
        case 1:
        case 2:
            // 1 = onglet normal (témoin), 2 = PWA installée. Même code : c'est le
            // contexte d'exécution qui diffère, pas l'appel.
            await conclure(variante, ouvrir(variante));
            break;

        case 3:
            tracer('variante 3 armée — cliquez n\'importe où pour déclencher', 'alerte');
            armementVariante3 = () => conclure(3, ouvrir(3), true);
            break;

        case 4: {
            const enregistrement = await navigator.serviceWorker.getRegistration();
            if (!enregistrement) {
                tracer('variante 4 impossible : aucun service worker enregistré', 'echec');
                return;
            }
            if (Notification.permission !== 'granted') {
                tracer('variante 4 impossible : permission notifications non accordée', 'echec');
                return;
            }
            await enregistrement.showNotification('Le jeu est prêt', {
                body: 'Cliquez pour ouvrir sa fenêtre (variante 4).',
                data: { url: '/opened.html?variant=4' },
                tag: 'spike-4',
            });
            tracer('notification affichée — cliquez dessus, fenêtre en arrière-plan', 'alerte');
            await conclure(4, 'sans-objet');
            break;
        }
    }
}

const socket = new WebSocket(`${location.protocol === 'https:' ? 'wss' : 'ws'}://${location.host}/ws`);

socket.addEventListener('open', () => {
    etat.textContent = 'connecté';
    etat.className = 'connecte';
});

socket.addEventListener('close', () => {
    etat.textContent = 'déconnecté';
    etat.className = 'deconnecte';
});

socket.addEventListener('message', (evenement) => {
    const message = JSON.parse(evenement.data);
    if (message.type === 'fire') {
        tracer(`ordre reçu du serveur pour la variante ${message.variant}`);
        executerVariante(message.variant);
    } else if (message.type === 'alive') {
        const resolveur = vies.get(message.variant);
        if (resolveur) {
            vies.delete(message.variant);
            resolveur(true);
        }
    }
});

// Armement : le bouton ne déclenche rien lui-même, il demande au serveur de
// déclencher plus tard. Le compte à rebours dépasse SEUIL_ACTIVATION_MS.
for (const bouton of document.querySelectorAll('[data-variante]')) {
    bouton.addEventListener('click', () => {
        const variante = Number(bouton.dataset.variante);
        let restant = Math.ceil(DELAI_ARMEMENT_MS / 1000);
        bouton.disabled = true;
        tracer(
            `variante ${variante} armée : déclenchement dans ${restant} s ` +
            `(> ${SEUIL_ACTIVATION_MS / 1000} s d'activation transitoire). Ne touchez à rien.`,
        );

        const compteur = setInterval(() => {
            restant -= 1;
            bouton.textContent = `${bouton.dataset.libelle} — ${restant} s`;
            if (restant <= 0) {
                clearInterval(compteur);
                bouton.disabled = false;
                bouton.textContent = bouton.dataset.libelle;
                fetch('/fire', {
                    method: 'POST',
                    headers: { 'content-type': 'application/json' },
                    body: JSON.stringify({ variant: variante }),
                }).catch((erreur) => tracer(`échec du déclenchement : ${erreur}`, 'echec'));
            }
        }, 1000);
    });
}

// Enregistrement du service worker : requis pour l'installabilité PWA (variantes
// 2 et 3) et pour la variante 4. Le fichier sw.js n'est PAS couvert par les
// exceptions de politique Pomerium (qui ne visent que manifest.json, .ico et
// .png) : son chargement dépend donc du cookie de session. Un échec ici est un
// résultat du spike, pas un incident — il est tracé comme tel.
if ('serviceWorker' in navigator) {
    navigator.serviceWorker.register('/sw.js')
        .then((enregistrement) => tracer(`service worker enregistré (portée ${enregistrement.scope})`))
        .catch((erreur) => tracer(`service worker refusé : ${erreur} — vérifier la politique Pomerium`, 'echec'));
}

tracer(`mode d'affichage : ${matchMedia('(display-mode: standalone)').matches ? 'standalone (PWA installée)' : 'onglet navigateur'}`);
