// The clipboard received from the VM, browser side: what to write, when, and
// what to say when it does not work.
//
// **PURE — no `document`, no `navigator`, no promise.** This module
// decides; it is `presse-papier-dom.ts` that calls `writeText` — received by
// injection, from `main.ts` — and reports the result back to it (`confirmer`,
// `echouer`). It is the pattern of `status.ts` and `resize.ts`, and it is what
// makes it testable without a DOM.
//
// ⚠️ **This sentence said "it is `main.ts` that […] reports the
// result back to it", and it was true when written.** The extraction of
// `presse-papier-dom.ts` — played BEFORE the addition, in the same branch —
// moved the wiring: `main.ts` no longer builds `PressePapierLocal` nor
// reports anything back to it (`grep -c PressePapierLocal client/src/main.ts`
// returns **0**). Fixed by the cross-cutting review of August 20th, 2026. It is the
// dominant failure mode of this repository: a statement that became false
// **in its own branch**.
//
// It imports nothing from `proto/ts/control.ts`: it takes a local `Recu`.
// It is deliberate — decoupling it from the protocol is what keeps it pure, and a
// change in the message's shape must not travel all the way here.

/// What the agent announced.
export interface Recu {
    /// The text to write, or `null` when the agent REFUSED the content because
    /// it exceeded its bound. `null` is not "nothing": it is a refusal,
    /// and it is voiced.
    texte: string | null;
    /// The size in bytes — that of the emitted text, or that of the refused content.
    octets: number;
}

/// What the failure message must carry: HOW to restore, not only
/// that something is missing. Same rule as the mic's `DETAIL_REFUS`
/// (`micro.ts`), and for the same reason — a message that only states the
/// symptom leaves the user with no gesture to make.
export const MESSAGE_ECHEC =
    "copie de la VM non recopiée ici — cliquez dans la fenêtre pour lui rendre le focus, puis recopiez";

/// Number of CONSECUTIVE failures before shouting.
///
/// **Two, not one**: a first failure is the ordinary case of a window that
/// does not have focus when the agent pushes, and shouting about it would make a
/// permanent banner on a product that works.
export const FAILURES_BEFORE_MESSAGE = 2;

/// Maximum size, in **UTF-8 bytes**, of a text the page agrees
/// to emit towards the agent (sub-block P2).
///
/// 🔴 **IT IS A COPY, AND NOTHING IN THE LANGUAGE CONFRONTS IT WITH ITS
/// SOURCE.** The authoritative value is `agent::presse_papier::PRESSE_PAPIER_MAX`
/// (`agent/src/presse_papier.rs`), and `client/` cannot import Rust.
/// The repository has already paid for this class — two constants written in two
/// languages with no possible `import` diverge SILENTLY (the platform's
/// sub-block P2). The remedy used is the same as then: **a test that rereads
/// the Rust file and refuses the divergence**, in `presse-papier.test.ts`.
///
/// **Why the bound is here and not only at the agent**: without it,
/// the agent would indeed enforce it, but the control channel would ALREADY have
/// carried the payload, and the banner would never appear — the agent refuses while
/// logging, without sending anything back. It is here, and here only, that
/// the user can be warned.
export const PRESSE_PAPIER_MAX = 64 * 1024;

/// The refusal message, which NAMES the size — "too large" alone does not tell
/// the user what they must reduce.
export function messageDeRefus(octets: number): string {
    const kio = Math.round(octets / 1024);
    return `copie trop volumineuse (${kio} Kio) — elle n'a pas été recopiée ici, réduisez la sélection`;
}

export class PressePapierLocal {
    /// The last text received and not written yet. **Only one**, never a
    /// queue: an obsolete write is impossible because we only keep
    /// the last.
    private enAttente: string | undefined;
    /// The last text actually written — we do not rewrite it.
    private written: string | undefined;
    /// Consecutive write failures.
    private echecs = 0;
    /// The refusal to voice, **consumable**: otherwise the banner would show again at
    /// each round.
    private refus: string | undefined;
    /// The last text RECEIVED from the agent and not yet re-emitted — **D5's guard
    /// no. 3**, and it is **consumed**.
    ///
    /// It is set by `recevoir`, never by `confirmer`: a text received without
    /// focus stays waiting to be written, and the user may paste
    /// meanwhile. The two cases are indistinguishable from outside, and the
    /// choice is to keep quiet — one round trip too many avoided costs a repeated
    /// paste the user can redo, whereas one round trip too many
    /// is traffic nothing bounds (P1's legacy no. 4).
    private recuNonReemis: string | undefined;

