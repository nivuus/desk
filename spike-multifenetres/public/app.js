// Driving of the four variants of the spike.
//
// Central invariant: a variant is NEVER triggered by the click on its
// own button. The button arms the variant; the open order arrives afterwards
// through the WebSocket, either after the countdown, or via POST /fire from
// another machine. That is the whole condition under test.
//
// Second principle, which explains most of the guards below: this instrument
// must refuse to measure rather than measure wrong. A missing verdict is rerun;
// a wrong verdict propagates all the way to the product decision.

import { classer, SEUIL_ACTIVATION_MS } from './lib/classify.js';

const DELAI_ARMEMENT_MS = 15000;   // > SEUIL_ACTIVATION_MS, with a comfortable margin

// Two timers, two unrelated phenomena despite the same unit: the one
// for variants 1 to 3 measures a page load (near instantaneous), the one for
// variant 4 measures a human reaction time to a notification. A
// single 3 s delay would make `attendreIssue` expire before the user
// has even seen the notification, and would freeze variant 4 on a systematic false
// "opened-but-lost" — whereas it is the documented fallback
// of the product framing.
const DELAI_SIGNAL_VIE_MS = 3000;      // variants 1 to 3: beyond this, the window is deemed lost
const DELAI_SIGNAL_VIE_V4_MS = 60000;  // variant 4: waits for a human click on the notification

// Arming variant 3 waits for any click. Without expiry, it
// would survive the operator giving up and would fire minutes
// later on an unrelated click, producing a verdict attributed to the wrong gesture.
const DELAI_EXPIRATION_V3_MS = 30000;

const RECONNEXION_MIN_MS = 500;
const RECONNEXION_MAX_MS = 15000;

const journal = document.getElementById('journal');
const etat = document.getElementById('etat');
const boutons = [...document.querySelectorAll('[data-variante]')];

let dernierGeste = 0;
let armementVariante3 = null;
let expirationVariante3 = null;

// A running variant must not be restarted in parallel: `src/index.ts`
// documents a trigger through curl and the server broadcasts to ALL clients,
// so a tab left open next to the PWA window receives the same order.
const variantesEnCours = new Set();

// Waits indexed by run nonce, never by variant number: two
// successive runs of the same variant have distinct identities, and the
// timer of one can no longer remove the wait of the other.
const attentes = new Map();

// Handle of the last trial, to close the previous window before the next one
// (see `ouvrir`).
let dernierePoignee = null;

// `crypto.randomUUID` only exists in a secure context. The spike is served
// over HTTPS behind Pomerium, but a local trial on http://<lan-ip>:3445 would
// crash the page at the first arming — an instrument that does not start is
// yet another lost measurement.
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

// Removes the arming of variant 3 and returns the action attached to it, without
// running it: up to the caller to decide whether it fires (click) or gives up
// (expiry).
function desarmerVariante3() {
    const executer = armementVariante3 ?? (() => {});
    armementVariante3 = null;
    clearTimeout(expirationVariante3);
    expirationVariante3 = null;
    return executer;
}

// Every user gesture is timestamped: that is what will allow asserting, with figures
// in hand, that the transient activation had indeed expired at the time of open().
for (const evenement of ['pointerdown', 'keydown']) {
    window.addEventListener(evenement, (donnees) => {
        dernierGeste = Date.now();
        if (evenement !== 'pointerdown' || !armementVariante3) return;
        // The page buttons are commands of the instrument, not the click
        // under test. Without this guard, a click on "Arm 4" would first run
        // variant 3, attribute a verdict to it, and leave one more window
        // open.
        if (donnees.target instanceof Element && donnees.target.closest('button')) return;
        desarmerVariante3()();
    }, true);
}

// Waits for the outcome of the run: a sign of life, a block reported by the service
// worker, or silence at the end of the applicable delay.
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

    // The block reported by the service worker (variant 4) counts as a null handle:
    // that is exactly what `poigneeNulle` represents for the other variants.
    // `classer()` thus does not need to know about this new channel.
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

