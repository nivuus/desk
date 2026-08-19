// Le jeton de recette : comment les pilotes CDP du chantier D continuent de se
// connecter maintenant que la poignée de main `client` est gardée (P2, E6).
//
// 🔴 LE JETON EST OBTENU DE LA PLATEFORME, JAMAIS FORGÉ ICI. Un script qui
// signerait lui-même son jeton porterait le secret de signature du service
// (`PLATEFORME_SECRET_JETON`) dans un fichier versionné ou dans l'argv d'un
// processus : le trou serait DÉPLACÉ, pas fermé. Ce module ne sait donc rien
// signer — il sait appeler `POST /auth/connexion`, et rien de plus.
//
// 🔴 LE MOT DE PASSE VIENT DE L'ENVIRONNEMENT, JAMAIS D'UN FICHIER VERSIONNÉ.
// Le compte de recette se crée par la ligne de commande d'administration
// (`plateforme/src/admin/creer-utilisateur.ts`), qui lit lui aussi son mot de
// passe sur l'entrée standard et refuse un `--mot-de-passe` en argv.
//
// ⚠️ ABSENCE DE CONFIGURATION = AVERTISSEMENT BRUYANT, PAS ÉCHEC. Ces trois
// outils ne sont pas des critères de P2 ; ils peuvent être lancés contre un
// service qui n'a pas la garde. Ce qui serait inacceptable, c'est un silence :
// sans les variables, on le DIT, en nommant ce qui manque, et la session sera
// refusée si le service a la garde. En revanche, des identifiants POSÉS mais
// REFUSÉS lèvent — c'est un défaut réel, et l'avaler ferait diagnostiquer le
// refus de la garde à la place du mauvais mot de passe.

import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

/// Les clés du coffre du navigateur. Elles sont écrites ici ET dans
/// `client/src/jeton.ts` — deux langages, aucun `import` possible entre eux.
/// C'est exactement la façon dont deux constantes divergent en silence, donc
/// `verifierLesCles()` ci-dessous relit le fichier TypeScript et refuse de
/// semer si les valeurs ne correspondent plus.
const CLE_ACCES = 'guac.jeton.acces';
const CLE_RAFRAICHISSEMENT = 'guac.jeton.rafraichissement';

const RACINE = join(dirname(fileURLToPath(import.meta.url)), '..');

function verifierLesCles() {
    const source = readFileSync(join(RACINE, 'src', 'jeton.ts'), 'utf8');
    for (const [nom, valeur] of [
        ['CLE_ACCES', CLE_ACCES],
        ['CLE_RAFRAICHISSEMENT', CLE_RAFRAICHISSEMENT],
    ]) {
        if (!source.includes(`export const ${nom} = '${valeur}';`)) {
            throw new Error(
                `la clé ${nom} de ce module ne correspond plus à celle de client/src/jeton.ts : ` +
                    `le jeton serait semé sous un nom que le client ne lit pas, et la session ` +
                    `serait refusée sans que rien ne dise pourquoi`,
            );
        }
    }
}

/// La configuration du compte de recette, ou `undefined` si elle est absente.
export function configurationRecette(env = process.env) {
    const email = env.RECETTE_EMAIL;
    const motdepasse = env.RECETTE_MOTDEPASSE;
    if (!email || !motdepasse) return undefined;
    return {
        email,
        motdepasse,
        plateformeUrl: env.PLATEFORME_URL ?? 'http://127.0.0.1:8080',
    };
}

/// Obtient une paire de jetons de la plateforme. LÈVE si le service refuse ou
/// ne répond pas : voir l'en-tête.
export async function obtenirPaire(config) {
    const reponse = await fetch(`${config.plateformeUrl}/auth/connexion`, {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ email: config.email, motdepasse: config.motdepasse }),
    });
    const corps = await reponse.json().catch(() => undefined);
    if (!reponse.ok) {
        throw new Error(
            `POST ${config.plateformeUrl}/auth/connexion a rendu ${reponse.status} ` +
                `(${corps?.refus ?? 'sans motif'}) — le compte de recette RECETTE_EMAIL ` +
                `existe-t-il, et son mot de passe est-il celui de RECETTE_MOTDEPASSE ?`,
        );
    }
    return { acces: corps.acces, rafraichissement: corps.rafraichissement };
}

/// Sème la paire dans le `localStorage` de toute page à venir.
///
/// ⚠️ `Page.addScriptToEvaluateOnNewDocument` NE COURT PAS sur une page ouverte
/// par `window.open` — piège mesuré au sous-bloc D5. Les trois pilotes
/// naviguent directement, donc ils ne sont pas concernés ; mais c'est bien
/// pourquoi le jeton doit vivre dans `localStorage`, partagé entre les onglets
/// d'une même origine, plutôt que dans une variable injectée que les fenêtres
/// ouvertes par la page-shell ne verraient jamais.
///
/// Rend `true` si le jeton a été semé, `false` s'il n'y avait rien à semer.
export async function semerJeton(cdp, env = process.env) {
    const config = configurationRecette(env);
    if (!config) {
        console.warn(
            "⚠️ aucun jeton semé : RECETTE_EMAIL et RECETTE_MOTDEPASSE ne sont pas posées. " +
                "Depuis le sous-bloc P2, un pair `client` sans jeton est REFUSÉ par la garde " +
                '(motif `jeton-absent`) et son socket fermé. Poser aussi PLATEFORME_URL si ' +
                "le service n'écoute pas sur http://127.0.0.1:8080.",
        );
        return false;
    }
    verifierLesCles();
    const paire = await obtenirPaire(config);
    await cdp.send('Page.addScriptToEvaluateOnNewDocument', {
        source: `
            try {
                localStorage.setItem(${JSON.stringify(CLE_ACCES)}, ${JSON.stringify(paire.acces)});
                localStorage.setItem(${JSON.stringify(CLE_RAFRAICHISSEMENT)}, ${JSON.stringify(paire.rafraichissement)});
            } catch (e) {
                // \`about:blank\` et les origines opaques n'ont pas de stockage
                // accessible : y échouer est normal, et lever ici tuerait la
                // page avant même la navigation vers l'origine réelle.
            }
        `,
    });
    console.log(`jeton de recette semé pour ${config.email} (${config.plateformeUrl})`);
    return true;
}
