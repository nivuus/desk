// The log line for the NEW events of P5. PURE function: it RETURNS the
// line, it does not WRITE it.
//
// 🔴 WHY THE GENERAL MIGRATION DOES NOT HAPPEN, and here is the count that
// settled it rather than an opinion. Taken on 20 August 2026:
//   - `console.<methode>` outside tests and outside harnesses gives NINETEEN
//     occurrences in ELEVEN files, one of which is a CODE STRING in
//     `base/pilote-sqlite.ts:29` and not a call — hence EIGHTEEN real sites.
//     The busiest: `agents/canal.ts` (5), `signaling/trace.ts` (3),
//     `signaling/relais.ts` (2), `http/routes-applications.ts` (2);
//
//     ⚠️ **THIS COUNT WAS MADE STALE BY P5 ITSELF, TWO TASKS AFTER IT WAS
//     WRITTEN** (cross-cutting review, 20 August 2026, remeasured by the command):
//     **TWENTY-ONE** occurrences, still in **ELEVEN** files, and
//     `agents/canal.ts` carries **SIX** of them. The two new sites are
//     `http/routes-auth.ts:262` and `agents/canal.ts:399` — the brake lines
//     that tasks 7 and 8 added, **and both go through this
//     module**. **THE DEBT, ON THE OTHER HAND, HAS NOT MOVED: it is still
//     EIGHTEEN free-form sites**, since the two additions are
//     structured. That is the number to pick up, not the occurrence count;
//   - `grep -rln 'spyOn(console' --include='*.test.ts'` gives EIGHT test
//     files that capture the console AND ASSERT ON THE CONTENT of the message:
//     `agents/canal.test.ts`, `agents/canal-apps.test.ts`,
//     `agents/registre.test.ts`, `http/routes-applications.test.ts`,
//     `http/routes-auth.test.ts`, `orchestration/inventaire-statique.test.ts`,
//     `signaling/garde-fil.test.ts`, `signaling/trace.test.ts`;
//
//   ⚠️ THESE NUMBERS ARE THOSE OF 20 AUGUST 2026, TAKEN BY THE COMMAND, AND THEY
//   ARE NOT THOSE OF THE P5 PLAN — which announces 15 occurrences, 9 files and
//   5 test files. The gap is not a mistake of the plan: sub-block G1
//   was executed ENTIRELY between its writing and this one, and it added
//   `agents/registre.ts`, `http/routes-applications.ts` and their tests. The
//   cost of the migration has therefore GROWN by three test files since
//   the decision not to do it was taken — which strengthens it rather
//   than weakening it. Recount them before relying on them: they will drift
//   again.
//   - `index.ts` carries a NAMED COUPLING: its announcement line must contain
//     `le port <n>`, otherwise `signaling/resilience.test.ts` times out after
//     10 s WITHOUT ANYTHING POINTING AT THE CAUSE.
// Eighteen sites, eight test files asserting on messages, a
// named coupling, and NO criterion to judge the result: that is exactly
// the unmeasured churn this repository punishes, at the last sub-block of a branch.
//
// WHAT THIS MODULE THEREFORE SERVES: the NEW events of P5 — `frein`, `sante`,
// `enrolement freine` — and THOSE ALONE. The fourteen existing sites keep
// their free form.
//
// ⚠️ THE COST OF THE MIGRATION IS COUNTED ABOVE SO THAT THE WORK THAT
// DOES IT DOES NOT HAVE TO RECOUNT IT. What it requires, in order: rework
// the eight test files that assert on messages (they are the
// cost, not the eighteen `console.` calls), then the `index.ts` coupling, which is
// the only one whose breakage is SILENT.
//
// ⚠️ AND IT DOES NOT WRITE. Returning the line rather than writing it is what makes it
// testable without `spyOn(console)` — hence what avoids adding a SIXTH
// file to the list above.

/// True if the value must be quoted. An unquoted space or `=` makes
/// the line ambiguous: one could not tell where the value ends and where the
/// next field begins.
function doitEtreCitee(value: string): boolean {
    return value === '' || /[\s="]/.test(value);
}

/// Returns `evenement k=v k=v`.
///
/// 🔴 NO VALUE IS TRUNCATED, and it is not a detail: a truncated
/// address is AMBIGUOUS — `203.0.113.7` and `203.0.113.70` would read the same
/// —, and the operator could no longer recognise the address of their proxy.
/// Yet that is the ONLY remedy for the failure mode named in
/// `http/adresse-source.ts`: a proxy whose trust was not declared
/// makes the per-address brake degenerate into a GLOBAL brake, and the only thing that
/// makes it visible is this line.
///
/// ⚠️ THE FIELD ORDER FOLLOWS THE OBJECT'S, never a sort: two lines of the
/// same event must be comparable by eye.
export function ligne(evenement: string, champs: Record<string, string | number>): string {
    const morceaux = [evenement];
    for (const [cle, brut] of Object.entries(champs)) {
        const value = String(brut);
        morceaux.push(
            `${cle}=${doitEtreCitee(value) ? `"${value.replace(/"/g, '\\"')}"` : value}`,
        );
    }
    return morceaux.join(' ');
}
