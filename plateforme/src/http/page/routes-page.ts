// Le servant de la page bâtie. Il applique le verdict de `resolution.ts`,
// mais NE S'ARRÊTE PAS LÀ.
//
// 🔴 « TOUTE LA SÉCURITÉ VIT DANS LA RÈGLE PURE » ÉTAIT FAUX, ET C'EST UNE
// REVUE PAR EXÉCUTION QUI L'A ÉTABLI (round 2, Critique 3), PAS UNE LECTURE :
// `resolve()`, employé plus bas, est LEXICAL — il ne suit AUCUN lien
// symbolique. Un lien posé dans la racine bâtie (`/lien.json → ../dessus/
// secret.json`) traverse la garde de `resolution.ts` SANS QU'AUCUN `..`
// N'APPARAISSE JAMAIS DANS L'URL : la composition entière — règle pure PLUS
// ce module — est la frontière, pas la règle seule. La garantie réelle contre
// les liens est le `realpath` ci-dessous, sur le chemin CANONIQUE.
//
// 🔴 CE MODULE LIT LE DISQUE, ET C'EST CE QUI LE REND DIFFÉRENT DE
// `resolution.ts` : un trou jugé « sans exploitabilité » là-bas (parce que
// cette règle ne touche rien) peut être exploitable ICI. Trois fois payé dans
// ce lot — le nom vide, le lien symbolique, et le flux non détruit ci-dessous.

import { createReadStream } from 'node:fs';
import { realpath, stat } from 'node:fs/promises';
import type { IncomingMessage, ServerResponse } from 'node:http';
import { resolve, sep } from 'node:path';
import type { Readable } from 'node:stream';
import { pipeline } from 'node:stream/promises';
import { ENTETES_DOCUMENT, ENTETES_RESSOURCE } from './entetes-page';
import { resoudre } from './resolution';

export interface DependancesPage {
    /// Absente OU VIDE ⇒ le servant se retire, et le 404 générique reprend
    /// la main. ⚠️ `''` DOIT être traité identiquement à `undefined` : le
    /// type l'autorise, et `resolve('')` rend le RÉPERTOIRE COURANT du
    /// processus — un servant qui ne testerait que `=== undefined`
    /// publierait le dépôt entier sur une configuration presque vide.
    racinePage?: string;
}

export async function servirPage(
    req: IncomingMessage,
    rep: ServerResponse,
    deps: DependancesPage,
): Promise<boolean> {
    return servirAvecFlux(req, rep, deps, (chemin) => createReadStream(chemin));
}

