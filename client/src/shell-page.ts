// Câblage de la page-shell : WebSocket du signaling d'un côté, DOM de
// l'autre. Aucune règle ici — elles sont dans `shell.ts`, qui est testé.

import { creerBureau, type Ton } from './shell';
import { installerSelecteurDeThemeAuDOM } from './design/selecteur-theme';
import { jetonAcces } from './jeton';
import { composer, lirePrefixe } from './prefixe';
import { creerAdaptateur } from './fichiers/adaptateur';
import { creerEcrivain, type RacineInscriptible } from './fichiers/ecriture';
import { creerServeur } from './fichiers/protocole';
import { choisirDossier, connecterCanalFichiers, sessionDuPont, type CanalFichiers } from './fichiers/canal';
import { adresseSignaling } from './adresse-plateforme';

const params = new URLSearchParams(window.location.search);
// 🔴 L'ADRESSE SUIT LE PROTOCOLE ET LE PORT DE LA PAGE, elle n'est plus le
// littéral `ws://<hôte>:8080` — qui était du contenu mixte derrière le proxy
// TLS, donc refusé par le navigateur sans qu'aucun test Node ne le voie.
// `?signaling=` reste prioritaire, pour les essais locaux.
const signalingUrl = adresseSignaling(window.location, params.get('signaling'));
// Nom réservé de la session de contrôle : le superviseur s'y déclare en
// `agent`, cette page en `client`.
//
// ⚠️ CE N'EST PLUS UN IDENTIFIANT DE SESSION À LUI SEUL (sous-bloc P3) : il
// est précédé du préfixe de la VM, sans quoi deux VMs ouvriraient toutes deux
// `bureau` et la seconde serait refusée. Sans préfixe connu, la composition
// rend `bureau` — exactement le nom d'avant P3.
const NOM_SESSION_DE_CONTROLE = 'bureau';
const prefixe = lirePrefixe();
const SESSION_DE_CONTROLE = composer(prefixe, NOM_SESSION_DE_CONTROLE);

const statut = document.querySelector<HTMLDivElement>('#statut')!;
const liste = document.querySelector<HTMLUListElement>('#fenetres')!;
const boutonDossier = document.querySelector<HTMLButtonElement>('#choisir-dossier')!;
const etatFichiers = document.querySelector<HTMLDivElement>('#etat-fichiers')!;
const ecrituresDues = document.querySelector<HTMLDivElement>('#ecritures-dues')!;
const modeleFenetre = document.querySelector<HTMLTemplateElement>('#modele-fenetre')!;

// Le sélecteur de thème du produit (spec §5.2, famille 3). Il n'y a AUCUNE
// règle ici non plus : le module pose ses trois boutons et gère le multi-
// fenêtres, et il est testé.
installerSelecteurDeThemeAuDOM(document.querySelector<HTMLElement>('#themes')!);

/* ── LE TON D'UN BANDEAU : UNE TABLE, PAS UNE RÈGLE ───────────────────────
   QUEL ton porte quel message est décidé dans `shell.ts`, qui est testé. Ce
   qui suit ne fait que traduire un ton en classe de la famille `message` —
   une correspondance, sans aucune décision de domaine. Une condition sur le
   SENS d'un message qui apparaîtrait ici serait au mauvais endroit.

   ⚠️ CES TROIS CLASSES SONT INVISIBLES AU CONTRÔLE §7.9, et c'est une limite
   connue et déclarée de ce contrôle, pas un contournement : il ne voit que les
   littéraux passés à `classList.add('…')` et à `className = '…'`, jamais une
   classe qui transite par une variable. Elles sont bien DÉCLARÉES par
   `design/primitives/message.css` et bien EMPLOYÉES par `primitives.html`, si
   bien qu'aucune n'est morte — mais c'est la galerie et l'œil qui le disent
   ici, pas la commande. */
const CLASSE_DE_TON: Record<Ton, string> = {
    neutre: '',
    succes: 'message--succes',
    alerte: 'message--alerte',
    danger: 'message--danger',
};

function poserTon(element: HTMLElement, ton: Ton): void {
    element.classList.remove('message--succes', 'message--alerte', 'message--danger');
    const classe = CLASSE_DE_TON[ton];
    if (classe !== '') element.classList.add(classe);
}

