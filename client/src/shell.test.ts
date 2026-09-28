import { describe, it, expect, vi } from 'vitest';
import { createDesktop, type Ton } from './shell';

/**
 * 🔴 `show` COLLECTS, IT NO LONGER DOES NOTHING. The version from before
 * sub-block S3 set `show: () => {}` — a NO-OP, exactly the pattern
 * sub-block D10 named: "a fake source that implements a side
 * effect as a NO-OP makes a whole family of defects invisible to host
 * tests", 456 green tests on a mute product. As long as it was there, NONE of the
 * four tone tests below could fail — and the banner itself
 * was checked by nothing.
 */
function bureauDeTest() {
    const ouvertes = new Map<string, { closed: boolean; close: () => void }>();
    const envoyes: unknown[] = [];
    const filesStates: string[] = [];
    const bandeaux: Array<{ message: string; ton: Ton }> = [];
    const etats: Array<{ texte: string; ton: Ton }> = [];
    /**
     * 🔴 THE COUNTER IS COLLECTED AS IS — two NUMBERS, a text, a tone —
     * and not reformatted here. Reading it from a sentence would replay the trap F1
     * paid nine minutes for: "Drive … mounted" and "could not be mounted"
     * share a substring, the driver tested `includes('mont')`, and a
     * whole measurement ran on a bridge that was NOT mounted.
     */
    const compteurs: Array<{ dues: number; vues: number; texte: string; ton: Ton }> = [];
    const retenues: boolean[] = [];
    const bureau = createDesktop({
        ouvrirFenetre: (session) => {
            const f = { closed: false, close: () => { f.closed = true; } };
            ouvertes.set(session, f);
            return f as unknown as Window;
        },
        envoyer: (message) => { envoyes.push(message); },
        show: (message, ton) => { bandeaux.push({ message, ton }); },
        showFilesState: (texte, ton) => {
            filesStates.push(texte);
            etats.push({ texte, ton });
        },
        showRetained: (r: boolean) => {
            retenues.push(r);
        },
        showPendingWrites: (dues, vues, texte, ton) => {
            compteurs.push({ dues, vues, texte, ton });
        },
    });
    return { bureau, ouvertes, envoyes, filesStates, bandeaux, etats, compteurs, retenues };
}

describe('F5 — the HELD writes', () => {
    /**
     * 🔴 **`retenues` GOES UP, AND IT IS FILTERED BY "there are pending writes".**
     *
     * Holding back without any pending write makes no sense: the button would offer to
     * resume what there is nothing to resume. The bridge does not emit it that way,
     * but relying on it would make the interface depend on a property
     * no type guarantees.
     */
    it('held with dues is announced', () => {
        const { bureau, retenues } = bureauDeTest();
        bureau.ecrituresDues([{ chemin: 'a.txt', octets: 1 }], true);
        expect(retenues.at(-1)).toBe(true);
    });

    it('🔴 held WITHOUT any due is NOT announced', () => {
        const { bureau, retenues } = bureauDeTest();
        bureau.ecrituresDues([], true);
        expect(retenues.at(-1)).toBe(false);
    });

    it('NON-held dues do not announce it', () => {
        const { bureau, retenues } = bureauDeTest();
        bureau.ecrituresDues([{ chemin: 'a.txt', octets: 1 }], false);
        expect(retenues.at(-1)).toBe(false);
    });

    /**
     * 🔴 **HOLDING BACK IS AN ALERT, NEVER A `neutre`.** Nothing will restart without
     * a user gesture, and a neutral tone would suggest the bridge
     * is still working.
     */
    it('🔴 a held resumption carries the « alerte » tone', () => {
        const { bureau, compteurs } = bureauDeTest();
        bureau.ecrituresDues([{ chemin: 'a.txt', octets: 1 }], true);
        expect(compteurs.at(-1)?.ton).toBe('alerte');
    });

    /**
     * The state is the BRIDGE's, not the interface's: it drops when the bridge
     * stops holding back, and not before.
     */
    it('the state drops back when the bridge stops holding', () => {
        const { bureau, retenues } = bureauDeTest();
        bureau.ecrituresDues([{ chemin: 'a.txt', octets: 1 }], true);
        expect(retenues.at(-1)).toBe(true);
        bureau.ecrituresDues([{ chemin: 'a.txt', octets: 1 }], false);
        expect(retenues.at(-1)).toBe(false);
    });
});

