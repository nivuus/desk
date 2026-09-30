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
import { installThemeSelectorInDOM } from '../design/selecteur-theme';
import { assurerAccesFrais, paireDeReponse } from '../jeton';
import { lirePrefixe, retenirLePrefixe } from '../prefixe';
import {
    lireIcone,
    listerApplications,
    listerVms,
    type ApplicationListee,
    type DepsCatalogue,
} from './catalogue';
import { batirCarte } from './cartes';
import { deposer, type Ton } from './depot';
import { launchWhenReady } from './lancement';
import { batirManifeste } from './manifeste';

const CLASSE_DE_TON: Record<Ton, string> = {
    neutre: 'message',
    succes: 'message message--succes',
    danger: 'message message--danger',
};

const params = new URLSearchParams(window.location.search);
const base = adressePlateforme(window.location, params.get('plateforme'));

const elMessage = document.getElementById('message') as HTMLDivElement;
const elList = document.getElementById('applications') as HTMLUListElement;
const elDepot = document.getElementById('depot') as HTMLElement;
const elChoisir = document.getElementById('choisir') as HTMLButtonElement;
const elThemes = document.getElementById('themes');

if (elThemes !== null) installThemeSelectorInDOM(elThemes);

function dire(ton: Ton, texte: string): void {
    elMessage.className = `${CLASSE_DE_TON[ton]} hub__message`;
    elMessage.textContent = texte;
}

// 🔴 `deps` CARRIES AN EMPTY TOKEN UNTIL `start()` (at the foot of the
// file) HAS OBTAINED IT — see its header for what this fix repairs.
// `let`, and not `const`: the closures that follow (`processOneFile`,
// `entree`, `peupler`) read `deps` AT CALL TIME, never at declaration,
// so they see the final value once `start()` has resolved — none is
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
        if (issue.etat === 'ok') icone = issue.value;
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
async function processOneFile(file: File): Promise<void> {
    dire('neutre', `Uploading ${file.name}…`);
    const resume = await deposer(file, {
        ...deps,
        maintenant: () => Date.now(),
        progression: (p) => {
            if (p.total > 0) {
                const pourcent = Math.floor((p.octets / p.total) * 100);
                dire('neutre', `${file.name} — ${p.phase} ${String(pourcent)} %`);
            }
        },
    });
    dire(resume.ton, resume.texte);
}

/* ── THE LIST ─────────────────────────────────────────────────────────── */

function entree(application: ApplicationListee): DocumentFragment {
    return batirCarte(application, {
        modele: document.querySelector<HTMLTemplateElement>('#modele-application')!,
        urlIcone: application.icone_url === null ? null : `${base}${application.icone_url}`,
        lancer: () => {
            // 🔴 THE HUB IS NOW THE ONLY SURFACE (the owner's
            //    decision, August 31st, 2026): it holds the control
            //    session itself (`installerLeBureau`, at the foot of the file), and the
            //    launched window will appear in the "My windows" section of
            //    THIS page — no need any more to open a second one from this
            //    click. The fix of August 30th (opening `shell.html` from this
            //    same gesture) therefore has no purpose any more.
            dire('neutre', `Launching ${application.nom}…`);
            void jetonFrais().then((frais) => {
                if (frais === undefined) {
                    dire('danger', 'Your session has expired. Reload the page to sign in again.');
                    return;
                }
                const horloge = {
                    now: () => Date.now(),
                    sleep: (ms: number) => new Promise<void>((r) => setTimeout(r, ms)),
                };
                return launchWhenReady(application.id, deps, horloge, () =>
                    dire('neutre', `The VM is starting… ${application.nom} will launch as soon as it is ready.`),
                ).then((resultat) => {
                    if (resultat.kind === 'vm-timeout') {
                        dire('danger', 'The VM did not start in time. Try again in a moment.');
                        return;
                    }
                    const { issue } = resultat;
                    if (issue.etat !== 'ok') {
                        dire('danger', `${application.nom} could not be launched: ${issue.refus.motif}.`);
                        return;
                    }
                    dire('succes', `${application.nom} was launched.`);
                });
            }).catch((erreur: unknown) => {
                // A network failure, at any attempt of the wait: without this the
                // "starting" message would stay on screen and the rejection unhandled.
                console.error('launch failed', erreur);
                dire('danger', `${application.nom} could not be launched: the platform did not answer.`);
            });
        },
        installer: () => {
            void publierLeManifeste(application).then(() => {
                dire(
                    'neutre',
                    `${application.nom} is ready to be installed: use the browser's « Install app ».`,
                );
            });
        },
    });
}

