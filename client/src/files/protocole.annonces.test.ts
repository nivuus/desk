import { describe, expect, it } from 'vitest';
import { TYPE_DUES, TYPE_FAIT, encoder } from '../../../proto/ts/fichiers';
import type { Adaptateur } from './adaptateur';
import { createServer } from './protocole';

/**
 * The ANNOUNCEMENTS family of the files protocol, extracted from
 * `protocole.test.ts`.
 *
 * # What this extraction IS, and what it is NOT
 *
 * **It adds no test.** The `describe` is transposed **VERBATIM**
 * (l. 324-358 of the parent), and the count of `npx vitest run` is **unchanged**.
 * It comes **BEFORE** the addition it welcomes: `protocole.test.ts`
 * was at **484** lines, margin **16**, and F5 must add to it the tests of the
 * TWO new announcements — `Bonjour` and `Rafraichir`, the **fourth family**
 * of the protocol, the one going from the browser to the bridge. It is the gesture
 * D9 invented and D10 played three times: **never a compression**.
 *
 * ⚠️ **`fauxAdaptateur` is copied rather than imported**, and it is deliberate:
 * exporting it from the parent would make the test file a module that two
 * files share, and a `describe` of the parent could then depend on
 * a change made here without anything saying so. A six-line
 * duplicate costs less than a coupling between two suites.
 */

/** A fake adapter: the protocol knows NO file system. */
function fauxAdaptateur(surcharge: Partial<Adaptateur> = {}): Adaptateur {
    return {
        lister: async () => [{ nom: 'a.txt', repertoire: false, taille: 7, modifie: 42 }],
        attributs: async () => ({
            nom: 'Nom Stocké.txt',
            repertoire: false,
            taille: 1234,
            modifie: 1_690_000_000_000,
        }),
        lire: async () => new Uint8Array([9, 8, 7]),
        ...surcharge,
    };
}

describe('the ANNOUNCEMENT of the due writes', () => {
    it('🔴 answers NOTHING, and calls the injected callback', async () => {
        // Returning a frame would make the bridge receive an answer to a
        // correlation it does not know, and it would drop it with `debug!` —
        // SILENTLY. It is the catch-all arm paid for four times on
        // `capteur/pont_media.rs`.
        const vues: unknown[] = [];
        const serveur = createServer(fauxAdaptateur(), () => {}, {
            onDues: (dues, retenues) => vues.push({ dues, retenues }),
        });
        const reponse = await serveur.traiter(
            encoder(TYPE_DUES, 0, { dues: [{ chemin: 'note.txt', octets: 12 }], retenues: false }),
        );
        expect(reponse).toBeNull();
        expect(vues).toEqual([{ dues: [{ chemin: 'note.txt', octets: 12 }], retenues: false }]);
    });

    /**
     * 🔴 **F5 — `retenues` GOES ALL THE WAY UP TO THE CALLBACK, and that is what lets the
     * shell page say WHY the counter does not go down.**
     *
     * Without this field, a held-back resumption would be indistinguishable from a broken
     * bridge: a frozen count of pending writes, and nothing explaining it.
     */
    it('🔴 a HELD announcement says so to the callback', async () => {
        const vues: boolean[] = [];
        const serveur = createServer(fauxAdaptateur(), () => {}, {
            onDues: (_dues, retenues) => vues.push(retenues),
        });
        await serveur.traiter(
            encoder(TYPE_DUES, 0, { dues: [{ chemin: 'note.txt', octets: 12 }], retenues: true }),
        );
        expect(vues).toEqual([true]);
    });

    /**
     * ⚠️ **An announcement WITHOUT `retenues` is REFUSED, never completed by
     * default.** A default of `false` would mean "the bridge pushes", that is
     * the opposite of what `Bonjour` exists to prevent.
     */
    it('🔴 an announcement WITHOUT `retenues` is refused, not completed', async () => {
        const messages: string[] = [];
        const vues: unknown[] = [];
        const serveur = createServer(fauxAdaptateur(), (m) => messages.push(m), {
            onDues: (dues) => vues.push(dues),
        });
        expect(
            await serveur.traiter(encoder(TYPE_DUES, 0, { dues: [] })),
        ).toBeNull();
        expect(vues).toEqual([]);
        expect(messages.join(' ')).toMatch(/retenues/);
    });

    it('an unreadable announcement is logged, never fatal', async () => {
        const messages: string[] = [];
        const serveur = createServer(fauxAdaptateur(), (m) => messages.push(m), {
            onDues: () => {
                throw new Error('never reached');
            },
        });
        expect(await serveur.traiter(encoder(TYPE_DUES, 0, { dues: 'not an array' }))).toBeNull();
        expect(messages.join(' ')).toMatch(/dues/);
    });

    it('a FAIT received by the browser is IGNORED: it requests nothing', async () => {
        const messages: string[] = [];
        const serveur = createServer(fauxAdaptateur(), (m) => messages.push(m));
        expect(await serveur.traiter(encoder(TYPE_FAIT, 3, {}))).toBeNull();
        expect(messages.join(' ')).toMatch(/requests nothing/);
    });
});
