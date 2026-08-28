// `POST /session` : l'enchaînement de la spec §4 — l'utilisateur demande une
// session, la plateforme vérifie l'attribution ET la fraîcheur, et rend le
// préfixe.
//
// 🔴 LES CRITÈRES ③ ET ④ SONT DANS CE FICHIER, et chacune de leurs assertions
// a son propre `it()` : `expect` interrompt un test à la première assertion
// fausse, si bien qu'une seconde assertion placée à côté ne serait éprouvée
// par rien (leçon ①A/①A-bis de P2).
//
// ⚠️ DIX TESTS ET NON LES NEUF DU PLAN, annoncé avant d'être lu : la requête
// préalable `OPTIONS`, sans laquelle la route est inatteignable depuis un
// navigateur (même défaut de plan qu'à la tâche 9).

import { afterEach, describe, expect, it } from 'vitest';
import { createServer, type Server } from 'node:http';
import { baseNeuve, MOTEUR } from '../base/harnais';
import type { Pilote } from '../base/pilote';
import { SEUIL_INJOIGNABLE_MS } from '../agents/fraicheur';
import { enroler, marquerVu } from '../depot/agent';
import { creerUtilisateur } from '../depot/utilisateur';
import { signer } from '../identite/jeton';
import { BACKEND_STATIQUE } from '../orchestration/refus';
import { Frein, REQUETES_MAX_ADRESSE } from '../securite/frein';
import { servirSession } from './routes-session';

const SECRET = 'un-secret-de-plateforme-de-quarante-octets';
const ORIGINE = 'http://127.0.0.1:5173';
const MS = 1_787_136_773_742;

/// 🔴 CETTE BORNE A ÉTÉ MESURÉE AVANT D'ÊTRE ÉCRITE, jamais devinée. Le
/// protocole est celui du plan : une requête de CHAUFFE d'abord — la première
/// requête d'un test porte l'établissement de connexion, qui n'est pas ce
/// qu'on mesure —, puis dix requêtes chronométrées.
///
/// RELEVÉ le 20 août 2026 sur cette machine, par le test lui-même instrumenté
/// puis restauré — QUATRE exécutions, pire cas de chacune :
///
///     sqlite   : 3,49 ms   puis 5,93 ms
///     postgres : 7,27 ms   puis 18,73 ms
///
/// Pire relevé toutes exécutions confondues : **18,73 ms**, sous Postgres. La
/// borne de 250 ms est **13,3 fois** ce pire cas, et **8 fois en dessous** des
/// 2 000 ms que la mutation de ③b insère — les deux marges comptent : la
/// première évite un test instable, la seconde garantit que le contrôle PEUT
/// échouer. Une borne posée sans avoir été mesurée serait un contrôle dont on
/// ignore s'il peut échouer, patron que ce dépôt a payé quatre fois.
///
/// ⚠️ QUATRE EXÉCUTIONS NE SONT PAS UN TAUX, et la machine porte une charge
/// étrangère variable : ce chiffre borne ce qui a été observé, rien de plus.
const BORNE_MS = 250;

let base: Pilote | undefined;
let http: Server | undefined;

afterEach(async () => {
    if (http) await new Promise<void>((r) => http!.close(() => r()));
    http = undefined;
    await base?.fermer();
    base = undefined;
});

/// L'horloge est INJECTÉE : c'est ce qui rend la transition du critère ④c
/// observable. Un `Date.now()` lu dans le module ne laisserait qu'un instant.
///
/// ⚠️ `frein` EST UN PARAMÈTRE, DÉFAUT NEUF PAR APPEL : chaque test isole
/// ainsi son propre budget, sans qu'aucun ne puisse en épuiser un autre.
async function servir(
    nom: string,
    instant = MS,
    origineClient?: string,
    frein: Frein = new Frein(),
): Promise<string> {
    base = await baseNeuve(nom);
    const b = base;
    http = createServer((req, rep) => {
        void servirSession(req, rep, {
            base: b,
            secretJeton: SECRET,
            origineClient,
            maintenant: () => instant,
            frein,
            proxyDeConfiance: new Set<string>(),
        })
            .then((servie) => {
                if (servie) return;
                rep.writeHead(404, { 'content-type': 'text/plain; charset=utf-8' });
                rep.end('introuvable\n');
            })
            .catch((cause) => {
                rep.writeHead(500, { 'content-type': 'application/json; charset=utf-8' });
                rep.end(JSON.stringify({ refus: 'interne', cause: String(cause) }));
            });
    });
    await new Promise<void>((r) => http!.listen(0, '127.0.0.1', () => r()));
    const a = http!.address();
    return `http://127.0.0.1:${typeof a === 'object' && a ? a.port : 0}`;
}

