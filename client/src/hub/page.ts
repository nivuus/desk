// THE HUB'S WIRING: a DOM on one side, `catalogue.ts`, `depot.ts` and
// `manifeste.ts` on the other.
//
// ⚠️ THIS FILE IS NOT UNIT TESTED, and it is DECLARED rather than
// endured: it is the convention of `connexion.ts`, `bureau/porteur-dom.ts` and
// `main.ts`. ⚠️ THIS LIST NAMED `shell-page.ts` until the final review
// of August 31st, 2026: since task 9 it is only a sixteen-line
// redirect, hence a precedent that no longer says anything about wiring.
// What makes it tenable is the clause accompanying it, tightened by P4's cross-cutting
// review: **a condition is a RULE if changing it changes what
// the product DECIDES; it is WIRING if it only routes a
// decision already taken elsewhere, and tested there.** Any rule this file
// carried must move down into `catalogue.ts`, `depot.ts` or `manifeste.ts`,
// which are pure and tested.

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

// 🔴 `deps` CARRIES AN EMPTY TOKEN UNTIL `demarrer()` (at the foot of the
// file) HAS OBTAINED IT — see its header for what this fix repairs.
// `let`, and not `const`: the closures that follow (`traiterUnFichier`,
// `entree`, `peupler`) read `deps` AT CALL TIME, never at declaration,
// so they see the final value once `demarrer()` has resolved — none is
// invoked before.
let deps: DepsCatalogue = { base, jeton: '', fetch: window.fetch.bind(window) };

/* ── THE PER-APPLICATION MANIFEST, PUBLISHED AS `blob:` ───────────────────── */

/// 🔴 PATH V1, PASSED BY GATE P0. A `<link rel="manifest">` is fetched
/// by the browser **without an `Authorization` header**, exactly
/// like the icons it names, and ⑤ sets **no cookie** — its bearer
/// lives in `localStorage`, which does not travel on any request the browser
/// emits on its own. Serving this manifest would therefore require opening an
/// authenticated route, that is, a SECURITY DECISION
/// `routes-icone.ts:20-26` leaves to the repository owner. The page, for its part, is
/// authenticated: it reads everything through `fetch`, and publishes what it read.
///
/// **Measured (2 runs per probe)**: the `blob:` manifest with a `data:` icon
/// is loaded, parsed, and judged installable — `getInstallabilityErrors` empty and
/// `beforeinstallprompt` fired —, and the control served over ordinary HTTP returns
/// EXACTLY the same report. **What differs between the two is: nothing.**
///
/// ⚠️ WHAT V1 COSTS, AND IT MUST BE SAID: the manifest only exists in
/// the tab that built it. An installed PWA that fetched its
/// manifest again later would find a dead `blob:` URL. **Chromium's behaviour
/// in that case is measured by nothing**, and it is a legacy of G5.
async function publierLeManifeste(application: ApplicationListee): Promise<void> {
    let icone: Uint8Array | undefined;
    if (application.icone !== null) {
        const issue = await lireIcone(application, deps);
        if (issue.etat === 'ok') icone = issue.valeur;
    }
    // The background colour is READ FROM THE LIVE THEME, never written in a
    // `.ts`: §7.2 sweeps `.ts` files as much as `.css` ones, and there is
    // only one source of truth for a colour — `tokens.css`. It is the path
    // ⑥'s spec §4.1 sanctions and `design/galerie.ts` uses.
    const fond = getComputedStyle(document.documentElement).getPropertyValue('--fond-0').trim();
    const manifeste = batirManifeste(
        {
            id: application.id,
            nom: application.nom,
            icone,
            // ⚠️ `?? undefined` AND NOT `?? fond`: `null` means "this
            // icon has NO dominant colour", and the manifest must then OMIT
            // `theme_color` rather than invent one. Reusing the background
            // would make a chosen colour appear where there is none.
            accent: application.accent ?? undefined,
            associations: application.associations,
        },
        window.location.origin,
        fond,
    );
    const url = URL.createObjectURL(
        new Blob([JSON.stringify(manifeste)], { type: 'application/manifest+json' }),
    );
    // 🔴 THE HUB'S LINK IS REPLACED, NEVER DOUBLED: a document has only one
    //    manifest, and the second would be silently ignored — one would believe one had
    //    set the application's while keeping the hub's.
    for (const ancien of document.querySelectorAll('link[rel="manifest"]')) ancien.remove();
    const lien = document.createElement('link');
    lien.rel = 'manifest';
    lien.href = url;
    document.head.appendChild(lien);
    document.title = application.nom;
}

/* ── DROPPING AN INSTALLER — THE CONVERGENCE POINT ────────────────── */

/// 🔴 BOTH PATHS CALL THIS, AND NOTHING ELSE (decision D10). It is what
/// makes criterion ②'s RED run decidable: removing `launchQueue` must
/// leave drag-and-drop GREEN.
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
