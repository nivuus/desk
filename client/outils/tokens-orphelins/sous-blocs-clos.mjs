// THE MITIGATION OF CLAUSE ③ OF CHECK §7.6, AND ALL ITS DOCTRINE.
//
// 🔴 EXTRACTED FROM `attente.mjs` BY TASK 6 OF SUB-BLOCK S4, AND THE DOCTRINE
// LEFT WITH ITS DATA — the rule `attente.mjs` itself carries, and that
// `CLAUDE.md` explicitly requires ("extract, never compress";
// `serveur/instances.rs` took `TAMPON` with the comment that
// justifies it). What forced the extraction is MEASURED, and the measurement is written
// here rather than elsewhere:
//
//   Task 6 was supposed to SLIM DOWN `attente.mjs` by removing its last
//   entry. Measured by the command on August 20th, 2026:
//
//     227  before the task
//     243  after removing the entry AND writing what it means
//          (+16, whereas the conditional extraction threshold is 240)
//
//   The entry that left weighed ~18 lines; what had to be written for the
//   file not to be deleted by the next sub-block weighed
//   more. It is, IN SMALL, the lesson this repository paid for in large — "an
//   addition of comment can cancel an extraction". The S4 plan
//   prescribed the outcome in advance and unambiguously: "if, against all
//   expectations, the task makes it grow beyond, IT EXTRACTS, IT DOES NOT
//   COMPRESS". No line of doctrine was shortened to reach a
//   number: planing it down would have traded a truth for a count.
//
// ⚠️ THIS FILE HAS NO TEST, and it does not need one: it carries NO
// logic, only data and its justification. What uses it is
// `tokens-orphelins.mjs`, whose clause ③ fails when an entry names a
// sub-block from here.
// ═══════════════════════════════════════════════════════════════════════════

/**
 * THE CLOSED SUB-BLOCKS OF ⑥ — no entry of the list below has the right
 * to name one.
 *
 * 🔴 ITS UPKEEP CLAUSE IS THAT OF THE LIST ITSELF: "this number is kept
 * up to date by the task that makes it wrong, never by a clean-up task
 * later". A sub-block registers itself in the commit that completes its
 * implementation.
 *
 * ⚠️ `S4` REGISTERS ITSELF IN TURN, AND WITH THE SAME PROPERTY AS `S3`: the
 * acceptance run and the cross review of S4 have not run yet when this
 * line is written. If either of them were to re-label an entry towards "S4",
 * the check would go red — intended behaviour, a token S4 did NOT consume
 * must not claim S4. ⚠️ AND CLAUSE ③ IS NOW THE ONLY ONE OF THE THREE
 * THAT CAN STILL BITE ON THIS FILE, the list being empty: that is
 * exactly the reason why it stays.
 *
 * ⚠️ `S3` REGISTERED ITSELF, AND ONE MUST SAY WHAT THAT MEANS: at the
 * moment this line is written, the acceptance run and the cross review of S3
 * have not run yet. It is deliberate, and the property obtained is the
 * right one — if either of them were to re-label an entry towards "S3", the
 * check would go red, which is exactly the intended behaviour: a token
 * S3 did NOT consume must not claim S3. Neither of them
 * re-labels anything; task 7 itself re-labels
 * none — it changes the FORM, not the content.
 */
const SOUS_BLOCS_CLOS = new Set(['S1', 'S2', 'S3', 'S4']);

export { SOUS_BLOCS_CLOS };
