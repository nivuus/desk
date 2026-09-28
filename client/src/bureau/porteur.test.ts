import { describe, expect, it } from 'vitest';
import {
    batirDemande,
    batirEtat,
    elire,
    estDemandeEtat,
    estPlacePrise,
    fenetresAPeindre,
    lireEtat,
    lireTrame,
    ouvertureParLeBureau,
    promouvoir,
} from './porteur';

describe('elire', () => {
    it('without a lock API, the tab becomes holder: the fallback is OPTIMISTIC', () => {
        // ⚠️ Optimistic and not pessimistic: without a lock, declaring itself a follower
        // would mean NO tab would ever open the session. The platform
        // will decide, and `estPlacePrise` will catch the loser.
        let role = '';
        elire('v', {
            devenirPorteur: () => { role = 'porteur'; },
            devenirSuiveur: () => { role = 'suiveur'; },
        });
        expect(role).toBe('porteur');
    });

    it('with the API, the holder is proclaimed ONLY once the lock is obtained', () => {
        let role = '';
        let relacher: (() => void) | undefined;
        elire('v', {
            // A lock that NEVER calls `pendant`: the lock is not
            // obtained, so this tab is not the holder.
            verrou: (_nom, pendant) => { relacher = () => void pendant(); },
            devenirPorteur: () => { role = 'porteur'; },
            devenirSuiveur: () => { role = 'suiveur'; },
        });
        expect(role).toBe('suiveur');
        // Then the lock is released: the waiting tab is promoted.
        relacher!();
        expect(role).toBe('porteur');
    });
});

describe('estPlacePrise', () => {
    it('recognises the TYPED reason', () => {
        expect(estPlacePrise({ type: 'error', reason: 'whatever', motif: 'role-occupe' })).toBe(true);
    });

    it('does NOT recognise a refusal for another cause', () => {
        // 🔴 THIS CASE IS THE POINT: a volume brake must stay VISIBLE.
        // Swallowing it would make this batch the silent failure it claims to avoid.
        expect(estPlacePrise({ type: 'error', reason: 'too many requests', motif: 'trop-de-requetes' })).toBe(false);
    });

    it('does NOT recognise a refusal without a reason, even if its sentence says so', () => {
        // ⚠️ The F1 trap: a sentence in prose gets reworded. We do not
        // guess, we read the reason code -- or we display.
        expect(estPlacePrise({ type: 'error', reason: 'a client is already connected to session s' })).toBe(false);
    });

    it('does not throw on an entry that is not an object', () => {
        expect(estPlacePrise(undefined)).toBe(false);
        expect(estPlacePrise('role-occupe')).toBe(false);
    });
});

describe('lireEtat', () => {
    it('returns the list of a well-formed state', () => {
        const etat = batirEtat([{ session: 's', titre: 'Bloc-notes', ouverte: true }]);
        expect(lireEtat(etat)?.[0]?.titre).toBe('Bloc-notes');
    });

    it('returns undefined on a message from ANOTHER sender', () => {
        // A `BroadcastChannel` is shared per origin: not everything that goes through it
        // necessarily comes from us.
        expect(lireEtat({ type: 'autre-chose', fenetres: [] })).toBeUndefined();
    });

    it('returns undefined when `fenetres` is not an array', () => {
        expect(lireEtat({ type: 'etat-bureau', fenetres: 'trois' })).toBeUndefined();
    });

    it('DISCARDS a malformed entry instead of letting it through', () => {
        const lu = lireEtat({
            type: 'etat-bureau',
            fenetres: [{ session: 's', titre: 'bon', ouverte: false }, { session: 42 }],
        });
        expect(lu?.length).toBe(1);
    });
});

describe('fenetresAPeindre', () => {
    it('a follower that has received nothing yet paints nothing', () => {
        expect(fenetresAPeindre('suiveur', [], undefined)).toEqual([]);
    });

    it(
        'a follower that received N windows keeps them on the next timer tick, ' +
            'EVEN WHEN ITS OWN LIST IS EMPTY',
        () => {
            // 🔴 IT IS THIS CASE THAT CATCHES THE ROUND 1 DEFECT (critique ①):
            // the timer repaints at 1 Hz from `bureau.list()`, which is
            // STRUCTURALLY EMPTY for a follower -- no socket, so
            // no `fenetreOuverte` ever feeds it. A rule that
            // painted `ownList` for a follower would therefore erase, on the
            // round FOLLOWING a broadcast, what it had just shown.
            const recues = [{ session: 's', titre: 'Bloc-notes', ouverte: true }];
            expect(fenetresAPeindre('suiveur', [], recues)).toEqual(recues);
        },
    );

    it('the holder ALWAYS paints its own list, never a stale received state', () => {
        const propre = [{ session: 's', titre: 'Bloc-notes', ouverte: false }];
        const recuPerime = [{ session: 's', titre: 'Bloc-notes', ouverte: true }];
        expect(fenetresAPeindre('porteur', propre, recuPerime)).toEqual(propre);
    });
});

/* ══ WHAT THE FINAL REVIEW OF AUGUST 31ST 2026 ADDED ═════════════════════ */

