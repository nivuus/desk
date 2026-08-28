// `GET /vm` et `POST /vm/:id/:operation`, éprouvées À TRAVERS un serveur HTTP
// réel, dans le style de `routes-auth.test.ts`.
//
// 🔴 LE CRITÈRE ① EST DANS CE FICHIER : `POST /vm/<id>/instantane` rend 501 et
// un corps typé, jamais un silence ni un 200.
//
// ⚠️ QUINZE TESTS ET NON LES TREIZE DU PLAN, annoncé avant d'être lu. Les deux
// de plus : la requête préalable `OPTIONS` (sans laquelle aucune des deux
// routes n'est atteignable depuis un navigateur — voir `routes-vm.ts`), et le
// refus de méthode sur `/vm`.

import { afterEach, describe, expect, it } from 'vitest';
import { createServer, type Server } from 'node:http';
import { baseNeuve, MOTEUR } from '../base/harnais';
import type { Pilote } from '../base/pilote';
import { enroler, marquerVu } from '../depot/agent';
import { creerUtilisateur } from '../depot/utilisateur';
import { ouvrirSession } from '../depot/session';
import { signer } from '../identite/jeton';
import { BACKEND_STATIQUE } from '../orchestration/refus';
import { Frein, REQUETES_MAX_ADRESSE } from '../securite/frein';
import { servirVm } from './routes-vm';

const SECRET = 'un-secret-de-plateforme-de-quarante-octets';
const ORIGINE = 'http://127.0.0.1:5173';
const MS = 1_787_136_773_742;

let base: Pilote | undefined;
let http: Server | undefined;

afterEach(async () => {
    if (http) await new Promise<void>((r) => http!.close(() => r()));
    http = undefined;
    await base?.fermer();
    base = undefined;
});

