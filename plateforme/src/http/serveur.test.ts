// Trois propriétés, dont deux sont des critères de recette de P1.
//
// ⚠️ Le premier test ne peut PAS prouver l'inaccessibilité depuis une autre
// interface : sur une machine de développement, `127.0.0.1` et l'adresse de
// l'interface sont toutes deux locales, et une sonde depuis l'extérieur
// exigerait une machine hors du réseau. Il prouve que le service écoute sur
// l'adresse NOMMÉE, pas qu'il n'écoute nulle part ailleurs. La garantie réelle
// du critère ④ vient de `config.ts` — pas de défaut, donc pas d'écoute non
// nommée — et son test est `config.test.ts`, vu rouge à la tâche 1.

import { afterEach, describe, expect, it } from 'vitest';
import { WebSocket } from 'ws';
import { baseNeuve } from '../base/harnais';
import type { Pilote } from '../base/pilote';
import type { Config } from '../config';
import { signer } from '../identite/jeton';
import { demarrerServeur, type ServicePlateforme } from './serveur';
import type { Pilote as TypePilote } from '../base/pilote';

// Un secret de test EXPLICITE, jamais `''` : `lireConfig` refuse la chaîne
// vide, et un littéral `Config` construit à la main doit porter une valeur
// qu'un service accepterait réellement.
const SECRET = 'un-secret-de-plateforme-de-quarante-octets';

/// Le préfixe de la VM simulée, de la vraie longueur qu'`agents/prefixe.ts`
/// produit. La garde exige que le sujet du jeton d'agent préfixe la session.
const P = 'RhH1x2QmTz9kLpVbNc7dAw';

const CONFIG: Config = {
    hote: '127.0.0.1',
    port: 0,
    base: 'sqlite',
    urlBase: ':memory:',
    secretJeton: SECRET,
    // Aucun proxy declare : voir `config.ts`, l'ensemble vide est le defaut
    // et signifie « ne croire l'adresse annoncee par personne ».
    proxyDeConfiance: new Set(),
};

let service: ServicePlateforme | undefined;
let base: Pilote | undefined;

afterEach(async () => {
    await service?.close();
    service = undefined;
    await base?.fermer();
    base = undefined;
});

/// Le serveur exige une base : elle est REQUISE, pas optionnelle (voir
/// `demarrerServeur`). Ces trois tests-ci ne mesurent pas la trace — c'est
/// `signaling/trace.test.ts` qui la mesure —, ils ont seulement besoin d'une
/// base vivante pour démarrer.
async function servir(nom: string): Promise<ServicePlateforme> {
    base = await baseNeuve(nom);
    return demarrerServeur(CONFIG, base);
}

/// Ouvre un socket et rend son issue : `ouvert` s'il a atteint `open`, sinon
/// `ferme`. Une borne de temps évite qu'un test pende indéfiniment.
function tenter(url: string, borneMs = 3000): Promise<'ouvert' | 'ferme'> {
    return new Promise((resolve, reject) => {
        const w = new WebSocket(url);
        const minuteur = setTimeout(() => {
            w.terminate();
            reject(new Error(`aucune issue pour ${url} en ${borneMs} ms`));
        }, borneMs);
        const finir = (issue: 'ouvert' | 'ferme') => {
            clearTimeout(minuteur);
            w.removeAllListeners();
            w.terminate();
            resolve(issue);
        };
        w.on('open', () => finir('ouvert'));
        w.on('error', () => finir('ferme'));
        w.on('close', () => finir('ferme'));
    });
}