async function peupler(): Promise<void> {
    // ⚠️ NO GUARD ON THE TOKEN HERE: `peupler` is only called by
    // `start()` (foot of the file) AFTER `assurerAccesFrais` has
    // returned one — it is that function that decides, and `jeton.test.ts` holds it.
    const vms = await listerVms(deps);
    if (vms.etat !== 'ok') {
        dire('danger', `The machines could not be read: ${vms.refus.motif}.`);
        return;
    }
    if (vms.value.length === 0) {
        dire('neutre', "No machine is assigned to you: there is nothing to show.");
        return;
    }
    const vm = vms.value[0];

    // 🔴 **THE VM'S PREFIX IS RETAINED HERE, AND IT WAS RETAINED NOWHERE**
    // (critical ② of the final review of August 31st, 2026). `poserPrefixe` had
    // only one production caller — `connexion.ts::fetchTheSession` —, which
    // runs **only on the sign-in page**. Yet this workstream makes
    // precisely a visitor behind Pomerium obtain their token ON THE
    // HUB (`assurerAccesFrais` → `/auth/moi`) without ever going through that
    // screen: `lirePrefixe()` returned `''`, the hub listened on the session
    // `bureau` while the agent announced on `<prefixe>:bureau`, and
    // **no `fenetre-ouverte` ever arrived**. The value was there all along,
    // three lines away: `routes-vm.ts` returns it, `catalogue.ts` already parses it
    // into `VmListee.prefixe`.
    //
    // 🔴 **THE ORDER IS THE POINT**: `start()` only installs the desktop
    // AFTER this call, so that `lirePrefixe()` composes the right session
    // and lock names. The decision "which prefix to retain?" lives in
    // `prefixe.ts::prefixeDeLaVm`, pure and tested; what remains here is
    // wiring.
    retenirLePrefixe(window.localStorage, vm.prefixe);

    const applications = await listerApplications(vm.id, deps);
    if (applications.etat !== 'ok') {
        dire('danger', `The catalogue could not be read: ${applications.refus.motif}.`);
        return;
    }
    elList.replaceChildren(...applications.value.map(entree));
    dire('neutre', `${String(applications.value.length)} application(s) on ${vm.nom}.`);

    // `?app=<uuid>`: the page publishes THIS application's manifest, which
    // makes it installable. It is also what `start_url` will reopen.
    const demandee = params.get('app');
    if (demandee !== null) {
        const cible = applications.value.find((a) => a.id === demandee);
        if (cible !== undefined) await publierLeManifeste(cible);
        else dire('danger', "The requested application is not in this catalogue.");
    }
}

/* ── THE TWO DROP PATHS ────────────────────────────────────────── */

elDepot.addEventListener('dragover', (e) => {
    e.preventDefault();
    elDepot.classList.add('hub__depot--survol');
});
elDepot.addEventListener('dragleave', () => elDepot.classList.remove('hub__depot--survol'));
elDepot.addEventListener('drop', (e) => {
    e.preventDefault();
    elDepot.classList.remove('hub__depot--survol');
    const file = e.dataTransfer?.files?.[0];
    if (file !== undefined) void processOneFile(file);
});

elChoisir.addEventListener('click', () => {
    const saisie = document.createElement('input');
    saisie.type = 'file';
    saisie.addEventListener('change', () => {
        const file = saisie.files?.[0];
        if (file !== undefined) void processOneFile(file);
    });
    saisie.click();
});

// 🔴 `launchQueue` DOES NOT EXIST OUTSIDE AN INSTALLED PWA, and the test of its
// presence is an EXPLICIT `in`, never a `try`: a silent exception
// would make the two paths indistinguishable, and that is precisely what
// criterion ② must be able to tell apart. **The drag-and-drop above depends
// on nothing of what follows** — it is the amendment of 28/07/2026.
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
        void premier.getFile().then(processOneFile);
    });
}

/* ── ACCESS: THE VAULT FIRST, POMERIUM NEXT, SIGN-IN AS A LAST
   RESORT ────────────────────────────────────────────────────────────────

   🔴 WHAT THIS BLOCK REPAIRS — A DEFECT FOUND IN PRODUCTION ON AUGUST 30TH, 2026:
   the day before, this file merely READ the vault and complained
   if it was empty ("No token: sign in first.", no button, no
   link, nothing to do). The only code that knew how to obtain a token through
   Pomerium (`connexion.ts::tenterPomerium`) ran ONLY WHEN THE SIGN-IN PAGE
   LOADED. As long as the root served the session page,
   no one had seen a visitor land DIRECTLY on the hub without having
   gone through that screen — the batch that put the hub at the root had checked
   that `/` SERVES the hub, never that a visitor WITHOUT A TOKEN could use it:
   yet another check unable to turn red.

   🔴 THE RULE ("try the vault, then the refresh, then Pomerium,
   otherwise send back to sign-in") LIVES IN `jeton.ts::assurerAccesFrais`,
   NOT HERE: in the sense of the criterion set at the head of this file, changing it
   would change what the product DECIDES, so it is not wiring.
   `assurerAccesFrais` reuses `rafraichirSiNecessaire` and
   `accesParPomerium` — the latter is the path of
   `connexion.ts::tenterPomerium`, EXTRACTED rather than copied — and all three
   are held by `jeton.test.ts`. What remains HERE is pure wiring: read
   the result, and either populate or redirect. */