describe('promote: the promotion cycle', () => {
    it(
        'requests a FRESH token again, then installs the bridge, THEN opens the socket',
        async () => {
            // 🔴 IT IS THE JUNCTION THAT WAS WRONG, NOT THE FRESHNESS
            // RULE. The socket was opened with `deps.jeton`, a string
            // FROZEN AT LOAD TIME; yet a follower is only promoted at the death of the
            // holder, potentially hours later, and an access token
            // lives TEN MINUTES. A test of `assurerAccesFrais` could
            // not see this defect: it only lives here.
            const ordre: string[] = [];
            let demandes = 0;
            await promouvoir({
                jetonFrais: () => { demandes += 1; return Promise.resolve('frais-a-la-promotion'); },
                installerPont: () => { ordre.push('pont'); },
                ouvrirSocket: (jeton) => { ordre.push(`socket:${jeton}`); },
                sansJeton: () => { ordre.push('sans-jeton'); },
            });
            expect(demandes, 'the token must be REQUESTED AGAIN on promotion').toBe(1);
            expect(ordre).toEqual(['pont', 'socket:frais-a-la-promotion']);
        },
    );

    it('without an obtainable token, opens NO socket and SAYS so', async () => {
        // ⚠️ Opening a socket doomed to refusal would display a refusal that
        // `canalDeControlePerdu` would immediately overwrite with "Reload the
        // page": the user would not know their session had expired.
        const ordre: string[] = [];
        await promouvoir({
            jetonFrais: () => Promise.resolve(undefined),
            installerPont: () => { ordre.push('pont'); },
            ouvrirSocket: () => { ordre.push('socket'); },
            sansJeton: () => { ordre.push('sans-jeton'); },
        });
        expect(ordre).toEqual(['sans-jeton']);
    });
});

describe('elire: the lock is RELEASED', () => {
    it('a dismissed holder gives back its lock, and its partition can elect another one', async () => {
        // 🔴 IMPORTANT ③: the held promise was a `Promise<never>` that
        // nothing resolved -- a holder dismissed by `estPlacePrise` kept
        // the lock FOREVER, and its partition would NEVER AGAIN have had a
        // holder.
        let rendu = false;
        let tenue: Promise<void> | undefined;
        const election = elire('v', {
            verrou: (_nom, pendant) => { tenue = pendant(); },
            devenirPorteur: () => {},
            devenirSuiveur: () => {},
        });
        void tenue!.then(() => { rendu = true; });
        // As long as nobody releases, the promise does not settle.
        await Promise.resolve();
        expect(rendu, 'the lock is NOT released by itself').toBe(false);
        election.relacher();
        // ⚠️ WE DO NOT AWAIT `tenue`: an `await` on a promise that might
        // NEVER settle would go red by TIMEOUT, and a timeout does not
        // say WHICH assertion failed. We let the microtasks of the
        // `then` run, then we ASSERT.
        await Promise.resolve();
        await Promise.resolve();
        expect(rendu, 'the lock must be RELEASED when it is let go').toBe(true);
    });

    it('release is IDEMPOTENT, and inert without a held lock', () => {
        // The fallback without `navigator.locks` holds no lock: there is
        // nothing to give back, and saying so must not throw.
        const election = elire('v', { devenirPorteur: () => {}, devenirSuiveur: () => {} });
        expect(() => { election.relacher(); election.relacher(); }).not.toThrow();
    });
});

describe('estDemandeEtat', () => {
    it('recognises the request a new tab makes on mount', () => {
        // 🔴 IMPORTANT ①: `diffuserSiChange` only posts on CHANGE. In
        // steady state -- three windows, nothing moving -- a joining tab
        // showed an EMPTY list FOREVER.
        expect(estDemandeEtat(batirDemande())).toBe(true);
    });

    it('does NOT mistake a state broadcast for a request', () => {
        expect(estDemandeEtat(batirEtat([]))).toBe(false);
    });

    it('does not throw on an entry that is not an object', () => {
        expect(estDemandeEtat(undefined)).toBe(false);
        expect(estDemandeEtat('demande-etat')).toBe(false);
    });
});

describe('lireTrame', () => {
    it('returns the object of a well-formed frame', () => {
        expect(lireTrame('{"type":"fenetre-ouverte","session":"s"}')?.type).toBe('fenetre-ouverte');
    });

    it('returns undefined on a NON-JSON frame, instead of throwing', () => {
        // 🔴 MINOR ③: `JSON.parse(evenement.data)` was BARE in the listener
        // of the control socket.
        expect(lireTrame('not json')).toBeUndefined();
    });

    it('returns undefined on `null`, `42` and an ARRAY -- which JSON.parse accepts', () => {
        // `null.type` throws a `TypeError`; an array and a number do have
        // an `undefined` `.type`, but they are not frames.
        expect(lireTrame('null')).toBeUndefined();
        expect(lireTrame('42')).toBeUndefined();
        expect(lireTrame('[1,2]')).toBeUndefined();
    });

    it('returns undefined on what is not even a string', () => {
        expect(lireTrame(new ArrayBuffer(4))).toBeUndefined();
    });
});

describe('ouvertureParLeBureau', () => {
    it('the holder goes through the desktop, which REMEMBERS the handle', () => {
        expect(ouvertureParLeBureau('porteur')).toBe(true);
    });

    it('a follower opens directly: its `bureau` is fed by nothing', () => {
        expect(ouvertureParLeBureau('suiveur')).toBe(false);
    });
});
