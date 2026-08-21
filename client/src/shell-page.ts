// Câblage de la page-shell : WebSocket du signaling d'un côté, DOM de
// l'autre. Aucune règle ici — elles sont dans `shell.ts`, qui est testé.

import { creerBureau, type Ton } from './shell';
import { installerSelecteurDeThemeAuDOM } from './design/selecteur-theme';
import { jetonAcces } from './jeton';
import { composer, lirePrefixe } from './prefixe';
import { creerAdaptateur } from './fichiers/adaptateur';
import { creerEcrivain, type RacineInscriptible } from './fichiers/ecriture';
import { creerMutateur } from './fichiers/mutation-service';
import type { RacineMutable } from './fichiers/mutation';
import { creerServeur, trameBonjour, trameRafraichir } from './fichiers/protocole';
import { choisirDossier, connecterCanalFichiers, sessionDuPont, type CanalFichiers } from './fichiers/canal';
import { adressePlateforme, adresseSignaling } from './adresse-plateforme';

const params = new URLSearchParams(window.location.search);
// 🔴 L'ADRESSE SUIT LE PROTOCOLE ET LE PORT DE LA PAGE, elle n'est plus le
// littéral `ws://<hôte>:8080` — qui était du contenu mixte derrière le proxy
// TLS, donc refusé par le navigateur sans qu'aucun test Node ne le voie.
// `?signaling=` reste prioritaire, pour les essais locaux.
const signalingUrl = adresseSignaling(window.location, params.get('signaling'));

/**
 * L'injection de fautes du pont fichiers est-elle ARMÉE ?
 *
 * 🔴 **LUE UNE FOIS, ICI, ET PASSÉE EN ARGUMENT.** La lire depuis
 * `fichiers/noms.ts` le rendrait intestable — et surtout : un utilisateur qui
 * créerait un dossier nommé `.faute-disque-plein` casserait son propre pont.
 * C'est la convention de `PLEIN_ECRAN` et de `PART_SONDAGE` côté agent : le
 * mécanisme lit un drapeau qu'on lui donne, jamais l'environnement.
 *
 * 🔴 **VARIABLE DE BANC, jamais une configuration livrée.** Elle rend
 * atteignables les quatre causes du §5 qu'AUCUN geste réel ne peut produire sur
 * ce montage : `acces-refuse` (OPFS n'a aucun modèle de permission),
 * `disque-plein` (`QuotaExceededError` n'y est pas provocable) et
 * `delai-depasse` (il faudrait un navigateur qui ne réponde jamais).
 *
 * ⚠️ **UNE INJECTION PROUVE QUE LA TABLE N'EST PAS DÉCORATIVE ; ELLE NE PROUVE
 * PAS QUE LA CAUSE EST ATTEIGNABLE EN EXPLOITATION.** Les deux colonnes sont
 * distinguées au §0.5 du plan de F3, et le document de résultats les garde
 * distinctes.
 */
