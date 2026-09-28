import { describe, expect, it, vi } from 'vitest';

import { attacherPressePapierAuDOM, type EvenementCollage } from './presse-papier-dom';
import { CONTROL_VERSION } from '../../proto/ts/control';
import { MESSAGE_ECHEC, PRESSE_PAPIER_MAX, messageDeRefus } from './presse-papier';

/// A minimal event target, without a DOM: the module only needs
/// `focus` and `paste`, and injecting it is what makes this file testable
/// without jsdom.
function cibleFactice() {
    const rappels = new Map<string, Set<(event: EvenementCollage) => void>>();
    return {
        addEventListener(nom: string, rappel: (event: EvenementCollage) => void) {
            if (!rappels.has(nom)) rappels.set(nom, new Set());
            rappels.get(nom)!.add(rappel);
        },
        removeEventListener(nom: string, rappel: (event: EvenementCollage) => void) {
            rappels.get(nom)?.delete(rappel);
        },
        declencher(nom: string, event?: EvenementCollage) {
            for (const rappel of rappels.get(nom) ?? []) rappel(event as EvenementCollage);
        },
        compte(nom: string) {
            return rappels.get(nom)?.size ?? 0;
        },
    };
}

/// Un `paste` factice portant `texte` en `text/plain`.
function collage(texte: string | null) {
    return {
        clipboardData: texte === null ? null : { getData: () => texte },
    };
}

describe('attacherPressePapierAuDOM', () => {
    it('writes the received text when the window has the focus', async () => {
        const write = vi.fn().mockResolvedValue(undefined);
        const cible = cibleFactice();
        const attache = attacherPressePapierAuDOM({
            write,
            focalise: () => true,
            cible,
            surMessage: vi.fn(),
            emettre: vi.fn(),
        });

        attache.recevoir({ texte: 'bonjour', octets: 7 });
        await Promise.resolve();

        expect(write).toHaveBeenCalledWith('bonjour');
        attache.detacher();
    });

    /// D3's deferred deposit, and the only DOM line of this module: without
    /// focus nothing is attempted (`writeText` would fail), and the return of focus
    /// is what lets the text out.
    it("writes nothing without focus, then writes when focus comes back", async () => {
        const write = vi.fn().mockResolvedValue(undefined);
        const cible = cibleFactice();
        let focalise = false;
        const attache = attacherPressePapierAuDOM({
            write,
            focalise: () => focalise,
            cible,
            surMessage: vi.fn(),
            emettre: vi.fn(),
        });

        attache.recevoir({ texte: 'differe', octets: 7 });
        await Promise.resolve();
        expect(write).not.toHaveBeenCalled();

        focalise = true;
        cible.declencher('focus');
        await Promise.resolve();

        expect(write).toHaveBeenCalledWith('differe');
        attache.detacher();
    });

    /// A refusal is SAID, never kept quiet — and it triggers no write.
    it('says the refusal and writes nothing', async () => {
        const write = vi.fn().mockResolvedValue(undefined);
        const surMessage = vi.fn();
        const attache = attacherPressePapierAuDOM({
            write,
            focalise: () => true,
            cible: cibleFactice(),
            surMessage,
            emettre: vi.fn(),
        });

        attache.recevoir({ texte: null, octets: 100_000 });
        await Promise.resolve();

        expect(write).not.toHaveBeenCalled();
        expect(surMessage).toHaveBeenCalledWith(messageDeRefus(100_000));
        attache.detacher();
    });

    /// `FAILURES_BEFORE_MESSAGE` is 2: the first failure is the ordinary case
    /// of a window losing focus during the write, and shouting about it
    /// would make a permanent banner on a product that works.
    it('only shouts on the second consecutive failure', async () => {
        const write = vi.fn().mockRejectedValue(new Error('refused'));
        const surMessage = vi.fn();
        const cible = cibleFactice();
        const attache = attacherPressePapierAuDOM({
            write,
            focalise: () => true,
            cible,
            surMessage,
            emettre: vi.fn(),
        });

        attache.recevoir({ texte: 'un', octets: 2 });
        await Promise.resolve();
        await Promise.resolve();
        expect(surMessage).not.toHaveBeenCalled();

        attache.recevoir({ texte: 'deux', octets: 4 });
        await Promise.resolve();
        await Promise.resolve();
        expect(surMessage).toHaveBeenCalledWith(MESSAGE_ECHEC);
        attache.detacher();
    });

    /// Without this detach, the `focus` listener would survive the end of the session
    /// and would write the local clipboard of a dead session — the same defect
    /// the three neighbouring detaches of `main.ts` exist to avoid.
    it('detaches its focus listener', () => {
        const cible = cibleFactice();
        const attache = attacherPressePapierAuDOM({
            write: vi.fn().mockResolvedValue(undefined),
            focalise: () => true,
            cible,
            surMessage: vi.fn(),
            emettre: vi.fn(),
        });

        expect(cible.compte('focus')).toBe(1);
        attache.detacher();
        expect(cible.compte('focus')).toBe(0);
    });

    /// 🔴 `readText()` is called NOWHERE, neither on focus nor ever: it is
    /// the gesture of the old product (`web/index.js`), it requires a permission,
    /// and it reads a private resource OUTSIDE any intention to paste.
    /// This test guards this property against a future regression — the module
    /// receives no read function, and its interface therefore cannot
    /// acquire one without this file ceasing to compile.
    ///
    /// ✅ **THIS GUARD BIT IN SUB-BLOCK P2, AND THAT IS EXACTLY ITS JOB.**
    /// Adding `emettre` turned it red, forcing a look at the new key and
    /// a decision: `emettre` writes on the control channel **towards the agent**,
    /// it reads nothing from the user's clipboard. The only place in the
    /// product where the latter is read remains the TRUSTED `paste` event, which
    /// is not a received capability but a user gesture — and it goes
    /// through none of these keys. The list is therefore extended **knowingly**,
    /// and not as an accommodation.
    it('receives no clipboard READ capability', () => {
        const options = {
            write: vi.fn().mockResolvedValue(undefined),
            focalise: () => true,
            cible: cibleFactice(),
            surMessage: vi.fn(),
            emettre: vi.fn(),
        };
        expect(Object.keys(options).sort()).toEqual([
            'cible',
            'emettre',
            'focalise',
            'surMessage',
            'write',
        ]);
        attacherPressePapierAuDOM(options).detacher();
    });
});