    /// A message arrived from the agent. **Always remembered**, even without
    /// focus: it is the deferred write.
    recevoir(recu: Recu): void {
        if (recu.texte === null) {
            // A refusal does NOT overwrite the last remembered text: otherwise it
            // would erase a valid content not written yet.
            this.refus = messageDeRefus(recu.octets);
            return;
        }
        this.enAttente = recu.texte;
        // Arms guard no. 3: this very text will not go back to the agent.
        this.recuNonReemis = recu.texte;
    }

    /// What must be written NOW, or `undefined`.
    ///
    /// Without focus we return nothing: `navigator.clipboard.writeText` fails
    /// on a document that does not have focus, and the failure would cost a counter
    /// for nothing. The text stays waiting and will go out when focus returns.
    ///
    /// ⚠️ **THIS SENTENCE ASSERTED AS A FACT WHAT THE SPEC HAS DECLARED
    /// ASSUMED SINCE JULY 28TH, 2026** (§3.3). Sub-block P3 measured
    /// it, and the verdict is finer than "true" or "false":
    ///
    /// - the cell that SETTLES it — no focus, BUT under user
    ///   activation — is **UNREACHABLE** on this setup: the trusted
    ///   gesture GIVES focus back to the window receiving it, and
    ///   `Page.bringToFront` no longer takes it away. §3.3 therefore stays
    ///   **assumed in the strict sense**;
    /// - **but it is CORROBORATED by one piece of evidence**: without focus and without a gesture,
    ///   `writeText` refuses while NAMING focus —
    ///   `NotAllowedError: … Document is not focused.` — whereas the activation
    ///   refusal says `… Write permission denied.` Both carry the
    ///   SAME NAME and DIFFERENT MESSAGES: **a refusal path specific to
    ///   focus exists, and it names itself**. What stays unmeasured is
    ///   whether it survives an activation.
    ///
    /// 🔵 **AND WITH N WINDOWS, THIS TEST DOES SOMETHING OTHER THAN PROTECT AGAINST A
    /// REFUSAL — it ELECTS the single local writer.** The sensor pushes the content
    /// to ALL windows (D3), each has its own `PressePapierLocal`,
    /// and if all wrote, N concurrent calls to `writeText` would go out
    /// for a single copy, the last one winning arbitrarily. That
    /// justification holds **independently** of §3.3, and that is why the
    /// rule stays even if §3.3 were one day refuted. Removing
    /// the rule would then be a **decision of the repository owner**, with
    /// its named cost — a regime nothing measures —, never a mechanical
    /// consequence of a probe verdict.
    ///
    /// ⚠️ **Probes filed**: `journaux-presse-papier-p3/p3-writetext-{1,2}.json`
    /// (the 2×2) and `p3-focus-{1,2}.json` (measurability of focus with N
    /// windows), two runs each, identical reports.
    toWrite(focalise: boolean): string | undefined {
        if (!focalise) return undefined;
        if (this.enAttente === undefined) return undefined;
        if (this.enAttente === this.written) return undefined;
        return this.enAttente;
    }

    /// The write succeeded.
    confirmer(texte: string): void {
        this.written = texte;
        // A success resets the counter to zero: without that, a failure at startup
        // and a failure an hour later would shout together.
        this.echecs = 0;
    }

    /// The write failed. Returns the message to display at the SECOND consecutive
    /// failure, `undefined` before.
    echouer(): string | undefined {
        this.echecs += 1;
        return this.echecs >= FAILURES_BEFORE_MESSAGE ? MESSAGE_ECHEC : undefined;
    }

    /// Returns the text to emit towards the agent, or `undefined` if it is the echo
    /// of a content we have just received from it — **D5's guard no. 3**.
    ///
    /// **It only holds for the FIRST send-back**, and it is deliberate: a
    /// user who pastes the same text twice wants it twice. The
    /// duplicate costs nothing on the VM side — the agent's guard no. 2 absorbs it,
    /// the Windows clipboard already carrying that content.
    ///
    /// ⚠️ **A refusal does not arm this guard**: nothing was written locally,
    /// so nothing can be its echo.
    aEmettre(texte: string): string | undefined {
        const recu = this.recuNonReemis;
        this.recuNonReemis = undefined;
        return recu === texte ? undefined : texte;
    }

    /// The refusal to voice, or `undefined`. **Is consumed.**
    refusADire(): string | undefined {
        const refus = this.refus;
        this.refus = undefined;
        return refus;
    }
}
