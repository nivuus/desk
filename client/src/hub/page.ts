// LE CÂBLAGE DU HUB : un DOM d'un côté, `catalogue.ts`, `depot.ts` et
// `manifeste.ts` de l'autre.
//
// ⚠️ CE FICHIER N'EST PAS TESTÉ UNITAIREMENT, et c'est DÉCLARÉ plutôt que
// subi : c'est la convention de `connexion.ts`, `bureau/porteur-dom.ts` et
// `main.ts`. ⚠️ CETTE LISTE NOMMAIT `shell-page.ts` jusqu'à la revue finale
// du 31 août 2026 : depuis la tâche 9 il n'est plus qu'une redirection de
// seize lignes, donc un précédent qui ne dit plus rien d'un câblage.
// Ce qui la rend tenable est la clause qui l'accompagne, resserrée par la revue
// transverse de P4 : **une condition est une RÈGLE si la changer change ce que
// le produit DÉCIDE ; elle est du CÂBLAGE si elle ne fait que router une
// décision déjà prise ailleurs, et testée là-bas.** Toute règle que ce fichier
// porterait doit descendre dans `catalogue.ts`, `depot.ts` ou `manifeste.ts`,
// qui sont purs et testés.

import { adressePlateforme, adresseSignaling } from '../adresse-plateforme';
import { installerLeBureau } from '../bureau/porteur-dom';
import { installerSelecteurDeThemeAuDOM } from '../design/selecteur-theme';
import { assurerAccesFrais, paireDeReponse } from '../jeton';
import { lirePrefixe, retenirLePrefixe } from '../prefixe';
import {
    lancerApplication,
    lireIcone,
    listerApplications,
    listerVms,
    type ApplicationListee,
    type DepsCatalogue,
} from './catalogue';
import { batirCarte } from './cartes';
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

if (elThemes !== null) installerSelecteurDeThemeAuDOM(elThemes);

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

