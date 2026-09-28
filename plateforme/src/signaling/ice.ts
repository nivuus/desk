// Ephemeral TURN credentials and ICE configuration delivered to both peers.
//
// The secret shared with coturn NEVER leaves this process: the peers only
// receive a derived, dated password, specific to their session. An
// intercepted credential expires on its own.
//
// Separated from the WebSocket server to be testable without opening a socket.

import { createHmac } from 'node:crypto';

/// Default validity of a credential, in seconds.
///
/// Generous on purpose: the credential is used to ALLOCATE, and an allocation is
/// then refreshed with the same credentials. A short duration would make
/// the refresh fail in the middle of a long gaming session.
const DUREE_SECONDES = 86_400;

export interface Identifiants {
    username: string;
    credential: string;
}

export interface ConfigurationIce {
    iceServers: Array<{ urls: string; username: string; credential: string }>;
}

/// Builds a username/password pair accepted by coturn in
/// `use-auth-secret` mode (`--static-auth-secret`).
///
/// `maintenant` is passed as a parameter rather than read from the clock: that is
/// what makes the derivation testable with an exact expected value.
export function deriverIdentifiants(
    secret: string,
    session: string,
    dureeSecondes: number,
    maintenant: number,
): Identifiants {
    const expiration = Math.floor(maintenant / 1000) + dureeSecondes;
    const username = `${expiration}:${session}`;
    const credential = createHmac('sha1', secret).update(username).digest('base64');
    return { username, credential };
}

/// ICE configuration to send to both peers, or `undefined` if no TURN
/// server is configured.
///
/// Both variables must be present together: a half-set
/// configuration would produce allocations refused with 401, with a
/// much more obscure diagnosis than a plain absence of relay.
export function configurationIce(
    env: Record<string, string | undefined>,
    session: string,
    maintenant: number,
): ConfigurationIce | undefined {
    const urls = env.TURN_URL;
    const secret = env.TURN_SECRET;
    if (!urls || !secret) return undefined;

    const { username, credential } = deriverIdentifiants(
        secret,
        session,
        DUREE_SECONDES,
        maintenant,
    );
    return { iceServers: [{ urls, username, credential }] };
}