/// La même route, avec le flux de LECTURE injecté.
///
/// 🔴 EXPORTÉE SÉPARÉMENT POUR LA SEULE RAISON DE LA RENDRE ÉPROUVABLE SANS
/// DANGER : un test qui veut observer « la lecture échoue APRÈS que les
/// en-têtes sont partis » (Critique 2) ne peut pas provoquer un vrai
/// épuisement de descripteurs (`EMFILE`) sans risquer de faire mourir le
/// PROCESSUS DE TEST lui-même — c'est la classe de défaut que
/// `signaling/resilience.test.ts` documente déjà pour une autre route. En
/// production, `servirPage` ci-dessus n'appelle jamais que `createReadStream`.
export async function servirAvecFlux(
    req: IncomingMessage,
    rep: ServerResponse,
    deps: DependancesPage,
    ouvrirFlux: (chemin: string) => Readable,
): Promise<boolean> {
    if (!deps.racinePage) return false;
    // 🔴 HORS GET/HEAD, ON SE RETIRE — jamais un 405. Voir le § 4.3 de la spec :
    // le repli SPA résout n'importe quel chemin, donc un 405 masquerait la
    // faute de frappe d'un appel d'API au lieu de la nommer.
    if (req.method !== 'GET' && req.method !== 'HEAD') return false;

    const chemin = new URL(req.url ?? '/', 'http://placeholder').pathname;
    const verdict = resoudre(chemin);
    // Un refus rend `false` : la chaîne se termine sur le 404 générique, plutôt
    // que d'inventer une SECONDE forme de 404 que rien ne testerait.
    if (!verdict.ok) return false;

    const racine = resolve(deps.racinePage);
    const candidat = resolve(racine, verdict.fichier);
    // Ceinture LEXICALE. Elle ne protège que d'une composition qui sortirait
    // par SEGMENTS (`..`) — `resolution.ts` l'a déjà refusée en amont, donc
    // ceci ne coûte rien de plus ; elle ne protège PAS d'un lien symbolique,
    // qui ne laisse jamais paraître de `..` ici. La garantie contre les
    // liens est le bloc `realpath` qui suit.
    if (candidat !== racine && !candidat.startsWith(racine + sep)) return false;

    let racineReelle: string;
    let fichierReel: string;
    try {
        // 🔴 LA RACINE ELLE-MÊME PEUT LÉGITIMEMENT ÊTRE UN LIEN — la résoudre
        // aussi, jamais seulement le fichier : comparer un chemin canonique
        // à un chemin qui ne l'est pas rendrait la comparaison de préfixe
        // arbitraire.
        racineReelle = await realpath(racine);
        fichierReel = await realpath(candidat);
    } catch {
        // Un lien MORT ou un fichier absent lève ici — c'est le nouveau
        // « fichier introuvable », traité identiquement : refus silencieux,
        // jamais un 500.
        return false;
    }
    // 🔴 LA GARANTIE RÉELLE CONTRE LES LIENS SYMBOLIQUES : la comparaison
    // porte sur les DEUX chemins CANONIQUES, après résolution des liens par
    // `realpath`. C'est ce que la ceinture lexicale ci-dessus NE pouvait pas
    // offrir — mesuré (round 2, Critique 3) : `/lien.json → ../dessus/
    // vole.json` et `/lien-rep/vole.html → ../dessus/vole.html` rendaient
    // tous deux `200` avec le contenu VOLÉ avant ce bloc.
    if (fichierReel !== racineReelle && !fichierReel.startsWith(racineReelle + sep)) {
        return false;
    }

    let infos;
    try {
        infos = await stat(fichierReel);
    } catch {
        return false;
    }
    if (!infos.isFile()) return false;

    rep.writeHead(200, {
        'content-type': verdict.mime,
        ...(verdict.document ? ENTETES_DOCUMENT : ENTETES_RESSOURCE),
    });
    if (req.method === 'HEAD') {
        rep.end();
        return true;
    }

    try {
        // 🔴 `pipeline()`, JAMAIS `.pipe()` — mesuré (round 2, Critiques 1 et
        // 2), et les deux se corrigent PAR LE MÊME CHANGEMENT :
        // `.pipe()` NE DÉTRUIT PAS LA SOURCE quand la destination meurt
        // (client qui abandonne — rechargement, navigation, onglet fermé) :
        // le descripteur reste ouvert pour toujours, monotone, jamais rendu
        // — `0 abandons → 1 fd`, `40 → 41`, `160 → 161`. `pipeline()` détruit
        // les DEUX bouts sur une fermeture prématurée OU une erreur, des
        // deux côtés.
        // `.pipe()` n'attache non plus AUCUN gestionnaire d'erreur sur la
        // source : une lecture qui casse APRÈS que les en-têtes sont partis
        // (un `EMFILE`, atteint quand le leg ci-dessus s'est assez accumulé)
        // émettrait alors une erreur SANS ÉCOUTEUR — et un `EventEmitter` qui
        // émet `error` sans écouteur LÈVE, hors de toute portée qu'un
        // `try/catch` de ce fichier pourrait attraper : mesuré, le service
        // ENTIER meurt (signaling compris).
        await pipeline(ouvrirFlux(fichierReel), rep);
    } catch {
        // Les en-têtes sont déjà PARTIS : le statut ne peut plus changer, il
        // n'y a donc rien à renvoyer de plus juste qu'un refus. La seule
        // décision qui reste est de ne PAS laisser la réponse pendre sur un
        // corps chunked jamais terminé (`.pipe()` n'appelle `end()` que sur
        // l'évènement `'end'`, jamais sur une erreur) : on détruit la
        // connexion plutôt que de la laisser ouverte indéfiniment.
        if (!rep.destroyed) rep.destroy();
    }
    return true;
}
