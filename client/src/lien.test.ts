import { describe, expect, it } from 'vitest';
import { texteLien } from './lien';
import type { LinkMessage } from '../../proto/ts/control';

function lien(partiel: Partial<LinkMessage>): LinkMessage {
    return {
        v: 3,
        type: 'link',
        bitrate: 8_000_000,
        width: 1920,
        height: 1080,
        quality: 'bonne',
        adaptation: 'active',
        ...partiel,
    };
}

describe('texteLien', () => {
    it('reports nothing in particular when all is well', () => {
        const t = texteLien(lien({}));
        expect(t.alerte).toBe(false);
        expect(t.resume).toContain('8.0 Mb/s');
    });

    it('says why the image went soft when the resolution is reduced', () => {
        const t = texteLien(lien({ quality: 'degradee', width: 1280, height: 720 }));
        expect(t.alerte).toBe(true);
        expect(t.resume).toContain('1280×720');
        // Le texte doit nommer la CAUSE, pas seulement l'effet : un
        // utilisateur qui lit « 1280×720 » sans explication croit à un bug.
        expect(t.resume.toLowerCase()).toContain('network');
    });

    it('warns explicitly when the link no longer allows twitchy gaming', () => {
        const t = texteLien(lien({ quality: 'insuffisante' }));
        expect(t.alerte).toBe(true);
        expect(t.resume.toLowerCase()).toContain('insufficient');
    });

    it('tells an unavailable adaptation from a degraded link', () => {
        const t = texteLien(lien({ adaptation: 'indisponible' }));
        // Pas une alerte : le lien peut très bien être excellent.
        expect(t.alerte).toBe(false);
        expect(t.resume.toLowerCase()).toContain('adaptation unavailable');
    });
});
