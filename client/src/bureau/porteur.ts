// ELECTING THE TAB THAT HOLDS THE CONTROL SESSION — the rule, pure
// and tested. The wiring (socket, `BroadcastChannel`, DOM) lives in
// `porteur-dom.ts`.
//
// 🔴 WHY AN ELECTION EXISTS. The `client` role is EXCLUSIVE per session
// (`plateforme/src/signaling/appariement.ts::declarer` — "a client is already
// connected to session …"). As long as the desktop lived in a
// NAMED window (`window.open(url, 'nivuus-bureau')`), there could not be
// two. Since the hub — reached through the root URL, hence openable in as many
// tabs as one wants — carries this session, the question really arises.
//
// 🔵 THE PRIMITIVE IS **Web Locks**, TAKEN AS IS RATHER THAN
// REBUILT. A tab that obtains the lock holds it until its death, and
// the browser releases it itself: it is exactly "the first keeps the
// session, and its closing promotes another". A hand-written election
// on `BroadcastChannel` would have to DETECT A PEER'S DEATH, which no
// event signals — it is the hole `setInterval(redessiner, 1000)`
// already plugs elsewhere, for lack of anything better.

import type { FenetreConnue } from '../shell';

/// The lock's name. ⚠️ **IT IS COMPOSED WITH THE VM PREFIX** before use
/// (`prefixe.ts::composer`), exactly like the session name: without it,
/// two different VMs opened in two tabs would exclude each
/// other — the defect P3 fixed on the session name, reintroduced
/// through the back door.
export const NOM_VERROU = 'nivuus-bureau';

export type Role = 'porteur' | 'suiveur';

export interface DepsElection {
    /// Requests the exclusive lock. `pendant` is called WHEN it is obtained,
    /// and the lock is held as long as the promise it returns is not
    /// settled.
    ///
    /// 🔴 **ITS TYPE WAS `Promise<never>` UNTIL THE FINAL REVIEW OF AUGUST 31st,
    /// 2026, AND THAT WAS THE DEFECT** (Important ③): a `never` promise can
    /// only settle through a REJECTION, so that the carrier dismissed by
    /// `estPlacePrise` had NO way to give back its lock — its partition
    /// would then never again have had a carrier. It is `Promise<void>` and
    /// resolves **exactly once**, when `Election::relacher` is
    /// called; on the nominal path, nobody calls it and the property
    /// "never resolves" is preserved word for word.
    ///
    /// `undefined` when `navigator.locks` does not exist.
    verrou?: (nom: string, pendant: () => Promise<void>) => void;
    /// This tab holds the session: it opens the socket.
    devenirPorteur(): void;
    /// This tab follows: it opens NO socket and displays the broadcast state.
    devenirSuiveur(): void;
}

/// What `elire` returns: the means to GIVE BACK the lock held.
export interface Election {
    /// Releases the lock, if held. **Idempotent**, and without effect
    /// as long as the lock has not been obtained — there is then nothing to give back.
    ///
    /// ⚠️ **THIS TAB DOES NOT PUT ITSELF BACK IN THE QUEUE**, and that is deliberate:
    /// requesting the lock again at once would retake it instantly (nobody
    /// else waits in THIS partition), the platform would refuse it
    /// again, and the refusal → release → retake loop would spin forever.
    /// **Declared limit**: after a resignation, this tab stays a follower
    /// until it is reloaded. What is gained is that the lock is FREE,
    /// hence another tab — or a reload — can take the place,
    /// which was impossible before.
    relacher(): void;
}