// 🔴 C'EST ICI, ET NULLE PART AILLEURS, QUE L'ON REDIRIGE VERS LA CONNEXION.
// Cette page est l'entrée réelle de l'utilisateur ; les pages de session, elle
// les ouvre elle-même, sur la même origine, avec le jeton déjà posé. Sans
// jeton, ouvrir le socket ne mènerait qu'à un refus de la garde
// (`plateforme/src/identite/garde.ts`), et l'utilisateur n'aurait aucun moyen
// de savoir quoi faire.
const jeton = jetonAcces();
if (jeton === undefined) {
    // `replace` et non `href` : un retour arrière ramènerait sur cette page
    // qui redirigerait de nouveau, et l'utilisateur serait piégé dans
    // l'historique.
    window.location.replace('connexion.html?suite=shell.html');
}

const socket = new WebSocket(signalingUrl);

const bureau = creerBureau({
    ouvrirFenetre(session) {
        return window.open(`/?session=${encodeURIComponent(session)}`, `guac-${session}`);
    },
    envoyer(message) {
        if (socket.readyState === WebSocket.OPEN) socket.send(JSON.stringify(message));
    },
    afficher(message, ton) {
        statut.textContent = message;
        poserTon(statut, ton);
    },
    afficherEtatFichiers(texte, ton) {
        etatFichiers.textContent = texte;
        poserTon(etatFichiers, ton);
    },
    afficherEcrituresDues(dues, vues, texte, ton) {
        // 🔴 LES NOMBRES VONT DANS DES ATTRIBUTS `data-*`, LE TEXTE DANS LA
        // PAGE. Le pilote de recette lit `data-dues` et `data-vues`, JAMAIS le
        // texte — piège de F1, payé neuf minutes sur deux messages qui
        // partageaient une sous-chaîne.
        ecrituresDues.dataset.dues = String(dues);
        ecrituresDues.dataset.vues = String(vues);
        ecrituresDues.textContent = texte;
        poserTon(ecrituresDues, ton);
    },
});

/* ── `beforeunload`, ET SES TROIS LIMITES ─────────────────────────────────
   ⛔ ① LE MESSAGE PERSONNALISÉ EST IGNORÉ par tous les navigateurs modernes :
      ils n'affichent qu'un libellé générique de leur choix. La spec §6.2
      demande « un texte qui nomme les fichiers » — CE TEXTE N'EXISTE PAS. Les
      fichiers sont nommés DANS LA PAGE, à côté du compteur.
   ⛔ ② IL NE SE DÉCLENCHE PAS DU TOUT si l'onglet est tué par le gestionnaire
      de tâches, si le navigateur plante, si la machine s'éteint, ou si l'onglet
      est écarté faute de mémoire.
   ⛔ ③ IL NE PEUT RIEN VIDER. L'événement est SYNCHRONE, et un envoi sur un
      `RTCDataChannel` amorcé dedans n'a aucune garantie de partir.

   🔴 `beforeunload` AVERTIT ; IL NE SAUVE PAS. Ce qui sauve est le journal du
   pont, et lui seul.

   ⚠️ Il exige en outre une ACTIVATION UTILISATEUR PERSISTANTE pour afficher son
   dialogue. Elle est acquise : monter le lecteur passe par un clic. */
window.addEventListener('beforeunload', (evenement) => {
    // La RÈGLE est dans `shell.ts`, qui est testé ; ici il n'y a que le câblage.
    if (!bureau.doitPrevenir()) return;
    evenement.preventDefault();
});

/* ── LE LECTEUR « MES FICHIERS » ──────────────────────────────────────────
   Aucune règle ici non plus : les messages sont dans `shell.ts`, la logique de
   protocole dans `fichiers/protocole.ts` et `fichiers/adaptateur.ts`, tous
   trois testés. Ce bloc n'est que du câblage, comme le reste de ce fichier. */

let pont: CanalFichiers | null = null;

boutonDossier.addEventListener('click', () => {
    // 🔴 `showDirectoryPicker()` EXIGE UNE ACTIVATION UTILISATEUR TRANSITOIRE,
    // et c'est pourquoi il est appelé ici, dans le gestionnaire de clic, et
    // jamais depuis un message de canal. Le gestionnaire n'est pas `async` : un
    // `await` avant l'appel consommerait l'activation, et le sélecteur serait
    // refusé sans que rien ne le dise. Même contrainte que `window.open()`, que
    // cette page connaît déjà.
    void monterLeLecteur();
});

