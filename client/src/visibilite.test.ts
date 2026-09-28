import { describe, it, expect, vi } from 'vitest';
import { attachVisibilite, type CibleVisibilite } from './visibilite';
import { CONTROL_VERSION } from '../../proto/ts/control';

function cibleFactice(): CibleVisibilite & { declencher: (nom: string) => void } {
    const ecouteurs = new Map<string, () => void>();
    return {
        hidden: false,
        focalisee: true,
        addEventListener(nom: string, rappel: () => void) {
            ecouteurs.set(nom, rappel);
        },
        removeEventListener(nom: string) {
            ecouteurs.delete(nom);
        },
        declencher(nom: string) {
            ecouteurs.get(nom)?.();
        },
    };
}

describe('attachVisibilite', () => {
    it('announces the current state as soon as it attaches', () => {
        const envoyer = vi.fn(() => true);
        attachVisibilite(cibleFactice(), envoyer);
        expect(envoyer).toHaveBeenCalledWith(
            JSON.stringify({ v: CONTROL_VERSION, type: 'visibility', visible: true, focused: true }),
        );
    });

    it('announces the disappearance when the page is hidden', () => {
        const cible = cibleFactice();
        const envoyer = vi.fn(() => true);
        attachVisibilite(cible, envoyer);
        envoyer.mockClear();
        cible.hidden = true;
        cible.focalisee = false;
        cible.declencher('visibilitychange');
        expect(envoyer).toHaveBeenCalledWith(
            JSON.stringify({ v: CONTROL_VERSION, type: 'visibility', visible: false, focused: false }),
        );
    });

    it('does not announce the same state twice', () => {
        // The control channel is reliable and ordered: re-emitting an unchanged
        // state would bring nothing and would be paid for on every spurious
        // blur/focus.
        const cible = cibleFactice();
        const envoyer = vi.fn(() => true);
        attachVisibilite(cible, envoyer);
        envoyer.mockClear();
        cible.declencher('focus');
        expect(envoyer).not.toHaveBeenCalled();
    });

    it('announces the focus loss without a visibility loss', () => {
        const cible = cibleFactice();
        const envoyer = vi.fn(() => true);
        attachVisibilite(cible, envoyer);
        envoyer.mockClear();
        cible.focalisee = false;
        cible.declencher('blur');
        expect(envoyer).toHaveBeenCalledWith(
            JSON.stringify({ v: CONTROL_VERSION, type: 'visibility', visible: true, focused: false }),
        );
    });

    it('detaches its three listeners', () => {
        const cible = cibleFactice();
        const detacher = attachVisibilite(cible, vi.fn(() => true));
        detacher();
        const envoyer = vi.fn(() => true);
        cible.declencher('visibilitychange');
        expect(envoyer).not.toHaveBeenCalled();
    });

    it('retries a refused send on the next signal, instead of losing it', () => {
        // Fixed defect: if the channel is not open yet at attach time,
        // `envoyer` returns `false`. Memorising `last` despite this failure
        // would make the state look already announced, and no later visibility
        // change would ever re-emit it — the window would stay
        // asleep forever on the agent side, without any observable symptom.
        const cible = cibleFactice();
        const envoyer = vi.fn(() => false);
        attachVisibilite(cible, envoyer);
        expect(envoyer).toHaveBeenCalledTimes(1);

        // The channel opens: the next signal must re-emit the SAME state
        // (visible=true, focused=true), not only a different state —
        // it is precisely what the old deduplication prevented.
        envoyer.mockClear();
        envoyer.mockImplementation(() => true);
        cible.declencher('focus');
        expect(envoyer).toHaveBeenCalledWith(
            JSON.stringify({ v: CONTROL_VERSION, type: 'visibility', visible: true, focused: true }),
        );
    });
});
