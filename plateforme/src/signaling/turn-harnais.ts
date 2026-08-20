// Le harnais des variables TURN AMBIANTES, pour les fichiers de test qui
// ouvrent un vrai relais.
//
// 🔴 POURQUOI IL EXISTE. `relais.ts` lit `process.env` à CHAQUE déclaration de
// pair (`configurationIce(process.env, …)`) et, si `TURN_URL` et `TURN_SECRET`
// sont tous deux posés, envoie un `ice-config` au pair AVANT tout autre
// message. Un test qui lit « le message suivant » reçoit alors cet
// `ice-config` à la place de ce qu'il attendait. La conséquence n'est pas
// théorique : `scripts/verify-all.sh` lancé depuis un shell où l'on a fait
// `set -a && source .env && set +a` — la séquence que `CLAUDE.md` prescrit
// pour tout le reste du dépôt — faisait échouer SIX tests de
// `server.test.ts`, quand le même script depuis un shell nu sortait à 0.
// Le service était sain, les tests mesuraient l'environnement du développeur.
//
// 🔴 ET POURQUOI ON NE RESTAURE PAS « À LA MAIN ». La restauration naïve
//
//     const avant = process.env.TURN_URL;   // undefined si absente
//     …
//     process.env.TURN_URL = avant;         // ⚠️ écrit la CHAÎNE "undefined"
//
// ne rend pas la variable à son absence : `process.env` coerce tout en chaîne,
// et `"undefined"` est TRUTHY. Une variable ainsi « restaurée » fait donc
// délivrer une configuration ICE dont l'URL est le mot `undefined` — mesuré.
// Seul `delete` rend une variable absente.

/// Les deux variables que `configurationIce` lit, et elles seules.
const CLES = ['TURN_URL', 'TURN_SECRET'] as const;

export interface TurnAmbiant {
    url?: string;
    secret?: string;
}

/// Pose l'état TURN ambiant demandé et rend la fonction qui rétablit l'état
/// d'avant — `delete` compris, pour les clés qui étaient absentes.
///
/// Appelé sans argument (ou avec `{}`), il NEUTRALISE : le relais se comporte
/// alors comme sur une machine sans serveur TURN configuré, ce qui est la
/// seule façon pour un test de « message suivant » d'être hermétique à
/// l'environnement de celui qui le lance.
export function poserTurnAmbiant(valeurs: TurnAmbiant = {}): () => void {
    const avant = CLES.map((cle) => [cle, process.env[cle]] as const);

    appliquer('TURN_URL', valeurs.url);
    appliquer('TURN_SECRET', valeurs.secret);

    return () => {
        for (const [cle, valeur] of avant) appliquer(cle, valeur);
    };
}

function appliquer(cle: (typeof CLES)[number], valeur: string | undefined): void {
    if (valeur === undefined) delete process.env[cle];
    else process.env[cle] = valeur;
}
