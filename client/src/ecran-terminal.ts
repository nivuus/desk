// THE FULL-FRAME SCREEN OF TERMINAL STATES — sub-project ⑥, sub-block S4,
// task 9. It is family 1 of §5.2 of the spec: "a terminal state stops
// taking up six lines in a banner with a 12 px margin".
//
// ═══════════════════════════════════════════════════════════════════════════
// 🔴 THIS MODULE MOVES NO BOUNDARY, IT MAKES VISIBLE THE ONE THE CODE
// ALREADY CARRIES. "Terminal" is not a word invented here: `status.ts` has
// distinguished since workstream E a TERMINAL message from a PERSISTENT one and from an
// ordinary one, and EXACTLY TWO calls of the product pass
// `terminal: true` — the end of session and the connection failure, both in
// `main.ts`. The eleven other calls of that file stay on the banner, including
// the persistent messages of the degraded link and of sleep.
//
// 🔴 THE SCREEN PLUGS INTO `createStatus`, NOT INTO ONE MORE CALLER.
// `status.ts` exists precisely so that "no caller can forget the
// guard": adding an OPTIONAL target to it keeps this single write point,
// whereas an `ecran.montrer(...)` written next to `statut.show(...)` in
// `main.ts` would be two writes nothing forces to stay in agreement.
//
// ⚠️ PURE, INJECTED DEPENDENCIES, TESTABLE WITHOUT A DOM — the convention of
// `status.ts`, `audio.ts` and `fullscreen.ts`. The only point touching the DOM
// is `createTerminalScreenInDOM`, the one-line adapter called by `main.ts`,
// exactly like `armerPleinEcranAuDOM` in `fullscreen.ts`.
//
// ⚠️ THIS SCREEN CARRIES NO ACTION — neither "retry" nor "close". An
// action is a PRODUCT behaviour, which belongs to sub-project ②, and
// inventing it here would give it birth without an acceptance run. WHETHER IT IS THE RIGHT SHAPE IS
// A HUMAN JUDGEMENT (spec §8): no command will say so, and no eye has
// gone over ⑥ from one end to the other.
//
// ⚠️ DECLARED DUPLICATION, NOT RESOLVED. `client/src/shell.ts` already declares a
// `Ton` type and `client/src/connexion.ts` a `CLASSE_DE_TON` table. S4 does not
// unify them: that would touch two closed surfaces, with no criterion able
// to catch a regression and no eye to see it. It is legacy item no. 8 of the
// list ⑥ leaves open.
// ═══════════════════════════════════════════════════════════════════════════

/// The tone of a terminal state. A NORMAL end (the user closed
/// the remote application) is not an error; a connection failure is
/// one. The two terminal sites of `main.ts` change ONLY to carry this
/// word.
export type TonTerminal = 'neutre' | 'danger';

/// What this module needs from the screen, and nothing more.
export interface CibleEcranTerminal {
    /// The full-frame container. `hidden` as long as no terminal state has
    /// occurred — and the stylesheet must set `.ecran[hidden] { display: none }`
    /// EXPLICITLY, a class selector winning in specificity over the
    /// `[hidden]` of the user agent's stylesheet. Without this rule,
    /// the screen would be visible from load time, on every session;
    /// `client/src/style.test.ts` makes it a command.
    racine: { hidden: boolean };
    /// The title, written from the TONE and never from the caller.
    titre: { textContent: string };
    /// The reason, written from the message, and which carries the tone class.
    raison: { textContent: string; className: string };
}

export interface EcranTerminal {
    /// Raises the screen, writes the message into it, and sets the tone. A second call
    /// REPLACES the first — the last definitive information
    /// wins, the same rule `status.ts` applies to the banner.
    montrer(message: string, ton: TonTerminal): void;
}

/// ⚠️ THE TITLE IS DERIVED FROM THE TONE, AND NOT COPIED INTO THE HTML. A
/// static title would be WRONG in one of the two cases: "Session ended" lies about a
/// failure where no session ever started. Deriving it from the tone costs this
/// table and makes it right on both sides.
/// ⚠️ WHETHER THESE TWO LABELS ARE THE RIGHT WORDS IS A HUMAN JUDGEMENT, and
/// it is one more than the seven S4's plan provided for — it is declared
/// rather than passed over in silence.
const TITRE: Record<TonTerminal, string> = {
    neutre: 'Session ended',
    danger: 'Session failed',
};

/// The whole class is REWRITTEN at each call, never added to: that is what
/// makes a second, neutral terminal state erase the first one's `--danger`.
const CLASSE: Record<TonTerminal, string> = {
    neutre: 'message',
    danger: 'message message--danger',
};

export function createTerminalScreen(cible: CibleEcranTerminal): EcranTerminal {
    return {
        montrer(message, ton) {
            cible.titre.textContent = TITRE[ton];
            cible.raison.textContent = message;
            cible.raison.className = CLASSE[ton];
            cible.racine.hidden = false;
        },
    };
}

/// The DOM adapter, called by `main.ts` — the
/// `armerPleinEcranAuDOM` convention of `fullscreen.ts`. It carries NO rule: everything
/// that is tested lives in `createTerminalScreen` above.
export function createTerminalScreenInDOM(): EcranTerminal {
    return createTerminalScreen({
        racine: document.querySelector<HTMLDivElement>('#fin')!,
        titre: document.querySelector<HTMLHeadingElement>('#fin .ecran__titre')!,
        raison: document.querySelector<HTMLParagraphElement>('#fin-raison')!,
    });
}
