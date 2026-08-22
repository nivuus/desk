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

/// La même route, avec le flux de LECTURE injecté — seul point d'extension
/// par rapport à `servirPage`, qui n'appelle jamais que `createReadStream`.
/// Il n'existe QUE pour rendre éprouvable le chemin d'erreur d'en dessous :
/// une lecture qui casse APRÈS que les en-têtes sont partis.
///
/// 🔴 CE COMMENTAIRE A MENTI TROIS FOIS DE SUITE SUR CE MÊME SUJET, CHAQUE
/// CORRECTION EN PRODUISANT UNE NEUVE — le patron que `CLAUDE.md` nomme
/// « corriger une affirmation fausse peut en produire une autre ». D'où la
/// forme ci-dessous : CHAQUE affirmation porte la commande qui l'établit.
/// Ne croire aucune d'elles ; les relancer. Toutes se lancent depuis la
/// racine du dépôt, sauf les deux `vitest`, depuis `plateforme/`.
///
/// ① LE RAPPEL PAR DÉFAUT N'EST PAS UN TROU DE COUVERTURE. Le seul appelant
/// de `servirAvecFlux` dans le produit est `servirPage` lui-même ; tout le
/// reste passe par `chaine.ts`, qui n'appelle que `servirPage`. Dans
/// `routes-page.test.ts`, 18 des 20 tests montent un VRAI serveur
/// (`demarrerServeur` → `servirTout` → `servirPage`, donc un vrai
/// `createReadStream`) ; les 2 autres sont les tests unitaires d'ici.
///   grep -rn 'servirPage\|servirAvecFlux' plateforme/src
///   grep -c 'await demarrerServeur(' plateforme/src/http/page/routes-page.test.ts   → 18
///   grep -c '    it(' plateforme/src/http/page/routes-page.test.ts                  → 20
///
/// ② CE QUE `signaling/resilience.test.ts` ÉTABLIT — le PATRON, et lui seul :
/// éprouver la mort d'un processus EST possible dans ce dépôt. Il lance le
/// vrai point d'entrée `src/index.ts` en processus ENFANT (`spawn(tsxBin,
/// …)`) et observe sa SURVIE (`child.exitCode`, `child.killed`). Prétendre
/// qu'un tel précédent manque serait faux — c'était le mensonge n°1.
///   grep -n 'spawn(tsxBin\|child.exitCode' plateforme/src/signaling/resilience.test.ts
///
/// ③ 🔴 CE QU'IL N'ÉTABLIT PAS — c'était le mensonge n°2, qui lui prêtait
/// « un vrai `createReadStream` » : il n'exerce AUCUNE lecture de fichier
/// servie au réseau, et ne touche ni ce module ni aucune route HTTP. Ses 3
/// tests n'ouvrent QUE des WebSocket, sur `/signal` et `/agent`, contre une
/// trame `null` et une trame au-delà de `maxPayload`. Il n'est donc un
/// précédent que pour la FORME du montage, jamais pour son objet.
///   grep -n createReadStream plateforme/src/signaling/resilience.test.ts   → RIEN
///   grep -n 'new WebSocket(' plateforme/src/signaling/resilience.test.ts
///     (les 3 seules connexions du fichier ; aucun `http.get`/`http.request`)
///   grep -c WebSocket        plateforme/src/signaling/resilience.test.ts   → 8
///     (témoin négatif du même fichier : le RIEN ci-dessus est une absence
///      mesurée, pas un grep qui ne peut pas trouver — `CLAUDE.md`, « un zéro
///      n'est interprétable qu'avec un témoin négatif »)
///   grep -n 'describe(' plateforme/src/signaling/resilience.test.ts
///
/// ④ POURQUOI CE PATRON N'A PAS ÉTÉ REPRIS ICI — un CHOIX, pas une
/// impossibilité (③ dit pourquoi il ne se transposerait pas tel quel, mais
/// rien n'interdisait d'en écrire l'équivalent) : son coût, et une suite
/// dont la durée a déjà régressé une fois dans ce lot (round 3, Neuf 4).
/// Mesuré le 22 août 2026, depuis `plateforme/`, QUATRE passes de chaque :
///   npx vitest run src/signaling/resilience.test.ts
///     → 3 tests, `tests` de 523 ms à 1,15 s, soit ~175 à ~385 ms par test
///   npx vitest run src/http/page/routes-page.test.ts
///     → 20 tests, `tests` de 516 à 872 ms, soit ~26 à ~44 ms par test
/// 🔴 NE PAS CITER UNE PASSE UNIQUE : ces durées varient du simple au double
/// d'une passe à l'autre, et une valeur isolée se lira comme fausse au
/// premier relanceur — la première rédaction de ce bloc a fait exactement
/// cela. Ce qui porte l'argument est l'ORDRE DE GRANDEUR, pas un chiffre :
/// en appariant les EXTRÊMES, le montage à processus reste de 4× (175/44)
/// à 15× (385/26) plus cher par test — jamais moins de 4×. ⚠️ Et vitest
/// bascule en `1.15s` au-delà de la seconde : un dépouillement qui ne
/// cherche que `ms` PERD la passe la plus lente.
/// Ce facteur est en outre le cas le PLUS favorable au montage à processus :
/// `resilience.test.ts` amortit son `spawn` sur tout le fichier par un
/// `beforeAll` unique, là où le flux fautif d'ici se refabrique par test.
///   grep -c '^beforeAll(' plateforme/src/signaling/resilience.test.ts   → 1
///     (⚠️ `grep -c beforeAll` rendrait 2 : la ligne d'`import` compte aussi)
///
/// ⑤ 🔴 CE QUE CE CHOIX COÛTE, ET QUE PERSONNE NE DOIT LIRE COMME COUVERT :
/// les DEUX tests qui appellent `servirAvecFlux` (par une fabrique commune)
/// n'observent que la FONCTION face à une erreur de flux, sur un `req`/`rep`
/// FABRIQUÉS — ni socket, ni port, ni processus serveur. AUCUN test de
/// `plateforme/src` n'a jamais vu une erreur de lecture MI-RÉPONSE sur un
/// serveur HTTP réel, ni la survie du service à un VRAI `EMFILE` : la seule
/// erreur mi-flux jamais exercée est poussée à la main, et elle le dit.
///   grep -rn EMFILE plateforme/src | grep -vE ':[0-9]+: *//'
///     → UNE seule ligne, dans le test : un `this.emit('error', …)` sur un
///       flux fabriqué. Tout le reste n'est que du commentaire.
///     ⚠️ LE FILTRE `grep -v` N'EST PAS UN ORNEMENT : sans lui, ce grep SE
///       COMPTE LUI-MÊME — le paragraphe que vous lisez nomme `EMFILE` trois
///       fois. La première rédaction de ce bloc annonçait « 3 commentaires
///       de ce fichier » et était fausse à l'instant où elle s'écrivait.
///       N'en tirer AUCUN nombre sans le filtre.
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
        //
        // ⚠️ COURSE `realpath` → `open` (TOCTOU), jugée et classée (round 3) :
        // réelle, mais elle ne compte pas ici — il faudrait que l'attaquant
        // puisse ÉCRIRE dans la racine bâtie, où il dispose déjà d'une
        // attaque plus simple, et `createReadStream` plus bas ouvre le
        // chemin CANONIQUE (`fichierReel`), pas le lien.
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
    } catch (cause) {
        // Les en-têtes sont déjà PARTIS : le statut ne peut plus changer, il
        // n'y a donc rien à renvoyer de plus juste qu'un refus. La seule
        // décision qui reste est de ne PAS laisser la réponse pendre sur un
        // corps chunked jamais terminé (`.pipe()` n'appelle `end()` que sur
        // l'évènement `'end'`, jamais sur une erreur) : on détruit la
        // connexion plutôt que de la laisser ouverte indéfiniment.
        //
        // 🔴 UN `catch` MUET ÉTAIT LE DÉFAUT NEUF 1 DU ROUND 3 : sans cette
        // ligne, un `EMFILE` passait de FATAL ET BRUYANT (avant ce lot) à
        // SILENCIEUX ET SANS TRACE — la panne la plus discrète possible, ce
        // que `CLAUDE.md` combat en premier. Le POINT D'APPEL de `servirTout`
        // dans `demarrerServeur` (`serveur.ts`) ne peut RIEN voir : `return
        // true` en fin de fonction lui dit que la route a été servie. Le
        // chemin demandé est journalisé, PAS le corps de la requête — ce
        // dépôt n'écrit jamais dans un journal ce qui pourrait porter un
        // secret.
        // ⚠️ CE COMMENTAIRE CITAIT AUPARAVANT UN NUMÉRO DE LIGNE
        // (`serveur.ts:308`) QUE `CLAUDE.md` PROSCRIT, ET QUI ÉTAIT FAUX DÈS
        // SA POSE (la ligne réelle était 282, jamais 308 — vérifié par
        // `git show 912f1e2:plateforme/src/http/serveur.ts | grep -n
        // servirTout`) : un relevé ultérieur du même round l'a même requalifié
        // à tort d'« exact aujourd'hui ». Corrigé en NOMMANT la chose plutôt
        // qu'en comptant les lignes qui l'en séparent — revue transverse de
        // fin de lot, 22 août 2026.
        console.error(`page servie en echec de lecture, chemin=${chemin} : ${String(cause)}`);
        if (!rep.destroyed) rep.destroy();
    }
    return true;
}