async function start(): Promise<void> {
    dire('neutre', 'identification…');
    const acces = await jetonFrais();
    if (acces === undefined) {
        const suite = `/${window.location.search}`;
        window.location.href = `connexion.html?suite=${encodeURIComponent(suite)}`;
        return;
    }
    deps = { base, jeton: acces, fetch: window.fetch.bind(window) };
    dire('neutre', '');

    // 🔴 **THE DESKTOP IS INSTALLED EVEN IF THE CATALOGUE FAILS** (Important ④
    // of the final review). `await peupler()` preceded `installerLeBureau`
    // without a guard: a network failure on `GET /vm` propagated uncaught —
    // `catalogue.ts` declares that an ENVIRONMENT failure propagates as
    // is —, `#message` had already been emptied two lines above, and
    // the user saw a **blank page, without a desktop and without
    // explanation**. Before this workstream the desktop lived elsewhere and survived
    // a catalogue failure: **this coupling is new**.
    //
    // ⚠️ **THE INTERACTION WITH CRITICAL ② IS THE DELICATE POINT**: the
    // prefix MUST be known before installation (see `peupler`), and it
    // comes precisely from the call that can fail. The remedy is therefore
    // to catch and install **with what we know** — that is, the
    // prefix already in the vault, set by an earlier load or by
    // `connexion.ts` —, never to give up the desktop.
    try {
        await peupler();
    } catch (e) {
        dire('danger', `The catalogue could not be read: ${(e as Error).message}.`);
    }

    // ── THE DESKTOP, IN THIS PAGE ────────────────────────────────────────
    // 🔴 THE HUB IS NOW THE ONLY SURFACE (the owner's decision,
    // August 31st, 2026). The "My desktop" link and opening on clicking
    // "Launch" were the fixes of August 30th; they have no purpose any more.
    installerLeBureau({
        signalingUrl: adresseSignaling(window.location, params.get('signaling')),
        // 🔴 **A PROVIDER, NEVER `acces`** (critical ① of the final
        // review): `ouvrirLaSession` only runs, for a follower, at the moment
        // of its PROMOTION — potentially hours later —, and an
        // access token lives ten minutes.
        jetonFrais,
        // 🔴 **READ HERE, HENCE AFTER `peupler()`**: that is what gives the lock
        // and the control session the VM's prefix (critical ②).
        prefixe: lirePrefixe(),
        fautesArmees: params.get('faute-fichiers') === '1',
        elements: {
            statut: document.querySelector<HTMLDivElement>('#statut')!,
            list: document.querySelector<HTMLUListElement>('#fenetres')!,
            modele: document.querySelector<HTMLTemplateElement>('#modele-fenetre')!,
            sectionFenetres: document.querySelector<HTMLElement>('#section-fenetres')!,
            filesSection: document.querySelector<HTMLDetailsElement>('#files-section')!,
            boutonDossier: document.querySelector<HTMLButtonElement>('#choisir-dossier')!,
            filesState: document.querySelector<HTMLDivElement>('#files-state')!,
            ecrituresDues: document.querySelector<HTMLDivElement>('#ecritures-dues')!,
            filesActions: document.querySelector<HTMLParagraphElement>('#files-actions')!,
            boutonRafraichir: document.querySelector<HTMLButtonElement>('#rafraichir')!,
            boutonReprendre: document.querySelector<HTMLButtonElement>('#reprendre-enregistrement')!,
        },
    });
}

/// 🔴 **CALLED BEFORE EACH USE, AND IT IS THE POINT OF THE DECISION OF
/// AUGUST 31ST, 2026.** A local expiry test, a network call only if it
/// is stale: the load, each launch and each icon read
/// go through here, without any timer to calibrate.
async function jetonFrais(): Promise<string | undefined> {
    const acces = await assurerAccesFrais(
        window.localStorage,
        base,
        window.fetch.bind(window),
        Date.now(),
        async (corps) => {
            // 🔴 **WRAPPED, WHERE `accesParPomerium` (`jeton.ts:175-186`)
            // HAS ALWAYS BEEN**: this path had stayed WITHOUT A PRODUCTION
            // CALLER until this task, hence never put to the test
            // of an unreachable network. Without this `try/catch`, an exception
            // (offline, DNS, CORS) would propagate uncaught through
            // `assurerAccesFrais` → `jetonFrais()` → `start()` (unhandled rejection
            // on `void start()`) or the click handler, and the
            // page would stay stuck on "identifying…" instead of
            // falling back to Pomerium (step ③ of `assurerAccesFrais`).
            try {
                const reponse = await fetch(`${base}/auth/rafraichir`, {
                    method: 'POST',
                    headers: { 'content-type': 'application/json' },
                    body: JSON.stringify(corps),
                });
                if (!reponse.ok) return undefined;
                // The shape is VALIDATED, never asserted: an `ok: true`
                // but incomplete body would be written as is to the vault (`poser`, in
                // `rafraichirSiNecessaire`) — the "poisoned vault" scenario
                // `accesDeReponse` exists to prevent on `/auth/moi`.
                return paireDeReponse(await reponse.json().catch(() => undefined));
            } catch {
                return undefined;
            }
        },
    );
    if (acces !== undefined) deps = { ...deps, jeton: acces };
    return acces;
}

void start();
