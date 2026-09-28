// WHAT A KNOWN WINDOW BECOMES ON SCREEN — the rule, pure and tested.
// Placing it in the template lives in `fenetres-dom.ts`, which decides nothing.
//
// 🔴 WHY THIS SPLIT RATHER THAN A SINGLE DOM MODULE. `client/` has **neither
// jsdom nor happy-dom**, and it is not a lack: `accent-dom.test.ts`
// declares it as the repository's convention — "dependencies are injected, and
// that is what makes this module testable where `main.ts` is not". This
// file takes the other route, also accepted: moving the DECISION out of the
// wiring, and leaving facing the DOM only what sets already
// computed values.

import type { FenetreConnue } from '../shell';

export interface LigneFenetre {
    session: string;
    titre: string;
    /// The word shown in the badge — text for a human, hence accented.
    etat: 'ouverte' | 'closed';
    /// 🔴 A BOOLEAN, NOT A CLASS NAME: `design/classes.ts::
    /// classesEmployeesTs` only recognises `classList.add('…')` and
    /// `className = '…'` with a literal IN the call — a class that
    /// went through a variable would be invisible to check §7.9. The
    /// pure rule therefore decides the STATE, never the class NAME; the literal
    /// name lives in the wiring (`fenetres-dom.ts`), the only form
    /// the check can see.
    ouverte: boolean;
    /// An open window has nothing to reopen: its button GOES rather than
    /// being disabled — there is no action to suggest.
    rouvrable: boolean;
}

export function lignes(fenetres: FenetreConnue[]): LigneFenetre[] {
    return fenetres.map((f) => ({
        session: f.session,
        titre: f.titre,
        etat: f.ouverte ? 'ouverte' : 'closed',
        ouverte: f.ouverte,
        rouvrable: !f.ouverte,
    }));
}

/// Must the "My windows" section appear?
///
/// 🔴 ABSENT, NOT EMPTY (spec §4.1): a section permanently showing
/// "no open window" would be noise on the NOMINAL state of a hub
/// just opened. ⚠️ A CLOSED window counts: it carries its
/// "Reopen" button, so hiding it would deprive the user of the only gesture that
/// brings it back.
export function sectionVisible(fenetres: FenetreConnue[]): boolean {
    return fenetres.length > 0;
}
