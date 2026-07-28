// Pilotage des quatre variantes du spike.
//
// Invariant central : une variante n'est JAMAIS déclenchée par le clic sur son
// propre bouton. Le bouton arme la variante ; l'ordre d'ouverture arrive ensuite
// par le WebSocket, soit après le compte à rebours, soit via POST /fire depuis un
// autre poste. C'est toute la condition testée.
//
// Second principe, qui explique la plupart des gardes ci-dessous : cet instrument
// doit refuser de mesurer plutôt que mesurer faux. Un verdict absent se relance ;
// un verdict faux se propage jusqu'à la décision produit.

import { classer, SEUIL_ACTIVATION_MS } from './lib/classify.js';

const DELAI_ARMEMENT_MS = 15000;   // > SEUIL_ACTIVATION_MS, avec marge confortable

// Deux chronomètres, deux phénomènes sans rapport malgré la même unité : celui
// des variantes 1 à 3 mesure un chargement de page (quasi instantané), celui de
// la variante 4 mesure un temps de réaction humain face à une notification. Un
// délai unique de 3 s ferait expirer `attendreIssue` avant même que l'utilisateur
// ait vu la notification, et figerait la variante 4 sur un faux
// « ouverte-mais-perdue » systématique — alors qu'elle est le repli documenté
// du cadrage produit.
const DELAI_SIGNAL_VIE_MS = 3000;      // variantes 1 à 3 : au-delà, la fenêtre est réputée perdue
const DELAI_SIGNAL_VIE_V4_MS = 60000;  // variante 4 : attend un clic humain sur la notification

// L'armement de la variante 3 attend un clic quelconque. Sans expiration, il
// survivrait au renoncement de l'opérateur et se déclencherait des minutes plus
// tard sur un clic sans rapport, produisant un verdict attribué au mauvais geste.
const DELAI_EXPIRATION_V3_MS = 30000;

const RECONNEXION_MIN_MS = 500;
const RECONNEXION_MAX_MS = 15000;

const journal = document.getElementById('journal');
const etat = document.getElementById('etat');
const boutons = [...document.querySelectorAll('[data-variante]')];

let dernierGeste = 0;
let armementVariante3 = null;
let expirationVariante3 = null;

// Une variante en cours ne doit pas être relancée en parallèle : `src/index.ts`
// documente un déclenchement par curl et le serveur diffuse à TOUS les clients,
// donc un onglet resté ouvert à côté de la fenêtre PWA reçoit le même ordre.
const variantesEnCours = new Set();

// Attentes indexées par nonce de passage, jamais par numéro de variante : deux
// passages successifs de la même variante ont des identités distinctes, et le
// minuteur de l'un ne peut plus supprimer l'attente de l'autre.
const attentes = new Map();

// Poignée du dernier essai, pour refermer la fenêtre précédente avant le suivant
// (voir `ouvrir`).
let dernierePoignee = null;

// `crypto.randomUUID` n'existe que dans un contexte sécurisé. Le spike est servi
// en HTTPS derrière Pomerium, mais un essai local sur http://<ip-lan>:3445 ferait
// planter la page au premier armement — un instrument qui ne démarre pas est
// encore une mesure perdue.
function nouveauNonce() {
    if (globalThis.crypto?.randomUUID) return crypto.randomUUID().replaceAll('-', '');
    return `${Date.now().toString(36)}${Math.random().toString(36).slice(2, 12)}`;
}

function modeAffichage() {
    return matchMedia('(display-mode: standalone)').matches ? 'standalone' : 'onglet';
}

function tracer(texte, niveau = 'info') {
    const ligne = document.createElement('div');
    ligne.className = `ligne ${niveau}`;
    ligne.textContent = `${new Date().toLocaleTimeString('fr-FR')} — ${texte}`;
    journal.prepend(ligne);
    console.log(`[spike] ${texte}`);
}

// Retire l'armement de la variante 3 et rend l'action qui y était attachée, sans
// l'exécuter : à l'appelant de décider s'il déclenche (clic) ou abandonne
// (expiration).
function desarmerVariante3() {
    const executer = armementVariante3 ?? (() => {});
    armementVariante3 = null;
    clearTimeout(expirationVariante3);
    expirationVariante3 = null;
    return executer;
}

