import { createHmac } from 'node:crypto';
import { describe, expect, it } from 'vitest';
import { configurationIce, deriverIdentifiants } from './ice';

describe('deriverIdentifiants', () => {
    it("préfixe le nom d'utilisateur par l'instant d'expiration", () => {
        // `maintenant` est en MILLISECONDES (comme `Date.now()`), la durée en
        // SECONDES : 1 000 000 ms = 1 000 s, plus 3 600 s, donc 4 600.
        // Confondre les deux unités produit des identifiants valides mille
        // fois trop longtemps — d'où ce test sur une valeur exacte.
        const { username } = deriverIdentifiants('secret', 'ma-session', 3600, 1_000_000);
        // Format imposé par coturn en mode use-auth-secret : <expiration>:<qui>
        expect(username).toBe('4600:ma-session');
    });

    it('dérive le mot de passe par HMAC-SHA1 du nom, encodé en base64', () => {
        const { username, credential } = deriverIdentifiants('secret', 's', 60, 0);
        const attendu = createHmac('sha1', 'secret').update(username).digest('base64');
        expect(credential).toBe(attendu);
    });

    it('produit des identifiants différents pour deux sessions', () => {
        const a = deriverIdentifiants('secret', 'a', 60, 0);
        const b = deriverIdentifiants('secret', 'b', 60, 0);
        expect(a.credential).not.toBe(b.credential);
    });
});

describe('configurationIce', () => {
    it('rend undefined quand aucun serveur TURN n’est configuré', () => {
        // Absence de configuration = déploiement local sans TURN. Ce n'est
        // pas une erreur : la session doit continuer avec les seuls
        // candidats hôtes.
        expect(configurationIce({}, 's', 0)).toBeUndefined();
    });

    it('rend une entrée iceServers complète quand tout est configuré', () => {
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

    it('rend undefined si l’URL est là mais pas le secret', () => {
        // Une configuration à moitié posée est une erreur de déploiement.
        // Émettre une entrée sans identifiants valides ferait échouer toutes
        // les allocations en 401, avec un diagnostic bien plus obscur.
        expect(configurationIce({ TURN_URL: 'turn:x:3478' }, 's', 0)).toBeUndefined();
    });
});
