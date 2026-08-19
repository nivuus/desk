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
import { demarrerServeur, type ServicePlateforme } from './serveur';

// Un secret de test EXPLICITE, jamais `''` : `lireConfig` refuse la chaîne
// vide, et un littéral `Config` construit à la main doit porter une valeur
// qu'un service accepterait réellement.
const SECRET = 'un-secret-de-plateforme-de-quarante-octets';

const CONFIG: Config = {
    hote: '127.0.0.1',
    port: 0,
    base: 'sqlite',
    urlBase: ':memory:',
    secretJeton: SECRET,
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

    it('sert le relais de signaling sur le chemin racine, poignée de main comprise', async () => {
        // Un pair 'agent' et un pair 'client' sur '/' : l'offre du client
        // parvient à l'agent — exactement ce que `server.test.ts` éprouve déjà,
        // rejoué ici à travers le serveur HTTP pour prouver que le passage par
        // l'upgrade ne change rien.
        service = await servir('http-racine');
        const url = `ws://127.0.0.1:${service.port}/`;

        const agent = new WebSocket(url);
        await new Promise((r) => agent.once('open', r));
        agent.send(JSON.stringify({ role: 'agent', session: 'racine-1' }));

        const client = new WebSocket(url);
        await new Promise((r) => client.once('open', r));
        client.send(JSON.stringify({ role: 'client', session: 'racine-1' }));

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