// The window is opened WITHOUT a name. A name (`spike-3`) navigates an
// already open window of the same name instead of creating one: the popup blocker is
// then never consulted, the handle comes back non-null and `opened.html` sends back
// its sign of life — a "success" that tested nothing. The nominal flow led
// there, `opened.html` not closing by itself. The previous handle is
// moreover closed before each trial, so as not to let windows pile up
// that the operator would confuse with the result of the current trial.
function ouvrir(variante, nonce) {
    if (dernierePoignee && !dernierePoignee.closed) dernierePoignee.close();
    const poignee = window.open(`/opened.html?variant=${variante}&nonce=${nonce}`);
    dernierePoignee = poignee;
    return poignee === null;
}

// Variants 1 and 2 share the same code: only the context tells them apart,
// and the context is checked at measurement time, not at page load.
// Variant 1 is the control — if it runs by mistake in the installed PWA
// (the natural case: we install the PWA for variant 2, then carry on
// from that window), a success would lead to concluding "the test is wrong" and
// would invalidate the whole instrument while only the context was wrong.
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
            // The wait is held here so that `variantesEnCours` also covers the
            // arming window: without it a second order would re-arm
            // variant 3 on top of the first.
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
        // A button in countdown stays disabled whatever happens.
        bouton.disabled = !connecte || bouton.dataset.arme === 'oui';
    }
}

// The nonce is checked before resolving: `/alive` is a plain GET, and the
// server broadcasts to all clients. A window left open then reloaded,
// a second machine, or any third-party page visited during the test
// could until now resolve the current wait — and for variant 4, where the
// handle is "not applicable", a foreign `alive` was enough to produce a success.
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

// Without a socket, POST /fire still answers — the server simply has no
// listener. The button would re-enable as after a success and the measurement would be
// silently empty. Hence the automatic reconnection, and the ban on
// arming while the link is broken.
function connecter() {
    socket = new WebSocket(`${location.protocol === 'https:' ? 'wss' : 'ws'}://${location.host}/ws`);

    socket.addEventListener('open', () => {
        tentativesReconnexion = 0;
        etat.textContent = 'connecté';
        etat.className = 'connecte';
        majEtatBoutons();
    });

    // `error` is always followed by `close`: reconnection is only scheduled
    // in `close`, so as not to double the attempts.
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

// Bounded exponential backoff: the most likely cut is a Pomerium timeout
// or the machine going to sleep, from which we come back without intervention.
function planifierReconnexion() {
    const delai = Math.min(RECONNEXION_MIN_MS * 2 ** tentativesReconnexion, RECONNEXION_MAX_MS);
    tentativesReconnexion += 1;
    afficherDeconnexion(`déconnecté — reconnexion dans ${Math.max(1, Math.round(delai / 1000))} s`);
    setTimeout(connecter, delai);
}

// --- Armement ----------------------------------------------------------------

// Arming: the button triggers nothing itself, it asks the server to
// trigger later. The countdown exceeds SEUIL_ACTIVATION_MS.
for (const bouton of boutons) {
    bouton.disabled = true;   // lifted when the WebSocket opens
    bouton.addEventListener('click', () => {
        const variante = Number(bouton.dataset.variante);
        let restant = Math.ceil(DELAI_ARMEMENT_MS / 1000);
        bouton.dataset.arme = 'oui';
        majEtatBoutons();

        // The instruction specific to variant 4 is given at arming and not after
        // the notification is shown: it is during the countdown that
        // the operator must send the window to the background, and it is that
        // background state that defines the variant.
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

// Service worker registration: required for PWA installability (variants
// 2 and 3) and for variant 4. The sw.js file is NOT covered by the
// Pomerium policy exceptions (which only target manifest.json, .ico and
// .png): its loading thus depends on the session cookie. A failure here is a
// result of the spike, not an incident — it is traced as such.
if ('serviceWorker' in navigator) {
    navigator.serviceWorker.register('/sw.js')
        .then((enregistrement) => tracer(`service worker enregistré (portée ${enregistrement.scope})`))
        .catch((erreur) => tracer(`service worker refusé : ${erreur} — vérifier la politique Pomerium`, 'echec'));
}

tracer(`mode d'affichage : ${modeAffichage() === 'standalone' ? 'standalone (PWA installée)' : 'onglet navigateur'}`);
connecter();
