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
import { demarrerServeur, TRAME_MAX_OCTETS, type ServicePlateforme } from './serveur';
import type { Pilote as TypePilote } from '../base/pilote';
import { mkdtempSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

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
    repertoireIcones: join(mkdtempSync(join(tmpdir(), 'g2-icones-')), 'icones'),
    repertoireTeleversements: join(mkdtempSync(join(tmpdir(), 'g3-tranches-')), 'televersements'),
    auth: 'pomerium',
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
async function servir(nom: string, config: Config = CONFIG): Promise<ServicePlateforme> {
    base = await baseNeuve(nom);
    return demarrerServeur(config, base);
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
        await expect(tenter(`ws://127.0.0.1:${service.port}/signal`)).resolves.toBe('ouvert');
    });

    it('refuse la montée WebSocket sur un chemin inconnu', async () => {
        service = await servir('http-chemin');
        await expect(tenter(`ws://127.0.0.1:${service.port}/inconnu`)).resolves.toBe('ferme');
    });

    it('🔴 accepte la montée WebSocket sur /agent — la seconde branche', async () => {
        // 🔴 La rouge : ne pas ajouter la branche. Le `404` écrit à la main
        // pour tout chemin autre que `/signal` la ferme, et c'est exactement
        // ce que le test « refuse la montée sur un chemin inconnu » éprouve.
        service = await servir('http-agent');
        await expect(tenter(`ws://127.0.0.1:${service.port}/agent`)).resolves.toBe('ouvert');
    });

    // 🔴 LE RELAIS A DÉMÉNAGÉ DE LA RACINE VERS `/signal` LE 21 AOÛT 2026 —
    // POUR LE PROXY POMERIUM, PAS LE GOÛT. Voir l'en-tête de `serveur.ts`.
    // Les trois tests qui suivent SONT le déplacement : sans le premier, la
    // racine pourrait rester ouverte (Pomerium croirait garder le relais, et
    // le relais répondrait à côté de sa garde) ; le second est le témoin qui
    // rend le premier interprétable ; le troisième est le témoin que /agent
    // n'a pas bougé.
    it('🔴 accepte la montée WebSocket sur /signal — le relais a déménagé', async () => {
        service = await servir('http-signal');
        await expect(tenter(`ws://127.0.0.1:${service.port}/signal`)).resolves.toBe('ouvert');
    });

    it('🔴 REFUSE désormais la montée sur la RACINE', async () => {
        service = await servir('http-racine-fermee');
        await expect(tenter(`ws://127.0.0.1:${service.port}/`)).resolves.toBe('ferme');
    });

    it('/agent est INCHANGÉ', async () => {
        service = await servir('http-agent-inchange');
        await expect(tenter(`ws://127.0.0.1:${service.port}/agent`)).resolves.toBe('ouvert');
    });

    it('🔴 refuse TOUJOURS un chemin inconnu — /agent n’ouvre pas le service', async () => {
        // 🔴 La rouge : remplacer `if (chemin !== CHEMIN_SIGNAL)` par une
        // comparaison à une liste noire (`if (chemin === '/inconnu')`), ce qui
        // rendrait le service ouvert à TOUT chemin. Le test l. 79 existe déjà
        // et doit rester vert ; celui-ci ajoute la borne d'à côté — un chemin
        // qui COMMENCE par `/agent` sans l'être n'est pas `/agent`, et un
        // chemin qui COMMENCE par `/signal` sans l'être n'est pas `/signal`
        // (revue de la tâche 5, constat I2 : la comparaison EXACTE de
        // `/signal` n'était gardée par AUCUN test — une mutation en
        // `startsWith` passait les 16 tests du fichier).
        service = await servir('http-inconnu-encore');
        await expect(tenter(`ws://127.0.0.1:${service.port}/inconnu`)).resolves.toBe('ferme');
        await expect(tenter(`ws://127.0.0.1:${service.port}/agentaire`)).resolves.toBe('ferme');
        await expect(tenter(`ws://127.0.0.1:${service.port}/signalement`)).resolves.toBe('ferme');
    });

    it('sert le relais de signaling sur /signal, poignée de main comprise', async () => {
        // Un pair 'agent' et un pair 'client' sur '/signal' : l'offre du
        // client parvient à l'agent — exactement ce que `server.test.ts`
        // éprouve déjà, rejoué ici à travers le serveur HTTP pour prouver que
        // le passage par l'upgrade ne change rien. ⚠️ CE TEST OUVRAIT DEUX
        // SOCKETS SUR '/' AVANT LE 21 AOÛT 2026 : corrigé avec le déplacement
        // du relais, sans quoi il aurait rougi sans qu'aucune assertion ne le
        // dise.
        service = await servir('http-signal-poignee');
        const url = `ws://127.0.0.1:${service.port}/signal`;

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
        // ⚠️ `auth: 'motdepasse'` LOCAL (tâche 3) : ce test éprouve que
        // `/auth/connexion` est bien CHAÎNÉ dans `demarrerServeur` — une
        // propriété de P2/P4, distincte du mode d'authentification. En mode
        // `pomerium` (le défaut de `CONFIG`), cette route N'EXISTE PLUS DU
        // TOUT (voir `routes-auth.ts`), et l'assertion `400` ci-dessous
        // deviendrait `404` pour une raison hors du champ de ce test.
        service = await servir('http-chaine', { ...CONFIG, auth: 'motdepasse' });
        const url = `http://127.0.0.1:${service.port}`;

        // `/auth/connexion` répond TOUJOURS EN MODE MOTDEPASSE : P2 n'est pas
        // cassé par P4. Sans corps il rend 400 `{refus:'forme'}`, ce qui
        // prouve qu'il a été SERVI — un 404 dirait qu'il ne l'a pas été.
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

    it('🔴 les DEUX montées WebSocket sont INCHANGÉES PAR LE CHAÎNAGE HTTP', async () => {
        // 🔴 La rouge : toucher au routage de l'`upgrade` DEPUIS CE CHAÎNAGE.
        // C'est hors sujet de cette tâche, et ce test le fige — le POINT
        // D'APPEL du chaînage HTTP (`void servirTout(...)`, dans
        // `demarrerServeur`) et le routage WebSocket vivent dans la même
        // fonction, donc à portée de main. ⚠️ DEPUIS L'EXTRACTION DU 22 AOÛT
        // 2026, LA LISTE DES ROUTEURS ELLE-MÊME NE VIT PLUS ICI : elle est
        // dans `./chaine.ts` (`servirTout`, exporté) — seul le POINT D'APPEL
        // reste voisin du routage `upgrade`, et c'est ce voisinage-là que ce
        // test tient. ⚠️ Ce test vérifiait `/` avant le 21 août 2026 : le
        // relais a déménagé vers `/signal` DANS CE MÊME COMMIT (voir l'en-tête
        // de `serveur.ts`), pour une raison hors du champ de CE test — la
        // preuve du déplacement lui-même vit dans les trois tests dédiés plus
        // haut. Ce qui reste à sa charge est ajouté : que la racine reste
        // FERMÉE, comme `/signal` et `/agent` restent ce qu'ils sont, même
        // après le chaînage des quatre routeurs HTTP.
        service = await servir('http-chaine-ws');
        await expect(tenter(`ws://127.0.0.1:${service.port}/signal`)).resolves.toBe('ouvert');
        await expect(tenter(`ws://127.0.0.1:${service.port}/agent`)).resolves.toBe('ouvert');
        await expect(tenter(`ws://127.0.0.1:${service.port}/vm`)).resolves.toBe('ferme');
        await expect(tenter(`ws://127.0.0.1:${service.port}/`)).resolves.toBe('ferme');
    });
});

