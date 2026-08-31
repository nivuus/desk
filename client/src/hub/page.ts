// LE CÂBLAGE DU HUB : un DOM d'un côté, `catalogue.ts`, `depot.ts` et
// `manifeste.ts` de l'autre.
//
// ⚠️ CE FICHIER N'EST PAS TESTÉ UNITAIREMENT, et c'est DÉCLARÉ plutôt que
// subi : c'est la convention de `connexion.ts`, `shell-page.ts` et `main.ts`.
// Ce qui la rend tenable est la clause qui l'accompagne, resserrée par la revue
// transverse de P4 : **une condition est une RÈGLE si la changer change ce que
// le produit DÉCIDE ; elle est du CÂBLAGE si elle ne fait que router une
// décision déjà prise ailleurs, et testée là-bas.** Toute règle que ce fichier
// porterait doit descendre dans `catalogue.ts`, `depot.ts` ou `manifeste.ts`,
// qui sont purs et testés.

import { adressePlateforme } from '../adresse-plateforme';
import { installerSelecteurDeThemeAuDOM } from '../design/selecteur-theme';
import { assurerAccesFrais } from '../jeton';
import {
    lancerApplication,
    lireIcone,
    listerApplications,
    listerVms,
    type ApplicationListee,
    type DepsCatalogue,
} from './catalogue';
import { NOM_FENETRE_BUREAU, PAGE_DU_BUREAU, ouvrirLeBureau } from './bureau';
import { deposer, type Ton } from './depot';
import { batirManifeste } from './manifeste';

const CLASSE_DE_TON: Record<Ton, string> = {
    neutre: 'message',
    succes: 'message message--succes',
    danger: 'message message--danger',
};

const params = new URLSearchParams(window.location.search);
const base = adressePlateforme(window.location, params.get('plateforme'));

const elMessage = document.getElementById('message') as HTMLDivElement;
const elListe = document.getElementById('applications') as HTMLUListElement;
const elDepot = document.getElementById('depot') as HTMLElement;
const elChoisir = document.getElementById('choisir') as HTMLButtonElement;
const elThemes = document.getElementById('themes');
const elBureau = document.getElementById('bureau') as HTMLAnchorElement | null;

if (elThemes !== null) installerSelecteurDeThemeAuDOM(elThemes);

/* ── LE CHEMIN VERS LE BUREAU ─────────────────────────────────────────────
   🔴 DÉFAUT TROUVÉ EN PRODUCTION LE 30 AOÛT 2026, ET CONSÉQUENCE DIRECTE DU
   PASSAGE DU HUB À LA RACINE (lot 14). Le hub est la seule surface que
   l'utilisateur atteint, il n'ouvre aucune connexion de signaling, et rien
   ici ne menait à `shell.html` — la seule page qui traite `fenetre-ouverte`.
   Le superviseur annonçait donc ses fenêtres à un pair ABSENT (no-op
   silencieux de `signaling/relais.ts`) et les refusait trente secondes plus
   tard. Le raisonnement complet — pourquoi ce chemin plutôt que « le hub
   tient lui-même la session », et ce que le rôle `client` EXCLUSIF impose —
   est dans l'en-tête de `hub/bureau.ts`.

   L'`href` et le `target` viennent de là-bas, jamais du HTML : une seule
   source de vérité pour la page du bureau et le nom de sa fenêtre. */
if (elBureau !== null) {
    elBureau.href = PAGE_DU_BUREAU;
    elBureau.target = NOM_FENETRE_BUREAU;
}

function dire(ton: Ton, texte: string): void {
    elMessage.className = `${CLASSE_DE_TON[ton]} hub__message`;
    elMessage.textContent = texte;
}

// 🔴 `deps` PORTE UN JETON VIDE JUSQU'À CE QUE `demarrer()` (en pied de
// fichier) L'AIT OBTENU — voir son en-tête pour ce que ce correctif répare.
// `let`, et non `const` : les fermetures qui suivent (`traiterUnFichier`,
// `entree`, `peupler`) lisent `deps` À L'APPEL, jamais à la déclaration,
// donc voient la valeur finale une fois `demarrer()` résolue — aucune n'est
// invoquée avant.
let deps: DepsCatalogue = { base, jeton: '', fetch: window.fetch.bind(window) };

