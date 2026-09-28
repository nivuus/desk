// The acceptance token: how the CDP drivers of project D keep
// connecting now that the `client` handshake is guarded (P2, E6).
//
// 🔴 THE TOKEN IS OBTAINED FROM THE PLATFORM, NEVER FORGED HERE. A script that
// signed its own token would carry the service's signing secret
// (`PLATEFORME_SECRET_JETON`) in a versioned file or in the argv of a
// process: the hole would be MOVED, not closed. This module therefore knows how to sign
// nothing — it knows how to call `POST /auth/connexion`, and nothing more.
//
// 🔴 THE PASSWORD COMES FROM THE ENVIRONMENT, NEVER FROM A VERSIONED FILE.
// The acceptance account is created by the administration command line
// (`plateforme/src/admin/creer-utilisateur.ts`), which also reads its
// password on standard input and refuses a `--mot-de-passe` in argv.
//
// ⚠️ MISSING CONFIGURATION = LOUD WARNING, NOT FAILURE. These three
// tools are not P2 criteria; they may be run against a
// service that does not have the guard. What would be unacceptable is silence:
// without the variables, we SAY so, naming what is missing, and the session will be
// refused if the service has the guard. On the other hand, credentials SET but
// REFUSED throw — it is a real defect, and swallowing it would make one diagnose the
// guard's refusal instead of the wrong password.

import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

/// The keys of the browser store. They are written here AND in
/// `client/src/jeton.ts` — two languages, no `import` possible between them.
/// It is exactly how two constants silently diverge, so
/// `verifierLesCles()` below re-reads the TypeScript file and refuses to
/// seed if the values no longer match.
const CLE_ACCES = 'guac.jeton.acces';
const CLE_RAFRAICHISSEMENT = 'guac.jeton.rafraichissement';

const RACINE = join(dirname(fileURLToPath(import.meta.url)), '..');

function verifierLesCles() {
    const source = readFileSync(join(RACINE, 'src', 'jeton.ts'), 'utf8');
    for (const [nom, value] of [
        ['CLE_ACCES', CLE_ACCES],
        ['CLE_RAFRAICHISSEMENT', CLE_RAFRAICHISSEMENT],
    ]) {
        if (!source.includes(`export const ${nom} = '${value}';`)) {
            throw new Error(
                `the key ${nom} of this module no longer matches the one in client/src/jeton.ts: ` +
                    `the token would be seeded under a name the client does not read, and the session ` +
                    `would be refused without anything saying why`,
            );
        }
    }
}

/// The configuration of the acceptance account, or `undefined` if it is absent.
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

/// Gets a pair of tokens from the platform. THROWS if the service refuses or
/// does not answer: see the header.
export async function obtenirPaire(config) {
    const reponse = await fetch(`${config.plateformeUrl}/auth/connexion`, {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ email: config.email, motdepasse: config.motdepasse }),
    });
    const corps = await reponse.json().catch(() => undefined);
    if (!reponse.ok) {
        throw new Error(
            `POST ${config.plateformeUrl}/auth/connexion returned ${reponse.status} ` +
                `(${corps?.refus ?? 'no reason'}) — does the test account RECETTE_EMAIL ` +
                `exist, and is its password the one in RECETTE_MOTDEPASSE?`,
        );
    }
    return { acces: corps.acces, rafraichissement: corps.rafraichissement };
}

/// Seeds the pair into the `localStorage` of any page to come.
///
/// ⚠️ `Page.addScriptToEvaluateOnNewDocument` DOES NOT RUN on a page opened
/// by `window.open` — a trap measured in sub-block D5. The three drivers
/// navigate directly, so they are not affected; but that is indeed
/// why the token must live in `localStorage`, shared between the tabs
/// of the same origin, rather than in an injected variable that windows
/// opened by the shell page would never see.
///
/// Returns `true` if the token was seeded, `false` if there was nothing to seed.
export async function semerJeton(cdp, env = process.env) {
    const config = configurationRecette(env);
    if (!config) {
        console.warn(
            "⚠️ no token seeded: RECETTE_EMAIL and RECETTE_MOTDEPASSE are not set. " +
                "Since sub-block P2, a `client` peer without a token is REFUSED by the guard " +
                '(reason `jeton-absent`) and its socket closed. Also set PLATEFORME_URL if ' +
                "the service is not listening on http://127.0.0.1:8080.",
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
                // \`about:blank\` and opaque origins have no accessible
                // storage: failing there is normal, and throwing here would kill the
                // page before the navigation to the real origin even happens.
            }
        `,
    });
    console.log(`test token seeded for ${config.email} (${config.plateformeUrl})`);
    return true;
}
