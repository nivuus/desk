// The refusal vocabulary: what an orchestrator returns when it cannot
// do what it is asked.
//
// 🔴 THIS MODULE IS PURE.
//
// 🔴 A REFUSAL IS A VALUE, NEVER A SILENCE. Neither `Promise<void>`, nor
// `boolean`, nor exception: spec §3.6 calls this "the most important shape
// decision", and its reason fits in one sentence — a `Promise<void>`
// that does nothing would be indistinguishable from a `Promise<void>` that does the
// work, that is a silent failure. A `boolean` would not say WHY,
// and an exception would make the answer 500 where the service must admit 501.
//
// 🔴 THE REASON IS A SHORT CODE, NOT A SENTENCE. Spec §3.6 proposed
// `{ refus: 'not supported by this backend' }`; the whole rest of the service uses
// a code (`{refus:'identifiants'}` in `http/routes-auth.ts`,
// `{refus:'methode'}` and `{refus:'interne'}` in `http/serveur.ts`, the four
// reasons of `identite/garde.ts`). A sentence cannot be compared, cannot be translated,
// and gets rewritten without anything breaking.

import type { Operation } from './interface';

/// Who refuses. ⚠️ It is an EXPORTED CONSTANT, never a literal copied at the
/// point of use: its value shows the day a second backend exists, and
/// two copies of a name diverge as soon as one is renamed.
export const BACKEND_STATIQUE = 'inventaire-statique';

/// The backend that wakes the VM through the host's control socket.
export const BACKEND_HOTE = 'hote';

/// All the reasons, without exception.
///
/// 🔴 THE ARRAY PRODUCES THE TYPE, never the reverse — same reason as in
/// `orchestration/interface.ts`: the runtime list and the type list
/// are THE SAME OBJECT.
export const MOTIFS = [
    /// The backend cannot do it — 501, and logged.
    'non-supporte',
    /// Unknown, OR belonging to someone else: the SAME refusal, and that is
    /// deliberate. Telling them apart would make an ENUMERATION ORACLE — a
    /// user would learn which VMs exist by reading the status
    /// code. Third application of the rule, after `routes-auth.ts` and
    /// `agents/enrolement.ts` (D8).
    'vm-inconnue',
    /// The VM already has an owner. ⚠️ This reason NEVER leaves through an HTTP
    /// route: it would only leave through an assignment, which is not exposed there.
    'vm-deja-attribuee',
    /// The user already has a VM — it is the partial index `vm_un_utilisateur` (policy: allow-fr - frozen wire key or SQLite column)
    /// that says so, by THROWING. Same exposure caveat as above.
    'utilisateur-servi',
    /// The user has no VM assigned.
    'aucune-vm',
    /// The host's control channel is absent, refused, or did not answer.
    /// This reason says the WAKE-UP was not requested; it says nothing about
    /// the state of the VM.
    'hote-inaccessible',
    /// `vu_a` too old, or null (`agents/fraicheur.ts`).
    'agent-injoignable',
] as const;
export type Motif = (typeof MOTIFS)[number];

/// What an orchestration operation returns.
///
/// ⚠️ Success carries NO data, and that is enough: the three verbs
/// that succeed in v1 (`lister`, `etat`, `attribuer`) return either their
/// own value, or this `Outcome`. Slipping an optional field into it would make
/// `ok:true` an object whose content would have to be checked.
export type Outcome =
    | { ok: true }
    | { ok: false; motif: Motif; operation: Operation; backend: string };

/// The HTTP code of each reason.
///
/// 🔴 `Record<Motif, number>` IS THE STRUCTURAL REMEDY, and it is chosen
/// deliberately AGAINST a handwritten list. Adding a reason without giving
/// it its code is a COMPILE ERROR, which `npm run typecheck` —
/// a step of `scripts/verify-all.sh` — catches. This repository paid four times for
/// a catch-all `match` that silently killed a thread (`pont_media.rs`,
/// sub-blocks D5 to D8), and `proto/ts/control.ts` still carries a `TYPES_AGENT`
/// handwritten without being confronted with its union.
///
/// ⚠️ `tsc` DOES NOT SEE AN EXTRA KEY slipped in by an `as any`: it is
/// `Object.keys(CODE_HTTP)` in the test that sees it. Both guards are
/// necessary, and the red of each one was played.
export const CODE_HTTP: Record<Motif, number> = {
    // 501 and not 500: the service is fine, it is its backend that cannot
    // do it. A 500 would send people looking for a nonexistent failure.
    'non-supporte': 501,
    'vm-inconnue': 404,
    'vm-deja-attribuee': 409,
    'utilisateur-servi': 409,
    'aucune-vm': 409,
    // 503: the VM does belong to this user, it does not answer. It is a state
    // of the world, not an error in the request.
    'agent-injoignable': 503,
    // 503: the service is fine, it is the host that does not answer the request.
    'hote-inaccessible': 503,
};

/// Builds a refusal. `backend` has `BACKEND_STATIQUE` as DEFAULT — never a
/// literal copied at each call.
export function refuser(
    motif: Motif,
    operation: Operation,
    backend: string = BACKEND_STATIQUE,
): Outcome {
    return { ok: false, motif, operation, backend };
}