function entree(application: ApplicationListee): DocumentFragment {
    return batirCarte(application, {
        modele: document.querySelector<HTMLTemplateElement>('#modele-application')!,
        urlIcone: application.icone_url === null ? null : `${base}${application.icone_url}`,
        lancer: () => {
            // 🔴 LE HUB EST DÉSORMAIS LA SEULE SURFACE (décision du
            //    propriétaire, 31 août 2026) : il tient lui-même la session de
            //    contrôle (`installerLeBureau`, en pied de fichier), et la
            //    fenêtre lancée paraîtra dans la section « Mes fenêtres » de
            //    CETTE page — plus besoin d'en ouvrir une seconde depuis ce
            //    clic. Le correctif du 30 août (ouvrir `shell.html` depuis ce
            //    même geste) n'a donc plus d'objet.
            dire('neutre', `Lancement de ${application.nom}…`);
            void jetonFrais().then((frais) => {
                if (frais === undefined) {
                    dire('danger', 'Votre session a expiré. Rechargez la page pour vous reconnecter.');
                    return;
                }
                return lancerApplication(application.id, deps).then((issue) => {
                    if (issue.etat !== 'ok') {
                        dire('danger', `${application.nom} n'a pas pu être lancée : ${issue.refus.motif}.`);
                        return;
                    }
                    dire('succes', `${application.nom} a été lancée.`);
                });
            });
        },
        installer: () => {
            void publierLeManifeste(application).then(() => {
                dire(
                    'neutre',
                    `${application.nom} est prête à être installée : employez « Installer l'application » du navigateur.`,
                );
            });
        },
    });
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

    // 🔴 **LE PRÉFIXE DE LA VM EST RETENU ICI, ET IL NE L'ÉTAIT NULLE PART**
    // (critique ② de la revue finale du 31 août 2026). `poserPrefixe` n'avait
    // qu'un appelant de production — `connexion.ts::chercherLaSession` —, qui
    // ne court **que sur la page de connexion**. Or ce chantier fait
    // précisément qu'un visiteur derrière Pomerium obtienne son jeton SUR LE
    // HUB (`assurerAccesFrais` → `/auth/moi`) sans jamais passer par cet
    // écran : `lirePrefixe()` rendait `''`, le hub écoutait la session
    // `bureau` pendant que l'agent annonçait sur `<prefixe>:bureau`, et
    // **aucun `fenetre-ouverte` n'arrivait jamais**. La valeur était pourtant
    // là, à trois lignes : `routes-vm.ts` la renvoie, `catalogue.ts` la parse
    // déjà dans `VmListee.prefixe`.
    //
    // 🔴 **L'ORDRE EST LE POINT** : `demarrer()` n'installe le bureau
    // qu'APRÈS cet appel, pour que `lirePrefixe()` compose les bons noms de
    // session et de verrou. La décision « quel préfixe retenir ? » vit dans
    // `prefixe.ts::prefixeDeLaVm`, pure et testée ; ce qui reste ici est du
    // câblage.
    retenirLePrefixe(window.localStorage, vm.prefixe);

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

    // 🔴 **LE BUREAU EST INSTALLÉ MÊME SI LE CATALOGUE ÉCHOUE** (Important ④
    // de la revue finale). `await peupler()` précédait `installerLeBureau`
    // sans garde : une panne réseau sur `GET /vm` remontait non rattrapée —
    // `catalogue.ts` déclare qu'une panne d'ENVIRONNEMENT remonte telle
    // quelle —, `#message` avait déjà été vidé deux lignes plus haut, et
    // l'utilisateur voyait une page **blanche, sans bureau et sans
    // explication**. Avant ce chantier le bureau vivait ailleurs et survivait
    // à une panne du catalogue : **ce couplage est neuf**.
    //
    // ⚠️ **L'INTERACTION AVEC LA CRITIQUE ② EST LE POINT DÉLICAT** : le
    // préfixe DOIT être connu avant l'installation (voir `peupler`), et il
    // vient justement de l'appel qui peut échouer. Le remède est donc
    // d'attraper et d'installer **avec ce qu'on sait** — c'est-à-dire le
    // préfixe déjà au coffre, posé par un chargement antérieur ou par
    // `connexion.ts` —, jamais de renoncer au bureau.
    try {
        await peupler();
    } catch (e) {
        dire('danger', `Le catalogue n'a pas pu être lu : ${(e as Error).message}.`);
    }

    // ── LE BUREAU, DANS CETTE PAGE ────────────────────────────────────────
    // 🔴 LE HUB EST DÉSORMAIS LA SEULE SURFACE (décision du propriétaire,
    // 31 août 2026). Le lien « Mon bureau » et l'ouverture au clic sur
    // « Lancer » étaient les correctifs du 30 août ; ils n'ont plus d'objet.
    installerLeBureau({
        signalingUrl: adresseSignaling(window.location, params.get('signaling')),
        // 🔴 **UN FOURNISSEUR, JAMAIS `acces`** (critique ① de la revue
        // finale) : `ouvrirLaSession` ne court, pour un suiveur, qu'au moment
        // de sa PROMOTION — potentiellement des heures plus tard —, et un
        // jeton d'accès vit dix minutes.
        jetonFrais,
        // 🔴 **LU ICI, DONC APRÈS `peupler()`** : c'est ce qui donne au verrou
        // et à la session de contrôle le préfixe de la VM (critique ②).
        prefixe: lirePrefixe(),
        fautesArmees: params.get('faute-fichiers') === '1',
        elements: {
            statut: document.querySelector<HTMLDivElement>('#statut')!,
            liste: document.querySelector<HTMLUListElement>('#fenetres')!,
            modele: document.querySelector<HTMLTemplateElement>('#modele-fenetre')!,
            sectionFenetres: document.querySelector<HTMLElement>('#section-fenetres')!,
            sectionFichiers: document.querySelector<HTMLDetailsElement>('#section-fichiers')!,
            boutonDossier: document.querySelector<HTMLButtonElement>('#choisir-dossier')!,
            etatFichiers: document.querySelector<HTMLDivElement>('#etat-fichiers')!,
            ecrituresDues: document.querySelector<HTMLDivElement>('#ecritures-dues')!,
            actionsFichiers: document.querySelector<HTMLParagraphElement>('#actions-fichiers')!,
            boutonRafraichir: document.querySelector<HTMLButtonElement>('#rafraichir')!,
            boutonReprendre: document.querySelector<HTMLButtonElement>('#reprendre-enregistrement')!,
        },
    });
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
            // 🔴 **ENVELOPPÉ, LÀ OÙ `accesParPomerium` (`jeton.ts:175-186`)
            // L'EST DEPUIS TOUJOURS** : ce chemin-ci était resté SANS APPELANT
            // DE PRODUCTION jusqu'à cette tâche, donc jamais mis à l'épreuve
            // d'un réseau injoignable. Sans ce `try/catch`, une exception
            // (hors ligne, DNS, CORS) remonterait non rattrapée à travers
            // `assurerAccesFrais` → `jetonFrais()` → `demarrer()` (rejet non
            // géré sur `void demarrer()`) ou le gestionnaire de clic, et la
            // page resterait bloquée sur « identification… » au lieu de
            // retomber sur Pomerium (étape ③ d'`assurerAccesFrais`).
            try {
                const reponse = await fetch(`${base}/auth/rafraichir`, {
                    method: 'POST',
                    headers: { 'content-type': 'application/json' },
                    body: JSON.stringify(corps),
                });
                if (!reponse.ok) return undefined;
                // La forme est VALIDÉE, jamais affirmée : un corps `ok: true`
                // mais incomplet écrirait tel quel au coffre (`poser`, dans
                // `rafraichirSiNecessaire`) — le scénario « coffre empoisonné »
                // qu'`accesDeReponse` existe pour empêcher sur `/auth/moi`.
                return paireDeReponse(await reponse.json().catch(() => undefined));
            } catch {
                return undefined;
            }
        },
    );
    if (acces !== undefined) deps = { ...deps, jeton: acces };
    return acces;
}

void demarrer();
