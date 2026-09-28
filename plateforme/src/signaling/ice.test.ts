import { createHmac } from 'node:crypto';
import { describe, expect, it } from 'vitest';
import { configurationIce, deriverIdentifiants } from './ice';

describe('deriverIdentifiants', () => {
    it("prefixes the user name with the expiry instant", () => {
        // `maintenant` is in MILLISECONDS (like `Date.now()`), the duration in
        // SECONDS: 1,000,000 ms = 1,000 s, plus 3,600 s, hence 4,600.
        // Confusing the two units produces credentials valid a thousand
        // times too long — hence this test on an exact value.
        const { username } = deriverIdentifiants('secret', 'ma-session', 3600, 1_000_000);
        // Format imposed by coturn in use-auth-secret mode: <expiration>:<who>
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
        // No configuration = local deployment without TURN. It is
        // not an error: the session must go on with host
        // candidates only.
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
        // A half-set configuration is a deployment error.
        // Emitting an entry without valid credentials would make every
        // allocation fail with 401, with a much more obscure diagnosis.
        expect(configurationIce({ TURN_URL: 'turn:x:3478' }, 's', 0)).toBeUndefined();
    });
});
