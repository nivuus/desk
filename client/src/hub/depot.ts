// THE CONVERGENCE POINT OF THE TWO PATHS FOR DROPPING AN INSTALLER.
//
// 🔴 IT IS THIS MODULE THAT MAKES CRITERION ② DECIDABLE, and its existence is
// a decision, not tidying. The amendment of 28/07/2026 to the product framing
// says: "**attempt registration, with a silent fallback to
// drag-and-drop**", and **no feature may depend** on
// `file_handlers`. Two paths therefore lead a file here:
//
//   ① DRAG-AND-DROP (`drop`, `DataTransfer.files`) and the file
//      picker — the NOMINAL path, which must work ALONE;
//   ② the LAUNCH QUEUE (`launchQueue.setConsumer`), which only exists if
//      the browser honoured `file_handlers` — hence only in an installed
//      PWA, and not at all elsewhere.
//
// **Both call `deposer`, and nothing else.** That is what gives its
// meaning to criterion ②'s RED run: removing ② must leave ① GREEN. Had they
// each their own sequence, the red run would only measure the half it
// removes, and a drop broken on path ① would go unnoticed.
//
// 🔴 NO DOM HERE. `televerser` already does all the work — fingerprint,
// create, upload the missing slices, seal — and it is PURE, its
// dependencies injected. This module only **converges** and **translates the
// result into a sentence**, which is the only thing the two paths
// had in common and that neither should carry twice.

import { televerser, type DepsTeleversement, type Issue } from './televersement';

/// What the hub shows after a drop: a tone and a sentence.
///
/// ⚠️ `Ton` IS COPIED RATHER THAN IMPORTED, and it is a KNOWN debt of the repository,
/// not negligence: `Ton` and `CLASSE_DE_TON` are already duplicated between
/// `shell.ts`, `connexion.ts` and `ecran-terminal.ts` — it is legacy item no. 8 of ⑥,
/// whose named landing place is the `design/` layer. G5 does not unify it
/// (⑥ is closed, and ④ has no jurisdiction over its modules) and **does not make it worse
/// either**: it reuses the vocabulary instead of inventing a
/// fourth one.
export type Ton = 'neutre' | 'succes' | 'danger';

export interface Resume {
    ton: Ton;
    texte: string;
    /// The upload identifier, when there is one — it is what
    /// would allow RESUMING. Present even on a refusal, `televerser`
    /// capturing it rather than passing it at each outcome.
    id?: string;
}

/// Drops a file, and returns what should be said about it.
///
/// ⚠️ THIS FUNCTION DOES NOT THROW ON A REFUSAL — it returns a `Resume` for it.
/// An ENVIRONMENT failure (the `fetch` that rejects outside an interruption)
/// propagates as is: it is the arbitration of
/// `plateforme/src/orchestration/refus.ts`, which `televersement.ts` already holds,
/// and disguising it here would pass a failure off as a protocol decision.
export async function deposer(file: File, deps: DepsTeleversement): Promise<Resume> {
    return resumer(file, await televerser(file, deps));
}

/// Translating an outcome into a sentence. SEPARATED from `deposer` to be
/// testable without setting up a complete fake `fetch`.
export function resumer(file: File, issue: Issue): Resume {
    if (issue.etat === 'scelle') {
        return {
            ton: 'succes',
            texte: `${file.name} a été téléversé et scellé (${issue.deposees.length} tranche(s) déposée(s)).`,
            id: issue.id,
        };
    }
    const r = issue.refus;
    // ⚠️ THE SERVICE'S REASON IS RETURNED AS IS, NEVER REWRITTEN. The vocabulary
    // of refusals belongs to `plateforme/`, which `client/` cannot import;
    // translating it here would make a copy no type confronts with its
    // source, silently wrong on renaming — the defect
    // `connexion.ts` declares about `aucune-vm` and which P4 bequeathed without closing it.
    const texte =
        r.source === 'client'
            ? `${file.name} n'a pas été téléversé : ${r.motif} (${r.detail}).`
            : `${file.name} a été refusé par le service à l'étape « ${r.etape} » : ${r.motif}.`;
    return { ton: 'danger', texte, id: issue.id };
}
