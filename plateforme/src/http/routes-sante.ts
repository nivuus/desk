// `GET /sante` : la base répond-elle ? Et rien d'autre.
//
// 🔴 CE QUE CETTE ROUTE NE REND PAS, ET C'EST L'ESSENTIEL : ni version, ni
// compte de sessions, ni URL de base, ni nom de moteur, ni durée de
// fonctionnement. Une page de santé bavarde est un INVENTAIRE offert à un
// anonyme. Son test compare l'objet ENTIER, jamais une sous-chaîne,
// précisément pour qu'un ajout futur le fasse rougir.
//
// 🔴 « DEPUIS P2, C'EST PAR CONSTRUCTION LA SEULE ROUTE NON AUTHENTIFIÉE DU
// SERVICE » — CETTE PHRASE ÉTAIT ICI, ET ELLE EST MORTE LE 22 AOÛT 2026 : le
// servant de page (`http/page/routes-page.ts`, armé par `PLATEFORME_PAGE`) en
// est une SECONDE, et il ne consulte aucun jeton. Elle est réécrite plutôt que
// supprimée, parce que ce qu'elle protégeait reste vrai sous une forme PLUS
// ÉTROITE et plus utile : `/sante` est la seule route non authentifiée qui
// TOUCHE LA BASE. C'est exactement ce que le cache ci-dessous existe pour
// borner — le servant, lui, ne lit qu'un disque, et rien d'anonyme n'y
// traduit une requête HTTP en requête SQL.
//
// 🔴 LE VERDICT EST MIS EN CACHE, ET LE CACHE EST LE POINT DE CETTE ROUTE, PAS
// UN RAFFINEMENT. Sans lui, `/sante` traduit une requête HTTP ANONYME en
// requête SQL, à volonté : c'est une amplification, sur la route même qu'un
// équilibreur de charge appelle en boucle. Un attaquant n'aurait qu'à la
// marteler pour faire porter sa charge à la base.
//
// ⚠️ ELLE N'EST PAS FREINÉE, ET C'EST DÉLIBÉRÉ : une sonde d'équilibreur
// freinée déclarerait le service MORT, et provoquerait la panne qu'elle
// surveille. Le cache est ce qui la rend sûre SANS frein — c'est pourquoi les
// deux décisions vivent dans le même paragraphe.
//
// ⚠️ CE N'EST PAS UNE SONDE DE CORRECTION. Elle dit que la base RÉPOND, jamais
// que le service SERT : une route peut être rompue, un canal muet, une session
// jamais appariée, et `/sante` rendra `ok`. Elle ne dit pas davantage que le
// service est PRÊT — `demarrage.ts` garantit déjà que le port ne s'ouvre
// qu'après la base et ses migrations, et son ordre est commenté « NON
// NÉGOCIABLE ». `/sante` ne fait que RAPPORTER ; elle ne doit jamais devenir
// un second endroit qui décide si le service est prêt.

import type { IncomingMessage, ServerResponse } from 'node:http';
import type { Pilote } from '../base/pilote';
import { entetesCors } from './cors';
import { ENTETES_SECURITE } from './entetes';

/// La durée pendant laquelle un verdict est réemployé.
///
/// ⚠️ NON CALIBRÉE : une seconde est un ordre de grandeur, choisi pour être
/// très inférieur à l'intervalle usuel d'une sonde d'équilibreur (5 à 30 s) —
/// de sorte que le cache ne masque jamais une panne à celui qui surveille —
/// tout en absorbant une rafale. Aucune mesure ne l'a fixée.
export const PERIODE_SANTE_MS = 1000;

const CHEMIN = '/sante';

export interface DependancesSante {
    base: Pilote;
    origineClient?: string;
    /// 🔴 L'HORLOGE EST UN PARAMÈTRE, jamais `Date.now()` lu ici : c'est ce
    /// qui rend l'expiration du cache assertable sur une valeur EXACTE dans
    /// une exécution de test, où il n'y aurait autrement qu'un seul instant.
    maintenant: () => number;
    cache: CacheSante;
}

/// Le verdict, et sa date. Vit pour la durée du service, comme
/// `ProprieteDeSession` et `RegistreAgents`.
export class CacheSante {
    private verdictRetenu: boolean | undefined;
    private prisA = 0;
    /// 🔴 LA REQUÊTE EN VOL, ET SANS ELLE LE CACHE NE SERT À RIEN SOUS LA
    /// CHARGE QU'IL EXISTE POUR ABSORBER. Le cas réel est plusieurs sondes
    /// d'équilibreur en vol au même instant : sans déduplication, chacune
    /// lancerait sa propre requête, et le cache n'agirait qu'APRÈS la rafale.
    private enVol: Promise<boolean> | undefined;