// ---------------------------------------------------------------------------
// Sub-block P2 — the `paste` listener, the browser → VM direction.
// ---------------------------------------------------------------------------

describe("the paste listener", () => {
    function monter(surMessage = vi.fn()) {
        const cible = cibleFactice();
        const emettre = vi.fn();
        const attache = attacherPressePapierAuDOM({
            write: vi.fn().mockResolvedValue(undefined),
            focalise: () => true,
            cible,
            surMessage,
            emettre,
        });
        return { cible, emettre, surMessage, attache };
    }

    // 🔴 RED if the listener is absent: nothing would ever go up to the agent.
    // The emitted shape is the one `proto/src/control.rs` deserialises, with
    // `deny_unknown_fields` — a home-made encoder would be refused by serde.
    it('emits the pasted text on the control channel', () => {
        const { cible, emettre } = monter();
        cible.declencher('paste', collage('bonjour'));
        expect(emettre).toHaveBeenCalledOnce();
        expect(JSON.parse(emettre.mock.calls[0][0] as string)).toEqual({
            v: CONTROL_VERSION,
            type: 'clipboard',
            text: 'bonjour',
        });
    });

    // 🔴 RED if an empty string were emitted: it would EMPTY the
    // VM clipboard without the user having asked for it.
    it("an empty paste emits nothing", () => {
        const { cible, emettre } = monter();
        cible.declencher('paste', collage(''));
        expect(emettre).not.toHaveBeenCalled();
    });

    // A `paste` without `clipboardData` (an image, an unknown format) is the
    // same case: nothing to emit, and nothing to say.
    it("a paste without clipboardData emits nothing", () => {
        const { cible, emettre } = monter();
        cible.declencher('paste', collage(null));
        expect(emettre).not.toHaveBeenCalled();
    });

    // 🔴 **THE CLIENT-SIDE BOUND IS MANDATORY.** Without it, the agent would
    // enforce it — but the channel would ALREADY have carried the load, and the banner would
    // never appear: the agent refuses by logging, without sending anything back.
    it('beyond the bound: nothing emitted, and the refusal is SAID', () => {
        const surMessage = vi.fn();
        const { cible, emettre } = monter(surMessage);
        const trop = 'a'.repeat(PRESSE_PAPIER_MAX + 1);
        cible.declencher('paste', collage(trop));
        expect(emettre).not.toHaveBeenCalled();
        expect(surMessage).toHaveBeenCalledWith(messageDeRefus(PRESSE_PAPIER_MAX + 1));
    });

    // The exact limit case passes: red if the comparison is a `>=`.
    it('exactly the bound passes', () => {
        const { cible, emettre } = monter();
        cible.declencher('paste', collage('a'.repeat(PRESSE_PAPIER_MAX)));
        expect(emettre).toHaveBeenCalledOnce();
    });

    // 🔴 **THE BOUND COUNTS UTF-8 BYTES, NOT UTF-16 UNITS.**
    // RED if the implementation is `texte.length`: this text counts
    // `PRESSE_PAPIER_MAX / 2` UTF-16 units — so it would pass — for
    // `PRESSE_PAPIER_MAX * 2` bytes, that is TWICE what the agent accepts.
    // The client would then emit a payload the agent would silently refuse.
    it('the bound counts UTF-8 bytes, not UTF-16 units', () => {
        const { cible, emettre, surMessage } = monter();
        const emojis = '😀'.repeat(PRESSE_PAPIER_MAX / 4);
        expect(emojis.length).toBe(PRESSE_PAPIER_MAX / 2);
        cible.declencher('paste', collage(emojis + '😀'));
        expect(emettre).not.toHaveBeenCalled();
        expect(surMessage).toHaveBeenCalled();
    });

    // 🔴 **GUARD NO. 3 WIRED**: a text just received from the agent
    // is not re-emitted towards it. Without this call, every paste of a content
    // coming from the VM would produce a full round trip.
    it("does not re-emit a text that was just received", () => {
        const { cible, emettre, attache } = monter();
        attache.recevoir({ texte: 'venu-de-la-vm', octets: 13 });
        cible.declencher('paste', collage('venu-de-la-vm'));
        expect(emettre).not.toHaveBeenCalled();
    });

    // The twin of the previous one: without it, an `aEmettre` that always returned
    // `undefined` would pass the test above and pasting would be dead.
    it('does re-emit a DIFFERENT text after a reception', () => {
        const { cible, emettre, attache } = monter();
        attache.recevoir({ texte: 'venu-de-la-vm', octets: 13 });
        cible.declencher('paste', collage('something else'));
        expect(emettre).toHaveBeenCalledOnce();
    });

    // RED if `detacher` forgot the `paste`: the listener would survive the end
    // of the session and would emit for a dead session.
    it('detacher ALSO removes the paste listener', () => {
        const { cible, attache } = monter();
        expect(cible.compte('paste')).toBe(1);
        attache.detacher();
        expect(cible.compte('paste')).toBe(0);
        expect(cible.compte('focus')).toBe(0);
    });

});