async function monterLeLecteur(): Promise<void> {
    // Un second clic remplace le dossier : l'ancien pont part d'abord, sans
    // quoi deux `PeerConnection` se disputeraient la session `…:fichiers` et
    // la seconde serait refusée par le relais.
    pont?.close();
    pont = null;
    bureau.lecteurDemonte();

    const choix = await choisirDossier().catch((e: unknown) => {
        bureau.lecteurEchoue((e as Error).message);
        return undefined;
    });
    // `null` = annulation délibérée, `undefined` = échec déjà signalé.
    if (choix === null || choix === undefined) return;

    // 🔴 LA MÊME POIGNÉE SERT À LIRE ET À ÉCRIRE, et le transtypage est le
    // CONTRÔLE DE COMPATIBILITÉ STRUCTURELLE de F2 : `RacineInscriptible`
    // décrit un sous-ensemble de `FileSystemDirectoryHandle`, et si la vraie
    // poignée cessait de le satisfaire, `tsc --noEmit` le dirait ICI plutôt
    // qu'en session réelle.
    const racineInscriptible: RacineInscriptible = choix.racine as RacineInscriptible;
    const ecrivain = creerEcrivain(racineInscriptible);
    const serveur = creerServeur(creerAdaptateur(choix.racine), (m) => console.warn(m), {
        ecrivain,
        onDues: (dues) => bureau.ecrituresDues(dues),
        onEchecEcriture: (chemin, code) => bureau.ecritureEchouee(chemin, code),
    });
    try {
        pont = await connecterCanalFichiers({
            signalingUrl,
            sessionId: sessionDuPont(),
            onStatus: (m) => console.info(m),
            traiter: (octets) => serveur.traiter(octets),
        });
    } catch (e) {
        bureau.lecteurEchoue((e as Error).message);
        return;
    }
    bureau.lecteurMonte(choix.nom);

    // ⚠️ LE DÉMONTAGE SUIT LA CONNEXION, PAS LE CANAL SEUL : un canal fermé sur
    // une connexion qui se rétablit serait rouvert par l'agent, alors qu'une
    // connexion `failed` ou `closed` est définitive pour ce pont-ci.
    //
    // ⚠️ LE PONT COURANT EST CAPTURÉ, et l'écouteur se tait s'il n'est plus
    // celui-là. Sans cette garde, la fermeture de l'ANCIEN pont — que le
    // remontage vient de provoquer — effacerait l'état du NOUVEAU : un
    // écouteur qui survit à son objet est le patron exact d'une course qu'on
    // ne voit qu'en cliquant deux fois.
    const ce = pont;
    ce.pc.addEventListener('connectionstatechange', () => {
        if (pont !== ce) return;
        const etat = ce.pc.connectionState;
        if (etat === 'failed' || etat === 'closed') bureau.lecteurDemonte();
    });
    // ⚠️ **LES FLUX OUVERTS SE FERMENT AVEC LE CANAL.** Un flux
    // `createWritable()` laissé ouvert garde son fichier d'échange, et son
    // fichier de destination reste INCHANGÉ — la committaison est au `close()`.
    ce.canal.addEventListener('close', () => ecrivain.abandonner());
}

function redessiner(): void {
    liste.replaceChildren();
    for (const f of bureau.liste()) {
        // Le balisage vient du `<template>` de `shell.html`, pas d'ici : les
        // classes restent dans le HTML, où le contrôle §7.9 les lit sans avoir
        // à analyser du TypeScript.
        const item = modeleFenetre.content.cloneNode(true) as DocumentFragment;
        item.querySelector('[data-titre]')!.textContent = f.titre;

        const pastille = item.querySelector<HTMLElement>('[data-etat]')!;
        pastille.textContent = f.ouverte ? 'ouverte' : 'fermée';
        // Deux littéraux, et non une classe composée : une classe calculée est
        // invisible au contrôle §7.9 (voir la table des tons ci-dessus).
        if (f.ouverte) pastille.classList.add('bureau__pastille--ouverte');
        else pastille.classList.add('bureau__pastille--fermee');

        const bouton = item.querySelector<HTMLButtonElement>('[data-rouvrir]')!;
        if (f.ouverte) {
            // Une fenêtre ouverte n'a rien à rouvrir : le bouton part, plutôt
            // que d'être désactivé — il n'y a pas d'action à suggérer.
            bouton.remove();
        } else {
            bouton.addEventListener('click', () => { bureau.rouvrir(f.session); redessiner(); });
        }
        liste.append(item);
    }
}

socket.addEventListener('open', () => {
    // Le champ est AJOUTÉ, aucun n'est retiré (spec §10.2) — même règle que
    // `webrtc.ts`, et pour la même raison de compatibilité descendante.
    socket.send(JSON.stringify({ role: 'client', session: SESSION_DE_CONTROLE, jeton }));
    statut.textContent = 'bureau connecté';
    poserTon(statut, 'neutre');
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
