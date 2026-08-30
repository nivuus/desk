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
import { assurerAcces } from '../jeton';
import {
    lancerApplication,
    lireIcone,
    listerApplications,
    listerVms,
    type ApplicationListee,
    type DepsCatalogue,
} from './catalogue';
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
        dire('neutre', `Lancement de ${application.nom}…`);
        void lancerApplication(application.id, deps).then((issue) => {
            if (issue.etat === 'ok') dire('succes', `${application.nom} a été lancée.`);
            else dire('danger', `${application.nom} n'a pas pu être lancée : ${issue.refus.motif}.`);
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
    // `demarrer()` (pied de fichier) qu'APRÈS que `assurerAcces` en a rendu
    // un — c'est cette fonction-là qui décide, et `jeton.test.ts` la tient.
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

   🔴 LA RÈGLE (« essayer le coffre, puis Pomerium, sinon renvoyer vers la
   connexion ») VIT DANS `jeton.ts::assurerAcces`, PAS ICI : au sens du
   critère posé en tête de ce fichier, la changer changerait ce que le
   produit DÉCIDE, ce n'est donc pas du câblage. `assurerAcces` réutilise
   `accesParPomerium` — le chemin de `connexion.ts::tenterPomerium`, EXTRAIT
   plutôt que recopié — et les deux sont tenus par `jeton.test.ts`. Ce qui
   reste ICI est du câblage pur : lire le résultat, et soit peupler, soit
   rediriger. */
async function demarrer(): Promise<void> {
    dire('neutre', 'identification…');
    const acces = await assurerAcces(window.localStorage, base, window.fetch.bind(window));
    if (acces === undefined) {
        // ⚠️ REDIRIGER VERS UN ÉCRAN OÙ L'UTILISATEUR PEUT AGIR, JAMAIS SUR
        // UN MESSAGE QUI NE DIT PAS QUOI FAIRE — la règle que `connexion.ts`
        // s'impose déjà. `suite` reconduit vers CETTE page, chaîne de
        // requête comprise (`?app=…`), pour qu'une connexion réussie revienne
        // ici plutôt que sur la shell.
        const suite = `hub.html${window.location.search}`;
        window.location.href = `connexion.html?suite=${encodeURIComponent(suite)}`;
        return;
    }
    deps = { base, jeton: acces, fetch: window.fetch.bind(window) };
    dire('neutre', '');
    await peupler();
}

void demarrer();