    /// Rend `true` si la base répond, en réemployant le verdict de moins de
    /// `PERIODE_SANTE_MS`.
    verdict(base: Pilote, maintenant: number): Promise<boolean> {
        if (this.verdictRetenu !== undefined && maintenant - this.prisA < PERIODE_SANTE_MS) {
            return Promise.resolve(this.verdictRetenu);
        }
        if (this.enVol !== undefined) return this.enVol;

        this.enVol = this.demander(base, maintenant);
        return this.enVol;
    }

    private async demander(base: Pilote, maintenant: number): Promise<boolean> {
        let vivante: boolean;
        try {
            // ⚠️ `SELECT 1` PORTE UNE VALEUR LITTÉRALE, ET C'EST SANS DANGER
            // ICI : la règle du dépôt — « aucune valeur littérale dans une
            // requête » — vise les valeurs qui viennent d'un DEMANDEUR, et le
            // lint de `base/sous-ensemble.test.ts` ne porte que sur les
            // MIGRATIONS. `rendreMarqueurs` ne refuse que les CHAÎNES
            // littérales (apostrophe ou guillemet) ; `1` n'en est pas une, et
            // la requête ne prend aucun paramètre.
            await base.interroger('SELECT 1', []);
            vivante = true;
        } catch {
            // ⚠️ LA CAUSE N'EST PAS JOURNALISÉE ICI, et ce n'est pas un oubli :
            // une base injoignable fait échouer TOUTES les routes, qui
            // journalisent déjà leur propre échec (`http/serveur.ts` le fait
            // pour les cinq). Une ligne de plus PAR SONDE, sur une route qu'un
            // équilibreur appelle en boucle, rendrait le service
            // amplificateur au moment précis où il va mal — c'est la règle
            // « jamais tracer par paquet » du chantier TURN, appliquée au pire
            // moment possible pour l'enfreindre.
            vivante = false;
        }
        this.verdictRetenu = vivante;
        this.prisA = maintenant;
        this.enVol = undefined;
        return vivante;
    }
}

function repondre(
    rep: ServerResponse,
    code: number,
    corps: unknown,
    cors: Record<string, string> | undefined,
): void {
    rep.writeHead(code, {
        'content-type': 'application/json; charset=utf-8',
        // ⚠️ INCONDITIONNELS, et posés sur TOUTE réponse — y compris les
        // réponses d'ERREUR (401, 405, 413, 429, 500, 503), qui portent
        // souvent plus d'information qu'une réponse normale. Ils sont étalés
        // AVANT `cors` pour que la politique d'origine, qui est facultative,
        // ne puisse jamais les écraser par mégarde.
        ...ENTETES_SECURITE,
        ...(cors ?? {}),
    });
    rep.end(JSON.stringify(corps));
}

/// Rend `true` si la requête a été servie, `false` si elle ne concerne pas la
/// santé — le serveur répond alors 404, comme les neuf autres routeurs.
///
/// ⚠️ CETTE PHRASE N'EST PLUS VRAIE SANS CONDITION DEPUIS LE 22 AOÛT 2026 :
/// quand `PLATEFORME_PAGE` est armée, un DIXIÈME routeur — le servant de
/// page — est chaîné APRÈS tous les autres, et il résout n'importe quel
/// chemin. Sur un `GET`, c'est LUI qui répond `200 text/html` au `false`
/// rendu ici ; hors `GET`/`HEAD` il se retire, et le 404 générique reprend la
/// main. Voir `http/chaine.ts`, qui porte le compte et la règle.
export async function servirSante(
    req: IncomingMessage,
    rep: ServerResponse,
    deps: DependancesSante,
): Promise<boolean> {
    const chemin = new URL(req.url ?? '/', 'http://placeholder').pathname;
    // Comparaison EXACTE, jamais un `startsWith` : `/santelle` n'est pas
    // `/sante`, et un préfixe ouvrirait une famille de chemins que personne
    // n'a décidés.
    if (chemin !== CHEMIN) return false;

    const cors = entetesCors(req.headers.origin, deps.origineClient);

    if (req.method === 'OPTIONS') {
        rep.writeHead(204, { ...ENTETES_SECURITE, ...(cors ?? {}) });
        rep.end();
        return true;
    }
    if (req.method !== 'GET') {
        repondre(rep, 405, { refus: 'methode' }, cors);
        return true;
    }

    const vivante = await deps.cache.verdict(deps.base, deps.maintenant());
    // 503 et non 500 : le service est TEMPORAIREMENT indisponible, ce qui est
    // exactement ce qu'un équilibreur doit lire pour retirer l'instance du
    // service sans la déclarer définitivement morte.
    repondre(rep, vivante ? 200 : 503, { etat: vivante ? 'ok' : 'degrade' }, cors);
    return true;
}
