import { createHmac } from 'node:crypto';
import { describe, expect, it } from 'vitest';
import { configurationIce, deriverIdentifiants } from './ice';

describe('deriverIdentifiants', () => {
    it("prefixes the user name with the expiry instant", () => {
        // `maintenant` est en MILLISECONDES (comme `Date.now()`), la durée en
        // SECONDES : 1 000 000 ms = 1 000 s, plus 3 600 s, donc 4 600.
        // Confondre les deux unités produit des identifiants valides mille
        // fois trop longtemps — d'où ce test sur une valeur exacte.
        const { username } = deriverIdentifiants('secret', 'ma-session', 3600, 1_000_000);
        // Format imposé par coturn en mode use-auth-secret : <expiration>:<qui>
        expect(username).toBe('4600:ma-session');
    });

    it('derives the password by HMAC-SHA1 of the name, base64 encoded', () => {
        const { username, credential } = deriverIdentifiants('secret', 's', 60, 0);
        const attendu = createHmac('sha1', 'secret').update(username).digest('base64');
        expect(credential).toBe(attendu);
    });

    it('produces different identifiers for two sessions', () => {
        const a = deriverIdentifiants('secret', 'a', 60, 0);
        const b = deriverIdentifiants('secret', 'b', 60, 0);
        expect(a.credential).not.toBe(b.credential);
    });
});

describe('configurationIce', () => {
    it('returns undefined when no TURN server is configured', () => {
        // Absence de configuration = déploiement local sans TURN. Ce n'est
        // pas une erreur : la session doit continuer avec les seuls
        // candidats hôtes.
        expect(configurationIce({}, 's', 0)).toBeUndefined();
    });

    it('returns a complete iceServers entry when everything is configured', () => {
        const config = configurationIce(
            { TURN_URL: 'turn:192.168.3.1:3478', TURN_SECRET: 'secret' },
            'ma-session',
            0,
        );
        expect(config).toEqual({
            iceServers: [
                {
                    urls: 'turn:192.168.3.1:3478',
                    username: '86400:ma-session',
                    credential: expect.any(String),
                },
            ],
        });
    });

    it('returns undefined if the URL is there but not the secret', () => {
        // Une configuration à moitié posée est une erreur de déploiement.
        // Émettre une entrée sans identifiants valides ferait échouer toutes
        // les allocations en 401, avec un diagnostic bien plus obscur.
        expect(configurationIce({ TURN_URL: 'turn:x:3478' }, 's', 0)).toBeUndefined();
    });
});
