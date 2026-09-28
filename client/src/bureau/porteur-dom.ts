// THE DESKTOP'S WIRING IN THE HUB: election, control session socket,
// cross-tab channel, DOM.
//
// 🔴 THIS FILE IS THE SUCCESSOR OF `shell-page.ts`, AND IT DOES NOT CARRY OVER ITS
// DEFECT: its dependencies are INJECTED, and the three rules it uses
// (`porteur.ts`, `fenetres-dom.ts`, `shell.ts`) are tested elsewhere.
//
// ⚠️ NO RULE HERE. A condition that would decide something about the product
// must move down into `porteur.ts` or `shell.ts`.
//
// 🔴 THIS DECLARATION WAS CAUGHT OUT TWICE, AND BOTH ARE
// NAMED RATHER THAN KEPT QUIET:
//   ① review round 1 — "which list does this tab paint" WAS a rule,
//      set here in the form of a `bureau.list()` called unconditionally by
//      the timer. Moved down into `porteur.ts::fenetresAPeindre`.
//   ② FINAL review (August 31st, 2026, Minor ④) — "who opens the window on a
//      Reopen click" was another, in the form of a ternary
//      `porteur ? bureau.rouvrir : window.open`. Moved down into
//      `porteur.ts::ouvertureParLeBureau`. **The promotion sequence**
//      (request a token again, install the bridge, open the socket) was
//      moved down into `porteur.ts::promouvoir` by the same review.
//
// ⚠️ WHAT STAYS HERE, AND WHICH THE DECLARATION MUST NOT CLAIM TO HAVE
// MOVED OUT: the choice of displayed TEXTS and the wiring of DOM listeners.
// Changing a text changes no product decision — it is this repository's
// reproducible criterion, and it is applied here rather than assumed.

// ⚠️ NO import of `adresseSignaling` HERE: the URL arrives through `deps`, computed
// by `hub/page.ts`. Importing it without using it would be a `TS6133`, that is,
// a FAILURE of `tsc --noEmit`, not a warning.
import { composer } from '../prefixe';
import { createDesktop, type FenetreConnue, type Ton } from '../shell';
import { dessinerFenetres } from './fenetres-dom';
import { installerLePont } from './fichiers-dom';
import {
    NOM_VERROU,
    batirDemande,
    batirEtat,
    elire,
    estDemandeEtat,
    estPlacePrise,
    fenetresAPeindre,
    lireEtat,
    lireTrame,
    ouvertureParLeBureau,
    promouvoir,
    type Election,
} from './porteur';

declare global {
    interface Navigator {
        locks?: { request(nom: string, options: { mode: 'exclusive' }, pendant: () => Promise<void>): Promise<void> };
    }
}

/// What a tab that does not hold the desktop says about ITSELF.
///
/// 🔴 **A FOLLOWER WAS SILENT ABOUT ITS OWN STATE** (Minor ⑥ of the final
/// review): the only text explaining it was written in `#etat-fichiers`,
/// **inside a collapsed `<details>`**, and `#statut` stayed empty.
/// ⚠️ **IT IS NOT AN ERROR MESSAGE** — the decision "no error on the
/// second tab" (spec §2) does not forbid INFORMING, and this line is what
/// makes understandable the fact that a "Launch" clicked here makes
/// the window appear in the other tab.
const TEXTE_SUIVEUR = 'Bureau tenu par un autre onglet.';

/// The election lock's name, PREFIXED by the VM.
///
/// 🔴 WITHOUT THE PREFIX, two different VMs opened in two tabs
/// would exclude each other: the defect P3 fixed on the session
/// name, reintroduced through the back door. `composer` returns the bare name
/// when no prefix is known — exactly the behaviour from before P3.
export function nomDuVerrou(prefixe: string): string {
    return composer(prefixe, NOM_VERROU);
}

export interface CanalDiffusion {
    postMessage(message: unknown): void;
}