/// Monte un serveur qui ne porte QUE cette route, plus le 404 générique de
/// `serveur.ts` reproduit mot pour mot : c'est ainsi qu'un `false` rendu par
/// `servirVm` devient observable.
///
/// ⚠️ `frein` EST UN PARAMÈTRE, PAS UNE VALEUR FIGÉE DANS LA FERMETURE : sans
/// lui, chaque test recevrait le MÊME défaut par la même expression, ce qui
/// serait sans conséquence ICI (chaque test appelle `servir` une fois), mais
/// c'est la même construction que `routes-auth.test.ts` emploie pour ses
/// propres tests de frein — la garder ici évite qu'un futur test de ce
/// fichier n'ait à la réinventer.
async function servir(
    nom: string,
    origineClient?: string,
    frein: Frein = new Frein(),
): Promise<string> {
    base = await baseNeuve(nom);
    const b = base;
    http = createServer((req, rep) => {
        void servirVm(req, rep, {
            base: b,
            secretJeton: SECRET,
            origineClient,
            maintenant: () => MS,
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

async function poserVm(p: Pilote, id: string, nom: string, prefixe: string): Promise<void> {
    await p.executer('INSERT INTO vm(id, nom, adresse) VALUES(?, ?, ?)', [id, nom, '192.168.3.2']);
    await enroler(p, id, 'empreinte-opaque', prefixe);
    await marquerVu(p, id, MS);
}

async function attribuer(p: Pilote, vmId: string, email: string): Promise<string> {
    const u = await creerUtilisateur(p, email, 'empreinte-opaque-de-test', MS);
    await p.executer('UPDATE vm SET utilisateur_id = ? WHERE id = ?', [u, vmId]);
    return u;
}

function jetonDe(sujet: string): string {
    return signer(sujet, SECRET, MS);
}

function avec(jeton?: string, autres: Record<string, string> = {}): Record<string, string> {
    return jeton === undefined ? autres : { authorization: `Bearer ${jeton}`, ...autres };
}

interface CorpsVm { vms?: Array<Record<string, unknown>>; refus?: string; motif?: string }
async function corpsDe(r: Response): Promise<CorpsVm & Record<string, unknown>> {
    return (await r.json()) as CorpsVm & Record<string, unknown>;
}

describe(`routes /vm, moteur=${MOTEUR}`, () => {
    it('`GET /vm` SANS en-tête → 401 jeton-absent', async () => {
        // 🔴 La rouge : servir sans jeton. L'inventaire deviendrait public.
        const url = await servir('rvm-401');
        const r = await fetch(`${url}/vm`);
        expect(r.status).toBe(401);
        expect((await corpsDe(r)).refus).toBe('jeton-absent');
    });

    it('🔴 `GET /vm` avec un jeton d’AGENT → 403 jeton-agent', async () => {
        // 🔴 La rouge : accepter le type `agent`. Un agent verrait l'inventaire
        // d'un humain — la seconde des deux confusions qu'`identite/jeton.ts`
        // énumère.
        const url = await servir('rvm-403');
        const jetonAgent = signer('PREFIXEdelaVM', SECRET, MS, undefined, 'agent');
        const r = await fetch(`${url}/vm`, { headers: avec(jetonAgent) });
        expect(r.status).toBe(403);
        expect((await corpsDe(r)).refus).toBe('jeton-agent');
    });

    it('🔴 `GET /vm` ne rend QUE les VMs du demandeur', async () => {
        // 🔴 La rouge : rendre l'inventaire entier. ⚠️ CE TEST EST DISTINCT DE
        // CELUI DE `vmsDe` : celui-là éprouve la fonction pure, celui-ci
        // éprouve qu'elle est bien APPELÉE. Une route qui n'appellerait pas le
        // filtre laisserait `selection.test.ts` parfaitement vert.
        const url = await servir('rvm-filtre');
        await poserVm(base!, 'v1', 'w1', 'PREFIXEv1');
        await poserVm(base!, 'v2', 'w2', 'PREFIXEv2');
        await poserVm(base!, 'v3', 'w3', 'PREFIXEv3'); // au vivier, à personne
        const alice = await attribuer(base!, 'v1', 'alice@exemple.test');
        await attribuer(base!, 'v2', 'bob@exemple.test');

        const r = await fetch(`${url}/vm`, { headers: avec(jetonDe(alice)) });
        expect(r.status).toBe(200);
        const corps = await corpsDe(r);
        expect(corps.vms!.map((v) => v.id)).toEqual(['v1']);
    });

    it('`GET /vm` rend `etat`, `prefixe` et `sessions_ouvertes`', async () => {
        // 🔴 La rouge : omettre un champ. Le hub n'aurait rien à afficher.
        const url = await servir('rvm-champs');
        await poserVm(base!, 'v1', 'w1', 'PREFIXEv1');
        const alice = await attribuer(base!, 'v1', 'alice@exemple.test');
        await ouvrirSession(base!, 'PREFIXEv1:bureau', MS, alice, 'v1');

        const r = await fetch(`${url}/vm`, { headers: avec(jetonDe(alice)) });
        const [vm] = (await corpsDe(r)).vms!;
        expect(vm.id).toBe('v1');
        expect(vm.nom).toBe('w1');
        expect(vm.etat).toBe('prete');
        expect(vm.prefixe).toBe('PREFIXEv1');
        expect(vm.sessions_ouvertes).toBe(1);
    });

    it('🔴 `GET /vm` ne rend PAS `adresse`', async () => {
        // 🔴 La rouge : l'inclure. L'adresse d'une VM est de la topologie
        // interne dont le navigateur n'a AUCUN usage — il parle au signaling,
        // jamais à la VM. La rendre l'exposerait à tout utilisateur
        // authentifié sans qu'aucun besoin ne l'exige (D7).
        const url = await servir('rvm-adresse');
        await poserVm(base!, 'v1', 'w1', 'PREFIXEv1');
        const alice = await attribuer(base!, 'v1', 'alice@exemple.test');
        const r = await fetch(`${url}/vm`, { headers: avec(jetonDe(alice)) });
        const [vm] = (await corpsDe(r)).vms!;
        expect(Object.keys(vm).sort()).toEqual(
            ['etat', 'id', 'nom', 'prefixe', 'sessions_ouvertes'].sort(),
        );
        expect(vm.adresse).toBeUndefined();
    });

    it('🔴 `POST /vm/<id>/instantane` → 501 et un corps TYPÉ — le critère ①', async () => {
        // 🔴 La rouge : rendre 200, ou un silence. C'est LITTÉRALEMENT le
        // critère ① de la spec §4 « P4 ».
        const url = await servir('rvm-instantane');
        await poserVm(base!, 'v1', 'w1', 'PREFIXEv1');
        const alice = await attribuer(base!, 'v1', 'alice@exemple.test');
        const r = await fetch(`${url}/vm/v1/instantane`, {
            method: 'POST',
            headers: avec(jetonDe(alice)),
        });
        expect(r.status).toBe(501);
        expect(await corpsDe(r)).toEqual({
            motif: 'non-supporte',
            operation: 'instantane',
            backend: BACKEND_STATIQUE,
        });
    });

    it('`POST /vm/<id>/demarrer` → 501', async () => {
        const url = await servir('rvm-demarrer');
        await poserVm(base!, 'v1', 'w1', 'PREFIXEv1');
        const alice = await attribuer(base!, 'v1', 'alice@exemple.test');
        const r = await fetch(`${url}/vm/v1/demarrer`, {
            method: 'POST',
            headers: avec(jetonDe(alice)),
        });
        expect(r.status).toBe(501);
        expect((await corpsDe(r)).operation).toBe('demarrer');
    });

    it('`POST /vm/<id>/arreter` → 501', async () => {
        const url = await servir('rvm-arreter');
        await poserVm(base!, 'v1', 'w1', 'PREFIXEv1');
        const alice = await attribuer(base!, 'v1', 'alice@exemple.test');
        const r = await fetch(`${url}/vm/v1/arreter`, {
            method: 'POST',
            headers: avec(jetonDe(alice)),
        });
        expect(r.status).toBe(501);
        expect((await corpsDe(r)).operation).toBe('arreter');
    });

    it('🔴 `POST /vm/<id>/attribuer` → 404 GÉNÉRIQUE, jamais 501 ni 200', async () => {
        // 🔴 La rouge : ajouter `attribuer` à `OPERATIONS_HTTP`. L'attribution
        // deviendrait atteignable par tout utilisateur authentifié — il
        // n'existe aucun rôle d'administration dans ce service, donc la route
        // ne pourrait rien exiger de plus qu'un jeton ordinaire (D8).
        //
        // ⚠️ Le corps est celui du 404 de `serveur.ts`, `introuvable\\n` : la
        // route ne SERT PAS ce chemin, elle ne le refuse pas.
        const url = await servir('rvm-attribuer');
        await poserVm(base!, 'v1', 'w1', 'PREFIXEv1');
        const alice = await attribuer(base!, 'v1', 'alice@exemple.test');
        const r = await fetch(`${url}/vm/v1/attribuer`, {
            method: 'POST',
            headers: avec(jetonDe(alice)),
        });
        expect(r.status).toBe(404);
        expect(await r.text()).toBe('introuvable\n');
    });

    it('🔴 un verbe INVENTÉ → 404 générique, jamais un 501 qui mentirait', async () => {
        // 🔴 La rouge : valider APRÈS avoir servi. Un verbe inconnu recevrait
        // un 501, qui affirmerait que l'opération EXISTE et n'est pas
        // supportée — alors qu'elle n'existe pas. La liste est BLANCHE.
        const url = await servir('rvm-invente');
        await poserVm(base!, 'v1', 'w1', 'PREFIXEv1');
        const alice = await attribuer(base!, 'v1', 'alice@exemple.test');
        for (const verbe of ['exploser', 'lister', 'etat', '']) {
            const r = await fetch(`${url}/vm/v1/${verbe}`, {
                method: 'POST',
                headers: avec(jetonDe(alice)),
            });
            expect(r.status).toBe(404);
        }
    });

    it('🔴 la VM d’AUTRUI → 404 `vm-inconnue`', async () => {
        // 🔴 La rouge : distinguer « inconnue » de « à quelqu'un d'autre ».
        const url = await servir('rvm-autrui');
        await poserVm(base!, 'v1', 'w1', 'PREFIXEv1');
        await poserVm(base!, 'v2', 'w2', 'PREFIXEv2');
        const alice = await attribuer(base!, 'v1', 'alice@exemple.test');
        await attribuer(base!, 'v2', 'bob@exemple.test');
        const r = await fetch(`${url}/vm/v2/instantane`, {
            method: 'POST',
            headers: avec(jetonDe(alice)),
        });
        expect(r.status).toBe(404);
        expect((await corpsDe(r)).motif).toBe('vm-inconnue');
    });

    it('🔴 …et son corps est IDENTIQUE, caractère pour caractère, à celui d’une VM inexistante', async () => {
        // ⚠️ `it()` DISTINCT du précédent, et c'est le cœur : `expect`
        // s'arrêterait à la première assertion, si bien que l'égalité des deux
        // corps — l'oracle même qu'on ferme — ne serait éprouvée par rien.
        // C'est la forme exacte du critère ② de P3.
        const url = await servir('rvm-autrui-identique');
        await poserVm(base!, 'v1', 'w1', 'PREFIXEv1');
        await poserVm(base!, 'v2', 'w2', 'PREFIXEv2');
        const alice = await attribuer(base!, 'v1', 'alice@exemple.test');
        await attribuer(base!, 'v2', 'bob@exemple.test');
        const j = avec(jetonDe(alice));

        const autrui = await fetch(`${url}/vm/v2/instantane`, { method: 'POST', headers: j });
        const inexistante = await fetch(`${url}/vm/v-jamais-creee/instantane`, {
            method: 'POST',
            headers: j,
        });
        expect(autrui.status).toBe(inexistante.status);
        // CARACTÈRE POUR CARACTÈRE, pas champ par champ : un espacement ou un
        // ordre de clés différent serait déjà un signal.
        expect(await autrui.text()).toBe(await inexistante.text());
    });

    it('les en-têtes CORS sont posés quand `origineClient` est configurée', async () => {
        // 🔴 La rouge : les omettre. Le navigateur refuserait de lire la
        // réponse SANS qu'aucun test Node ne le voie — `cors.ts` le dit de
        // lui-même, et c'est pourquoi cette assertion existe.
        const url = await servir('rvm-cors', ORIGINE);
        await poserVm(base!, 'v1', 'w1', 'PREFIXEv1');
        const alice = await attribuer(base!, 'v1', 'alice@exemple.test');
        const r = await fetch(`${url}/vm`, { headers: avec(jetonDe(alice), { origin: ORIGINE }) });
        expect(r.headers.get('access-control-allow-origin')).toBe(ORIGINE);
        expect(r.headers.get('vary')).toBe('Origin');
        // Et sur un REFUS aussi : une 401 que le navigateur ne peut pas lire
        // s'affiche comme une panne réseau, pas comme une invitation à se
        // reconnecter.
        const refus = await fetch(`${url}/vm`, { headers: { origin: ORIGINE } });
        expect(refus.status).toBe(401);
        expect(refus.headers.get('access-control-allow-origin')).toBe(ORIGINE);
    });

    it('🔴 la requête préalable `OPTIONS` est servie — sinon la route est INATTEIGNABLE', async () => {
        // 🔴 DÉFAUT DU PLAN, RELEVÉ ET NON RECOPIÉ : la tâche 9 ne prescrit
        // aucun traitement d'`OPTIONS`. Or `GET /vm` porte `Authorization`,
        // ce qui rend la requête NON SIMPLE : le navigateur émet d'abord une
        // requête préalable, à laquelle un 404 oppose un refus — et la vraie
        // requête n'est jamais envoyée. La route serait donc inatteignable
        // depuis un navigateur, exactement comme sans l'en-tête
        // `Access-Control-Allow-Headers: authorization` de la tâche 8.
        // 🔴 La rouge : ne pas traiter `OPTIONS`.
        const url = await servir('rvm-options', ORIGINE);
        for (const chemin of ['/vm', '/vm/v1/instantane']) {
            const r = await fetch(`${url}${chemin}`, {
                method: 'OPTIONS',
                headers: { origin: ORIGINE },
            });
            expect(r.status).toBe(204);
            expect(r.headers.get('access-control-allow-origin')).toBe(ORIGINE);
            expect(r.headers.get('access-control-allow-headers')).toContain('authorization');
        }
    });

    it('une méthode autre que GET sur `/vm` → 405, jamais 404', async () => {
        // Le chemin EXISTE ; c'est la méthode qui ne convient pas. Un 404
        // ferait chercher une route absente. Même choix que `routes-auth.ts`.
        const url = await servir('rvm-methode');
        const alice = await creerUtilisateur(base!, 'alice@exemple.test', 'e', MS);
        const r = await fetch(`${url}/vm`, { method: 'POST', headers: avec(jetonDe(alice)) });
        expect(r.status).toBe(405);
        expect((await corpsDe(r)).refus).toBe('methode');
    });

    it('🔴 le budget « toute requête » freine `GET /vm` après trop de requêtes de la même adresse', async () => {
        // 🔴 La rouge : ne jamais consulter `BUDGET_REQUETES`. Sans jeton,
        // chaque requête rendrait 401 indéfiniment — `GET /vm` n'a aucune
        // notion d'échec, et c'est exactement pourquoi ce budget existe
        // (voir `securite/frein.ts`).
        const url = await servir('rvm-frein-requetes');
        let dernier: Response | undefined;
        for (let i = 0; i < REQUETES_MAX_ADRESSE + 1; i++) {
            dernier = await fetch(`${url}/vm`);
        }
        expect(dernier!.status).toBe(429);
        expect((await corpsDe(dernier!)).refus).toBe('trop-de-requetes');
        const retry = dernier!.headers.get('retry-after');
        expect(retry).not.toBeNull();
        expect(Number(retry)).toBeGreaterThan(0);
    });
});