describe("the state received BEFORE attaching", () => {
    // ── THE STATE RECEIVED BEFORE ATTACHING (CLIENT half of P1's legacy item no. 3) ──
    //
    // 🔴 `client/src/main.ts` HAS NO TEST and cannot have any: entry
    // module, top-level side effects, not importable. The RULE therefore lives
    // here, and these four tests ARE its only coverage; the two wiring
    // lines of `main.ts`, for their part, have none, and their only end-to-end
    // check is criterion ① of the acceptance run.

    it('a supplied `initial` is written on mount if the window has the focus', async () => {
        const write = vi.fn().mockResolvedValue(undefined);
        // RED before the `initial` parameter: nothing is written at mount.
        attacherPressePapierAuDOM({
            write,
            focalise: () => true,
            cible: cibleFactice(),
            surMessage: vi.fn(),
            emettre: vi.fn(),
            initial: { texte: 'copie-avant-attache', octets: 19 },
        });
        await Promise.resolve();
        expect(write).toHaveBeenCalledWith('copie-avant-attache');
    });

    it("an `initial` supplied WITHOUT focus is not written on mount, and is when focus comes back", async () => {
        const write = vi.fn().mockResolvedValue(undefined);
        const cible = cibleFactice();
        let focalise = false;
        // RED = calling `write` directly at mount instead of going through
        // `writeIfPossible`: D3's DEFERRED DEPOSIT must stay the only
        // write path, including here.
        attacherPressePapierAuDOM({
            write,
            focalise: () => focalise,
            cible,
            surMessage: vi.fn(),
            emettre: vi.fn(),
            initial: { texte: 'differe', octets: 7 },
        });
        await Promise.resolve();
        expect(write).not.toHaveBeenCalled();

        focalise = true;
        cible.declencher('focus');
        await Promise.resolve();
        expect(write).toHaveBeenCalledWith('differe');
    });

    it('an `initial` carrying a REFUSAL says the banner on mount', async () => {
        const surMessage = vi.fn();
        // RED = only replaying texts: the window would wait for a content
        // that will never arrive, with nothing to tell it why.
        attacherPressePapierAuDOM({
            write: vi.fn().mockResolvedValue(undefined),
            focalise: () => true,
            cible: cibleFactice(),
            surMessage,
            emettre: vi.fn(),
            initial: { texte: null, octets: 123456 },
        });
        await Promise.resolve();
        expect(surMessage).toHaveBeenCalledWith(messageDeRefus(123456));
    });

    it("without `initial`, mounting writes nothing and says nothing", async () => {
        const write = vi.fn().mockResolvedValue(undefined);
        const surMessage = vi.fn();
        // RED = replaying an empty `Recu` when the parameter is absent: the
        // client would write an empty string to its local clipboard on every
        // attach. It is the OPTIONAL parameter that guarantees the
        // behaviour from before P3 is preserved word for word.
        attacherPressePapierAuDOM({
            write,
            focalise: () => true,
            cible: cibleFactice(),
            surMessage,
            emettre: vi.fn(),
        });
        await Promise.resolve();
        expect(write).not.toHaveBeenCalled();
        expect(surMessage).not.toHaveBeenCalled();
    });
});
