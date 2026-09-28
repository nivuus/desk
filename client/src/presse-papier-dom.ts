// Plugging the clipboard into the browser, IN BOTH DIRECTIONS: the
// mechanism's only line of DOM, and nothing else.
//
// ⚠️ This title said "the mechanism's only line of DOM" of a module that
// only listened to a `focus`; sub-block P2 added the `paste` to it, hence the
// browser → VM direction. The clause stays true — it is still the only module
// of the mechanism to touch a listener —, its scope has doubled.
//
// **Extracted BEFORE writing anything in `main.ts`** (task 15, August 20th,
// 2026): `main.ts` was at 460 lines for a project ceiling of 500, and the
// plan already named this file as the landing place if the addition crossed
// 480. The repository's rule is to extract BEFORE adding — and the prior
// extraction has a second benefit, which the plan declared out of reach: the
// wiring becomes TESTABLE. `main.ts` has no coverage; this file has
// some, because it touches neither `document` nor `navigator` directly but
// receives `ecrire`, `focalise`, `cible` and — since P2 — `emettre` by
// injection: the pattern of `attachFullscreenAuDOM` and `armerLeSon`.
//
// 🔴 **`navigator.clipboard.readText` is called NOWHERE, neither here nor
// elsewhere, neither on focus nor on click nor ever.** It was the old
// product's gesture (`web/index.js`), it requires a browser permission, and it reads
// a private resource OUTSIDE any paste intention. This module
// receives no read capability: its interface carries none, so it
// cannot acquire one by accident. **The new product asks for no
// clipboard permission**, and it is the best result of this workstream.

import { encodeClipboard } from '../../proto/ts/control';
import { PRESSE_PAPIER_MAX, PressePapierLocal, messageDeRefus, type Recu } from './presse-papier';

/// What this module needs from the window: the return of focus and pasting,
/// and nothing else. `window` conforms to it.
///
/// ⚠️ **The `paste` listener goes on the SAME target as the `focus`, that is,
/// `window` in production — never on the `<video>`.** The probe of August 20th, 2026
/// records `e.target = VIDEO#remote`: a listener set on `window` receives it
/// by BUBBLING, which the probe checks. Attaching it to the `<video>` would make it
/// mute the day a `video.focus()` gets lost — and it does get lost, the session
/// window having two corner buttons that take focus on click.
export interface CibleFocus {
    addEventListener(nom: 'focus' | 'paste', rappel: (event: EvenementCollage) => void): void;
    removeEventListener(nom: 'focus' | 'paste', rappel: (event: EvenementCollage) => void): void;
}

/// What we read from a `ClipboardEvent`. **Structural, never the DOM type**:
/// `clipboardData` is `null`able, and declaring it here allows testing that case
/// without jsdom.
///
/// ⚠️ **`preventDefault` does not appear in it, and it is not an oversight**: the module
/// never calls it. The target of the `paste` is the `<video>`, which is not
/// editable — the browser's default action pastes nothing there. Preventing an
/// action that does not happen would be noise, and would surprise the day focus
/// was in an input field.
export interface EvenementCollage {
    clipboardData: { getData(type: string): string } | null;
}

export interface OptionsPressePapier {
    /// `navigator.clipboard.writeText`, injected. **Write only.**
    ecrire: (texte: string) => Promise<void>;
    /// `document.hasFocus()`, injected: `writeText` fails on a document
    /// that does not have focus, and attempting it would cost a failure for nothing.
    focalise: () => boolean;
    /// The source of `focus` and `paste` — `window`, in production.
    cible: CibleFocus;
    /// Emits a message on the CONTROL channel.
    ///
    /// 🔴 **The control channel, never the input one**, and it matters:
    /// the input channel is `ordered: false, maxRetransmits: 0`, whereas the
    /// control one is `ordered: true`. A paste requires an ORDER — the
    /// Windows clipboard first, `Ctrl+V` next —, and an unordered
    /// channel would not provide it. Injected rather than taken from the session:
    /// that is what makes this file testable.
    emettre: (message: string) => void;
    /// The banner. Called for a size refusal, and for a repeated failure.
    surMessage: (texte: string) => void;
    /// The last content received BEFORE attachment, to replay at mount.
    ///
    /// 🔴 **It is the CLIENT half of P1's legacy no. 3, and it lives HERE because
    /// this is where it is TESTABLE.** `client/src/main.ts` does
    /// `pressePapier?.recevoir(...)` whereas `pressePapier` is only assigned
    /// in the `.then()` of `connectSession`, wired AFTER `onControl` — a
    /// message arriving in that interval was LOST SILENTLY. And since
    /// sub-block P3, the agent emits the current state AT REGISTRATION, that is,
    /// well before the browser connects: the message waits in
    /// `pending_control` and goes out as soon as the control channel opens,
    /// **that is, possibly BEFORE `main.ts` has assigned
    /// `pressePapier`**. The agent's emission therefore falls precisely in
    /// the defect's interval.
    ///
    /// ⚠️ **`main.ts` HAS NO TEST**, and it cannot have any: an entry
    /// module, top-level side effects, not importable — noted by
    /// the command, `ls client/src/*.test.ts` returns no `main.test.ts`, and
    /// no other covers it. It therefore only keeps TWO LINES OF WIRING,
    /// on the exact pattern of `micAnnonce`; the RULE — "replay the remembered one
    /// at mount" — is here, and it is tested here.
    ///
    /// ⚠️ **The replay goes through the PATH THAT ALREADY EXISTS** (`etat.recevoir`
    /// then `ecrireSiPossible`), never through a second one: D3's deferred write
    /// must stay the only write path, including at mount. A window
    /// without focus remembers and will write on its return.
    initial?: Recu;
}