/// Elects this tab, or installs it as a follower while waiting for its turn.
///
/// ⚠️ **THE FALLBACK WITHOUT `navigator.locks` IS OPTIMISTIC, AND THAT IS DELIBERATE**:
/// declaring itself a follower would mean NO tab ever opened the session,
/// and the product would be dead on that browser. We try, the platform
/// decides, and `estPlacePrise` silently catches up the loser.
export function elire(nomVerrou: string, deps: DepsElection): Election {
    if (deps.verrou === undefined) {
        deps.devenirPorteur();
        // No lock is held on this path: there is nothing to give back.
        return { relacher: () => {} };
    }
    let rendre: (() => void) | undefined;
    deps.devenirSuiveur();
    deps.verrou(nomVerrou, () => {
        deps.devenirPorteur();
        // 🔴 A PROMISE NOBODY RESOLVES ON THE NOMINAL PATH: it is
        // the Web Locks idiom for holding a lock until the death of the
        // context. The browser releases it when the tab closes, without
        // any code having to orchestrate it — including on a crash, where
        // no `beforeunload` would run. The only caller of `resoudre` is
        // `relacher`, below.
        return new Promise<void>((resoudre) => {
            rendre = resoudre;
        });
    });
    return {
        relacher: () => {
            rendre?.();
            rendre = undefined;
        },
    };
}

/* ── PROMOTION — WHAT A TAB THAT HAS JUST TAKEN THE PLACE DOES ───────────── */

/// 🔴 **THIS SEQUENCE LIVED IN `porteur-dom.ts`, AND THAT IS WHERE
/// CRITIQUE ① OF THE FINAL REVIEW LODGED.** The socket was opened with
/// `deps.jeton`, **a string frozen at page load**. Yet a follower
/// is only promoted on the carrier's death, potentially hours later,
/// and `DUREE_JETON_ACCES_MS` is **ten minutes**
/// (`plateforme/src/identite/jeton.ts`): it therefore presented an expired token,
/// `garde.verify` returned `motif: 'expire'`, `estPlacePrise` returned `false`,
/// and `canalDeControlePerdu()` overwrote the refusal with "Reload the page".
/// **Promotion — the only thing that justifies this whole election — could
/// not work in real use.**
///
/// 🔵 **SPEC §3 ALREADY ANNOUNCED `ouvrirSocket` AS AN INJECTED
/// DEPENDENCY; the implementation had not followed it**, and that is what
/// left this junction out of reach of any test.
export interface DepsPromotion {
    /// Requests a **fresh** token again (`jeton.ts::assurerAccesFrais`). A
    /// PROVIDER, never a value: that is the whole fix.
    jetonFrais(): Promise<string | undefined>;
    /// Installs the file bridge for this tab, now carrier.
    ///
    /// ⚠️ **IT DOES NOT RECEIVE THE TOKEN, AND THAT IS INTENDED**: "Choose my
    /// folder" is a gesture that can happen any time after
    /// promotion, so a token passed HERE would be stale at click time —
    /// the defect just fixed, reintroduced one notch lower. The
    /// bridge requests its own at click time (`bureau/files-dom.ts`).
    installerPont(): void;
    /// Opens the control session's socket, with the FRESH token.
    ouvrirSocket(jeton: string): void;
    /// No token obtainable: say so in an ACTIONABLE way, rather than opening
    /// a socket doomed to refusal.
    sansJeton(): void;
}

/// The order is the point: the bridge first, the socket next, and **the token
/// requested again before both**.
export async function promouvoir(deps: DepsPromotion): Promise<void> {
    const jeton = await deps.jetonFrais();
    if (jeton === undefined) {
        deps.sansJeton();
        return;
    }
    deps.installerPont();
    deps.ouvrirSocket(jeton);
}

/// Who opens the window when "Reopen" is clicked?
///
/// 🔴 **IT IS A RULE, AND IT WAS IN THE WIRING** (Minor ④ of the final
/// review): `porteur-dom.ts` declares "NO RULE HERE" and yet carried
/// this ternary. The carrier goes through `bureau.rouvrir`, which STORES the
/// `Window` handle and lets `shell.ts::list` say "open"; a follower
/// has no fed `bureau` and opens directly — its window is real,
/// but the carrier does not see it (declared legacy of the workstream).
export function ouvertureParLeBureau(role: Role): boolean {
    return role === 'porteur';
}