describe('la trame maximale acceptée avant toute authentification', () => {
    /// Ouvre un socket sur `url`, y envoie `octets` octets, et rend le code de
    /// fermeture — ou `'servi'` si le socket est toujours ouvert au bout de
    /// la borne.
    ///
    /// ⚠️ BORNÉE, jamais une attente infinie : un serveur qui n'appliquerait
    /// aucune borne laisserait le socket ouvert, et le test doit ROUGIR, pas
    /// pendre.
    function pousser(url: string, octets: number): Promise<number | 'servi'> {
        return new Promise((resolve, rejeter) => {
            const w = new WebSocket(url);
            const minuteur = setTimeout(() => {
                w.terminate();
                resolve('servi');
            }, 1500);
            w.once('open', () => {
                // 🔴 UNE TRAME UNIQUE, et son contenu est du JSON VALIDE une
                // fois tronqué mentalement : ce qui est mesuré est la TAILLE,
                // pas la forme. `ws` doit fermer AVANT que le gestionnaire
                // `message` ne voie quoi que ce soit.
                w.send('x'.repeat(octets));
            });
            w.once('close', (code) => {
                clearTimeout(minuteur);
                resolve(code);
            });
            w.once('error', () => {
                // Un socket fermé en cours d'écriture lève côté client : ce
                // n'est pas un échec du test, c'est la fermeture qu'il mesure.
            });
            setTimeout(() => rejeter(new Error('ni fermeture ni verdict en 3000 ms')), 3000);
        });
    }

    it('(a) 🔴 une trame TROP GRANDE ferme le socket en 1009, sur `/signal`', async () => {
        // Le pair est ANONYME : `signaling/relais.ts:84-86` dit lui-même que
        // le contrôle de FORME court une trentaine de lignes AVANT
        // `garde.verifier`. Sans borne, `JSON.parse` sur la trame est une
        // allocation puis un pic CPU, par socket et par trame, offerts à
        // quiconque atteint le port. ⚠️ CE TEST FRAPPAIT `/` AVANT LE 21 AOÛT
        // 2026 : le relais a déménagé vers `/signal` dans ce même commit (voir
        // l'en-tête de `serveur.ts`) — sans cette correction, la trame ne
        // ferait plus MÊME que monter, et le test rougirait pour une raison
        // qui n'a rien à voir avec `TRAME_MAX_OCTETS`.
        service = await servir('trame-signal');
        // 1009 = « message trop grand » (RFC 6455).
        await expect(pousser(`ws://127.0.0.1:${service.port}/signal`, TRAME_MAX_OCTETS + 1))
            .resolves.toBe(1009);
    });

    it('(a bis) 🔴 et sur `/agent` AUSSI, qui est l’autre porte anonyme', async () => {
        // Le canal d'enrôlement est ouvert avant toute identité : le borner
        // seulement sur `/signal` laisserait la moitié du problème entière.
        service = await servir('trame-agent');
        await expect(pousser(`ws://127.0.0.1:${service.port}/agent`, TRAME_MAX_OCTETS + 1))
            .resolves.toBe(1009);
    });

    it('(b) 🔴 une trame JUSTE SOUS la borne est acceptée et servie', async () => {
        // 🔴 SANS CE TEST, UN `maxPayload: 1` PASSERAIT LE TEST (a). C'est la
        // moitié qui empêche la borne de devenir un refus de service posé de
        // nos propres mains.
        service = await servir('trame-sous-borne');
        // Le socket reste ouvert : le message est mal formé, et le relais
        // laisse retenter un message malformé plutôt que de fermer.
        await expect(pousser(`ws://127.0.0.1:${service.port}/signal`, TRAME_MAX_OCTETS - 1))
            .resolves.toBe('servi');
    });

    it('(d) 🔴 LE PROCESSUS SURVIT à la trame refusée, et sert la requête suivante', async () => {
        // 🔴 CE TEST EXISTE PARCE QUE LE CORRECTIF DE (a) A FAILLI ÊTRE PIRE
        // QUE LE DÉFAUT. Poser `maxPayload` fait émettre `error` par `ws` sur
        // le socket SERVEUR ; or aucun socket serveur de ce service n'avait
        // d'écouteur `error` (vérifié le 20 août 2026 :
        // `grep -n "on('error'" relais.ts canal.ts serveur.ts` ne rendait que
        // le `http.once('error', reject)` du démarrage). Un `EventEmitter` qui
        // émet `error` sans écouteur LÈVE, et une exception non attrapée dans
        // un gestionnaire d'évènement Node ABAT TOUT LE PROCESS — le mode de
        // défaillance exact que `signaling/relais.ts` et `signaling/trace.ts`
        // documentent tous deux.
        //
        // Autrement dit : sans l'écouteur, UNE SEULE TRAME ANONYME TUAIT LE
        // SERVICE, là où avant elle ne faisait que le ralentir. Vitest l'a vu
        // (« Vitest caught 2 unhandled errors »), et ce test le fige.
        service = await servir('trame-survie');
        const url = `http://127.0.0.1:${service.port}`;
        // ⚠️ Frappait `/` avant le déplacement du relais vers `/signal` — voir
        // le test (a) plus haut.
        await expect(pousser(`ws://127.0.0.1:${service.port}/signal`, TRAME_MAX_OCTETS + 1))
            .resolves.toBe(1009);
        await expect(pousser(`ws://127.0.0.1:${service.port}/agent`, TRAME_MAX_OCTETS + 1))
            .resolves.toBe(1009);
        // Sans cette requête, un processus abattu se lirait exactement comme
        // un processus sain — le test aurait déjà rendu son verdict.
        const apres = await fetch(`${url}/inconnu`);
        expect(apres.status).toBe(404);
    });

    it('(c) la borne est celle que le module annonce, et elle est grande', () => {
        // ⚠️ NON CALIBRÉE, et son plancher est RAISONNÉ, pas mesuré : voir
        // l'en-tête de `serveur.ts`. Ce test fige la valeur pour qu'un
        // changement soit un geste délibéré.
        expect(TRAME_MAX_OCTETS).toBe(256 * 1024);
    });
});