const fautesFichiersArmees = params.get('faute-fichiers') === '1';
if (fautesFichiersArmees) {
    console.warn(
        'injection de fautes du pont fichiers ARMEE (?faute-fichiers=1) : ' +
            'banc, jamais une configuration livree',
    );
}
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
const actionsFichiers = document.querySelector<HTMLParagraphElement>('#actions-fichiers')!;
const boutonRafraichir = document.querySelector<HTMLButtonElement>('#rafraichir')!;
const boutonReprendre = document.querySelector<HTMLButtonElement>('#reprendre-enregistrement')!;

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
    afficherRetenues(retenues) {
        // 🔴 **`hidden` ET `data-retenues` : l'un pour l'œil, l'autre pour
        // l'instrument.** Le pilote de recette lit l'attribut, jamais le texte
        // ni la visibilité — piège de F1.
        boutonReprendre.hidden = !retenues;
        actionsFichiers.dataset.retenues = String(retenues);
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

    // 🔴 LA MÊME POIGNÉE SERT À LIRE, À ÉCRIRE ET À MUTER.
    //
    // ❌ **CE TRANSTYPAGE N'EST PAS UN CONTRÔLE, ET F2 LE DÉCLARAIT COMME TEL.**
    // Ces lignes disaient : « le transtypage est le CONTRÔLE DE COMPATIBILITÉ
    // STRUCTURELLE de F2 : si la vraie poignée cessait de le satisfaire,
    // `tsc --noEmit` le dirait ICI ». **C'est faux, et c'est mesuré** :
    // `choix.racine` est typée `Racine`, et `RacineInscriptible` en est un
    // SOUS-type — un `as` vers un sous-type ASSERTE, il ne vérifie pas.
    // Ajouter à `RacineInscriptible` une méthode que
    // `FileSystemDirectoryHandle` n'a pas ne faisait rougir QUE le faux de
    // test.
    //
    // ✅ **LE CONTRÔLE RÉEL VIT DÉSORMAIS DANS `fichiers/canal.ts`**, sur la
    // VRAIE poignée, avant tout élargissement — et il a trouvé une
    // incompatibilité de F2 dès qu'il a été posé (voir
    // `journaux-pont-fichiers-f3/t9-controle-structurel-de-f2-vacueux.txt`).
    // Ces deux lignes-ci ne sont plus que du câblage.
    const racineInscriptible: RacineInscriptible = choix.racine as RacineInscriptible;
    const racineMutable: RacineMutable = choix.racine as RacineMutable;
    const ecrivain = creerEcrivain(racineInscriptible);
    const mutateur = creerMutateur(racineMutable);
    const serveur = creerServeur(
        creerAdaptateur(choix.racine, fautesFichiersArmees),
        (m) => console.warn(m),
        {
            ecrivain,
            mutateur,
            onDues: (dues, retenues) => bureau.ecrituresDues(dues, retenues),
            onEchecEcriture: (chemin, code) => bureau.ecritureEchouee(chemin, code),
            onEchecMutation: (quoi, code) => bureau.mutationEchouee(quoi, code),
            // 🔵 **L'INSTRUMENTATION QUE LA SPEC §3.5.1 EXIGE**, et elle part
            // par le journal parce que le navigateur est le seul à SAVOIR ce
            // qu'il a fait. Le pont, lui, journalise ce que LUI sait — voir la
            // divergence déclarée dans `protocole.ts`.
            onRenommagePorCopie: (de, vers, octets, entrees) => {
                console.warn(
                    `renommage par copie « ${de} » → « ${vers} » : ${octets} octets, ` +
                        `${entrees} entree(s) — move() absente, repli LOCAL (zero octet sur le canal)`,
                );
            },
        },
    );
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

    // ════════════════════════════════════════════════════════════════════
    // 🔴 **F5 — `Bonjour` PART ICI, ET L'ORDRE N'EST PAS INDIFFÉRENT.**
    //
    // Il est envoyé **APRÈS** que l'écrivain, le mutateur et l'adaptateur sont
    // posés et que le canal est ouvert — jamais avant. C'est lui, et lui seul,
    // qui déclenche la reprise des écritures dues côté pont : avant F5, celle-ci
    // courait au démarrage du FIL, c'est-à-dire *sans savoir si un navigateur
    // est là, ni lequel, ni sur quel répertoire*. F2 a mesuré, deux fois sur
    // deux, la poussée du rejeu **0,8 s AVANT** cette annonce de montage, puis
    // une expiration **+30,2 s** plus tard.
    //
    // ⚠️ **`choix.nom` est le `name` de la poignée de répertoire**, et c'est la
    // MÊME valeur qu'un répertoire choisi par `showDirectoryPicker()` ou par
    // OPFS rendrait : c'est ce qui permet à la recette d'éprouver la règle sans
    // le sélecteur. **Elle n'éprouve pas pour autant le modèle de permission**,
    // qui n'est appelé nulle part dans ce dépôt.
    //
    // ⚠️ **`forcer: false` au montage, TOUJOURS.** Forcer est un geste de
    // l'utilisateur, jamais un défaut : un `true` ici rendrait le bouton
    // « Reprendre » inatteignable et réintroduirait le danger du §6.4 cas 2.
    // ════════════════════════════════════════════════════════════════════
    // 🔴 **L'ANNONCE ATTEND L'OUVERTURE DU CANAL, ET C'EST UN DÉFAUT QUE SEUL
    // LE CHEMIN RÉEL POUVAIT MONTRER.**
    //
    // *La première rédaction envoyait ici même, sans attendre.*
    // `connecterCanalFichiers` rend dès que la réponse SDP est reçue ; le canal
    // de données, lui, s'ouvre **après**. Mesuré sur la VM, dans cet ordre :
    // « pont fichiers : réponse reçue » → **`canal fichiers ferme : annonce non
    // envoyee`** → « connecting » → « connected » → « canal fichiers ouvert ».
    // Le `Bonjour` partait dans le vide, **et donc AUCUNE écriture due n'aurait
    // jamais été poussée** — un silence, c'est-à-dire pire que les trente
    // secondes que F2 avait mesurées et que F5 existe pour supprimer.
    //
    // 🔵 **C'est mon propre `console.warn` qui l'a dénoncé.** Un envoi qui
    // aurait échoué en silence aurait laissé la recette verte sur ses critères
    // de cache et muette sur celui-ci.
    //
    // ⚠️ **LES DEUX BRANCHES SONT NÉCESSAIRES** : le canal peut être déjà
    // ouvert quand on arrive ici (rien ne l'interdit), et n'écouter que
    // `'open'` manquerait alors l'événement pour toujours.
    const envoyerAuPont = (trame: ArrayBuffer): void => {
        if (!pont) {
            console.warn('aucun pont : annonce non envoyee');
            return;
        }
        const canal = pont.canal;
        if (canal.readyState === 'open') canal.send(trame);
        else if (canal.readyState === 'connecting') {
            canal.addEventListener('open', () => canal.send(trame), { once: true });
        } else console.warn('canal fichiers ferme : annonce non envoyee');
    };
    envoyerAuPont(trameBonjour(choix.nom, false));
    boutonRafraichir.onclick = () => envoyerAuPont(trameRafraichir());
    boutonReprendre.onclick = () => {
        // **Reprendre est un `Bonjour` FORCÉ**, et non un verbe de plus : c'est
        // exactement « je confirme que ce répertoire est le bon ». Le pont
        // mémorise alors le nom annoncé, et le bouton disparaît à l'annonce
        // suivante — sans qu'aucun état local n'ait à être remis à zéro ici.
        envoyerAuPont(trameBonjour(choix.nom, true));
    };

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
    void lancerLApplicationDemandee();
});