describe('page-shell', () => {
    it('opens a browser window when the supervisor announces a window', () => {
        const { bureau, ouvertes } = bureauDeTest();
        bureau.fenetreOuverte('w-1', 'Bloc-notes');
        expect(ouvertes.has('w-1')).toBe(true);
        expect(bureau.list()).toEqual([{ session: 'w-1', titre: 'Bloc-notes', ouverte: true }]);
    });

    it('passes on to the supervisor the viewport the page announces', () => {
        const { bureau, envoyes } = bureauDeTest();
        bureau.fenetreOuverte('w-1', 'Bloc-notes');
        bureau.viewportRecu('w-1', 1600, 900);
        expect(envoyes).toContainEqual({
            type: 'viewport', session: 'w-1', largeur: 1600, hauteur: 900,
        });
    });

    it("ignores a viewport for a session it did not open", () => {
        // The message comes from `postMessage`: any page of the same
        // origin can emit one. Only relay what was asked for.
        const { bureau, envoyes } = bureauDeTest();
        bureau.viewportRecu('w-inventee', 800, 600);
        expect(envoyes).toHaveLength(0);
    });

    it('closes the browser window when the Windows window disappears', () => {
        const { bureau, ouvertes } = bureauDeTest();
        bureau.fenetreOuverte('w-1', 'Bloc-notes');
        bureau.fenetreFermee('w-1');
        expect(ouvertes.get('w-1')!.closed).toBe(true);
        expect(bureau.list()).toEqual([]);
    });

    it('keeps the window in its list when only the page was closed', () => {
        // D1's decision: closing a page does not close the Windows
        // application. The shell must therefore be able to reopen it.
        const { bureau, ouvertes } = bureauDeTest();
        bureau.fenetreOuverte('w-1', 'Bloc-notes');
        ouvertes.get('w-1')!.closed = true;
        expect(bureau.list()).toEqual([{ session: 'w-1', titre: 'Bloc-notes', ouverte: false }]);
    });

    it('reopens a window whose page was closed', () => {
        const { bureau, ouvertes } = bureauDeTest();
        bureau.fenetreOuverte('w-1', 'Bloc-notes');
        ouvertes.get('w-1')!.closed = true;
        bureau.rouvrir('w-1');
        expect(ouvertes.get('w-1')!.closed).toBe(false);
        expect(bureau.list()).toEqual([{ session: 'w-1', titre: 'Bloc-notes', ouverte: true }]);
    });

    it('displays a refusal without opening anything', () => {
        const show = vi.fn();
        const bureau = createDesktop({
            ouvrirFenetre: () => { throw new Error('nothing must be opened'); },
            envoyer: () => {},
            show: show,
            showFilesState: () => {},
            showPendingWrites: () => {},
            showRetained: () => {},
        });
        bureau.refus('F9', 'no virtual output available any more');
        // The refusal reason is TAKEN AS IS, and the tone goes with it: the
        // sub-block S3 added the second argument, and a one-argument assertion
        // would stop describing the real call.
        expect(show).toHaveBeenCalledWith(
            expect.stringContaining('no virtual output available any more'),
            'danger',
        );
    });

    it("reports a pop-up block rather than ignoring it", () => {
        // `window.open` returns `null` when the browser blocks: without this
        // handling, the user would see a window listed as "open"
        // that does not exist.
        const show = vi.fn();
        const bureau = createDesktop({
            ouvrirFenetre: () => null,
            envoyer: () => {},
            show: show,
            showFilesState: () => {},
            showPendingWrites: () => {},
            showRetained: () => {},
        });
        bureau.fenetreOuverte('w-1', 'Bloc-notes');
        expect(show).toHaveBeenCalledWith(expect.stringContaining('pop-up'), 'danger');
        expect(bureau.list()).toEqual([{ session: 'w-1', titre: 'Bloc-notes', ouverte: false }]);
    });
});