/// Is this refusal "the place is already taken"?
///
/// 🔴 **ON THE TYPED REASON, NEVER ON THE SENTENCE.** `reason` is prose
/// meant for a human, and it gets reworded; a client comparing it would
/// break silently the day someone improves it. It is the trap of
/// F1, paid for with nine minutes on two messages that shared a substring.
///
/// ⚠️ **ANY OTHER REFUSAL RETURNS `false`, AND THAT IS THE POINT**: the volume
/// brake (`trop-de-requetes`, with its `retryApresS`) must stay VISIBLE.
/// Swallowing it would make this election the silent failure it claims to avoid.
export function estPlacePrise(message: unknown): boolean {
    if (typeof message !== 'object' || message === null) return false;
    return (message as { motif?: unknown }).motif === 'role-occupe';
}

/// The state the carrier broadcasts to the other tabs.
///
/// 🔴 **IT ONLY CARRIES STATE, NEVER AN ORDER.** `window.open` requires a
/// user activation **in the tab that has the gesture**: relaying a click
/// to the carrier would make it open outside activation, hence blocked. It would be
/// moving the wall one notch — which `porteur-dom.ts` explicitly refuses
/// to do (see its follower, which opens from ITS OWN click). Each
/// tab opens its own windows from its own clicks.
export interface EtatDiffuse {
    type: 'etat-bureau';
    fenetres: FenetreConnue[];
}

export function batirEtat(fenetres: FenetreConnue[]): EtatDiffuse {
    return { type: 'etat-bureau', fenetres };
}

/// Reads a message received on the channel, or returns `undefined` if it is not
/// one of ours.
///
/// ⚠️ **A `BroadcastChannel` IS SHARED PER ORIGIN**: everything passing through it
/// does not necessarily come from us, and a malformed entry is DISCARDED rather
/// than let through — a half-valid list is better than an `undefined`
/// on the `titre` field at painting time.
export function lireEtat(data: unknown): FenetreConnue[] | undefined {
    if (typeof data !== 'object' || data === null) return undefined;
    const message = data as { type?: unknown; fenetres?: unknown };
    if (message.type !== 'etat-bureau') return undefined;
    if (!Array.isArray(message.fenetres)) return undefined;
    return message.fenetres.filter(
        (f: unknown): f is FenetreConnue =>
            typeof f === 'object' &&
            f !== null &&
            typeof (f as FenetreConnue).session === 'string' &&
            typeof (f as FenetreConnue).titre === 'string' &&
            typeof (f as FenetreConnue).ouverte === 'boolean',
    );
}

/// The request a tab that has just arrived posts on the channel.
///
/// 🔴 **WITHOUT IT, A TAB JOINING AFTER STABILISATION NEVER RECEIVES
/// ANYTHING** (Important ① of the final review). `diffuserSiChange` only posts on a
/// fingerprint CHANGE, and the carrier ignored every message on the channel: in
/// steady state — three windows, nothing moving —, a second tab showed a
/// list that was **empty, forever**, which contradicts spec §4 ("Non-carrier
/// tabs display it identically").
///
/// 🔵 **IT IS NOT AN ORDER, AND §4 STILL HOLDS.** The channel only
/// carries state; a state request is a request for a
/// BROADCAST, never an order to open anything — no user
/// activation is involved.
export interface DemandeEtat {
    type: 'demande-etat';
}

export function batirDemande(): DemandeEtat {
    return { type: 'demande-etat' };
}

/// Is this message a state request?
///
/// ⚠️ **ON THE TYPE, NEVER ON PRESENCE**: a `BroadcastChannel` is
/// shared per origin, and everything passing through it does not come from us — same
/// reason as `lireEtat` below.
export function estDemandeEtat(data: unknown): boolean {
    if (typeof data !== 'object' || data === null) return false;
    return (data as { type?: unknown }).type === 'demande-etat';
}