// Tout geste utilisateur est horodaté : c'est ce qui permettra d'affirmer, chiffre
// à l'appui, que l'activation transitoire avait bien expiré au moment du open().
for (const evenement of ['pointerdown', 'keydown']) {
    window.addEventListener(evenement, (donnees) => {
        dernierGeste = Date.now();
        if (evenement !== 'pointerdown' || !armementVariante3) return;
        // Les boutons de la page sont des commandes de l'instrument, pas le clic
        // testé. Sans cette garde, un clic sur « Armer 4 » exécuterait d'abord la
        // variante 3, lui attribuerait un verdict, et laisserait une fenêtre
        // ouverte de plus.
        if (donnees.target instanceof Element && donnees.target.closest('button')) return;
        desarmerVariante3()();
    }, true);
}

// Attend l'issue du passage : signal de vie, blocage rapporté par le service
// worker, ou silence au bout du délai applicable.
function attendreIssue(nonce, variante) {
    const delai = variante === 4 ? DELAI_SIGNAL_VIE_V4_MS : DELAI_SIGNAL_VIE_MS;
    return new Promise((resolve) => {
        attentes.set(nonce, (issue) => {
            attentes.delete(nonce);
            resolve(issue);
        });
        setTimeout(() => {
            if (attentes.delete(nonce)) resolve('silence');
        }, delai);
    });
}

async function conclure(variante, nonce, poigneeNulle, gesteAttendu = false) {
    const msDepuisGeste = Date.now() - dernierGeste;
    const issue = await attendreIssue(nonce, variante);

    // Le blocage rapporté par le service worker (variante 4) vaut poignée nulle :
    // c'est exactement ce que `poigneeNulle` représente pour les autres variantes.
    // `classer()` n'a donc pas à connaître ce nouveau canal.
    const poignee = issue === 'bloquee' ? true : poigneeNulle;
    const vivante = issue === 'vivante';
    const verdict = classer({ poigneeNulle: poignee, vivante, msDepuisGeste, gesteAttendu });

    const niveau = verdict === 'succes' ? 'succes'
        : verdict === 'non-concluant' ? 'alerte' : 'echec';
    const etatVie = vivante ? 'reçue'
        : issue === 'bloquee' ? 'sans objet (blocage rapporté par le service worker)'
        : 'absente';
    tracer(
        `variante ${variante} → ${verdict.toUpperCase()} ` +
        `(mode ${modeAffichage()}, ` +
        `poignée ${poignee === 'sans-objet' ? 'sans objet' : poignee ? 'nulle' : 'rendue'}, ` +
        `vie ${etatVie}, ${msDepuisGeste} ms depuis le dernier geste)`,
        niveau,
    );
    document.querySelector(`#verdict-${variante}`).textContent = verdict;
}

// La fenêtre est ouverte SANS nom. Un nom (`spike-3`) fait naviguer une fenêtre
// déjà ouverte du même nom au lieu d'en créer une : le bloqueur de popups n'est
// alors jamais consulté, la poignée revient non nulle et `opened.html` renvoie
// son signal de vie — un « succès » qui n'a rien testé. Le déroulé nominal y
// menait, `opened.html` ne se fermant pas de lui-même. La poignée précédente est
// en outre refermée avant chaque essai, pour ne pas laisser s'accumuler des
// fenêtres que l'opérateur confondrait avec le résultat de l'essai en cours.
function ouvrir(variante, nonce) {
    if (dernierePoignee && !dernierePoignee.closed) dernierePoignee.close();
    const poignee = window.open(`/opened.html?variant=${variante}&nonce=${nonce}`);
    dernierePoignee = poignee;
    return poignee === null;
}