/// Broadcasts the state to the other tabs **only if it has changed**, and returns the
/// new fingerprint.
///
/// ⚠️ THE CARRIER REDRAWS AT 1 Hz (the user closing a window
/// warns nobody: we reread the state rather than wait for
/// an event that does not exist). Broadcasting at every round would wake all
/// tabs once per second for nothing.
export function diffuserSiChange(
    canal: CanalDiffusion,
    fenetres: FenetreConnue[],
    empreintePrecedente: string,
): string {
    const empreinte = JSON.stringify(fenetres);
    if (empreinte === empreintePrecedente) return empreintePrecedente;
    canal.postMessage(batirEtat(fenetres));
    return empreinte;
}

export interface DepsBureauPage {
    signalingUrl: string;
    /// 🔴 **A PROVIDER, NEVER A STRING — AND THIS FIELD CARRIED A STRING
    /// UNTIL THE FINAL REVIEW OF AUGUST 31st, 2026** (critique ①). See
    /// `porteur.ts::DepsPromotion` for the measured defect: a follower promoted
    /// hours later presented a **ten-minute** token, expired,
    /// and promotion could not work in real use.
    ///
    /// ⚠️ **A TEST FREEZES THE ABSENCE OF A SCALAR TOKEN IN THIS INTERFACE**
    /// (`porteur-dom.test.ts`): it is the JUNCTION that was wrong, not the
    /// freshness rule, and a test of `assurerAccesFrais` would never have
    /// seen it.
    jetonFrais(): Promise<string | undefined>;
    /// The VM prefix (`prefixe.ts::lirePrefixe`), or `''` if there is none.
    ///
    /// 🔴 **IT MUST BE READ AFTER `GET /vm` HAS ANSWERED** — critique ② of the
    /// final review: the hub set no prefix, `lirePrefixe()` returned
    /// `''`, and the hub listened on `bureau` while the agent announced on
    /// `<prefixe>:bureau`. It is `hub/page.ts::start` that guarantees this
    /// order; this module only receives the value.
    prefixe: string;
    /// `?faute-fichiers=1` — BENCH variable, never a shipped
    /// configuration. Read ONCE by the page and passed as an argument, never reread
    /// here: it is the convention of `PLEIN_ECRAN` and `PART_SONDAGE` on the
    /// agent side — the mechanism reads a flag it is given.
    fautesArmees: boolean;
    elements: {
        statut: HTMLDivElement;
        list: HTMLUListElement;
        modele: HTMLTemplateElement;
        sectionFenetres: HTMLElement;
        filesSection: HTMLDetailsElement;
        boutonDossier: HTMLButtonElement;
        filesState: HTMLDivElement;
        ecrituresDues: HTMLDivElement;
        filesActions: HTMLParagraphElement;
        boutonRafraichir: HTMLButtonElement;
        boutonReprendre: HTMLButtonElement;
    };
}