describe('file drive state', () => {
    it('mounting the drive displays the folder name', () => {
        const { bureau, filesStates } = bureauDeTest();
        bureau.lecteurMonte('Mes documents');
        expect(filesStates.at(-1)).toContain('Mes documents');
    });

    it('🔴 unmounting the drive ERASES the state', () => {
        // 🔴 It is the defect found in D5: the `#status` banner kept its
        // `textContent` after `expirer()`, so that reading the text proved
        // that a message had ARRIVED, never that it was DISPLAYED. A whole
        // acceptance run read a stale banner believing it was reading the current state.
        //
        // The empty string is therefore not a presentation detail: it is
        // the assertion itself.
        const { bureau, filesStates } = bureauDeTest();
        bureau.lecteurMonte('Mes documents');
        bureau.lecteurDemonte();
        expect(filesStates.at(-1)).toBe('');
    });

    it('a mount failure is told apart from an unmount', () => {
        // "nothing is shared" and "sharing failed, here is why"
        // do not call for the same user gesture: the second tells them
        // what to fix, the first only tells them to start over.
        const { bureau, filesStates } = bureauDeTest();
        bureau.lecteurEchoue('signaling unreachable');
        expect(filesStates.at(-1)).toContain('signaling unreachable');
        expect(filesStates.at(-1)).not.toBe('');
    });

    it('a remount replaces the previous name instead of adding to it', () => {
        const { bureau, filesStates } = bureauDeTest();
        bureau.lecteurMonte('Premier');
        bureau.lecteurMonte('Second');
        expect(filesStates.at(-1)).toContain('Second');
        expect(filesStates.at(-1)).not.toContain('Premier');
    });
});

describe('shell page — the banner TONE', () => {
    // The rule is HERE, in `shell.ts`, and not in the wiring: `shell-page.ts`
    // only relays. A condition that appeared there would be in the
    // wrong place.

    it('gives the DANGER tone to a refusal', () => {
        const { bureau, bandeaux } = bureauDeTest();
        bureau.refus('Bloc-notes', 'no output available any more');
        expect(bandeaux).toHaveLength(1);
        expect(bandeaux[0].ton).toBe('danger');
    });

    it('gives the DANGER tone to a blocked pop-up', () => {
        // The user must ACT — allow pop-ups. A neutral banner
        // would say the window is on its way; it will never exist.
        const bandeaux: Array<{ message: string; ton: Ton }> = [];
        const sansPopup = createDesktop({
            ouvrirFenetre: () => null,
            envoyer: () => {},
            show: (message, ton) => { bandeaux.push({ message, ton }); },
            showFilesState: () => {},
            showPendingWrites: () => {},
            showRetained: () => {},
        });
        sansPopup.fenetreOuverte('w-1', 'Bloc-notes');
        expect(bandeaux).toHaveLength(1);
        expect(bandeaux[0].ton).toBe('danger');
        expect(bandeaux[0].message).toContain('blocked the pop-up');
    });

    it('tells a SUCCESSFUL mount from a FAILURE by the tone, not only by the text', () => {
        // `shell.ts` already requires the two texts to be distinct; the tone
        // must not undo that distinction by making them identical to
        // the eye.
        const { bureau, etats } = bureauDeTest();
        bureau.lecteurMonte('Documents');
        bureau.lecteurEchoue('permission refused');
        expect(etats.map((e) => e.ton)).toEqual(['succes', 'danger']);
    });

    it('on an unmount, the string stays EMPTY', () => {
        const { bureau, etats } = bureauDeTest();
        bureau.lecteurDemonte();
        expect(etats).toHaveLength(1);
        expect(etats[0].texte).toBe('');
    });

    it("on an unmount, the banner takes NO tone — separate assertion", () => {
        // 🔴 SEPARATED FROM THE PREVIOUS ONE ON PURPOSE: the emptiness of the text and the
        // neutrality of the tone are two properties, and `expect` stops at the
        // first failure. Merging them would mean a `danger` tone on an empty
        // banner — a colour without a message — would go unnoticed as soon as
        // the text assertion fell first.
        const { bureau, etats } = bureauDeTest();
        bureau.lecteurDemonte();
        expect(etats[0].ton).toBe('neutre');
    });
});