/* ── LE MANIFESTE PAR APPLICATION, PUBLIÉ EN `blob:` ───────────────────── */

/// 🔴 LA VOIE V1, REÇUE PAR LA PORTE P0. Un `<link rel="manifest">` est allé
/// chercher par le navigateur **sans en-tête `Authorization`**, exactement
/// comme les icônes qu'il nomme, et ⑤ ne pose **aucun cookie** — son porteur
/// vit dans `localStorage`, qui ne voyage sur aucune requête que le navigateur
/// émet de lui-même. Servir ce manifeste demanderait donc d'ouvrir une route
/// authentifiée, c'est-à-dire une DÉCISION DE SÉCURITÉ que
/// `routes-icone.ts:20-26` laisse au propriétaire du dépôt. La page, elle, est
/// authentifiée : elle lit tout par `fetch`, et publie ce qu'elle a lu.
///
/// **Mesuré (2 exécutions par sonde)** : le manifeste `blob:` à icône `data:`
/// est chargé, analysé, et jugé installable — `getInstallabilityErrors` vide et
/// `beforeinstallprompt` déclenché —, et le témoin servi par HTTP ordinaire rend
/// EXACTEMENT le même relevé. **Ce qui diffère entre les deux est : rien.**
///
/// ⚠️ CE QUE V1 COÛTE, ET IL FAUT LE DIRE : le manifeste n'existe que dans
/// l'onglet qui l'a construit. Une PWA installée qui re-chercherait son
/// manifeste plus tard trouverait une URL `blob:` morte. **Le comportement de
/// Chromium dans ce cas n'est mesuré par rien**, et c'est un legs de G5.
async function publierLeManifeste(application: ApplicationListee): Promise<void> {
    let icone: Uint8Array | undefined;
    if (application.icone !== null) {
        const issue = await lireIcone(application, deps);
        if (issue.etat === 'ok') icone = issue.valeur;
    }
    // La couleur de fond est LUE SUR LE THÈME VIVANT, jamais écrite dans un
    // `.ts` : §7.2 balaie les `.ts` autant que les `.css`, et il n'existe
    // qu'une source de vérité pour une couleur — `tokens.css`. C'est la voie
    // que la spec §4.1 de ⑥ sanctionne et que `design/galerie.ts` emploie.
    const fond = getComputedStyle(document.documentElement).getPropertyValue('--fond-0').trim();
    const manifeste = batirManifeste(
        {
            id: application.id,
            nom: application.nom,
            icone,
            // ⚠️ `?? undefined` ET NON `?? fond` : `null` veut dire « cette
            // icône n'a AUCUNE dominante », et le manifeste doit alors OMETTRE
            // `theme_color` plutôt que d'en inventer un. Reprendre le fond
            // ferait paraître une couleur choisie là où il n'y en a pas.
            accent: application.accent ?? undefined,
            associations: application.associations,
        },
        window.location.origin,
        fond,
    );
    const url = URL.createObjectURL(
        new Blob([JSON.stringify(manifeste)], { type: 'application/manifest+json' }),
    );
    // 🔴 LE LIEN DU HUB EST REMPLACÉ, JAMAIS DOUBLÉ : un document n'a qu'un
    //    manifeste, et le second serait ignoré en silence — on croirait avoir
    //    posé celui de l'application en gardant celui du hub.
    for (const ancien of document.querySelectorAll('link[rel="manifest"]')) ancien.remove();
    const lien = document.createElement('link');
    lien.rel = 'manifest';
    lien.href = url;
    document.head.appendChild(lien);
    document.title = application.nom;
}

/* ── LE DÉPÔT D'UN INSTALLEUR — LE POINT DE CONVERGENCE ────────────────── */

/// 🔴 LES DEUX CHEMINS APPELLENT CECI, ET RIEN D'AUTRE (décision D10). C'est ce
/// qui rend la ROUGE du critère ② décidable : retirer `launchQueue` doit
/// laisser le glisser-déposer VERT.
async function traiterUnFichier(fichier: File): Promise<void> {
    dire('neutre', `Téléversement de ${fichier.name}…`);
    const resume = await deposer(fichier, {
        ...deps,
        maintenant: () => Date.now(),
        progression: (p) => {
            if (p.total > 0) {
                const pourcent = Math.floor((p.octets / p.total) * 100);
                dire('neutre', `${fichier.name} — ${p.phase} ${String(pourcent)} %`);
            }
        },
    });
    dire(resume.ton, resume.texte);
}

