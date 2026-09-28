// The WAITING LIST of check §7.6, and ALL the doctrine that justifies it.
//
// ⚠️ `SOUS_BLOCS_CLOS` IS NO LONGER HERE — extracted to `sous-blocs-clos.mjs` by
// task 6 of S4, WITH all its doctrine, because removing the last
// entry of this list made this file GROW from 227 to 243 for an
// extraction threshold at 240. The neighbouring file carries the measurement and the reason.
//
// 🔴 EXTRACTED FROM `tokens-orphelins.mjs` BY TASK 8 OF S2, AND THE DOCTRINE
// LEFT WITH ITS DATA — it is the gesture `CLAUDE.md` explicitly requires
// ("extract, never compress"; `serveur/instances.rs` took `TAMPON`
// with the comment that justifies it). What forced the extraction is measured,
// and the measurement is written here rather than elsewhere:
//
//   The S2 plan expected from task 8 a `tokens-orphelins.mjs` SHORTER
//   than at `56b975a` (233 lines), the 18 entries removed from this list being
//   supposed to slim it down. Measured by the command on August 20th, 2026, BEFORE extraction:
//
//     list entries      28 → 10   (−18)
//     comments         104 → 153  (+49)
//     code              87 →  93  (+6: the second exclusion and `--sans-exclusion`)
//     blank lines       14 →  14
//     TOTAL            233 → 270  (+37)
//
//   **The 18 removed entries were more than cancelled out by comment** —
//   in small, the lesson this repository paid for in large: "an addition of
//   comment can cancel an extraction". And the first draft of the
//   finding above, written AT THE TOP OF `tokens-orphelins.mjs`, took the
//   file to **296**, margin 4: a box that denounced the drift
//   produced it. None of the three new blocks is padding — the
//   MEASURED reason for the second exclusion (task 7), the box about the re-tags nothing
//   checks, and the S1 report redone instead of being erased —, and
//   planing them down would have traded a truth for a number.
//
// ⚠️ THIS FILE HAS NO TEST, and it does not need one: it carries NO
// logic, only data and its justification. What uses it is
// `tokens-orphelins.mjs`, whose check fails in both directions.
// ═══════════════════════════════════════════════════════════════════════════
// THE WAITING LIST — IT HAS BEEN EMPTY SINCE SUB-BLOCK S4, TASK 6.
//
// 🔴 AND IT DOES NOT DISAPPEAR FOR ALL THAT: THE S1 STATEMENT THAT PROMISED IT
// IS CORRECTED RATHER THAN CARRIED OUT. "The day it is empty, this whole block
// disappears with it" — written further down by S1, and WRONG. Deleting this file
// would delete the EQUALITY itself: it is what turns
// `NEW ORPHAN` red for any future token declared without a caller, and S4
// declares FOUR more (`--sur-voile`, then the three container tokens).
// An empty `Map` is what makes this check STRICT; deleting it would make it
// mute. Clause ③ (`SOUS_BLOCS_CLOS`) keeps biting for the same
// reason, and it is the third red of task 6.
//
// The doctrine leaves WITH its data — an extraction rule this file already
// carries —, and the file SLIMS DOWN: 227 → see the commit message.
//
// ⚠️ THIS NUMBER IS KEPT UP TO DATE BY THE TASK THAT MAKES IT WRONG, never by a
// clean-up task later: it had 28 at the end of S1, and the FOUR
// family commits of S2 brought it down to 10 — each one removing, IN ITS COMMIT,
// exactly the entries the check had just named "TO REMOVE FROM THE
// LIST". A count that belongs to nobody drifts — this repository has paid for that
// often enough.
//
// 🔴 IT IS NOT A LOOSENING OF THE CHECK, AND THE DIFFERENCE COMES DOWN TO ONE
// WORD: EQUALITY, not inclusion. The check requires the set of orphans
// to be EXACTLY this list. It therefore fails in BOTH DIRECTIONS:
//
//   • an orphan token absent from the list  → "new orphan"          (red)
//   • a token of the list that has a caller → "to remove from here" (red)
//
// The second half is the one that matters: it makes the list SELF-CLEANING.
// A threshold ("at most N orphans") would have rotted in place; a named
// list whose every removal is FORCED by the check shrinks by itself.
// ❌ "AND THE DAY IT IS EMPTY, THIS WHOLE BLOCK DISAPPEARS WITH IT" — written
// here by S1, REFUTED by S4 (task 6), the very day it emptied: see
// the top box. It is equality that counts, not the list, and equality needs
// this file.
//
// ── 🔴 RE-TAGGING AN ENTRY IS SEEN BY NO CHECK ─────────────────────────────
// The check compares SETS OF NAMES. Changing "S2" into "S3" in an
// annotation triggers nothing, in neither direction, ever. It is the
// weakest point of this arrangement, and the door through which one
// would loosen the list without any command saying so.
// REVIEW RULE, for lack of anything better: EVERY MODIFIED ANNOTATION CARRIES ITS REASON
// AND THE SUB-BLOCK THAT MODIFIED IT. Three were modified by S2 (task 8, August 20th) —
// `--t-xs`, `--e-1`, `--r-plein` —, and the reason is the same for all three:
// they describe a LABEL / PILL family that §6 of the spec does
// NOT entrust to S2. S2 is bounded to four families — button, field, surface,
// message —, "what a sign-in screen needs", and a sign-in
// screen has neither a label nor a pill. S1 had predicted S2; the prediction
// was wrong. Making up a pill for the sole purpose of emptying three lines
// would have been emptying one check to turn another green — the very gesture
// the box above refuses.
// ❌ "NO TECHNICAL MITIGATION IS POSSIBLE: a check on these prose
// strings could not fail usefully" — the S2 plan writes it three
// times (l. 369, l. 1379-1380, risk table), and it is WRONG. A
// PARTIAL mitigation exists, and it COULD fail usefully: *no entry must
// name an already closed sub-block*. It would have turned red at the end of S2 on
// the three entries annotated "S2", FORCING the decision instead of leaving it to
// a review rule.
// ✅ IT IS BUILT — SUB-BLOCK S3, TASK 7. `SOUS_BLOCS_CLOS` below
// names it, and `tokens-orphelins.mjs` makes it fail. The sub-block that builds it
// is the one that was about to need it: S3 re-labelled TWO entries
// (`--t-2xl` and `--t-3xl`, swapped relative to the spec, the second
// naming a hub that ⑥ does not deliver), and building a safeguard in the
// sub-block that will use it is the only way to know it bites.
// ⚠️ PARTIAL, and the word stays weighed: it judges the NAMED SUB-BLOCK, never the
// CONTENT of the annotation — "S4 — the gutter between cards" changed into
// "S4 — anything" escapes it —, and it DEPENDS ON A LIST KEPT BY
// HAND: a sub-block that does not declare itself there neutralises it. It is one more
// review rule, and it is declared rather than hidden.
// ⚠️ AND AFTER S3 IT ONLY GUARDS ONE ENTRY — a mechanism for one line.
// That is an objection, and here is the answer: that line is precisely
// the one whose prose says that "no sub-block has the right to leave it in
// place without deciding", an injunction NOTHING enforced; and a
// mitigation built AFTER the fault it was meant to prevent would have nothing
// left to prevent.
//
// ── WHY THIS PALETTE IS NOT SIMPLY REDUCED TO WHAT IS USED ─────────────────
// It was the obvious route, and it is REFUSED ON MEASUREMENT, taken on August 19th,
// 2026 (S1 report): of the 50 declared contrast pairs of §4.5 that
// check §7.1 verifies, **46 cite at least one token of this list**.
// Pruning the palette to turn §7.6 green would bring §7.1 down from 50 pairs to 4 — one
// would satisfy one check by emptying the other, which is exactly the gesture
// this repository fights. Measured by the command:
//
//   node --input-type=module -e "import {PAIRES} from './src/design/contraste.ts'; …"
//   → total pairs: 50 | pairs citing at least one token without a caller: 46
//
// 🔴 THIS REPORT IS DATED FROM S1, AND IT IS WRONG IN THE PRESENT — it is left DATED
// rather than erased, because a dated report stays true as history and
// it is what founded the decision. REDONE BY S2 (task 8) ON AUGUST 20TH, 2026,
// same command, on the TEN entries of the time (S3 brought them down to ONE):
//
//   → paires totales : 52 | citant un token en attente : 0
//
// **ZERO — and this zero says the opposite of what one would think it says.** It does not refute
// S1: it shows that S1's decision HELD TO THE END. None of the
// fourteen colours per theme is an orphan any more; the ten entries of the time
// were typographic, spacing, radius and font ones, and the contrast
// pairs only cite colours. Pruning in S1 would have removed
// colours S2 uses today. ⚠️ THE ZERO HOLDS AFTER S3, AND WITHOUT BEING
// REDONE: `--police-mono`, the only remaining entry, is not a colour.
// ⚠️ AND THAT TAKES AWAY THIS BLOCK'S ARGUMENT: "§7.1 would fall from 50 pairs
// to 4" NO LONGER PROTECTS ANYTHING, since a pruning would no longer reach any
// colour. What protects the remaining entry is now only one thing — its
// annotation, and the sub-block that carries it. See the box about re-tags.
//
// ⚠️ THIS CHECK IS THEREFORE RED BY CONSTRUCTION UNTIL S4 IF ONE TAKES IT
// AS A MEASURE OF "is the palette entirely used?". That is not
// what it measures. What it measures, from S1 on, is that **the gap between
// the palette and its use is KNOWN, ENUMERATED AND DECREASING** — and that, it
// can fail as of today, in both directions.
//
// Each entry names the sub-block that will consume it. Measured on August 20th, 2026,
// at the commit of task 8 of sub-block S2.
// ═══════════════════════════════════════════════════════════════════════════
//
// 🔴 SEVEN ENTRIES LEFT AT TASK 4 OF SUB-BLOCK S3, in the very commit
// that wrote their callers — `client/src/shell.css`, the sheet of the
// shell page. The check requires EQUALITY: removing them without writing the caller
// would have returned `NEW ORPHAN`, writing it without removing them
// `TO REMOVE FROM THE LIST`, and both directions were seen red in S1.
//
//   --t-2xl    the title of the shell page       (.bureau__titre)
//   --e-5      the gutter between cards          (.bureau__fenetres)
//   --e-6      the margin of the sections        (.bureau__section)
//   --e-7      the top margin of the surface     (.bureau)
//   --t-xs     the label of the pill             (.bureau__pastille)
//   --e-1      its inner spacing                 (.bureau__pastille)
//   --r-plein  its shape                         (.bureau__pastille)
//
// ⚠️ THE LABEL / PILL FAMILY HAD BEEN RE-TAGGED "S3 or later" BY
// S2, for lack of knowing whether a pill would exist. It does: it is the open /
// closed state of a window, told by the INK and never by a background, so
// that its contrast stays within the 52 measured pairs of §7.1.
// ═══════════════════════════════════════════════════════════════════════════
//
// 🔴 TWO MORE ENTRIES LEFT AT TASK 5 OF S3, with their callers
// in `client/src/connexion.css`:
//
//   --t-3xl     the title of the sign-in screen   (.connexion__titre)
//   --lh-large  the line height of its banner     (.connexion__message)
//
// ⚠️ AND THEIR TWO ANNOTATIONS WERE WRONG, EACH IN ITS OWN WAY — corrected
// here rather than copied, as the review rule of this file requires.
//
//   ① `--t-2xl` and `--t-3xl` WERE SWAPPED relative to the spec.
//      §4.4 of the spec writes "--t-2xl … page title" and "--t-3xl … sign-in
//      screen title"; this list said the opposite. THE SPEC WINS
//      — the 32 px step goes to the title the page only has once, the
//      24 px one to the title of a page that carries sections under it.
//   ② `--t-3xl` NAMED "the title of the hub", that is, a surface that
//      sub-project ⑥ DOES NOT DELIVER: its spec §6 says so in so many words, "the
//      hub does NOT appear in this breakdown". The entry therefore attributed to S3 a
//      caller S3 could not write, and it would have stayed waiting
//      forever if the spec had not settled it.
//
// ⚠️ `--lh-large` WAS THE MOST FRAGILE OF THE NINE, and it was NOT consumed
// to empty a line: the banner of the sign-in screen carries the
// longest prose of the product — refusal reason, VM state, admission of
// no restart, network cause quoted in full. The long paragraph already
// existed; making one up would have been emptying one check to turn another green.
// ═══════════════════════════════════════════════════════════════════════════

/**
 * ⚠️ THE SUB-BLOCK IS A STRUCTURED FIELD, IT IS NO LONGER BURIED IN A SENTENCE.
 * That is what makes the mitigation possible: as long as "S4" was only a
 * prose prefix, no command could read it without guessing. The field
 * `raison` carries the rest, and it stays out of any automatic reach.
 */
/**
 * ⚠️ EMPTY SINCE S4, TASK 6, AND IT IS A NORMAL STATE — not an invitation to
 * delete this file. See the top box: it is EQUALITY that counts.
 *
 * 🔴 THE LAST ENTRY TO LEAVE IS `--police-mono`, and its doctrine left
 * WITH it. What should be kept of it fits in three lines, because the fact
 * is worth more than the prose: three sub-blocks passed on "wire it or
 * remove it" for lack of the right to change the appearance of the session
 * window; S4 has it, and it WIRED it to `#stats` (`client/src/style.css`), which
 * spec §4.3 designated as its only planned caller since S1.
 */
const EN_ATTENTE_D_APPELANT = new Map([]);

export { EN_ATTENTE_D_APPELANT };