export function installerLeBureau(deps: DepsBureauPage): void {
    const el = deps.elements;
    const sessionDeControle = composer(deps.prefixe, 'bureau');
    const canal = new BroadcastChannel(nomDuVerrou(deps.prefixe));
    let empreinte = '';
    let porteur = false;
    /// The LAST state received on the channel, FOLLOWER side. `undefined` as long
    /// as no broadcast has arrived yet — that is what
    /// `fenetresAPeindre` distinguishes from an empty list broadcast for real.
    let lastReceivedState: FenetreConnue[] | undefined;
    // ⚠️ DECLARED BEFORE `createDesktop`, whose `envoyer` callback reads it: a
    // closure capturing a `let` declared further down compiles, but reads
    // badly — and the temporal dead zone is an error class avoided
    // by layout rather than by vigilance.
    let socket: WebSocket | undefined;
    /// Returned by `elire`. `undefined` as long as the election has not been set —
    /// the fallback without Web Locks calls `devenirPorteur` SYNCHRONOUSLY, hence
    /// before this assignment.
    let election: Election | undefined;

    const poserTon = (element: HTMLElement, ton: Ton): void => {
        element.classList.remove('message--succes', 'message--alerte', 'message--danger');
        if (ton !== 'neutre') element.classList.add(`message--${ton}`);
    };

    /// 🔴 `/index.html`, NOT `/`: the root serves the HUB since batch 14, and
    /// `/?session=…` would open the hub with a parameter it ignores, never
    /// a session.
    ///
    /// ⚠️ **A SINGLE PLACE BUILDS THIS URL**, used by the carrier (through
    /// `bureau.ouvrirFenetre`) AND by the follower: the final review found it
    /// written twice, in two places that would have had to be kept in agreement.
    const ouvrirUneFenetre = (session: string): Window | null =>
        window.open(`/index.html?session=${encodeURIComponent(session)}`, `guac-${session}`);

    const bureau = createDesktop({
        ouvrirFenetre(session) {
            return ouvrirUneFenetre(session);
        },
        envoyer(message) {
            if (socket?.readyState === WebSocket.OPEN) socket.send(JSON.stringify(message));
        },
        show(message, ton) { el.statut.textContent = message; poserTon(el.statut, ton); },
        showFilesState(texte, ton) { el.filesState.textContent = texte; poserTon(el.filesState, ton); },
        showPendingWrites(dues, vues, texte, ton) {
            // 🔴 NUMBERS IN `data-*` ATTRIBUTES, TEXT IN THE
            // PAGE. Acceptance drivers read the attributes, NEVER the
            // text — F1's trap.
            el.ecrituresDues.dataset.dues = String(dues);
            el.ecrituresDues.dataset.vues = String(vues);
            el.ecrituresDues.textContent = texte;
            poserTon(el.ecrituresDues, ton);
        },
        showRetained(retenues) {
            el.boutonReprendre.hidden = !retenues;
            el.filesActions.dataset.retenues = String(retenues);
            // A bridge holding back its writes has something to say NOW.
            if (retenues) el.filesSection.open = true;
        },
    });

    // ── THE SINGLE PAINTING PATH, FOR BOTH ROLES ────────────────────────────
    // 🔴 THE TIMER AND THE RECEPTION OF A BROADCAST EACH CALLED THEIR
    // OWN `dessinerFenetres(...)`, TWICE OVER — it is that duplication that
    // produced critique ①: the timer painted from `bureau.list()`
    // regardless of the role, erasing on a follower, less than a second
    // later, what the reception had just shown. There is now only one
    // path: `redessiner()`, called by BOTH triggers, which asks
    // `fenetresAPeindre` (pure, tested) what must be painted.
    const redessiner = (): void => {
        const role = porteur ? 'porteur' : 'suiveur';
        const ownList = bureau.list();
        const fenetres = fenetresAPeindre(role, ownList, lastReceivedState);
        dessinerFenetres(fenetres, {
            list: el.list,
            modele: el.modele,
            section: el.sectionFenetres,
            rouvrir: (session) => {
                if (ouvertureParLeBureau(role)) {
                    bureau.rouvrir(session);
                    redessiner();
                    return;
                }
                // ⚠️ A FOLLOWER OPENS ITS OWN WINDOW, FROM ITS OWN
                // CLICK: `window.open` requires the activation of THIS tab. It
                // does not warn the carrier, and NOTHING CATCHES UP AFTERWARDS:
                // `shell.ts::list` computes `ouverte` from the HANDLE the
                // carrier ITSELF holds (`e.fenetre !== null &&
                // !e.fenetre.closed`), and that handle stays closed
                // forever — the follower has just created ANOTHER
                // `Window` object, which the carrier never sees. The carrier's list
                // will therefore say "closed" PERMANENTLY, until the
                // CARRIER ITSELF clicks "Reopen". **Declared limit**:
                // the remedy would be an order on the channel, which the design
                // excludes (spec §4), or a new method on `shell.ts`, which
                // the spec leaves UNCHANGED (spec §6). Clicking "Reopen" again,
                // on the follower side, brings the same window to the foreground for
                // IT: no harm for it; only the carrier's view stays
                // wrong.
                //
                // 🔴 **AND A SECOND CONSEQUENCE, ADDED BY THE FINAL REVIEW
                // (Important ⑤) — AN INCOMPLETE COMMENT ON A DECLARED LIMIT
                // IS WORTH A FALSE PROOF: THE WINDOW THUS REOPENED
                // NEVER ANNOUNCES ITS VIEWPORT.** `viewport-dom.ts` posts through
                // `window.opener`, hence to THIS follower; `bureau.viewportRecu`
                // returns immediately there (its `connues` table is empty, no
                // `fenetreOuverte` having ever fed it) and `envoyer`
                // is a no-op for lack of a socket. **Consequence, that of batch
                // 33: the crop and the window size stay those of
                // the previous session, and nothing traces it.** Declared
                // limit, not fixed: fixing it would require relaying
                // a message to the carrier, which spec §4 excludes.
                ouvrirUneFenetre(session);
            },
        });
        // Broadcasting, for its part, stays reserved to the carrier, and carries ITS OWN
        // list — never `fenetres`, which on a follower is the last state
        // RECEIVED: rebroadcasting it would loop the echo instead of carrying anything new.
        if (porteur) empreinte = diffuserSiChange(canal, ownList, empreinte);
    };

    // ── THE FOLLOWER: it opens NO socket; it stores what is
    // broadcast to it and has it painted by THE SAME `redessiner()` as the timer
    // — a single path, never two that could diverge.
    canal.addEventListener('message', (evenement) => {
        // 🔴 **A TAB JOINING AFTER STABILISATION NEVER RECEIVED
        // ANYTHING** (Important ① of the final review): `diffuserSiChange` only
        // posts on CHANGE, and the carrier ignored every message on the
        // channel. The carrier now answers a state request by
        // RESETTING ITS FINGERPRINT TO `''`, which makes broadcasting restart at the
        // next `redessiner` — including for an empty list, whose
        // fingerprint `'[]'` differs from `''`. A follower ignores another
        // follower's request: it has nothing to broadcast.
        if (estDemandeEtat(evenement.data)) {
            if (!porteur) return;
            empreinte = '';
            redessiner();
            return;
        }
        if (porteur) return;
        const fenetres = lireEtat(evenement.data);
        if (fenetres === undefined) return;
        lastReceivedState = fenetres;
        redessiner();
    });

    // 🔴 **THE FILE BRIDGE FOLLOWS THE ELECTION, AND IT IS A CONSEQUENCE OF THIS
    // TASK, NOT AN OVERSIGHT** (Important ③, review round 1): the bridge's session
    // (`fichiers/canal.ts::sessionDuPont`) is FIXED PER VM and carries, it
    // too, the `client` role — EXCLUSIVE. The reasoning of `porteur.ts`
    // for the control session ("as long as the desktop lived in a
    // NAMED window, there could not be two") holds WORD FOR WORD
    // here. A follower installing it anyway would open a second
    // socket the platform would refuse — not an error to show: a
    // STATE to TELL, the follower tab not being at fault for not handling
    // files. **The two functions below are SHARED** between
    // `devenirSuiveur` and catching up the dismissed optimistic fallback
    // (`estPlacePrise`, below): both paths lead to the same "this
    // tab does not handle files".
    const desactiverLePont = (): void => {
        el.boutonDossier.disabled = true;
        el.filesState.textContent = 'Les fichiers sont gérés par l’onglet qui tient le bureau.';
        poserTon(el.filesState, 'neutre');
    };
    const activerLePont = (): void => {
        el.boutonDossier.disabled = false;
        el.filesState.textContent = '';
        poserTon(el.filesState, 'neutre');
    };

    const ouvrirLaSession = (): void => {
        porteur = true;
        // THIS tab has just been promoted: cancel the state `devenirSuiveur`
        // had set — including the text of `#statut`, which would otherwise say
        // "Desktop held by another tab" whereas it is THIS one that
        // holds it now.
        el.statut.textContent = 'connexion du bureau…';
        poserTon(el.statut, 'neutre');
        activerLePont();
        // 🔴 **THE SEQUENCE IS IN `porteur.ts::promouvoir`, PURE AND TESTED**
        // (critique ① of the final review). It requests a FRESH token again
        // before opening the socket: a follower is only promoted on the
        // carrier's death, potentially hours after loading, and a token
        // frozen at load time lives **ten minutes**.
        void promouvoir({
            jetonFrais: () => deps.jetonFrais(),
            // The bridge is installed HERE rather than at module mount — so
            // a tab promoted LATER (the ordinary case: follower at
            // load time, carrier only when the previous one closes)
            // gets it too, without extra code.
            installerPont: () =>
                installerLePont({
                    bureau,
                    signalingUrl: deps.signalingUrl,
                    // ⚠️ THE PROVIDER, NOT THE TOKEN JUST OBTAINED:
                    // "Choose my folder" is a gesture that can happen
                    // any time after promotion.
                    jetonFrais: () => deps.jetonFrais(),
                    fautesArmees: deps.fautesArmees,
                    boutonDossier: el.boutonDossier,
                    boutonRafraichir: el.boutonRafraichir,
                    boutonReprendre: el.boutonReprendre,
                    section: el.filesSection,
                }),
            ouvrirSocket: (jeton) => ouvrirLeSocket(jeton),
            sansJeton: () => {
                // ⚠️ ACTIONABLE, and not "an error occurred": the
                // reload restarts `assurerAccesFrais`, hence Pomerium.
                el.statut.textContent =
                    'Votre session a expiré. Rechargez la page pour vous reconnecter.';
                poserTon(el.statut, 'danger');
                desactiverLePont();
            },
        });
    };

    const ouvrirLeSocket = (jeton: string): void => {
        socket = new WebSocket(deps.signalingUrl);
        socket.addEventListener('open', () => {
            socket!.send(JSON.stringify({ role: 'client', session: sessionDeControle, jeton }));
            el.statut.textContent = 'bureau connecté';
            poserTon(el.statut, 'neutre');
        });
        socket.addEventListener('message', (evenement) => {
            // 🔴 BARE `JSON.parse` HERE UNTIL THE FINAL REVIEW (Minor ③): a
            // non-JSON frame threw in an event handler. The guard
            // lives in `porteur.ts::lireTrame`, twin of the one
            // `plateforme/src/signaling/relais.ts` had to add on its side.
            const message = lireTrame(evenement.data);
            if (message === undefined) return;
            if (message.type === 'fenetre-ouverte') bureau.fenetreOuverte(message.session as string, message.titre as string);
            else if (message.type === 'fenetre-fermee') bureau.fenetreFermee(message.session as string);
            else if (message.type === 'refus') bureau.refus(message.titre as string, message.motif as string);
            else if (message.type === 'error') {
                if (estPlacePrise(message)) {
                    // 🔴 "THE PLACE IS TAKEN" IS NOT AN ERROR TO
                    // SHOW. This tab tried to become carrier, another
                    // already held the session on the platform side. It
                    // becomes a follower again, SILENTLY — a second tab is
                    // not the user's fault. Any other refusal, including
                    // the volume brake, stays displayed.
                    //
                    // ⚠️ **THIS PATH IS NOT RESERVED TO THE FALLBACK WITHOUT WEB
                    // LOCKS** — a claim too broad (Minor round 1):
                    // Web Locks are partitioned PER STORAGE
                    // PARTITION (a private browsing window, or a
                    // second browser, holds ITS OWN partition), so a
                    // tab can very well hold ITS lock and still target
                    // the SAME session on the platform side. This path is
                    // therefore also reached OUTSIDE the fallback, whenever two
                    // distinct partitions target the same VM.
                    porteur = false;
                    socket?.close();
                    // 🔴 **THE LOCK IS GIVEN BACK, AND IT WAS NOT**
                    // (Important ③ of the final review): the promise held by
                    // `elire` was a `Promise<never>` nothing resolved,
                    // so that a dismissed carrier kept the lock
                    // FOREVER and its partition **never again** had a
                    // carrier. It goes back to follower, lock released.
                    // ⚠️ It does NOT put itself back in the queue: see
                    // `porteur.ts::Election::relacher` for the reason (a
                    // refusal → release → retake loop) and for the limit
                    // this leaves.
                    election?.relacher();
                    // Minor round 1: the banner still said "desktop
                    // connected", set optimistically when the
                    // socket opened, BEFORE knowing whether the platform would refuse.
                    // Say it as is after the demotion: this tab IS NO
                    // LONGER the one holding the desktop.
                    el.statut.textContent = TEXTE_SUIVEUR;
                    poserTon(el.statut, 'neutre');
                    // 🔴 THIS PATH (fallback WITHOUT Web Locks) installed the bridge
                    // OPTIMISTICALLY, AT THE SAME TIME as `porteur = true`
                    // above, before knowing whether the platform would refuse
                    // -- exactly like the banner. Catch it up the
                    // same way: it is no longer this tab that handles
                    // files.
                    desactiverLePont();
                    redessiner();
                    return;
                }
                bureau.canalDeControleRefuse(
                    message.reason as string | undefined,
                    message.motif as string | undefined,
                    message.retryApresS as number | undefined,
                );
            }
            redessiner();
        });
        socket.addEventListener('close', () => {
            // ⚠️ SILENT IF WE GAVE UP THE PLACE: `canalDeControlePerdu`
            // would say "Reload the page", which would be wrong here.
            if (porteur) bureau.canalDeControlePerdu();
        });
    };

    election = elire(nomDuVerrou(deps.prefixe), {
        verrou:
            typeof navigator !== 'undefined' && 'locks' in navigator
                ? (nom, pendant) => void navigator.locks!.request(nom, { mode: 'exclusive' }, pendant)
                : undefined,
        devenirPorteur: ouvrirLaSession,
        devenirSuiveur: () => {
            porteur = false;
            // ⚠️ **TELL ITS STATE, NOT ONLY KEEP QUIET** (Minor ⑥): without
            // this line, `#statut` stayed EMPTY and the only explanation of
            // this tab's role lived in `#etat-fichiers`, inside
            // a COLLAPSED `<details>`. A tab promoted later overwrites this
            // text from `ouvrirLaSession` on ("connecting the desktop…").
            el.statut.textContent = TEXTE_SUIVEUR;
            poserTon(el.statut, 'neutre');
            desactiverLePont();
        },
    });

    window.addEventListener('beforeunload', (evenement) => {
        if (!bureau.doitPrevenir()) return;
        evenement.preventDefault();
    });

    // Session pages announce their viewport through `postMessage` to their
    // opener — that is, here.
    window.addEventListener('message', (evenement) => {
        if (evenement.origin !== window.location.origin) return;
        const message = evenement.data;
        if (message?.type === 'viewport') {
            bureau.viewportRecu(message.session, message.largeur, message.hauteur);
        }
    });

    // 🔴 **THE STATE REQUEST, SENT AT MOUNT** (Important ① of the final
    // review). Without it, a tab joining AFTER stabilisation — three
    // windows, nothing moving — stays on an empty list FOREVER,
    // `diffuserSiChange` only posting on change.
    //
    // ⚠️ **POSTED WITHOUT A ROLE CONDITION, AND THAT IS CORRECT**: a
    // `BroadcastChannel` does NOT deliver to its own sender, and there is only
    // one carrier per partition — a tab that has just been elected carrier
    // therefore cannot wake itself up, and its request finds nobody to
    // ask. Waiting to know the role would require waiting for
    // Web Locks to decide, that is, delaying the only thing that makes a
    // follower useful.
    canal.postMessage(batirDemande());

    // The user closing a page warns nobody: we
    // reread the state periodically rather than wait for an event that
    // does not exist.
    setInterval(redessiner, 1000);
}