/* ── LA LISTE ─────────────────────────────────────────────────────────── */

function entree(application: ApplicationListee): HTMLLIElement {
    const li = document.createElement('li');
    li.className = 'carte hub__entree';

    const img = document.createElement('img');
    img.className = application.icone === null ? 'hub__icone hub__icone--absente' : 'hub__icone';
    img.alt = '';
    // 🔴 ❌ ~~L'ICÔNE NE PEUT PAS ÊTRE POSÉE PAR `src` VERS LA ROUTE : un
    //    `<img src>` ne porte pas d'`Authorization`. Elle est LUE par `fetch`
    //    authentifié, puis publiée en objet — la seule voie.~~ **PLUS VRAI
    //    DEPUIS LE 30 AOÛT 2026**, décision du propriétaire du dépôt : la
    //    route d'icône s'atteint par une URL SIGNÉE, que le catalogue frappe
    //    sous jeton porteur et rend dans `icone_url`. **Elle se pose
    //    directement dans `src`**, et c'est très exactement ce que le lot
    //    livre. Le détour par `fetch` + `createObjectURL` disparaît d'ici —
    //    il survit dans `publierLeManifeste`, qui a besoin des OCTETS pour
    //    bâtir le `data:` du manifeste, la seule forme que G5 ait mesurée
    //    installable et la seule qu'un manifeste atteigne sans cookie.
    //
    // ⚠️ CE QUE CETTE LIGNE N'ÉTABLIT PAS : qu'un navigateur RÉEL l'affiche.
    //    Ce fichier n'est pas testé unitairement (voir l'en-tête), et aucun
    //    jugement visuel n'a été porté sur le hub à ce jour.
    li.appendChild(img);
    if (application.icone_url !== null) img.src = `${base}${application.icone_url}`;

    const corps = document.createElement('div');
    corps.className = 'hub__corps';
    const nom = document.createElement('h2');
    nom.className = 'hub__nom';
    nom.textContent = application.nom;
    corps.appendChild(nom);

    const boutons = document.createElement('p');
    boutons.className = 'hub__boutons';
    const lancer = document.createElement('button');
    lancer.type = 'button';
    lancer.className = 'bouton bouton--principal';
    lancer.textContent = 'Lancer';
    lancer.addEventListener('click', () => {
        // 🔴 LE MÊME CLIC OUVRE LE BUREAU, ET C'EST LE FOND DU CORRECTIF DU
        //    30 AOÛT 2026. Un lien visible en en-tête ne suffit pas : il
        //    demande à l'utilisateur de savoir, AVANT de lancer, qu'une
        //    seconde surface existe. Ici c'est son geste de lancement qui
        //    ouvre la surface où la fenêtre paraîtra — et parce que c'est un
        //    GESTE, aucun navigateur ne bloque cette ouverture-là.
        //
        // ⚠️ AVANT le `POST /lancer`, jamais après : un `await` intercalé
        //    consommerait l'activation transitoire du clic, et l'ouverture
        //    redeviendrait une pop-up bloquable — c'est le mur que
        //    `shell-page.ts` heurte déjà, et qu'il ne s'agit pas de déplacer
        //    d'un cran.
        //
        // ⚠️ Le bureau ARRIVE APRÈS l'annonce dans le cas le plus rapide, et
        //    ce n'est pas un défaut : le relais dit à l'agent qu'un pair est
        //    présent (`pair-present`, lot 17) et le superviseur REDIT alors
        //    ses fenêtres en attente, compte à rebours remis à zéro.
        const bureau = ouvrirLeBureau({ ouvrir: (url, nom) => window.open(url, nom) });
        dire('neutre', `Lancement de ${application.nom}…`);
        void jetonFrais().then((frais) => {
            if (frais === undefined) {
                dire('danger', 'Votre session a expiré. Rechargez la page pour vous reconnecter.');
                return;
            }
            return lancerApplication(application.id, deps).then((issue) => {
                // Le corps existant, INCHANGÉ : le bandeau de succès, et
                // le bandeau de danger quand le lancement est refusé. Le
                // relire dans `hub/page.ts` plutôt que de le retaper — il
                // porte deux commentaires 🔴 qui expliquent pourquoi le
                // lancement a lieu même si l'ouverture a échoué.
                //
                // ⚠️ SEULE LA MENTION « Employez « Mon bureau » en haut de
                // page » devra partir, en tâche 8 : le lien disparaît, et une
                // consigne qui désigne un bouton absent est pire qu'aucune.
                if (issue.etat !== 'ok') {
                    dire('danger', `${application.nom} n'a pas pu être lancée : ${issue.refus.motif}.`);
                    return;
                }
                // 🔴 AUCUN ÉCHEC MUET, ET LE LANCEMENT A LIEU QUAND MÊME. Refuser
                //    de lancer parce que le bureau n'a pas pu s'ouvrir ferait
                //    d'une gêne une panne ; taire le bureau manquant ramènerait
                //    la panne d'origine — une application lancée que personne ne
                //    voit. On fait les deux, et on le dit.
                if (bureau) dire('succes', `${application.nom} a été lancée.`);
                else {
                    dire(
                        'danger',
                        `${application.nom} a été lancée, mais le navigateur a bloqué l’ouverture du bureau : ` +
                            'sa fenêtre ne peut pas paraître. Employez « Mon bureau » en haut de page.',
                    );
                }
            });
        });
    });
    boutons.appendChild(lancer);

    const installer = document.createElement('button');
    installer.type = 'button';
    installer.className = 'bouton bouton--discret';
    installer.textContent = 'Installer';
    installer.addEventListener('click', () => {
        void publierLeManifeste(application).then(() => {
            dire(
                'neutre',
                `${application.nom} est prête à être installée : employez « Installer l'application » du navigateur.`,
            );
        });
    });
    boutons.appendChild(installer);
    corps.appendChild(boutons);

    li.appendChild(corps);
    return li;
}

