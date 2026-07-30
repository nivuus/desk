// Identifiants TURN éphémères et configuration ICE délivrée aux deux pairs.
//
// Le secret partagé avec coturn ne quitte JAMAIS ce processus : les pairs ne
// reçoivent qu'un mot de passe dérivé, daté, et propre à leur session. Un
// identifiant intercepté expire tout seul.
//
// Séparé du serveur WebSocket pour être testable sans ouvrir de socket.

import { createHmac } from 'node:crypto';

/// Durée de validité par défaut d'un identifiant, en secondes.
///
/// Généreuse à dessein : l'identifiant sert à ALLOUER, et une allocation se
/// rafraîchit ensuite avec les mêmes identifiants. Une durée courte ferait
/// échouer le rafraîchissement au milieu d'une longue session de jeu.
const DUREE_SECONDES = 86_400;

export interface Identifiants {
    username: string;
    credential: string;
}

export interface ConfigurationIce {
    iceServers: Array<{ urls: string; username: string; credential: string }>;
}

/// Fabrique un couple identifiant/mot de passe accepté par coturn en mode
/// `use-auth-secret` (`--static-auth-secret`).
///
/// `maintenant` est passé en paramètre plutôt que lu de l'horloge : c'est ce
/// qui rend la dérivation testable avec une valeur attendue exacte.
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

/// Configuration ICE à envoyer aux deux pairs, ou `undefined` si aucun serveur
/// TURN n'est configuré.
///
/// Les deux variables doivent être présentes ensemble : une configuration à
/// moitié posée produirait des allocations refusées en 401, avec un
/// diagnostic beaucoup plus obscur qu'une absence franche de relais.
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
