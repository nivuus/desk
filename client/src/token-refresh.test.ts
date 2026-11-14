// The refresh of the token pair: what empties the store, and what two tabs
// refreshing at the same moment do.
//
// Separate from `jeton.test.ts`, which is close to the 500-line ceiling.

import { describe, expect, it, vi } from 'vitest';
import {
    CLE_ACCES,
    CLE_RAFRAICHISSEMENT,
    refreshOutcomeOf,
    rafraichirSiNecessaire,
    type Coffre,
    type RefreshOutcome,
    type LockRunner,
} from './jeton';
import { freshTokenProvider } from './fresh-token';

/// A token whose `exp` is `expMs` (the browser checks no signature).
function jetonExpirantA(expMs: number): string {
    return `x.${btoa(JSON.stringify({ exp: expMs })).replace(/=+$/, '')}.y`;
}

/// One store shared by every "tab" of a test, as `localStorage` is.
function sharedVault(initial: Record<string, string>): Coffre & { contenu: Map<string, string> } {
    const contenu = new Map(Object.entries(initial));
    return {
        contenu,
        getItem: (c) => contenu.get(c) ?? null,
        setItem: (c, v) => void contenu.set(c, v),
        removeItem: (c) => void contenu.delete(c),
    };
}

/// An in-memory exclusive lock with the semantics of `navigator.locks.request`:
/// one holder per name, the others queue, and the promise settles with the
/// work's result once the lock is released.
function inMemoryLock(): LockRunner {
    const files = new Map<string, Promise<unknown>>();
    return (nom, travail) => {
        const avant = files.get(nom) ?? Promise.resolve();
        const resultat = avant.then(travail, travail);
        files.set(nom, resultat.catch(() => undefined));
        return resultat;
    };
}

/// A service that rotates the refresh token and treats the presentation of an
/// already-rotated one as a replay: refused (and, on the real service, the
/// whole family revoked).
function serviceRefusingReuse() {
    const vus = new Set<string>();
    let rang = 0;
    const appels: string[] = [];
    const appel = async (corps: unknown): Promise<RefreshOutcome> => {
        const presente = (corps as { rafraichissement: string }).rafraichissement;
        appels.push(presente);
        // A network round trip: lets the other tab run in between.
        await new Promise((r) => setTimeout(r, 5));
        if (vus.has(presente)) return { outcome: 'refused' };
        vus.add(presente);
        rang += 1;
        return {
            outcome: 'renewed',
            pair: { acces: jetonExpirantA(10_000_000), rafraichissement: `R${String(rang + 1)}` },
        };
    };
    return { appel, appels };
}

describe('two tabs refreshing at the same moment', () => {
    it('make ONE network call and keep the new pair', async () => {
        const coffre = sharedVault({ [CLE_ACCES]: jetonExpirantA(0), [CLE_RAFRAICHISSEMENT]: 'R1' });
        const service = serviceRefusingReuse();
        const verrou = inMemoryLock();

        const [a, b] = await Promise.all([
            rafraichirSiNecessaire(coffre, 1_000, 30_000, service.appel, verrou),
            rafraichirSiNecessaire(coffre, 1_000, 30_000, service.appel, verrou),
        ]);

        expect(service.appels).toEqual(['R1']);
        expect(coffre.contenu.get(CLE_RAFRAICHISSEMENT)).toBe('R2');
        expect(coffre.contenu.get(CLE_ACCES)).toBe(jetonExpirantA(10_000_000));
        expect([a, b]).toEqual([true, true]);
    });
});

describe('what empties the store', () => {
    it('an unavailable service leaves the store untouched', async () => {
        const initial = { [CLE_ACCES]: jetonExpirantA(0), [CLE_RAFRAICHISSEMENT]: 'R1' };
        const coffre = sharedVault(initial);
        const ok = await rafraichirSiNecessaire(coffre, 1_000, 30_000, async () => ({ outcome: 'unavailable' }), inMemoryLock());
        expect(ok).toBe(false);
        expect(Object.fromEntries(coffre.contenu)).toEqual(initial);
    });

    it('a refusal by the service empties it', async () => {
        const coffre = sharedVault({ [CLE_ACCES]: jetonExpirantA(0), [CLE_RAFRAICHISSEMENT]: 'R1' });
        const ok = await rafraichirSiNecessaire(coffre, 1_000, 30_000, async () => ({ outcome: 'refused' }), inMemoryLock());
        expect(ok).toBe(false);
        expect(coffre.contenu.size).toBe(0);
    });

    it('a refusal does NOT remove a refresh token another writer set meanwhile', async () => {
        const coffre = sharedVault({ [CLE_ACCES]: jetonExpirantA(0), [CLE_RAFRAICHISSEMENT]: 'R1' });
        // A sign-in in another tab writes a new pair while this refresh is
        // in flight; the refusal of R1 says nothing about it.
        const appel = async (): Promise<RefreshOutcome> => {
            coffre.setItem(CLE_ACCES, jetonExpirantA(10_000_000));
            coffre.setItem(CLE_RAFRAICHISSEMENT, 'R-NEW');
            return { outcome: 'refused' };
        };
        await rafraichirSiNecessaire(coffre, 1_000, 30_000, appel, inMemoryLock());
        expect(coffre.contenu.get(CLE_RAFRAICHISSEMENT)).toBe('R-NEW');
    });

    it('a network rejection of the refresh call leaves the store untouched', async () => {
        const initial = { [CLE_ACCES]: jetonExpirantA(0), [CLE_RAFRAICHISSEMENT]: 'R1' };
        const coffre = sharedVault(initial);
        const fournir = freshTokenProvider({
            vault: coffre,
            base: 'https://h',
            // Refresh and Pomerium both unreachable.
            fetch: vi.fn(async () => {
                throw new TypeError('Failed to fetch');
            }),
            now: () => 1_000,
            lock: inMemoryLock(),
        });
        expect(await fournir()).toBeUndefined();
        expect(Object.fromEntries(coffre.contenu)).toEqual(initial);
    });
});

describe('refreshOutcomeOf', () => {
    it('a valid pair on a 200 is renewed', () => {
        expect(refreshOutcomeOf(200, true, { acces: 'A', rafraichissement: 'R' })).toEqual({
            outcome: 'renewed',
            pair: { acces: 'A', rafraichissement: 'R' },
        });
    });

    it('the service refusing the token (400, 401, 403, and 404 once the route is gone) is a refusal', () => {
        for (const statut of [400, 401, 403, 404]) {
            expect(refreshOutcomeOf(statut, false, { refus: 'x' })).toEqual({ outcome: 'refused' });
        }
    });

    it('a rate limit, a timeout or a server error is unavailability, not a refusal', () => {
        for (const statut of [408, 429, 500, 502, 503]) {
            expect(refreshOutcomeOf(statut, false, undefined)).toEqual({ outcome: 'unavailable' });
        }
    });

    it('a 200 without a usable pair is unavailability, and never written', () => {
        expect(refreshOutcomeOf(200, true, { acces: 'X' })).toEqual({ outcome: 'unavailable' });
    });
});