async function peupler(): Promise<void> {
    // ⚠️ AUCUNE GARDE SUR LE JETON ICI : `peupler` n'est appelée par
    // `demarrer()` (pied de fichier) qu'APRÈS que `assurerAccesFrais` en a
    // rendu un — c'est cette fonction-là qui décide, et `jeton.test.ts` la tient.
    const vms = await listerVms(deps);
    if (vms.etat !== 'ok') {
        dire('danger', `Les machines n'ont pas pu être lues : ${vms.refus.motif}.`);
        return;
    }
    if (vms.valeur.length === 0) {
        dire('neutre', "Aucune machine ne vous est attribuée : il n'y a rien à montrer.");
        return;
    }
    const vm = vms.valeur[0];
    const applications = await listerApplications(vm.id, deps);
    if (applications.etat !== 'ok') {
        dire('danger', `Le catalogue n'a pas pu être lu : ${applications.refus.motif}.`);
        return;
    }
    elListe.replaceChildren(...applications.valeur.map(entree));
    dire('neutre', `${String(applications.valeur.length)} application(s) sur ${vm.nom}.`);

    // `?app=<uuid>` : la page publie le manifeste de CETTE application, ce qui
    // la rend installable. C'est aussi ce que `start_url` rouvrira.
    const demandee = params.get('app');
    if (demandee !== null) {
        const cible = applications.valeur.find((a) => a.id === demandee);
        if (cible !== undefined) await publierLeManifeste(cible);
        else dire('danger', "L'application demandée n'est pas dans ce catalogue.");
    }
}

/* ── LES DEUX CHEMINS DE DÉPÔT ────────────────────────────────────────── */

elDepot.addEventListener('dragover', (e) => {
    e.preventDefault();
    elDepot.classList.add('hub__depot--survol');
});
elDepot.addEventListener('dragleave', () => elDepot.classList.remove('hub__depot--survol'));
elDepot.addEventListener('drop', (e) => {
    e.preventDefault();
    elDepot.classList.remove('hub__depot--survol');
    const fichier = e.dataTransfer?.files?.[0];
    if (fichier !== undefined) void traiterUnFichier(fichier);
});

elChoisir.addEventListener('click', () => {
    const saisie = document.createElement('input');
    saisie.type = 'file';
    saisie.addEventListener('change', () => {
        const fichier = saisie.files?.[0];
        if (fichier !== undefined) void traiterUnFichier(fichier);
    });
    saisie.click();
});

