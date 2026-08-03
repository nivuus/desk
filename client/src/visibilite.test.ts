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
    it('annonce l’état courant dès l’attache', () => {
        const envoyer = vi.fn(() => true);
        attachVisibilite(cibleFactice(), envoyer);
        expect(envoyer).toHaveBeenCalledWith(
            JSON.stringify({ v: CONTROL_VERSION, type: 'visibility', visible: true, focused: true }),
        );
    });

    it('annonce la disparition quand la page est cachée', () => {
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

    it('n’annonce pas deux fois le même état', () => {
        // Le canal de contrôle est fiable et ordonné : réémettre un état
        // inchangé n'apporterait rien et se paierait à chaque blur/focus
        // parasite.
        const cible = cibleFactice();
        const envoyer = vi.fn(() => true);
        attachVisibilite(cible, envoyer);
        envoyer.mockClear();
        cible.declencher('focus');
        expect(envoyer).not.toHaveBeenCalled();
    });

    it('annonce la perte de focus sans perte de visibilité', () => {
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

    it('détache ses trois écouteurs', () => {
        const cible = cibleFactice();
        const detacher = attachVisibilite(cible, vi.fn(() => true));
        detacher();
        const envoyer = vi.fn(() => true);
        cible.declencher('visibilitychange');
        expect(envoyer).not.toHaveBeenCalled();
    });

    it('retente un envoi refusé au signal suivant, au lieu de le perdre', () => {
        // Défaut corrigé : si le canal n'est pas encore ouvert à l'attache,
        // `envoyer` rend `false`. Mémoriser `dernier` malgré cet échec
        // ferait croire l'état déjà annoncé, et aucun changement de
        // visibilité ultérieur ne le réémettrait jamais — la fenêtre resterait
        // endormie pour toujours côté agent, sans aucun symptôme observable.
        const cible = cibleFactice();
        const envoyer = vi.fn(() => false);
        attachVisibilite(cible, envoyer);
        expect(envoyer).toHaveBeenCalledTimes(1);

        // Le canal s'ouvre : le signal suivant doit réémettre le MÊME état
        // (visible=true, focused=true), pas seulement un état différent —
        // c'est précisément ce que l'ancienne déduplication empêchait.
        envoyer.mockClear();
        envoyer.mockImplementation(() => true);
        cible.declencher('focus');
        expect(envoyer).toHaveBeenCalledWith(
            JSON.stringify({ v: CONTROL_VERSION, type: 'visibility', visible: true, focused: true }),
        );
    });
});