describe('demarrerServeur', () => {
    it("n'écoute QUE sur l'adresse nommée", async () => {
        service = await servir('http-adresse');
        expect(service.port).toBeGreaterThan(0);
        await expect(tenter(`ws://127.0.0.1:${service.port}/`)).resolves.toBe('ouvert');
    });

    it('refuse la montée WebSocket sur un chemin inconnu', async () => {
        service = await servir('http-chemin');
        await expect(tenter(`ws://127.0.0.1:${service.port}/inconnu`)).resolves.toBe('ferme');
    });

    it('🔴 accepte la montée WebSocket sur /agent — la seconde branche', async () => {
        // 🔴 La rouge : ne pas ajouter la branche. Le `404` écrit à la main
        // pour tout chemin autre que `/` la ferme, et c'est exactement ce que
        // le test « refuse la montée sur un chemin inconnu » éprouve.
        service = await servir('http-agent');
        await expect(tenter(`ws://127.0.0.1:${service.port}/agent`)).resolves.toBe('ouvert');
    });

    it('🔴 sert TOUJOURS le chemin racine — la branche est ÉTENDUE, pas remplacée', async () => {
        // 🔴 La rouge : remplacer la comparaison au lieu de l'étendre. Le
        // relais entier deviendrait injoignable, et le service serait vivant
        // sans servir personne — panne muette de la classe que ce dépôt
        // combat. Le premier test du fichier le voit aussi ; celui-ci le dit.
        service = await servir('http-racine-toujours');
        await expect(tenter(`ws://127.0.0.1:${service.port}/`)).resolves.toBe('ouvert');
    });

    it('🔴 refuse TOUJOURS un chemin inconnu — /agent n’ouvre pas le service', async () => {
        // 🔴 La rouge : remplacer `if (chemin !== '/')` par une comparaison à
        // une liste noire (`if (chemin === '/inconnu')`), ce qui rendrait le
        // service ouvert à TOUT chemin. Le test l. 79 existe déjà et doit
        // rester vert ; celui-ci ajoute la borne d'à côté — un chemin qui
        // COMMENCE par `/agent` sans l'être n'est pas `/agent`.
        service = await servir('http-inconnu-encore');
        await expect(tenter(`ws://127.0.0.1:${service.port}/inconnu`)).resolves.toBe('ferme');
        await expect(tenter(`ws://127.0.0.1:${service.port}/agentaire`)).resolves.toBe('ferme');
    });

    it('sert le relais de signaling sur le chemin racine, poignée de main comprise', async () => {
        // Un pair 'agent' et un pair 'client' sur '/' : l'offre du client
        // parvient à l'agent — exactement ce que `server.test.ts` éprouve déjà,
        // rejoué ici à travers le serveur HTTP pour prouver que le passage par
        // l'upgrade ne change rien.
        service = await servir('http-racine');
        const url = `ws://127.0.0.1:${service.port}/`;

        const agent = new WebSocket(url);
        await new Promise((r) => agent.once('open', r));
        // 🔴 Le rôle `agent` exige lui aussi son jeton depuis P3 : la fenêtre
        // anonyme de E2 est fermée. Le jeton est de TYPE `agent`, et son sujet
        // est le PRÉFIXE que la session doit porter.
        agent.send(JSON.stringify({
            role: 'agent',
            session: `${P}:racine-1`,
            jeton: signer(P, SECRET, Date.now(), undefined, 'agent'),
        }));

        const client = new WebSocket(url);
        await new Promise((r) => client.once('open', r));
        // Le rôle `client` exige désormais un jeton d'accès (sous-bloc P2) :
        // sans lui la garde refuse et ferme le socket. Le jeton est signé avec
        // le secret que porte `CONFIG`, celui-là même dont le service se sert.
        client.send(JSON.stringify({
            role: 'client',
            session: `${P}:racine-1`,
            jeton: signer('u-racine', SECRET, Date.now()),
        }));

        const offreRecue = new Promise<string>((resolve) => {
            agent.on('message', (brut) => {
                const m = JSON.parse(brut.toString());
                if (m.type === 'offer') resolve(m.sdp);
            });
        });
        // Laisse le client se déclarer avant d'émettre son offre.
        await new Promise((r) => setTimeout(r, 50));
        client.send(JSON.stringify({ type: 'offer', sdp: 'v=0 racine' }));

        await expect(offreRecue).resolves.toBe('v=0 racine');
        agent.terminate();
        client.terminate();
    });
});