// Les variantes 1 et 2 partagent le même code : seul le contexte les distingue,
// et le contexte se vérifie au moment de la mesure, pas au chargement de la page.
// La variante 1 est le témoin — si elle tourne par mégarde dans la PWA installée
// (le cas naturel : on installe la PWA pour la variante 2, puis on enchaîne
// depuis cette fenêtre), un succès ferait conclure « le test est faux » et
// invaliderait tout l'instrument alors que seul le contexte était mauvais.
function contexteValide(variante) {
    const mode = modeAffichage();
    if (variante === 1 && mode === 'standalone') {
        tracer(
            'variante 1 refusée : elle est le témoin et doit tourner dans un onglet ' +
            'ordinaire. Rouvrez le spike dans un onglet du navigateur (hors fenêtre PWA), ' +
            'puis rejouez-la depuis cet onglet.',
            'echec',
        );
        return false;
    }
    if (variante === 2 && mode !== 'standalone') {
        tracer(
            'variante 2 refusée : elle mesure la PWA installée. Installez le spike ' +
            '(menu du navigateur → Installer), ouvrez-le depuis son icône, puis rejouez-la ' +
            'depuis cette fenêtre-là.',
            'echec',
        );
        return false;
    }
    return true;
}

async function executerVariante(variante) {
    if (variantesEnCours.has(variante)) {
        tracer(
            `ordre ignoré : la variante ${variante} est déjà en cours dans cette page ` +
            '(le même ordre a été relayé deux fois — autre onglet du spike, ou curl)',
            'alerte',
        );
        return;
    }
    variantesEnCours.add(variante);
    try {
        await deroulerVariante(variante);
    } finally {
        variantesEnCours.delete(variante);
    }
}

async function deroulerVariante(variante) {
    const nonce = nouveauNonce();

    switch (variante) {
        case 1:
        case 2:
            if (!contexteValide(variante)) return;
            await conclure(variante, nonce, ouvrir(variante, nonce));
            break;

        case 3:
            tracer(
                `variante 3 armée — cliquez ailleurs que sur un bouton pour déclencher ` +
                `(abandon automatique dans ${DELAI_EXPIRATION_V3_MS / 1000} s)`,
                'alerte',
            );
            // L'attente est tenue ici pour que `variantesEnCours` couvre aussi la
            // fenêtre d'armement : sans cela un second ordre réarmerait la
            // variante 3 par-dessus le premier.
            await new Promise((resolve) => {
                armementVariante3 = () => conclure(3, nonce, ouvrir(3, nonce), true).then(resolve);
                expirationVariante3 = setTimeout(() => {
                    desarmerVariante3();
                    tracer('variante 3 : armement expiré sans clic, aucun verdict rendu', 'alerte');
                    resolve();
                }, DELAI_EXPIRATION_V3_MS);
            });
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
                data: { url: `/opened.html?variant=4&nonce=${nonce}` },
                tag: 'spike-4',
            });
            tracer(
                `notification affichée — cliquez dessus ` +
                `(${DELAI_SIGNAL_VIE_V4_MS / 1000} s pour réagir, rien ne presse)`,
                'alerte',
            );
            await conclure(4, nonce, 'sans-objet');
            break;
        }
    }
}

// --- Liaison WebSocket -------------------------------------------------------

let socket = null;
let tentativesReconnexion = 0;

function majEtatBoutons() {
    const connecte = socket?.readyState === WebSocket.OPEN;
    for (const bouton of boutons) {
        // Un bouton en compte à rebours reste désactivé quoi qu'il arrive.
        bouton.disabled = !connecte || bouton.dataset.arme === 'oui';
    }
}

// Le nonce est vérifié avant de résoudre : `/alive` est un simple GET, et le
// serveur diffuse à tous les clients. Une fenêtre restée ouverte puis rechargée,
// un second poste, ou n'importe quelle page tierce visitée pendant le test
// pouvaient jusqu'ici résoudre l'attente en cours — et pour la variante 4, où la
// poignée est « sans objet », un `alive` étranger suffisait à produire un succès.
function resoudrePassage(message, issue) {
    const resolveur = message.nonce ? attentes.get(message.nonce) : undefined;
    if (!resolveur) {
        tracer(
            `signal « ${issue === 'vivante' ? 'vie' : 'blocage'} » ignoré pour la variante ` +
            `${message.variant} : il n'appartient à aucun passage en cours dans cette page`,
            'alerte',
        );
        return;
    }
    resolveur(issue);
}

function afficherDeconnexion(texte) {
    etat.textContent = texte;
    etat.className = 'deconnecte';
    majEtatBoutons();
}

