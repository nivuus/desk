// The orchestration vocabulary: what a VM is for the service, which
// state it can be in, and which verbs exist.
//
// 🔴 THIS MODULE IS PURE. No database, no socket, no clock: it only holds
// types and two `as const` arrays.
//
// 🔴 THE DIRECTION OF DERIVATION IS "ARRAY → TYPE", NEVER THE REVERSE. Writing
// `type Operation = 'lister' | …` first then an array of values beside it
// produces TWO objects one hopes are equal; here the runtime list and the
// type list are THE SAME OBJECT. This repository paid four times for a silent
// catch-all (`pont_media.rs`, sub-blocks D5 to D8), and `proto/ts/control.ts`
// still carries a handwritten `TYPES_AGENT` never confronted with its
// union — that is the shape we do not reproduce.
//
// ⚠️ `satisfies readonly Operation[]` is NOT enough, and it must be said: it
// forbids writing a verb that does not exist, never FORGETTING one. It is
// the union test of `interface.test.ts` that forbids forgetting, and it runs
// under `vitest`. Each covers the blind spot of the other — the doctrine of
// `base/sous-ensemble.test.ts`. (policy: allow-fr - file name)

import type { EtatAgent } from '../agents/fraicheur';
// TYPE-only import, hence erased at compile time: the cycle
// `interface.ts` <-> `refus.ts` does not exist at runtime.
import type { Outcome } from './refus';

/// The state of a VM, as a v1 backend can know it.
///
/// 🔴 IT IS A RE-EXPORT OF `EtatAgent`, AND IT HAS EXACTLY TWO MEMBERS —
/// whereas spec §3.6 sets four (`arretee`, `demarrage`, `prete`,
/// `injoignable`). The first two assume a hypervisor, which NO
/// P4 backend drives (`InventaireStatique` only inventories what
/// an administrator enrolled): writing them would produce two variants that
/// nothing emits, that is dead code IN A TYPE — the kind hardest
/// to remove, because no test turns red on it.
///
/// The day a hypervisor backend produces them, it will widen the union — and
/// any exhaustiveness depending on it will break AT COMPILE TIME. That is the right
/// failure: noisy, and that is the reason the code table of
/// `refus.ts` is a `Record<Motif, number>` and not a free object.
export type EtatVm = EtatAgent;

/// A VM of the inventory, as the orchestrator returns it.
///
/// ⚠️ IT CARRIES `adresse`, AND THE HTTP ROUTES DO NOT COPY IT. The address
/// is internal topology: the browser talks to signaling, never to the
/// VM. Administration needs it, an authenticated user does not — that is
/// why it lives here and not in a response body (D7).
export interface Vm {
    id: string;
    nom: string;
    adresse: string;
    /// `null` = in the pool, belonging to nobody. ⚠️ "To nobody" is NOT "to
    /// everybody": `orchestration/selection.ts` returns it to no user.
    userId: string | null;
    /// `null` if the VM has never been enrolled as an agent.
    prefixe: string | null;
    /// The last heartbeat, `null` if the agent has never beaten. It is the
    /// only input of `fraicheur.etatDe`.
    vuA: number | null;
}

/// ALL the verbs of the orchestrator, without exception.
export const OPERATIONS = [
    'lister',
    'etat',
    'demarrer',
    'arreter',
    'instantane',
    'attribuer',
] as const;
export type Operation = (typeof OPERATIONS)[number];

/// The operations that `POST /vm/:id/:operation` recognises.
///
/// 🔴 IT IS AN ALLOW LIST, never a deny list — the word that
/// `http/serveur.ts` already uses for routing WebSocket upgrades. A
/// path carrying a verb missing from here is not "refused": it is NOT
/// SERVED, and the generic 404 applies. An unknown verb receiving a
/// 501 would lie about the existence of the operation.
/// ⚠️ The page server (`http/page/`, chained last since 22 August
/// 2026) does not override this 404, and for a precise reason rather than by
/// luck: these paths only arrive via `POST`, and the page server steps aside
/// outside `GET`/`HEAD`. See `http/chaine.ts`. (policy: allow-fr - file name)
///
/// 🔴 `attribuer` IS NOT IN IT, AND A NAMED TEST HOLDS THAT. There
/// is no administration role in this service (`identite/jeton.ts`
/// only knows `user` and `agent`): an assignment route would be
/// open to any authenticated user, hence a privilege escalation
/// on a plate. Assignment goes through `npm run admin:attribuer` (D8).
export const OPERATIONS_HTTP = [
    'demarrer',
    'arreter',
    'instantane',
] as const satisfies readonly Operation[];

/// The exact complement: the verbs that exist and that HTTP does not expose.
///
/// ⚠️ IT IS WRITTEN RATHER THAN COMPUTED, and that is deliberate: a computed
/// complement would be true by construction, hence could never turn red. Written
/// by hand, it is confronted with `OPERATIONS` by the union test — that is what
/// makes forgetting a verb DETECTABLE.
export const OPERATIONS_HORS_HTTP = [
    'lister',
    'etat',
    'attribuer',
] as const satisfies readonly Operation[];

/// What an orchestration backend can do — and what it refuses.
///
/// 🔴 THE THREE ACTION VERBS RETURN A `Outcome`, NEVER A
/// `Promise<void>`. That is the point spec §3.6 calls "the most important shape
/// decision": a `Promise<void>` that does nothing would be
/// indistinguishable from a `Promise<void>` that does the work. See
/// `orchestration/refus.ts`.
///
/// ⚠️ `lister` AND `etat` DO NOT RETURN A `Outcome`, and that is deliberate: an
/// empty inventory is an inventory, not a refusal, and an unknown VM has a
/// state — `injoignable` —, which is true and sufficient. That is already what
/// `agents/fraicheur.ts` says of a VM never seen.
export interface Orchestrateur {
    /// The WHOLE inventory. Filtering by user belongs to
    /// `orchestration/selection.ts`, which is pure.
    lister(): Promise<Vm[]>;
    /// The state of a VM. An unknown VM returns `injoignable`, never an
    /// exception.
    etat(vm: string): Promise<EtatVm>;
    start(vm: string): Promise<Outcome>;
    arreter(vm: string): Promise<Outcome>;
    instantane(vm: string, nom: string): Promise<Outcome>;
    attribuer(vm: string, user: string): Promise<Outcome>;
}