describe('le chaînage des quatre routeurs', () => {
    it('🔴 les QUATRE chemins répondent, et `/inconnu` rend le 404 MOT POUR MOT', async () => {
        // 🔴 La rouge : retirer un maillon de la chaîne. Sa route rend alors
        // 404 — et c'est la panne la plus discrète possible, puisque le service
        // répond, écoute, et sert les deux autres.
        service = await servir('http-chaine');
        const url = `http://127.0.0.1:${service.port}`;

        // `/auth/connexion` répond TOUJOURS : P2 n'est pas cassé par P4. Sans
        // corps il rend 400 `{refus:'forme'}`, ce qui prouve qu'il a été SERVI
        // — un 404 dirait qu'il ne l'a pas été.
        const auth = await fetch(`${url}/auth/connexion`, {
            method: 'POST',
            headers: { 'content-type': 'application/json' },
            body: '{}',
        });
        expect(auth.status).toBe(400);

        // `/vm` répond : sans jeton, 401 — pas 404.
        const vm = await fetch(`${url}/vm`);
        expect(vm.status).toBe(401);

        // `/session` répond : sans jeton, 401 — pas 404.
        const session = await fetch(`${url}/session`, { method: 'POST' });
        expect(session.status).toBe(401);

        // 🔴 `/applications` ET `/application/:id/lancer` RÉPONDENT : sans
        // jeton, 401 — pas 404. C'est LA SEULE LIGNE qui prouve que le
        // quatrième maillon est réellement chaîné dans `demarrerServeur`, et
        // c'est le même argument que celui du canal `/agent` : sans elle, le
        // pair verrait un service qui répond, écoute, sert les trois autres, et
        // rend 404 sur celui-ci.
        //
        // ⚠️ LE CORPS EST LU, PAS SEULEMENT LE CODE. Un 401 `{refus:...}` ne
        // peut venir que de la route ; un 404 générique porte `introuvable\n`.
        const liste = await fetch(`${url}/applications?vm=v-1`);
        expect([liste.status, await liste.json()]).toEqual([401, { refus: 'jeton-absent' }]);

        const lancer = await fetch(`${url}/application/a-1/lancer`, { method: 'POST' });
        expect([lancer.status, await lancer.json()]).toEqual([401, { refus: 'jeton-absent' }]);

        // Et le 404 de P1 est intact, CARACTÈRE POUR CARACTÈRE.
        const inconnu = await fetch(`${url}/inconnu`);
        expect(inconnu.status).toBe(404);
        expect(await inconnu.text()).toBe('introuvable\n');
        expect(inconnu.headers.get('content-type')).toBe('text/plain; charset=utf-8');
    });

    it('🔴 une route qui REJETTE rend 500 { refus: interne }, et le processus SURVIT', async () => {
        // 🔴 La rouge : retirer le `.catch`. Une promesse rejetée dans un
        // gestionnaire d'évènement Node ABAT TOUT LE PROCESSUS — c'est le mode
        // de défaillance que `serveur.ts` documente déjà, et le chaînage de P4
        // ajoute deux routeurs qui touchent la base, donc deux sources neuves
        // de rejet.
        //
        // La base est SABOTÉE : toute lecture lève. `GET /vm` avec un jeton
        // valide atteint alors `orchestrateur.lister()` et rejette.
        const sabotee: TypePilote = {
            async interroger<T>(): Promise<T[]> {
                throw new Error('base injoignable');
            },
            async executer() {
                throw new Error('base injoignable');
            },
            async transaction<T>(corps: (p: TypePilote) => Promise<T>): Promise<T> {
                return corps(sabotee);
            },
            async fermer() {},
        };
        service = await demarrerServeur(CONFIG, sabotee);
        const url = `http://127.0.0.1:${service.port}`;
        const r = await fetch(`${url}/vm`, {
            headers: { authorization: `Bearer ${signer('u-ada', SECRET, Date.now())}` },
        });
        expect(r.status).toBe(500);
        expect(await r.json()).toEqual({ refus: 'interne' });

        // 🔴 LE PROCESSUS SURVIT : la requête suivante est servie. Sans cette
        // seconde requête, un processus abattu se lirait exactement comme un
        // processus sain — le test aurait déjà rendu son verdict.
        const apres = await fetch(`${url}/inconnu`);
        expect(apres.status).toBe(404);
    });

    it('🔴 les DEUX montées WebSocket sont INCHANGÉES', async () => {
        // 🔴 La rouge : toucher au routage de l'`upgrade`. C'est hors sujet de
        // cette tâche, et ce test le fige — le chaînage HTTP et le routage
        // WebSocket vivent dans la même fonction, donc à portée de main.
        service = await servir('http-chaine-ws');
        await expect(tenter(`ws://127.0.0.1:${service.port}/`)).resolves.toBe('ouvert');
        await expect(tenter(`ws://127.0.0.1:${service.port}/agent`)).resolves.toBe('ouvert');
        await expect(tenter(`ws://127.0.0.1:${service.port}/vm`)).resolves.toBe('ferme');
    });
});
