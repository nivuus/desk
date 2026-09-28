import { describe, expect, it } from 'vitest';
import { SEUIL_TAMPON, contrePression, type CanalSortant } from './flux';

/** Un canal en mémoire dont on pilote le tampon et l'état. */
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
 * Une promesse est-elle résolue au tour suivant de la boucle d'événements ?
 *
 * ⚠️ **UN `setTimeout` ET NON UN `Promise.resolve`**, et la première rédaction
 * s'y est trompée : `Promise.resolve(marque)` gagne la course même contre une
 * promesse DÉJÀ résolue, parce que le `.then` de celle-ci ajoute un tour de
 * micro-tâche. Le contrôle rendait alors `false` pour tout le monde — six tests
 * rouges sur huit, dont ceux qui devaient être verts. **Un témoin qui ne peut
 * pas rendre `true` n'éprouve rien.**
 */
async function resolue(p: Promise<unknown>): Promise<boolean> {
    const marque = Symbol('en-attente');
    const sentinelle = new Promise((r) => setTimeout(() => r(marque), 0));
    return (await Promise.race([p, sentinelle])) !== marque;
}

describe('back-pressure', () => {
    it('sets `bufferedAmountLowThreshold` at construction', () => {
        // ⚠️ La spec §3.4 l'exige (« posé ») ; `canal.ts` ne le posait PAS
        // avant F3 — il ne passait que `{ ordered: true }`.
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
        // Rouge : envoyer quand même. Le faux voit `bufferedAmount` croître sans
        // borne, et le canal devient la source de latence de tout le reste.
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
        // Rouge : ne pas s'y abonner. **L'ATTENTE NE SE TERMINE JAMAIS, et le
        // pont expire** — un blocage PIRE que celui qu'on répare, puisqu'il
        // fige la page au lieu de ralentir un transfert.
        const c = fauxCanal();
        const cp = contrePression(c);
        c.poser(SEUIL_TAMPON * 2);
        const attente = cp.beforeSend();
        expect(await resolue(attente)).toBe(false);
        c.poser(SEUIL_TAMPON);
        expect(await resolue(attente)).toBe(true);
    });

    it('🔴 a channel CLOSED during the wait does not stay suspended', async () => {
        // Rouge : ne pas traiter `close`. Un canal fermé n'émettra plus jamais
        // `bufferedamountlow` : une lecture en cours figerait la page à la
        // fermeture de l'onglet distant.
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
        // 🔴 C'est le défaut relevé de l'ancien pont : un écouteur posé PAR
        // REQUÊTE et jamais retiré (`src/file.js:155`), dont le coût croissait
        // avec le nombre d'opérations passées, indéfiniment.
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
        // La course classique de tout mécanisme « tester puis attendre » :
        // l'événement passe pendant qu'on s'abonne, et l'attente ne se termine
        // jamais. Rouge : retirer le re-contrôle qui suit l'abonnement.
        const c = fauxCanal();
        const cp = contrePression(c);
        c.poser(SEUIL_TAMPON * 2);
        // Le tampon se vide au moment EXACT de l'abonnement, sans émettre —
        // c'est ce que fait un événement déjà passé.
        const vrai = c.addEventListener.bind(c);
        c.addEventListener = (type, e) => {
            vrai(type, e);
            if (type === 'bufferedamountlow') c.poser(0);
        };
        expect(await resolue(cp.beforeSend())).toBe(true);
    });
});
