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
// ⚠️ UNE MOITIÉ DE CE « À QUICONQUE » AVAIT SURVÉCU AU SOUS-BLOC P2, ET ELLE
// EST FERMÉE DEPUIS P3. Le texte disait ici qu'un pair se déclarant
// `{"role":"agent"}` était toujours accepté sans identité et recevait ses
// identifiants TURN de 86 400 s ; ce n'est plus vrai — le rôle `agent` exige
// son jeton, de type `agent` et à sujet préfixant la session
// (`identite/garde.ts`), et ce jeton s'obtient sur le canal `/agent` contre le
// secret d'enrôlement de la VM.
//
// **L'argument ci-dessus n'en perd RIEN**, et c'est pourquoi le paragraphe est
// corrigé plutôt que supprimé : `PLATEFORME_HOTE` borne QUI PEUT ATTEINDRE le
// port, ce qui vaut avant toute authentification et pour les deux chemins —
// le relais comme le canal d'enrôlement, dont les tentatives de secret ne sont
// bridées par rien à ce jour (c'est le sujet de P5).

//
// La lecture d'environnement se fait ICI et nulle part ailleurs : `env` est un
// paramètre, jamais `process.env` lu en douce, ce qui rend la fonction pure et
// testable sans salir l'environnement du processus de test.

import { LONGUEUR_SECRET_MIN } from './identite/jeton';

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
    /// PLATEFORME_SECRET_JETON — AUCUN défaut, `LONGUEUR_SECRET_MIN`
    /// caractères au moins. Voir le commentaire ci-dessous : un secret tiré
    /// au hasard au démarrage serait pire qu'une absence de secret.
    secretJeton: string;
    /// PLATEFORME_ORIGINE_CLIENT — FACULTATIVE. Absente, aucun en-tête CORS
    /// n'est émis et le navigateur refuse : le défaut est le refus.
    origineClient?: string;
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

    // 🔴 AUCUN DÉFAUT, et surtout pas un défaut ALÉATOIRE. Un secret tiré au
    // démarrage passerait tous les tests de forme, puis invaliderait à chaque
    // redémarrage l'ensemble des jetons délivrés — les utilisateurs seraient
    // déconnectés sans qu'aucune trace n'en donne la cause. C'est la même
    // décision que `PLATEFORME_HOTE` : une rupture bruyante vaut mieux qu'une
    // dégradation silencieuse.
    //
    // Le test de la chaîne VIDE est distinct de celui de l'absence, parce que
    // `env.X ?? 'defaut'` ne rattrape pas `''` — P1 a payé cette erreur exacte
    // à sa tâche 1, où un des deux rouges annoncés était en réalité vert.
    const secretJeton = env.PLATEFORME_SECRET_JETON;
    if (secretJeton === undefined || secretJeton === '') {
        throw new Error(
            "PLATEFORME_SECRET_JETON est obligatoire et n'a aucun défaut : sans lui " +
                'aucun jeton ne peut être signé, et un défaut aléatoire invaliderait ' +
                'toutes les sessions à chaque redémarrage.',
        );
    }
    if (secretJeton.length < LONGUEUR_SECRET_MIN) {
        throw new Error(
            `PLATEFORME_SECRET_JETON est trop court : ${secretJeton.length} caractères, ` +
                `${LONGUEUR_SECRET_MIN} au moins sont exigés — un secret devinable ` +
                "n'authentifie personne.",
        );
    }

    // FACULTATIVE, contrairement à `PLATEFORME_HOTE`, et l'asymétrie est dans
    // les conséquences : une origine absente produit un refus BRUYANT du
    // navigateur, que l'opérateur voit immédiatement ; une adresse d'écoute
    // absente produirait une écoute universelle SILENCIEUSE. Refuser de
    // démarrer pour elle casserait par ailleurs le déploiement de P5, où le
    // proxy inverse met le client et la plateforme sur la MÊME origine et où
    // aucune valeur n'aurait de sens.
    const brutOrigine = env.PLATEFORME_ORIGINE_CLIENT;
    const origineClient = brutOrigine === undefined || brutOrigine === '' ? undefined : brutOrigine;

    return { hote, port, base, urlBase, secretJeton, origineClient };
}
