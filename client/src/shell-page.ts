// Câblage de la page-shell : WebSocket du signaling d'un côté, DOM de
// l'autre. Aucune règle ici — elles sont dans `shell.ts`, qui est testé.
//
// 🔴 **CE FICHIER LUI-MÊME N'EST TESTÉ PAR RIEN, ET C'EST UN LEGS DÉCLARÉ,
// PAS UN CHOIX DE CONCEPTION** (requalifié à la revue, round de correction
// 2, 25 août 2026 — une réserve antérieure disait « choix assumé », ce qui
// était faux : ce dépôt a une convention pour rendre CE GENRE de câblage
// éprouvable, et elle n'est pas appliquée ici). Mesuré : retirer la branche
// `type === 'error'` ET l'écouteur `close` ci-dessous laisse `client/`
// entièrement vert (555/555) et `tsc --noEmit` propre — rien ne garde ce
// câblage. Le précédent existe : `accent-dom.ts` et `presse-papier-dom.ts`
// extraient leur câblage DOM/WebSocket dans un module à dépendances
// INJECTÉES (`ecrire`, `focalise`, `cible`, `emettre`…) plutôt que de
// toucher `document`/`window`/`WebSocket` directement — « injecté plutôt que
// pris de la session : c'est ce qui rend ce fichier éprouvable »
// (`presse-papier-dom.ts`). `shell-page.ts` ne suit pas ce patron : il
// construit son `WebSocket` en dur (`new WebSocket(signalingUrl)`,
// ci-dessous) et lit le DOM par des identifiants littéraux. Le porter au même
// patron est un chantier à part, non fait ici — il fallait le NOMMER plutôt
// que le confondre avec une décision.

import { creerBureau, type Ton } from './shell';
import { installerSelecteurDeThemeAuDOM } from './design/selecteur-theme';
import { jetonAcces } from './jeton';
import { composer, lirePrefixe } from './prefixe';
import { adressePlateforme, adresseSignaling } from './adresse-plateforme';
import { installerLePont } from './bureau/fichiers-dom';
import { dessinerFenetres } from './bureau/fenetres-dom';

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
        // 🔴 `/index.html`, PAS `/` — DEPUIS LE 30 AOÛT 2026 (« sers le hub
        // à la racine »). La racine sert désormais le HUB
        // (`plateforme/src/http/page/resolution.ts::PAGE`) ; la page de
        // SESSION que cette fenêtre doit ouvrir reste servie, mais à SON
        // PROPRE chemin explicite. Ouvrir `/?session=…` ouvrirait le hub
        // avec un paramètre de requête qu'il ignore, jamais une session.
        return window.open(`/index.html?session=${encodeURIComponent(session)}`, `guac-${session}`);
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

installerLePont({
    bureau,
    signalingUrl,
    fautesArmees: fautesFichiersArmees,
    boutonDossier,
    boutonRafraichir,
    boutonReprendre,
    // `shell.html` n'a aucun pli : sa section est toujours dépliée.
});

function redessiner(): void {
    dessinerFenetres(bureau.liste(), {
        liste,
        modele: modeleFenetre,
        // `shell.html` affiche la liste sans pli : aucune section à révéler.
        rouvrir: (session) => { bureau.rouvrir(session); redessiner(); },
    });
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
    // ⚠️ `JSON.parse` REND `any`, ET C'EST DÉCLARÉ ICI PLUTÔT QUE TU (revue,
    // round de correction 2) : `message.reason`/`message.motif`/
    // `message.retryApresS`, lus quelques lignes plus bas pour `type:'error'`,
    // ne sont appariés au vocabulaire que `signaling/relais.ts` émet
    // réellement (`retryApresS`, pas `retry_apres_s` — voir la convention
    // camelCase du fil, distincte du snake_case des journaux) NI PAR UN TYPE
    // PARTAGÉ NI PAR AUCUN TEST. Un renommage de champ côté serveur laisserait
    // ce fichier compiler et tourner, silencieusement muet sur le nouveau
    // nom — la même classe de risque que documente `webrtc.ts`
    // (`SignalingMessage`, un type discriminé PROPRE À ce fichier, jamais
    // partagé non plus). Non corrigé ici : un type partagé impliquerait de le
    // faire vivre dans `proto/`, épinglé des deux côtés, ce qui dépasse la
    // portée d'un round de correction.
    const message = JSON.parse(evenement.data);
    if (message.type === 'fenetre-ouverte') bureau.fenetreOuverte(message.session, message.titre);
    else if (message.type === 'fenetre-fermee') bureau.fenetreFermee(message.session);
    else if (message.type === 'refus') bureau.refus(message.titre, message.motif);
    // 🔴 CORRECTIF DU LEGS DES FREINS MANQUANTS (round de correction 1,
    // critique ④) — AUCUNE BRANCHE NE RECONNAISSAIT `type:'error'` AVANT CE
    // LOT, alors que `signaling/relais.ts` peut désormais le rendre AVANT
    // même de fermer le socket (refus de volume `trop-de-requetes`, voir son
    // `retryApresS`). Sans cette branche, la page restait affichée « bureau
    // connecté » et mourait en silence à la fermeture qui suit.
    else if (message.type === 'error') {
        bureau.canalDeControleRefuse(message.reason, message.motif, message.retryApresS);
    }
    redessiner();
});

// 🔴 CORRECTIF DU LEGS DES FREINS MANQUANTS (round de correction 1,
// critique ④) — CE SOCKET N'INSTALLAIT NI `close` NI `error` AVANT CE LOT.
// Une perte de connexion (frein, redémarrage du service, coupure réseau)
// laissait le bandeau à « bureau connecté » indéfiniment : c'est exactement
// la panne muette que ce dépôt combat (`CLAUDE.md`), et c'est ce lot lui-même
// qui l'ouvre en freinant `/signal`.
socket.addEventListener('close', () => bureau.canalDeControlePerdu());

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
