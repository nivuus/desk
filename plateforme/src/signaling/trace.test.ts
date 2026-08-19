// Le critère ② de P1 : une session appariée laisse une ligne en base, et la
// clôt en partant.
//
// Deux étages sont éprouvés ici, et il faut les deux :
//   - l'observateur SEUL, avec une horloge injectée, sur des VALEURS EXACTES ;
//   - le service ENTIER, deux pairs sur le chemin racine, où la seule preuve
//     possible est une attente BORNÉE.
//
// ⚠️ L'attente est bornée et échoue sur expiration, jamais une boucle infinie :
// l'écriture est délibérément lancée sans être attendue (voir `trace.ts`), donc
// une écriture perdue ne se manifeste que par une ligne qui n'arrive pas. Un
// test qui bouclerait sans borne pendrait au lieu de rougir.

import { afterEach, describe, expect, it } from 'vitest';
import { WebSocket } from 'ws';
import { baseNeuve } from '../base/harnais';
import type { Pilote } from '../base/pilote';
import type { Config } from '../config';
import { lireParNom, type LigneSession } from '../depot/session';
import { demarrerServeur, type ServicePlateforme } from '../http/serveur';
import { signer } from '../identite/jeton';
import { MOTIF_DEPART, observateurDeSession } from './trace';

// Un secret de test EXPLICITE, jamais `''` : `lireConfig` refuse la chaîne
// vide, et un littéral `Config` construit à la main doit porter une valeur
// qu'un service accepterait réellement.
const CONFIG: Config = {
    hote: '127.0.0.1',
    port: 0,
    base: 'sqlite',
    urlBase: ':memory:',
    secretJeton: 'un-secret-de-plateforme-de-quarante-octets',
};

let base: Pilote | undefined;
let service: ServicePlateforme | undefined;

afterEach(async () => {
    await service?.close();
    service = undefined;
    await base?.fermer();
    base = undefined;
});

/// Attend qu'une ligne satisfasse `predicat`, ou ÉCHOUE au bout de `borneMs`.
async function attendreLigne(
    p: Pilote,
    nom: string,
    predicat: (l: LigneSession) => boolean,
    quoi: string,
    borneMs = 2000,
): Promise<LigneSession> {
    const fin = Date.now() + borneMs;
    for (;;) {
        const lignes = await lireParNom(p, nom);
        const trouvee = lignes.find(predicat);
        if (trouvee) return trouvee;
        if (Date.now() > fin) {
            throw new Error(`aucune ligne session ${quoi} pour ${nom} en ${borneMs} ms`);
        }
        await new Promise((r) => setTimeout(r, 25));
    }
}

function connecter(
    url: string,
    role: string,
    session: string,
    sujet = 'u-trace',
): Promise<WebSocket> {
    return new Promise((resolve) => {
        const w = new WebSocket(url);
        w.once('open', () => {
            // Le rôle `client` exige un jeton d'accès depuis le sous-bloc P2 :
            // sans lui la garde refuse, le socket se ferme, et AUCUN
            // appariement n'a lieu — donc aucune ligne de trace. Le jeton est
            // signé avec le secret que porte `CONFIG`, celui du service.
            const jeton = role === 'client'
                ? signer(sujet, CONFIG.secretJeton, Date.now())
                : undefined;
            w.send(JSON.stringify({ role, session, jeton }));
            resolve(w);
        });
    });
}

function fermer(w: WebSocket): Promise<void> {
    return new Promise((resolve) => {
        w.once('close', () => resolve());
        w.close();
    });
}

describe('observateur de session', () => {
    it('ouvre à l’appariement et clôt au départ, sur des instants EXACTS', async () => {
        base = await baseNeuve('trace-unite');
        let instant = 5_000_000_000;
        const obs = observateurDeSession(base, () => instant);

        obs.apparie('u-1');
        await attendreLigne(base, 'u-1', (l) => l.fermee_a === null, 'ouverte');

        instant = 5_000_000_900;
        obs.separe('u-1');
        const close = await attendreLigne(base, 'u-1', (l) => l.fermee_a !== null, 'close');
        // Valeurs EXACTES : c'est l'assertion qui interdit un `Date.now()`
        // caché dans l'observateur.
        expect(Number(close.ouverte_a)).toBe(5_000_000_000);
        expect(Number(close.fermee_a)).toBe(5_000_000_900);
        expect(close.motif).toBe(MOTIF_DEPART);
    });

    it('un second appariement ne rouvre pas une seconde ligne', async () => {
        // Un pair qui se reconnecte pendant que l'autre reste en place
        // rapparie la session. Sans cette garde, la première ligne serait
        // orpheline — jamais close, jusqu'au balayage du prochain démarrage.
        base = await baseNeuve('trace-rappari');
        const obs = observateurDeSession(base, () => 7_000);
        obs.apparie('u-2');
        await attendreLigne(base, 'u-2', () => true, 'ouverte');
        obs.apparie('u-2');
        await new Promise((r) => setTimeout(r, 100));
        expect(await lireParNom(base, 'u-2')).toHaveLength(1);
    });

    it('un départ sans appariement préalable n’écrit rien', async () => {
        base = await baseNeuve('trace-sans');
        const obs = observateurDeSession(base, () => 7_000);
        obs.separe('jamais-apparie');
        await new Promise((r) => setTimeout(r, 100));
        expect(await lireParNom(base, 'jamais-apparie')).toHaveLength(0);
    });
});

describe('le service entier', () => {
    it('écrit une ligne à l’appariement, et la clôt à la déconnexion des deux pairs', async () => {
        base = await baseNeuve('trace-service');
        service = await demarrerServeur(CONFIG, base);
        const url = `ws://127.0.0.1:${service.port}/`;

        const agent = await connecter(url, 'agent', 'trace-1');
        const client = await connecter(url, 'client', 'trace-1');

        const ouverte = await attendreLigne(base, 'trace-1', () => true, 'ouverte');
        expect(Number(ouverte.ouverte_a)).toBeGreaterThan(0);
        expect(ouverte.fermee_a).toBeNull();

        await fermer(agent);
        await fermer(client);

        const close = await attendreLigne(base, 'trace-1', (l) => l.fermee_a !== null, 'close');
        expect(Number(close.fermee_a)).toBeGreaterThanOrEqual(Number(close.ouverte_a));
        expect(await lireParNom(base, 'trace-1')).toHaveLength(1);
    });

    it('n’écrit rien quand un seul pair s’est déclaré', async () => {
        // 🔴 Le premier test serait VERT avec une écriture posée trop tôt —
        // dès la première déclaration. C'est ce test-ci qui distingue
        // « appariement » de « connexion », et le superviseur se déclare seul
        // sur `bureau` des heures durant au démarrage de la VM.
        base = await baseNeuve('trace-solitaire');
        service = await demarrerServeur(CONFIG, base);
        const url = `ws://127.0.0.1:${service.port}/`;

        const agent = await connecter(url, 'agent', 'trace-2');
        await new Promise((r) => setTimeout(r, 300));
        expect(await lireParNom(base, 'trace-2')).toHaveLength(0);

        await fermer(agent);
        await new Promise((r) => setTimeout(r, 300));
        expect(await lireParNom(base, 'trace-2')).toHaveLength(0);
    });
});
