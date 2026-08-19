// PLATEFORME_HOTE n'a AUCUN défaut, et c'est le point de cette fonction.
//
// L'ex-`signaling/src/server.ts:67` — aujourd'hui `src/signaling/relais.ts`,
// le paquet `signaling/` ayant disparu au sous-bloc P1 — faisait
// `new WebSocketServer({ port })` sans
// `host` : le service écoutait sur toutes les interfaces, et délivrait des
// identifiants TURN valables 24 h (`ice.ts:16`) à quiconque atteignait le
// port. Poser un défaut ici — même `127.0.0.1` — ferait passer le critère ④
// du sous-bloc P1 sans rien garantir : l'opérateur ne saurait jamais sur quoi
// il écoute. Une rupture bruyante vaut mieux qu'une écoute universelle
// silencieuse.
//
// La lecture d'environnement se fait ICI et nulle part ailleurs : `env` est un
// paramètre, jamais `process.env` lu en douce, ce qui rend la fonction pure et
// testable sans salir l'environnement du processus de test.

export interface Config {
    /// PLATEFORME_HOTE — aucun défaut, voir le commentaire de tête.
    hote: string;
    /// PLATEFORME_PORT, défaut 8080.
    port: number;
    /// PLATEFORME_BASE, défaut 'sqlite'. Une valeur inconnue LÈVE : un repli
    /// silencieux sur sqlite ferait tourner la production sur un fichier
    /// local sans que rien ne le dise.
    base: 'sqlite' | 'postgres';
    /// PLATEFORME_BASE_URL — chemin de fichier SQLite ou URL de connexion pg.
    urlBase: string;
}

const BASES = ['sqlite', 'postgres'] as const;

export function lireConfig(env: Record<string, string | undefined>): Config {
    const hote = env.PLATEFORME_HOTE;
    if (hote === undefined || hote === '') {
        throw new Error(
            "PLATEFORME_HOTE est obligatoire et n'a aucun défaut : nommer l'adresse " +
                "d'écoute, sans quoi le service écouterait sur toutes les interfaces.",
        );
    }

    const brutPort = env.PLATEFORME_PORT;
    let port = 8080;
    if (brutPort !== undefined && brutPort !== '') {
        if (!/^\d+$/.test(brutPort)) {
            throw new Error(`PLATEFORME_PORT doit être un entier, reçu : ${brutPort}`);
        }
        port = Number(brutPort);
    }

    const brutBase = env.PLATEFORME_BASE;
    const base = (brutBase === undefined || brutBase === '' ? 'sqlite' : brutBase) as Config['base'];
    if (!(BASES as readonly string[]).includes(base)) {
        throw new Error(
            `PLATEFORME_BASE doit valoir ${BASES.join(' ou ')}, reçu : ${brutBase}`,
        );
    }

    const urlBase = env.PLATEFORME_BASE_URL ?? ':memory:';

    return { hote, port, base, urlBase };
}