describe('the due writes counter', () => {
    it('🔴 returns TWO numbers, one of them CUMULATIVE that never goes down', () => {
        // 🔴 Resetting it to zero would make a negative verdict INDISTINGUISHABLE from a
        // measurement not taken: `dues = 0` is also what a machine where
        // nothing has happened yet returns. *A negative verdict requires the measured
        // thing to be ABSENT, not merely zero.*
        const { bureau, compteurs } = bureauDeTest();
        bureau.ecrituresDues([{ chemin: 'a.txt', octets: 3 }], false);
        bureau.ecrituresDues([], false);
        expect(compteurs.map((c) => [c.dues, c.vues])).toEqual([
            [1, 1],
            [0, 1],
        ]);
    });

    it('the announcement OVERWRITES the state, it does not add to it', () => {
        // The bridge sends the full STATE of its log on each change:
        // accumulating would mean an acknowledged path would stay displayed FOREVER.
        const { bureau, compteurs } = bureauDeTest();
        bureau.ecrituresDues(
            [
                { chemin: 'a.txt', octets: 1 },
                { chemin: 'b.txt', octets: 2 },
            ],
            false,
        );
        bureau.ecrituresDues([{ chemin: 'b.txt', octets: 2 }], false);
        expect(compteurs.at(-1)?.dues).toBe(1);
        expect(compteurs.at(-1)?.texte).toContain('b.txt');
        expect(compteurs.at(-1)?.texte).not.toContain('a.txt');
    });

    it('🔴 NAMES the files, because `beforeunload` cannot', () => {
        // ⛔ The custom `beforeunload` message is IGNORED by all
        // modern browsers. Naming them IN THE PAGE is what remains.
        const { bureau, compteurs } = bureauDeTest();
        bureau.ecrituresDues([{ chemin: 'dossier/rapport final.docx', octets: 12 }], false);
        expect(compteurs.at(-1)?.texte).toContain('dossier/rapport final.docx');
    });

    it('🔴 a failure NAMES the file AND the cause', () => {
        // "A write failed" does not tell the user which document
        // to reopen, nor whether they must free up space or grant a permission back.
        const { bureau, compteurs } = bureauDeTest();
        bureau.ecrituresDues([{ chemin: 'note.txt', octets: 3 }], false);
        bureau.ecritureEchouee('note.txt', 'disque-plein');
        expect(compteurs.at(-1)?.texte).toContain('note.txt');
        expect(compteurs.at(-1)?.texte).toContain('disque-plein');
        expect(compteurs.at(-1)?.ton).toBe('danger');
    });

    it('🔴 zero due erases the text AND sets the neutral tone', () => {
        // 🔴 IT IS THE DEFECT OF D5, which `lecteurDemonte` already documents against
        // itself: a banner that keeps its text makes a STALE state read
        // as the current state. And a coloured tone without text would be an alarm
        // without a statement — the two properties are exercised SEPARATELY.
        const { bureau, compteurs } = bureauDeTest();
        bureau.ecrituresDues([{ chemin: 'a.txt', octets: 1 }], false);
        bureau.ecrituresDues([], false);
        expect(compteurs.at(-1)?.texte).toBe('');
        expect(compteurs.at(-1)?.ton).toBe('neutre');
    });

    it('a write that finally arrives erases its failure', () => {
        const { bureau, compteurs } = bureauDeTest();
        bureau.ecrituresDues([{ chemin: 'a.txt', octets: 1 }], false);
        bureau.ecritureEchouee('a.txt', 'interne');
        bureau.ecrituresDues([], false);
        expect(compteurs.at(-1)?.ton).toBe('neutre');
        expect(compteurs.at(-1)?.texte).toBe('');
    });

    it('🔴 does NOT warn when there is nothing to lose', () => {
        // Warning ALWAYS would teach the user to ignore
        // the warning, which would make it useless exactly on the day it
        // matters.
        const { bureau } = bureauDeTest();
        expect(bureau.doitPrevenir()).toBe(false);
        bureau.ecrituresDues([{ chemin: 'a.txt', octets: 1 }], false);
        expect(bureau.doitPrevenir()).toBe(true);
        bureau.ecrituresDues([], false);
        expect(bureau.doitPrevenir()).toBe(false);
    });
});

/** The harness of the mutation tests: `bureauDeTest` plus access to the last counter. */
function vues() {
    const h = bureauDeTest();
    return {
        bureau: h.bureau,
        dernieresDues: () => h.compteurs[h.compteurs.length - 1],
    };
}