/// `?app=<uuid>` — le point d'entrée d'une PWA par application (sous-bloc G5).
///
/// 🔴 C'EST LE `start_url` DES MANIFESTES QUE LE HUB PUBLIE, et le choix de
/// cette page-ci plutôt que du hub est raisonné : c'est le SEUL des trois
/// candidats où **la fenêtre de session** tourne dans une fenêtre de PWA, donc
/// le seul où le legs de S4 — « c'est à SA recette de regarder LA FENÊTRE DE
/// SESSION sous une barre superposée » — puisse être exercé (décision D9).
///
/// ⚠️ LE COÛT EST DÉCLARÉ, PAS MAQUILLÉ : la session s'ouvre par
/// `window.open`, donc dans une SECONDE fenêtre de la PWA. L'alternative —
/// héberger la session DANS cette page — est une refonte du client, hors
/// périmètre, et elle est nommée en legs.
///
/// ⚠️ CE CHEMIN N'EST EXERCÉ DE BOUT EN BOUT PAR AUCUN CRITÈRE DE G5, et le
/// dire vaut mieux que de le laisser croire : les trois critères portent sur
/// l'installabilité, le glisser-déposer et le test empirique de l'amendement.
/// Ce lancement est livré, jamais mesuré.
async function lancerLApplicationDemandee(): Promise<void> {
    const application = params.get('app');
    if (application === null) return;
    if (jeton === undefined) {
        statut.textContent = "aucun jeton : l'application demandée n'a pas été lancée";
        poserTon(statut, 'danger');
        return;
    }
    const url = `${adressePlateforme(window.location, params.get('plateforme'))}/application/${encodeURIComponent(application)}/lancer`;
    // ⚠️ AUCUN `catch` MUET : une panne de réseau doit se voir. Le bandeau est
    // le seul endroit où l'utilisateur d'une PWA verra que rien ne s'est
    // passé — il n'a ni console ouverte, ni barre d'adresse.
    try {
        const reponse = await fetch(url, {
            method: 'POST',
            headers: { authorization: `Bearer ${jeton}` },
        });
        if (!reponse.ok) {
            statut.textContent = `l'application n'a pas pu être lancée (${String(reponse.status)})`;
            poserTon(statut, 'danger');
        }
    } catch (erreur) {
        statut.textContent = `l'application n'a pas pu être lancée : ${(erreur as Error).message}`;
        poserTon(statut, 'danger');
    }
}

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
