// The freshness of an agent: deciding `prete` or `injoignable` from the
// `vu_a` column alone, which the heartbeat of the `/agent` channel advances.
//
// 🔴 THIS MODULE IS PURE. No database, no `Date.now()`: the instant is a
// PARAMETER. It is the rule of the repository (`depot/session.ts`, `identite/jeton.ts`,
// `signaling/ice.ts`), and here it carries more weight than elsewhere — criterion ④
// of the spec requires the test to SEE the transition, which a clock read
// inside would make impossible: there would only be a single observable
// instant, and the threshold would never be crossed in a test run.
//
// ✅ IT HAS HAD ITS PRODUCTION CALLER SINCE SUB-BLOCK P4 (20 August 2026), AND
// THE PARAGRAPH BELOW HAS BECOME HISTORY. `etatDe` is called by
// `orchestration/inventaire-statique.ts::etat`, which `GET /vm` and
// `POST /session` both read; it is legacy item no. 2 of P3, closed. The
// sentence "its two readers to date are its own test and the acceptance run of
// criterion ④" IS THEREFORE NO LONGER TRUE, and neither is the orphan declaration.
// What stays true, and is the reason this module exists: it is PURE,
// its instant is a PARAMETER, and it is the orchestrator that gives it its
// clock — which makes the transition of criterion ④ observable.
//
// 🔴 IT HAS NO PRODUCTION CALLER IN P3, AND THAT IS DECLARED RATHER THAN
// HIDDEN. The P3 plan prescribes none: what it decides — the state
// of a VM — is read by nobody as long as no view lists the VMs, which
// is the topic of P4. Its two readers to date are its own test and the
// acceptance run of criterion ④ ("a mute agent is seen as such"). Inventing a
// caller for it here would mean deciding in place of P4 where the state is displayed;
// writing it pure and tested makes it available without preempting anything. ⚠️ The repository
// has no doctrine on orphan code — two functions orphaned in
// the same branch were handled differently in sub-block D10 —, so this
// paragraph counts as a declaration, not a justification by precedent.
//
// ⚠️ THIS MODULE DOES NOT READ `agent_enrole` EITHER, and `depot/agent.ts` had
// announced it: "IT KNOWS NOTHING OF FRESHNESS: it returns `vu_a` as it is,
// `null` included. Deciding `prete` / `injoignable` is the job of a
// PURE module, with its clock as a parameter." This is that module.

/// Beyond this silence, a VM is held to be unreachable.
///
/// ⚠️ IT IS NOT CALIBRATED. No measurement judged it, no usage
/// judgement was passed on it: it joins the already long list of
/// uncalibrated constants of this repository — `BPP_MIN`, `FACTEUR_FOCUS`,
/// `PART_DORMANTE_BPS`, `HYSTERESIS`, `TAILLE_MAX_SORTIE`,
/// `DUREE_JETON_ACCES_MS`, `DUREE_SECONDES`, `OCTETS_PREFIXE`. It is a
/// minute and a half because that is several times the heartbeat period,
/// not because a bench established it; the day the heartbeat period is
/// chosen elsewhere, the two will be recalibrated TOGETHER.
export const SEUIL_INJOIGNABLE_MS = 90_000;

/// What we know about an enrolled VM. Two states only: neither "maybe",
/// nor "unknown". A VM that never beat is `injoignable`, which is
/// true and enough for any caller — there is nothing more to do with a
/// VM never seen than with a VM that went quiet.
export type EtatAgent = 'prete' | 'injoignable';

/// Decides the state of a VM whose last heartbeat is known.
///
/// 🔴 `vuA === null` RETURNS `injoignable`, never `prete`: the column is born
/// `null` at enrolment (`depot/agent.ts`), so a VM enrolled but never
/// started would be announced ready to whoever asked for it, and the error would
/// only show at the moment of opening a session on a VM that is off.
///
/// ⚠️ THE BOUND IS SHARP AND ON THE `prete` SIDE: a silence of exactly
/// `SEUIL_INJOIGNABLE_MS` is still tolerated, a silence one
/// millisecond longer no longer is. It is written this way so that the test
/// can besiege it from both sides — same shape as the expiry of
/// `identite/jeton.ts` (`maintenant >= exp`).
///
/// ⚠️ THIS FUNCTION SURVIVED A TYPE DEFECT BY ACCIDENT, and noting it
/// is worth more than the fix itself. Until the P3 acceptance run, `pg`
/// returned `agent_enrole.vu_a` as a **string** where `node:sqlite` returned a
/// `number`: `vuA` therefore received a `string` in production. Nothing
/// turned red — `maintenant - vuA` converts its operand, and the comparison
/// that follows is between two numbers. **But `vuA === null` stayed right by
/// luck, and any `+`, any `===` or any `>` placed here would have diverged depending on
/// the engine**: `'1787136773742' + 90000` is a concatenation. The defect
/// is fixed in the driver (`base/pilote-postgres.ts`); the type signature
/// is now true, and it was not.
///
/// ⚠️ A `vuA` IN THE FUTURE RETURNS `prete`, and that is not a made-up case:
/// the platform and the VM do not share the same clock, and `vu_a` is written by
/// the platform on receipt of the heartbeat. A negative gap is therefore a negative
/// silence, that is, no silence at all.
export function etatDe(vuA: number | null, maintenant: number): EtatAgent {
    if (vuA === null) return 'injoignable';
    return maintenant - vuA > SEUIL_INJOIGNABLE_MS ? 'injoignable' : 'prete';
}