/// Reads a RAW frame from the signaling socket, or returns `undefined` if it is not
/// a usable one.
///
/// 🔴 **ADDED BY THE FINAL REVIEW (Minor ③): `JSON.parse(evenement.data)`
/// WAS BARE IN THE CONTROL SOCKET'S LISTENER.** A non-JSON frame
/// threw there, and the exception rose into an event handler.
/// `plateforme/src/signaling/relais.ts` itself had to add the symmetric
/// guard, and `client/src/webrtc.ts::parseSignalingMessage` has carried the same
/// for a long time: `JSON.parse` succeeds on `"null"`, `"42"`, `'"x"'` and
/// `"[1,2]"`, and `null.type` throws a `TypeError`. We therefore validate **a non-null,
/// non-array object** before any property read.
export function lireTrame(brut: unknown): Record<string, unknown> | undefined {
    if (typeof brut !== 'string') return undefined;
    let analyse: unknown;
    try {
        analyse = JSON.parse(brut);
    } catch {
        return undefined;
    }
    if (typeof analyse !== 'object' || analyse === null || Array.isArray(analyse)) return undefined;
    return analyse as Record<string, unknown>;
}

/// What a tab must PAINT, given its role, its OWN list (the one
/// `shell.ts::createDesktop().list()` returns), and the LAST state received on
/// the channel — never `bureau.list()` alone.
///
/// 🔴 **THIS RULE LIVED IN `porteur-dom.ts`, WHOSE HEADER DECLARES IT
/// CARRIES NONE — AND THAT IS WHERE THE DEFECT LODGED** (review round
/// 1, critique ①). On a FOLLOWER, `bureau.list()` is structurally
/// EMPTY: no `fenetre-ouverte` message reaches its `bureau`, which
/// opens no socket (`porteur-dom.ts::ouvrirLaSession` ONLY runs on
/// the carrier), and its `rouvrir` calls `window.open` DIRECTLY without
/// going through `bureau.rouvrir`. A timer repainting from
/// `bureau.list()` on a follower would therefore ERASE, less than a second
/// after each broadcast received, the list it had just shown — not
/// an absence of information, FALSE information: the silent failure
/// this workstream claims to avoid.
///
/// 🔴 **DECLARED LIMIT, NOT FIXED (Important ② of the final review of
/// August 31st, 2026): PROMOTION ERASES THE LIST THE FOLLOWER DISPLAYED.**
/// A promoted tab switches to the `porteur` branch, whose `ownList` is
/// **structurally empty** — its `bureau` never received a single
/// `fenetre-ouverte`, for lack of a socket before promotion. It therefore paints `[]`
/// at the first round, and the state it showed disappears.
/// **The net effect, stated plainly**: *closing the carrier tab deprives
/// the other tabs of the window list for good* — the promoted carrier
/// will only get it back if the agent re-announces (windows in
/// `AttendLeViewport`, batch 17), never for an already `Vivante` window.
/// ⚠️ **IT IS NOT AN OVERSIGHT: THE FIX IS OUT OF THIS BATCH'S REACH.**
/// It would require a new method on `client/src/shell.ts` — seeding the
/// `bureau` with the received state —, yet spec §6 freezes this file by name
/// ("UNCHANGED: the desktop rule is already pure and tested"). It is a
/// design decision that belongs to the repository owner, not to a
/// fix wave.
export function fenetresAPeindre(
    role: Role,
    ownList: FenetreConnue[],
    lastReceivedState: FenetreConnue[] | undefined,
): FenetreConnue[] {
    // The carrier IS the source of truth: its own list, always — a
    // state received before its own promotion would be stale.
    if (role === 'porteur') return ownList;
    // The follower ONLY has what was broadcast to it. Nothing received yet is not
    // a lie: it is the exact initial state, before any broadcast.
    return lastReceivedState ?? [];
}