// 🔴 `launchQueue` N'EXISTE PAS HORS D'UNE PWA INSTALLÉE, et le test de sa
// présence est un `in` EXPLICITE, jamais un `try` : une exception silencieuse
// rendrait les deux chemins indiscernables, et c'est précisément ce que le
// critère ② doit pouvoir distinguer. **Le glisser-déposer ci-dessus ne dépend
// de rien de ce qui suit** — c'est l'amendement du 28/07/2026.
if ('launchQueue' in window) {
    interface FileLaunchParams {
        files: { getFile(): Promise<File> }[];
    }
    interface FileLaunchQueue {
        setConsumer(consommateur: (params: FileLaunchParams) => void): void;
    }
    (window as unknown as { launchQueue: FileLaunchQueue }).launchQueue.setConsumer((lancement) => {
        const premier = lancement.files[0];
        if (premier === undefined) return;
        void premier.getFile().then(traiterUnFichier);
    });
}

/* ── L'ACCÈS : LE COFFRE D'ABORD, POMERIUM ENSUITE, LA CONNEXION EN DERNIER
   RECOURS ────────────────────────────────────────────────────────────────

   🔴 CE QUE CE BLOC RÉPARE — DÉFAUT TROUVÉ EN PRODUCTION LE 30 AOÛT 2026 :
   ce fichier se contentait, la veille, de LIRE le coffre et de se plaindre
   s'il était vide ("Aucun jeton : connectez-vous d'abord.", sans bouton, sans
   lien, sans rien à faire). Le seul code qui savait obtenir un jeton par
   Pomerium (`connexion.ts::tenterPomerium`) ne courait QU'AU CHARGEMENT DE
   LA PAGE DE CONNEXION. Tant que la racine servait la page de session,
   personne n'avait vu un visiteur atterrir DIRECTEMENT sur le hub sans être
   passé par cet écran — le lot qui a mis le hub à la racine avait vérifié
   que `/` SERT le hub, jamais qu'un visiteur SANS JETON puisse s'en servir :
   encore un contrôle incapable de rougir.

   🔴 LA RÈGLE (« essayer le coffre, puis le rafraîchissement, puis Pomerium,
   sinon renvoyer vers la connexion ») VIT DANS `jeton.ts::assurerAccesFrais`,
   PAS ICI : au sens du critère posé en tête de ce fichier, la changer
   changerait ce que le produit DÉCIDE, ce n'est donc pas du câblage.
   `assurerAccesFrais` réutilise `rafraichirSiNecessaire` et
   `accesParPomerium` — le second est le chemin de
   `connexion.ts::tenterPomerium`, EXTRAIT plutôt que recopié — et les trois
   sont tenus par `jeton.test.ts`. Ce qui reste ICI est du câblage pur : lire
   le résultat, et soit peupler, soit rediriger. */
async function demarrer(): Promise<void> {
    dire('neutre', 'identification…');
    const acces = await jetonFrais();
    if (acces === undefined) {
        const suite = `/${window.location.search}`;
        window.location.href = `connexion.html?suite=${encodeURIComponent(suite)}`;
        return;
    }
    deps = { base, jeton: acces, fetch: window.fetch.bind(window) };
    dire('neutre', '');
    await peupler();
}

/// 🔴 **APPELÉE AVANT CHAQUE USAGE, ET C'EST LE POINT DE LA DÉCISION DU
/// 31 AOÛT 2026.** Un test local d'expiration, un appel réseau seulement s'il
/// est périmé : le chargement, chaque lancement et chaque lecture d'icône
/// passent par ici, sans aucune minuterie à calibrer.
async function jetonFrais(): Promise<string | undefined> {
    const acces = await assurerAccesFrais(
        window.localStorage,
        base,
        window.fetch.bind(window),
        Date.now(),
        async (corps) => {
            const reponse = await fetch(`${base}/auth/rafraichir`, {
                method: 'POST',
                headers: { 'content-type': 'application/json' },
                body: JSON.stringify(corps),
            });
            if (!reponse.ok) return undefined;
            return (await reponse.json()) as { acces: string; rafraichissement: string };
        },
    );
    if (acces !== undefined) deps = { ...deps, jeton: acces };
    return acces;
}

void demarrer();
