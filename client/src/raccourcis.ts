// Deciding whether a key is a PASTE shortcut, and nothing else.
//
// **PURE, DOM-free** — it knows neither `KeyboardEvent`, nor `window`, nor the
// channel: it takes the five fields that decide. It is the pattern of
// `resize.ts` and `presse-papier.ts`.
//
// 🔴 **This predicate is the ONLY defence against the spec's risk R5**, and
// that is why it lives apart rather than inlined in the listener of
// `input.ts`.
//
// ❌ **This paragraph said "TODAY `input.ts` calls `preventDefault()`
// UNCONDITIONALLY on every `keydown`", and task 18 of THIS VERY BRANCH
// made it false a few commits later.** Flagged by the cross-cutting review
// of August 21st, 2026; it is the dominant failure mode of this repository.
//
// The state BEFORE P2 — true until commit `4cf2206` — was indeed an
// unconditional `preventDefault()`: the browser saw no
// shortcut go by, which is precisely what makes the session window
// usable, `Ctrl+W` closing nothing there and `Ctrl+T` opening nothing.
// **Today the exception exists, it is exactly the one this predicate
// describes, and ANY broader condition would give the keyboard back to the browser.**
//
// A condition inlined in a listener cannot be tested; this one carries a
// truth table of **fifteen cases, thirteen of them refusals** — counted by
// `npx vitest run src/raccourcis.test.ts` on August 21st, 2026, `it.each` expanded,
// and not estimated. ⚠️ A first wording announced "thirteen cases, nine of them
// refusals": BOTH numbers were wrong, flagged by the cross-cutting review.
//
// ⚠️ **The exception can be this narrow, and it is MEASURED, not assumed.** The
// probe of August 20th, 2026 (`docs/superpowers/plans/journaux-presse-papier-p2/`,
// `p2-paste-video-{1,2}.json`, two runs) establishes that in the deciding
// cell — focus on the `<video>`, "narrow exception" regime — the `keydown`
// of `ControlLeft` **keeps its `preventDefault`** (`dp: true`) and **only `KeyV`
// goes through** (`dp: false`), the trusted `paste` arriving anyway. It is
// therefore enough to let the shortcut's own `keydown` through.

/// What the predicate needs from a `keydown`. A `KeyboardEvent`
/// conforms to it structurally, without conversion.
export interface ToucheObservee {
    ctrlKey: boolean;
    shiftKey: boolean;
    altKey: boolean;
    metaKey: boolean;
    /// `event.code` — the PHYSICAL key, never `event.key`: `key` depends on
    /// the layout, and on an AZERTY keyboard `Ctrl+V` does carry `code:
    /// 'KeyV'`. It is also the unit of `SCANCODES`.
    code: string;
}

/// True for the only two paste shortcuts D6 retains.
///
/// **The negations are written, and each has its test.** Without them,
/// `Ctrl+Shift+V` ("paste without formatting" in several applications)
/// and `Ctrl+Alt+V` would go through — that is, the Windows product would
/// lose them —, and `Meta+V` would be handled whereas the product does not handle it
/// in v1.
export function estUnRaccourciDeCollage(e: ToucheObservee): boolean {
    const ctrlV = e.ctrlKey && !e.shiftKey && !e.altKey && !e.metaKey && e.code === 'KeyV';
    const shiftInsert =
        e.shiftKey && !e.ctrlKey && !e.altKey && !e.metaKey && e.code === 'Insert';
    return ctrlV || shiftInsert;
}