async function poserVm(p: Pilote, id: string, nom: string, prefixe: string, vuA: number | null) {
    await p.executer('INSERT INTO vm(id, nom, adresse) VALUES(?, ?, ?)', [id, nom, '192.168.3.2']);
    await enroler(p, id, 'empreinte-opaque', prefixe);
    if (vuA !== null) await marquerVu(p, id, vuA);
}

async function attribuer(p: Pilote, vmId: string, email: string): Promise<string> {
    const u = await creerUtilisateur(p, email, 'empreinte-opaque-de-test', MS);
    await p.executer('UPDATE vm SET utilisateur_id = ? WHERE id = ?', [u, vmId]);
    return u;
}

function demander(url: string, jeton?: string, entetes: Record<string, string> = {}) {
    return fetch(`${url}/session`, {
        method: 'POST',
        headers: jeton === undefined ? entetes : { authorization: `Bearer ${jeton}`, ...entetes },
    });
}

async function corpsDe(r: Response): Promise<Record<string, unknown>> {
    return (await r.json()) as Record<string, unknown>;
}

describe(`route POST /session, moteur=${MOTEUR}`, () => {
    it('sans jeton → 401', async () => {
        // 🔴 La rouge : servir sans jeton. Le préfixe d'une VM serait délivré à
        // n'importe qui.
        const url = await servir('rs-401');
        const r = await demander(url);
        expect(r.status).toBe(401);
        expect((await corpsDe(r)).refus).toBe('jeton-absent');
    });

    it('🔴 avec un jeton d’AGENT → 403', async () => {
        // ⚠️ `it()` DISTINCT du précédent : le plan les range dans une seule
        // ligne, mais ce sont deux refus par deux chemins différents.
        // 🔴 La rouge : accepter le type `agent`.
        const url = await servir('rs-403');
        const r = await demander(url, signer('PREFIXEdelaVM', SECRET, MS, undefined, 'agent'));
        expect(r.status).toBe(403);
        expect((await corpsDe(r)).refus).toBe('jeton-agent');
    });

    it('🔴 succès → 200 { vm, nom, prefixe, etat }', async () => {
        // 🔴 La rouge : omettre `prefixe`. C'est LA source que P3 attend —
        // `client/src/prefixe.ts` dit en toutes lettres « P4 branchera la
        // source, et n'aura qu'à écrire dans le coffre ». Sans elle, tout le
        // sous-bloc ne livre rien au navigateur.
        const url = await servir('rs-ok');
        await poserVm(base!, 'v1', 'w1', 'PREFIXEv1', MS);
        const alice = await attribuer(base!, 'v1', 'alice@exemple.test');
        const r = await demander(url, signer(alice, SECRET, MS));
        expect(r.status).toBe(200);
        expect(await corpsDe(r)).toEqual({
            vm: 'v1',
            nom: 'w1',
            prefixe: 'PREFIXEv1',
            etat: 'prete',
        });
    });

    it('🔴 le corps ne porte NI `ice`, NI `adresse`, NI nom de session composé', async () => {
        // 🔴 La rouge : les ajouter (D7). ⚠️ `it()` DISTINCT du précédent.
        //
        // `ice` : la configuration ICE est PAR SESSION — `signaling/ice.ts`
        // compose `${expiration}:${session}` et signe le tout. Une VM ouvre
        // `<préfixe>:bureau` PLUS une session par fenêtre : la route n'en
        // connaîtrait qu'une sur N, et le relais continuerait de servir toutes
        // les autres. Un second chemin de délivrance qui couvre une session sur
        // N n'est pas une simplification, c'est un second endroit à garder
        // synchrone dont on n'a pas le droit de se servir.
        //
        // `bureau` : la constante vit déjà en Rust et en TypeScript
        // (`client/src/shell-page.ts` fait `composer(prefixe, …)`), et la spec
        // §2.6 nomme déjà cette duplication comme un défaut connu. En ajouter
        // une TROISIÈME pour économiser une concaténation au navigateur serait
        // aggraver un défaut qu'on sait nommer.
        const url = await servir('rs-sobre');
        await poserVm(base!, 'v1', 'w1', 'PREFIXEv1', MS);
        const alice = await attribuer(base!, 'v1', 'alice@exemple.test');
        const corps = await corpsDe(await demander(url, signer(alice, SECRET, MS)));
        expect(Object.keys(corps).sort()).toEqual(['etat', 'nom', 'prefixe', 'vm']);
        for (const interdit of ['ice', 'iceServers', 'ice_config', 'adresse', 'session']) {
            expect(corps[interdit]).toBeUndefined();
        }
        // Et le préfixe est NU : jamais `PREFIXEv1:bureau`.
        expect(corps.prefixe).toBe('PREFIXEv1');
        expect(String(corps.prefixe)).not.toContain(':');
    });

    it('🔴 ③a — un utilisateur SANS VM → 409 { motif: aucune-vm }', async () => {
        // 🔴 La rouge : rendre 200 avec une liste vide. Un listing vide n'est
        // PAS un refus : le navigateur écrirait une chaîne vide dans le coffre,
        // `lirePrefixe` retomberait sur `''`, et la page rejoindrait
        // SILENCIEUSEMENT l'espace de noms partagé — la panne muette exacte que
        // la spec §10 nomme.
        const url = await servir('rs-aucune');
        await poserVm(base!, 'v1', 'w1', 'PREFIXEv1', MS);
        await attribuer(base!, 'v1', 'bob@exemple.test');
        const carole = await creerUtilisateur(base!, 'carole@exemple.test', 'e', MS);
        const r = await demander(url, signer(carole, SECRET, MS));
        expect(r.status).toBe(409);
        expect((await corpsDe(r)).motif).toBe('aucune-vm');
    });

    it('🔴 ③b — la réponse arrive SOUS LA BORNE mesurée', async () => {
        // 🔴 La rouge : insérer `await new Promise(r => setTimeout(r, 2000))`
        // dans la route. MESURÉE ATTEIGNABLE — la borne est un ordre de
        // grandeur au-dessus du pire relevé et très en dessous de 2 000 ms.
        //
        // ⚠️ UNE REQUÊTE DE CHAUFFE PRÉCÈDE LA MESURE : la première requête
        // porte l'établissement de connexion, qui n'est pas ce qu'on mesure.
        const url = await servir('rs-borne');
        await poserVm(base!, 'v1', 'w1', 'PREFIXEv1', MS);
        const alice = await attribuer(base!, 'v1', 'alice@exemple.test');
        const jeton = signer(alice, SECRET, MS);

        await demander(url, jeton); // chauffe, non mesurée

        let pire = 0;
        for (let i = 0; i < 10; i += 1) {
            const debut = performance.now();
            const r = await demander(url, jeton);
            const duree = performance.now() - debut;
            expect(r.status).toBe(200);
            pire = Math.max(pire, duree);
        }
        expect(pire).toBeLessThan(BORNE_MS);
    });

    it('🔴 ④a — une VM dont `vu_a` est trop vieux → 503, corps portant `etat: injoignable`', async () => {
        // 🔴 La rouge : masquer derrière un `{motif:'reessayez'}` générique.
        // L'utilisateur doit savoir que SA VM ne répond pas, et non croire à
        // une indisponibilité du service.
        const url = await servir('rs-injoignable', MS + SEUIL_INJOIGNABLE_MS + 1);
        await poserVm(base!, 'v1', 'w1', 'PREFIXEv1', MS);
        const alice = await attribuer(base!, 'v1', 'alice@exemple.test');
        const r = await demander(url, signer(alice, SECRET, MS));
        expect(r.status).toBe(503);
        const corps = await corpsDe(r);
        expect(corps.motif).toBe('agent-injoignable');
        expect(corps.etat).toBe('injoignable');
        // Le préfixe est rendu QUAND MÊME : il est connu et juste, et le
        // navigateur en a besoin pour ne pas rejoindre l'espace partagé.
        expect(corps.prefixe).toBe('PREFIXEv1');
    });

    it('🔴 ④b — le MÊME corps porte `redemarrage: { possible:false, … }`', async () => {
        // 🔴 La rouge : retirer le champ. ⚠️ `it()` DISTINCT de ④a, et c'est
        // exactement le « et dit qu'elle ne sait pas la redémarrer » du
        // critère : `expect` s'arrêterait à la première assertion de ④a.
        //
        // C'est l'AVEU, pas la fonction : le cadrage promet « propose
        // redémarrage », et le backend v1 ne pilote aucun hyperviseur. Le dire
        // vaut mieux que de ne rien dire (D3, « conséquence produit à
        // assumer »).
        const url = await servir('rs-redemarrage', MS + SEUIL_INJOIGNABLE_MS + 1);
        await poserVm(base!, 'v1', 'w1', 'PREFIXEv1', MS);
        const alice = await attribuer(base!, 'v1', 'alice@exemple.test');
        const corps = await corpsDe(await demander(url, signer(alice, SECRET, MS)));
        expect(corps.redemarrage).toEqual({
            possible: false,
            motif: 'non-supporte',
            backend: BACKEND_STATIQUE,
        });
    });

    it('🔴 ④c — la TRANSITION est vue : 200 à vu_a + SEUIL, 503 une ms plus tard', async () => {
        // 🔴 La rouge : figer l'horloge injectée. La borne ne serait plus
        // assiégée des deux côtés, et un seuil jamais franchi ne prouve rien.
        // ⚠️ `it()` DISTINCT : c'est une propriété de BORNE, pas de corps.
        const juste = await servir('rs-borne-prete', MS + SEUIL_INJOIGNABLE_MS);
        await poserVm(base!, 'v1', 'w1', 'PREFIXEv1', MS);
        const alice = await attribuer(base!, 'v1', 'alice@exemple.test');
        expect((await demander(juste, signer(alice, SECRET, MS))).status).toBe(200);

        // Un second service, une milliseconde plus tard.
        await new Promise<void>((r) => http!.close(() => r()));
        http = undefined;
        await base!.fermer();
        base = undefined;
        const apres = await servir('rs-borne-injoignable', MS + SEUIL_INJOIGNABLE_MS + 1);
        await poserVm(base!, 'v1', 'w1', 'PREFIXEv1', MS);
        const alice2 = await attribuer(base!, 'v1', 'alice@exemple.test');
        expect((await demander(apres, signer(alice2, SECRET, MS))).status).toBe(503);
    });

    it('🔴 la requête préalable `OPTIONS` est servie', async () => {
        // 🔴 Même défaut de plan qu'à la tâche 9, relevé et non recopié :
        // `POST /session` porte `Authorization`, donc la requête est NON
        // SIMPLE, donc le navigateur émet d'abord un `OPTIONS`. Un 404 le
        // ferait abandonner avant d'envoyer la vraie requête.
        const url = await servir('rs-options', MS, ORIGINE);
        const r = await fetch(`${url}/session`, {
            method: 'OPTIONS',
            headers: { origin: ORIGINE },
        });
        expect(r.status).toBe(204);
        expect(r.headers.get('access-control-allow-origin')).toBe(ORIGINE);
        expect(r.headers.get('access-control-allow-headers')).toContain('authorization');
    });

    it('🔴 le budget « toute requête » freine `POST /session` après trop de requêtes de la même adresse', async () => {
        // 🔴 La rouge : ne jamais consulter `BUDGET_REQUETES`. Sans jeton,
        // chaque requête rendrait 401 indéfiniment — cette route n'a aucune
        // notion d'échec (voir `securite/frein.ts`).
        const url = await servir('rs-frein-requetes');
        let dernier: Response | undefined;
        for (let i = 0; i < REQUETES_MAX_ADRESSE + 1; i++) {
            dernier = await demander(url);
        }
        expect(dernier!.status).toBe(429);
        expect((await corpsDe(dernier!)).refus).toBe('trop-de-requetes');
        const retry = dernier!.headers.get('retry-after');
        expect(retry).not.toBeNull();
        expect(Number(retry)).toBeGreaterThan(0);
    });
});