export interface PressePapierAttache {
    /// An `AgentControl::Clipboard` has just arrived.
    ///
    /// ❌ **THIS DOC SAID "a message arriving before attachment is LOST, and
    /// it is declared", AND SUB-BLOCK P3 REFUTED IT — on BOTH its
    /// clauses.** It added "without consequence in practice: the agent only
    /// pushes on CHANGE, and its first poll takes the current state
    /// as a reference without announcing anything (D-P1-4), so the first announced
    /// copy necessarily follows the session's establishment".
    ///
    /// - **the message is no longer lost**: `main.ts` remembers the last
    ///   `clipboard` received and passes it as `initial`, which is replayed at mount;
    /// - **and "without consequence in practice" became FALSE**: P3 makes
    ///   the agent emit the current state AT the window's REGISTRATION, well
    ///   before the browser connects. That message does not follow
    ///   the session's establishment, it precedes it.
    recevoir(recu: Recu): void;
    /// Removes the focus listener. **Indispensable**: without it, it would outlive
    /// the end of the session and write the local clipboard for a dead
    /// session — the defect the neighbouring detachments of `main.ts` already exist
    /// to avoid.
    ///
    /// ⚠️ This sentence said "the FOUR neighbouring detachments": there are
    /// **six** (pointer, gamepad, fullscreen, arming, visibility, mic),
    /// and there already were when it was written. A quoted count must be
    /// reread, or not be quoted — fixed by the cross-cutting review of August 20th,
    /// 2026, which found the same "four" **in both places**.
    detacher(): void;
}

export function attacherPressePapierAuDOM(options: OptionsPressePapier): PressePapierAttache {
    const { ecrire, focalise, cible, surMessage, emettre, initial } = options;
    const etat = new PressePapierLocal();

    const ecrireSiPossible = (): void => {
        // The refusal is voiced BEFORE the write, and it is consumed: a received refusal
        // does not prevent a valid text remembered earlier from going out in the same
        // round, and it is not displayed again at the next round.
        const refus = etat.refusADire();
        if (refus !== undefined) surMessage(refus);

        const texte = etat.aEcrire(focalise());
        if (texte === undefined) return;
        void ecrire(texte).then(
            () => etat.confirmer(texte),
            () => {
                const message = etat.echouer();
                if (message !== undefined) surMessage(message);
            },
        );
    };

    // Replaying what arrived BEFORE attachment (client half of P1's
    // legacy no. 3). Placed AFTER `ecrireSiPossible`, which it uses, and BEFORE the two
    // listeners: nothing depends on it, but the reading order follows that of the
    // reasoning.
    if (initial !== undefined) {
        etat.recevoir(initial);
        ecrireSiPossible();
    }

    const surFocus = (): void => ecrireSiPossible();
    cible.addEventListener('focus', surFocus);

    /// The user pasted into the session window (sub-block P2).
    ///
    /// 🔴 **It is the only place in the product where the USER's clipboard
    /// is read**, and it is read through a TRUSTED `paste`
    /// event — never through `navigator.clipboard.readText()`, which would require
    /// a permission and would read a private resource OUTSIDE any
    /// paste intention. The new product asks for no clipboard
    /// permission, and it is the best result of this workstream.
    const surCollage = (event: EvenementCollage): void => {
        const texte = event.clipboardData?.getData('text/plain') ?? '';
        // An EMPTY paste emits nothing: emitting an empty string would empty the
        // VM's clipboard without the user having asked for it.
        if (texte === '') return;

        // 🔴 **THE CLIENT-SIDE BOUND IS MANDATORY, not a safety belt.** Without
        // it, the agent would indeed enforce it — but the control channel
        // would ALREADY have carried the payload, and the banner would never appear:
        // the agent refuses while logging, without sending anything back (D-P2-10). It is
        // here, and here only, that the user can be warned.
        //
        // The bound bears on UTF-8 BYTES, the same unit as the
        // agent's — `TextEncoder` rather than `texte.length`, which counts
        // UTF-16 units and would let through an emoji text twice the
        // size.
        const octets = new TextEncoder().encode(texte).length;
        if (octets > PRESSE_PAPIER_MAX) {
            surMessage(messageDeRefus(octets));
            return;
        }

        // D5's guard no. 3: we never send back to the agent what we have just
        // received from it. `aEmettre` consumes its marker — a user who
        // pastes the same text TWICE wants it twice.
        const aEmettre = etat.aEmettre(texte);
        if (aEmettre === undefined) return;
        emettre(encodeClipboard(aEmettre));
    };
    cible.addEventListener('paste', surCollage);

    return {
        recevoir(recu: Recu): void {
            etat.recevoir(recu);
            ecrireSiPossible();
        },
        detacher(): void {
            cible.removeEventListener('focus', surFocus);
            cible.removeEventListener('paste', surCollage);
        },
    };
}