describe('the failed mutations (F3)', () => {
    it('🔴 are NAMED, and a rename carries ITS TWO paths', () => {
        // "cannot rename X" does not say to what — and that is
        // precisely what the user must check: the destination
        // may already exist.
        const v = vues();
        v.bureau.mutationEchouee('brouillon.txt → note.txt', 'deja-present');
        expect(v.dernieresDues()?.texte).toContain('brouillon.txt → note.txt');
        expect(v.dernieresDues()?.texte).toContain('deja-present');
    });

    it('🔴 are a DANGER even without any due write', () => {
        // That is what sets them apart from a failed write: the two sides
        // have diverged, and nothing will reconcile them on its own.
        const v = vues();
        v.bureau.mutationEchouee('a → b', 'introuvable');
        expect(v.dernieresDues()?.ton).toBe('danger');
        expect(v.dernieresDues()?.dues).toBe(0);
    });

    it('🔴 DO NOT DISAPPEAR when the due writes drop back to zero', () => {
        // Red: erasing them in `ecrituresDues`, like write
        // failures. A permanent divergence would then erase itself,
        // and the user would never know a file was not renamed
        // on their workstation.
        const v = vues();
        v.bureau.ecrituresDues([{ chemin: 'x.txt', octets: 1 }], false);
        v.bureau.mutationEchouee('a → b', 'introuvable');
        v.bureau.ecrituresDues([], false);
        expect(v.dernieresDues()?.texte).toContain('a → b');
        expect(v.dernieresDues()?.ton).toBe('danger');
    });

    it('are erased when the drive is REMOUNTED, and only then', () => {
        const v = vues();
        v.bureau.mutationEchouee('a → b', 'introuvable');
        v.bureau.lecteurDemonte();
        expect(v.dernieresDues()?.texte).toBe('');
        expect(v.dernieresDues()?.ton).toBe('neutre');
    });

    it('accumulate, unlike the due writes which overwrite', () => {
        const v = vues();
        v.bureau.mutationEchouee('a → b', 'x');
        v.bureau.mutationEchouee('c', 'y');
        const texte = v.dernieresDues()?.texte ?? '';
        expect(texte).toContain('a → b');
        expect(texte).toContain('c');
        expect(texte).toContain('2 renames or removals');
    });
});

// 🔴 FIX OF THE MISSING-BRAKES LEGACY ITEM (correction round 1,
// critique ④), August 25th, 2026 — BEFORE THIS BATCH, `type:'error'` matched
// NO branch of the dispatch of `shell-page.ts`, and the socket
// installed neither `close` nor `error`: a refusal arriving on `/signal` (the
// "any request" budget this same batch opens, `signaling/relais.ts`)
// left the page showing "desktop connected", silently dead.
describe('shell page — the refusal and the loss of the control channel', () => {
    it('a channel refusal displays the REASON, as DANGER', () => {
        const { bureau, bandeaux } = bureauDeTest();
        bureau.canalDeControleRefuse('too many requests', 'trop-de-requetes', undefined);
        expect(bandeaux).toHaveLength(1);
        expect(bandeaux[0].ton).toBe('danger');
        expect(bandeaux[0].message).toContain('too many requests');
    });

    it('the VOLUME refusal carries the suggested delay, when supplied', () => {
        // `retryApresS` ONLY accompanies `trop-de-requetes` (`relais.ts`) —
        // it is the cheapest half of the remedy to the lock-out that
        // `agent/src/superviseur/boucle/surveillance_pont.rs` documents.
        const { bureau, bandeaux } = bureauDeTest();
        bureau.canalDeControleRefuse('too many requests', 'trop-de-requetes', 42);
        expect(bandeaux[0].message).toContain('42');
    });

    it("a handshake refusal WITHOUT a delay promises no wait", () => {
        const { bureau, bandeaux } = bureauDeTest();
        bureau.canalDeControleRefuse('invalid token', 'jeton-invalide', undefined);
        expect(bandeaux[0].message).not.toMatch(/attempt/);
    });

    it('without a `reason`, the `motif` serves as fallback', () => {
        const { bureau, bandeaux } = bureauDeTest();
        bureau.canalDeControleRefuse(undefined, 'trop-de-requetes', undefined);
        expect(bandeaux[0].message).toContain('trop-de-requetes');
    });

    it('losing the channel is announced, as DANGER — never silently', () => {
        const { bureau, bandeaux } = bureauDeTest();
        bureau.canalDeControlePerdu();
        expect(bandeaux).toHaveLength(1);
        expect(bandeaux[0].ton).toBe('danger');
        expect(bandeaux[0].message).toMatch(/lost|reload/i);
    });
});