// Sans socket, POST /fire répond quand même — le serveur n'a simplement aucun
// auditeur. Le bouton se réactiverait comme après un succès et la mesure serait
// silencieusement vide. D'où la reconnexion automatique, et l'interdiction
// d'armer tant que la liaison est rompue.
function connecter() {
    socket = new WebSocket(`${location.protocol === 'https:' ? 'wss' : 'ws'}://${location.host}/ws`);

    socket.addEventListener('open', () => {
        tentativesReconnexion = 0;
        etat.textContent = 'connecté';
        etat.className = 'connecte';
        majEtatBoutons();
    });

    // `error` est toujours suivi de `close` : la reconnexion n'est planifiée que
    // dans `close`, pour ne pas doubler les tentatives.
    socket.addEventListener('error', () => afficherDeconnexion('déconnecté'));

    socket.addEventListener('close', () => {
        afficherDeconnexion('déconnecté');
        planifierReconnexion();
    });

    socket.addEventListener('message', (evenement) => {
        const message = JSON.parse(evenement.data);
        switch (message.type) {
            case 'fire':
                tracer(`ordre reçu du serveur pour la variante ${message.variant}`);
                executerVariante(message.variant);
                break;
            case 'alive':
                resoudrePassage(message, 'vivante');
                break;
            case 'bloque':
                resoudrePassage(message, 'bloquee');
                break;
        }
    });
}

// Repli exponentiel borné : la coupure la plus probable est une temporisation de
// Pomerium ou une veille du poste, dont on revient sans intervention.
function planifierReconnexion() {
    const delai = Math.min(RECONNEXION_MIN_MS * 2 ** tentativesReconnexion, RECONNEXION_MAX_MS);
    tentativesReconnexion += 1;
    afficherDeconnexion(`déconnecté — reconnexion dans ${Math.max(1, Math.round(delai / 1000))} s`);
    setTimeout(connecter, delai);
}

// --- Armement ----------------------------------------------------------------

// Armement : le bouton ne déclenche rien lui-même, il demande au serveur de
// déclencher plus tard. Le compte à rebours dépasse SEUIL_ACTIVATION_MS.
for (const bouton of boutons) {
    bouton.disabled = true;   // levé à l'ouverture du WebSocket
    bouton.addEventListener('click', () => {
        const variante = Number(bouton.dataset.variante);
        let restant = Math.ceil(DELAI_ARMEMENT_MS / 1000);
        bouton.dataset.arme = 'oui';
        majEtatBoutons();

        // La consigne propre à la variante 4 est donnée à l'armement et non après
        // l'affichage de la notification : c'est pendant le compte à rebours que
        // l'opérateur doit basculer la fenêtre en arrière-plan, et c'est cet
        // arrière-plan qui définit la variante.
        const consigne = variante === 4
            ? 'Ne touchez ni souris ni clavier, à une exception près : mettez cette ' +
              'fenêtre en arrière-plan avant la fin du compte à rebours — c\'est ce que ' +
              'la variante 4 mesure.'
            : 'Ne touchez à rien.';
        tracer(
            `variante ${variante} armée : déclenchement dans ${restant} s ` +
            `(> ${SEUIL_ACTIVATION_MS / 1000} s d'activation transitoire). ${consigne}`,
        );

        const compteur = setInterval(async () => {
            restant -= 1;
            bouton.textContent = `${bouton.dataset.libelle} — ${restant} s`;
            if (restant > 0) return;

            clearInterval(compteur);
            delete bouton.dataset.arme;
            bouton.textContent = bouton.dataset.libelle;
            majEtatBoutons();
            try {
                const reponse = await fetch('/fire', {
                    method: 'POST',
                    headers: { 'content-type': 'application/json' },
                    body: JSON.stringify({ variant: variante }),
                });
                if (!reponse.ok) throw new Error(`HTTP ${reponse.status}`);
                const { clients } = await reponse.json();
                if (!clients) {
                    tracer(
                        `déclenchement perdu : le serveur n'a touché aucun client WebSocket. ` +
                        `Aucune mesure — attendez la reconnexion et rejouez la variante ${variante}.`,
                        'echec',
                    );
                }
            } catch (erreur) {
                tracer(`échec du déclenchement : ${erreur}`, 'echec');
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

tracer(`mode d'affichage : ${modeAffichage() === 'standalone' ? 'standalone (PWA installée)' : 'onglet navigateur'}`);
connecter();
