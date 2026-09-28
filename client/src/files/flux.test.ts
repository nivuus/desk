import { describe, expect, it } from 'vitest';
import { SEUIL_TAMPON, contrePression, type CanalSortant } from './flux';

/** An in-memory channel whose buffer and state we drive. */
function fauxCanal(): CanalSortant & {
    poser(octets: number): void;
    fermer(): void;
    abonnes: { bufferedamountlow: number; close: number };
} {
    const ecouteurs: Record<string, Set<() => void>> = {
        bufferedamountlow: new Set(),
        close: new Set(),
    };
    const abonnes = { bufferedamountlow: 0, close: 0 };
    let tampon = 0;
    let etat: 'open' | 'closed' = 'open';
    return {
        get bufferedAmount() {
            return tampon;
        },
        get readyState() {
            return etat;
        },
        bufferedAmountLowThreshold: 0,
        addEventListener(type, e) {
            ecouteurs[type].add(e);
            abonnes[type] += 1;
        },
        removeEventListener(type, e) {
            ecouteurs[type].delete(e);
            abonnes[type] -= 1;
        },
        poser(octets: number) {
            const baisse = octets < tampon;
            tampon = octets;
            if (baisse && octets <= this.bufferedAmountLowThreshold) {
                for (const e of [...ecouteurs.bufferedamountlow]) e();
            }
        },
        fermer() {
            etat = 'closed';
            for (const e of [...ecouteurs.close]) e();
        },
        abonnes,
    };
}

/**
 * Is a promise resolved on the next turn of the event loop?
 *
 * ⚠️ **A `setTimeout` AND NOT A `Promise.resolve`**, and the first draft
 * got it wrong: `Promise.resolve(marque)` wins the race even against an
 * ALREADY resolved promise, because the latter's `.then` adds a
 * microtask turn. The check then returned `false` for everyone — six tests
 * red out of eight, including those that should be green. **A witness that cannot
 * return `true` exercises nothing.**
 */
async function resolue(p: Promise<unknown>): Promise<boolean> {
    const marque = Symbol('en-attente');
    const sentinelle = new Promise((r) => setTimeout(() => r(marque), 0));
    return (await Promise.race([p, sentinelle])) !== marque;
}

describe('back-pressure', () => {
    it('sets `bufferedAmountLowThreshold` at construction', () => {
        // ⚠️ Spec §3.4 requires it ("set"); `canal.ts` did NOT set it
        // before F3 — it only passed `{ ordered: true }`.
        const c = fauxCanal();
        contrePression(c);
        expect(c.bufferedAmountLowThreshold).toBe(SEUIL_TAMPON);
    });

    it('waits for nothing when the buffer is under the threshold', async () => {
        const c = fauxCanal();
        const cp = contrePression(c);
        c.poser(0);
        expect(await resolue(cp.beforeSend())).toBe(true);
    });

    it('🔴 does NOT send while the buffer exceeds the threshold', async () => {
        // Red: sending anyway. The fake sees `bufferedAmount` grow without
        // bound, and the channel becomes the source of latency for everything else.
        const c = fauxCanal();
        const cp = contrePression(c);
        c.poser(SEUIL_TAMPON * 4);
        const attente = cp.beforeSend();
        await new Promise((r) => setTimeout(r, 5));
        expect(await resolue(attente)).toBe(false);
        c.poser(0);
        expect(await resolue(attente)).toBe(true);
    });

    it('🔴 `bufferedamountlow` releases the wait', async () => {
        // Red: not subscribing to it. **THE WAIT NEVER ENDS, and the
        // bridge times out** — a block WORSE than the one being fixed, since it
        // freezes the page instead of slowing a transfer down.
        const c = fauxCanal();
        const cp = contrePression(c);
        c.poser(SEUIL_TAMPON * 2);
        const attente = cp.beforeSend();
        expect(await resolue(attente)).toBe(false);
        c.poser(SEUIL_TAMPON);
        expect(await resolue(attente)).toBe(true);
    });

    it('🔴 a channel CLOSED during the wait does not stay suspended', async () => {
        // Red: not handling `close`. A closed channel will never again emit
        // `bufferedamountlow`: a read in progress would freeze the page when
        // the remote tab closes.
        const c = fauxCanal();
        const cp = contrePression(c);
        c.poser(SEUIL_TAMPON * 2);
        const attente = cp.beforeSend();
        expect(await resolue(attente)).toBe(false);
        c.fermer();
        expect(await resolue(attente)).toBe(true);
    });

    it('an ALREADY closed channel does not wait at all', async () => {
        const c = fauxCanal();
        const cp = contrePression(c);
        c.poser(SEUIL_TAMPON * 2);
        c.fermer();
        expect(await resolue(cp.beforeSend())).toBe(true);
    });

    it('🔴 BOTH LISTENERS ARE REMOVED, whichever one wins', async () => {
        // 🔴 It is the observed defect of the old bridge: a listener set PER
        // REQUEST and never removed (`src/file.js:155`), whose cost grew
        // with the number of past operations, indefinitely.
        const c = fauxCanal();
        const cp = contrePression(c);
        for (let i = 0; i < 5; i += 1) {
            c.poser(SEUIL_TAMPON * 2);
            const a = cp.beforeSend();
            c.poser(0);
            await a;
        }
        expect(c.abonnes.bufferedamountlow).toBe(0);
        expect(c.abonnes.close).toBe(0);
    });

    it('🔴 the buffer that drops BETWEEN the test and the subscription does not suspend forever', async () => {
        // The classic race of any "test then wait" mechanism:
        // the event goes by while we subscribe, and the wait never
        // ends. Red: removing the re-check that follows the subscription.
        const c = fauxCanal();
        const cp = contrePression(c);
        c.poser(SEUIL_TAMPON * 2);
        // The buffer empties at the EXACT moment of the subscription, without emitting —
        // which is what an event that already went by does.
        const vrai = c.addEventListener.bind(c);
        c.addEventListener = (type, e) => {
            vrai(type, e);
            if (type === 'bufferedamountlow') c.poser(0);
        };
        expect(await resolue(cp.beforeSend())).toBe(true);
    });
});
