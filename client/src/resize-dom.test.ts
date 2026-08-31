import { describe, expect, it, vi, beforeEach, afterEach } from 'vitest';

import { attacherResizeAuDOM } from './resize-dom';

// 🔴 **CE QUE CE FICHIER GARDE, ET POURQUOI IL EXISTE (lot 33).**
//
// Le remède du lot 33 vit dans DEUX processus de l'agent : le CAPTEUR fait
// suivre le recadrage et l'encodeur au `Resize` du canal de contrôle, le
// SUPERVISEUR fait suivre la fenêtre Windows au `viewport` du `postMessage`.
// Les deux appliquent la MÊME règle pure sur la MÊME borne — donc ils ne se
// battent que si on leur donne deux NOMBRES différents.
//
// **C'est ce fichier-ci qui tient l'invariant « un seul nombre, deux
// destinataires ».** Aucun test Rust ne peut le voir : la divergence naîtrait
// dans le navigateur, et les deux moitiés seraient correctes prises
// séparément — exactement la classe de défaut que `CLAUDE.md` décrit sous
// « une revue par tâche ne peut pas voir un défaut qui franchit une frontière
// de tâche ».

// ⚠️ **`client/` n'a NI jsdom NI happy-dom**, et c'est une propriété du dépôt,
// pas un manque : ses tests DOM injectent leurs dépendances
// (`accent-dom.test.ts` le dit en toutes lettres). `resize-dom.ts`, lui,
// atteint `window` directement — pour ses minuteurs et son
// `devicePixelRatio` —, il faut donc lui en poser un. Minimal, et délégant à
// `globalThis` pour que les faux minuteurs de Vitest le patchent quand même.
function poserUnWindow(dpr: number): void {
    (globalThis as unknown as { window: unknown }).window = {
        devicePixelRatio: dpr,
        setTimeout: (...a: Parameters<typeof setTimeout>) => setTimeout(...a),
        clearTimeout: (h: number) => clearTimeout(h),
        innerWidth: 0,
        innerHeight: 0,
    };
}

class ResizeObserverFactice {
    static dernier: ResizeObserverFactice | undefined;
    declencher: () => void;
    constructor(rappel: () => void) {
        this.declencher = rappel;
        ResizeObserverFactice.dernier = this;
    }
    observe(): void {}
    disconnect(): void {}
}

function monter(largeurCss: number, hauteurCss: number, dpr: number) {
    const envoyes: string[] = [];
    const canal = {
        readyState: 'open',
        send: (o: string) => envoyes.push(o),
        addEventListener: () => {},
    };
    const video = { clientWidth: largeurCss, clientHeight: hauteurCss } as HTMLVideoElement;
    poserUnWindow(dpr);
    const annonces: Array<[number, number]> = [];
    attacherResizeAuDOM(video, { controlChannel: canal } as never, (l, h) =>
        annonces.push([l, h]),
    );
    return { envoyes, annonces };
}

describe('attacherResizeAuDOM — le viewport et le Resize sortent de la MÊME mesure', () => {
    beforeEach(() => {
        vi.useFakeTimers();
        (globalThis as unknown as { ResizeObserver: unknown }).ResizeObserver =
            ResizeObserverFactice;
    });
    afterEach(() => vi.useRealTimers());

    it('émet un viewport pour chaque Resize, avec exactement les mêmes nombres', () => {
        const { envoyes, annonces } = monter(778, 491, 1);
        ResizeObserverFactice.dernier!.declencher();
        vi.advanceTimersByTime(250);

        expect(envoyes).toHaveLength(1);
        // `encodeResize` rend du JSON : on relit le message RÉELLEMENT émis
        // plutôt que de refaire le calcul, sans quoi le test comparerait sa
        // propre arithmétique à elle-même.
        const decode = JSON.parse(envoyes[0]!) as {
            type: string;
            width: number;
            height: number;
        };
        expect(decode.type).toBe('resize');
        // 🔴 L'ASSERTION QUI COMPTE : le viewport annoncé au superviseur doit
        // porter LES MÊMES NOMBRES que le `Resize` envoyé au capteur. Les
        // comparer l'un à l'autre ne serait PAS circulaire ici — ce sont deux
        // chemins de code distincts (`encodeResize` et le rappel), et c'est
        // précisément leur ÉGALITÉ qui est la propriété, pas leur valeur.
        expect(annonces).toEqual([[decode.width, decode.height]]);
    });

    it("n'annonce rien quand la page n'a pas d'ouvreur (rappel absent)", () => {
        // Le cas « page ouverte à la main » : `main.ts` ne passe alors aucun
        // annonceur. Le `Resize` doit partir quand même — la session vit.
        const envoyes: string[] = [];
        const canal = {
            readyState: 'open',
            send: (o: string) => envoyes.push(o),
            addEventListener: () => {},
        };
        const video = { clientWidth: 800, clientHeight: 600 } as HTMLVideoElement;
        poserUnWindow(1);
        attacherResizeAuDOM(video, { controlChannel: canal } as never);
        ResizeObserverFactice.dernier!.declencher();
        vi.advanceTimersByTime(250);
        expect(envoyes).toHaveLength(1);
    });

    it('multiplie par devicePixelRatio UNE seule fois', () => {
        // ⚠️ L'annonceur de `main.ts` ne remultiplie pas : si ce module et lui
        // appliquaient tous deux le facteur, un client HiDPI demanderait
        // QUATRE fois les pixels. Le nombre annoncé doit être celui, déjà
        // périphérique, que le `Resize` porte.
        const { annonces } = monter(800, 600, 2);
        ResizeObserverFactice.dernier!.declencher();
        vi.advanceTimersByTime(250);
        expect(annonces).toEqual([[1600, 1200]]);
    });
});
