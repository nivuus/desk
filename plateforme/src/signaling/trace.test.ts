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

import { afterEach, describe, expect, it, vi } from 'vitest';
import { WebSocket } from 'ws';
import { baseNeuve } from '../base/harnais';
import type { Pilote } from '../base/pilote';
import type { Config } from '../config';
import { enroler } from '../depot/agent';
import { lireParNom, type LigneSession } from '../depot/session';
import { demarrerServeur, type ServicePlateforme } from '../http/serveur';
import { signer } from '../identite/jeton';
import { hacher } from '../identite/mot-de-passe';
import { MOTIF_DEPART, observateurDeSession } from './trace';
import { mkdtempSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

// Un secret de test EXPLICITE, jamais `''` : `lireConfig` refuse la chaîne
// vide, et un littéral `Config` construit à la main doit porter une valeur
// qu'un service accepterait réellement.
const CONFIG: Config = {
    hote: '127.0.0.1',
    port: 0,
    base: 'sqlite',
    urlBase: ':memory:',
    secretJeton: 'un-secret-de-plateforme-de-quarante-octets',
    // Aucun proxy déclaré — voir `config.ts` : l'ensemble vide est le défaut,
    // et il signifie « ne croire l'adresse annoncée par personne ».
    proxyDeConfiance: new Set(),
    repertoireIcones: join(mkdtempSync(join(tmpdir(), 'g2-icones-')), 'icones'),
    repertoireTeleversements: join(mkdtempSync(join(tmpdir(), 'g3-tranches-')), 'televersements'),
    auth: 'pomerium',
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

/// Le préfixe de la VM simulée. Toutes les sessions de ce fichier le portent,
/// parce que la garde exige désormais que le sujet du jeton d'agent PRÉFIXE la
/// session demandée (sous-bloc P3).
const P = 'RhH1x2QmTz9kLpVbNc7dAw';

function connecter(
    url: string,
    role: string,
    session: string,
    sujet = 'u-trace',
): Promise<WebSocket> {
    return new Promise((resolve) => {
        const w = new WebSocket(url);
        w.once('open', () => {
            // 🔴 LES DEUX RÔLES EXIGENT UN JETON. Le rôle `client` depuis P2 ;
            // le rôle `agent` depuis P3, qui a fermé la fenêtre anonyme de E2.
            // Sans jeton, la garde refuse, le socket se ferme, et AUCUN
            // appariement n'a lieu — donc aucune ligne de trace, et ces tests
            // mesureraient un service qui n'apparie jamais rien.
            //
            // Le jeton d'agent est de TYPE `agent` et son sujet est le PRÉFIXE
            // de la VM : la garde exige que ce sujet préfixe la session.
            const jeton = role === 'client'
                ? signer(sujet, CONFIG.secretJeton, Date.now())
                : signer(P, CONFIG.secretJeton, Date.now(), undefined, 'agent');
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

/// Enrôle une VM à laquelle la session pourra se rattacher.
///
/// ⚠️ LA LIGNE `vm` D'ABORD : `agent_enrole.vm_id` la RÉFÉRENCE
/// (`0003-agents.sql`), et SQLite applique la clé étrangère. L'empreinte est
/// une VRAIE empreinte `scrypt`, jamais une chaîne courte — même règle que
/// `depot/agent.test.ts`, et pour la même raison : une valeur commode ne
/// mesure aucune longueur de colonne.
async function enrolerUneVm(p: Pilote, vmId: string, prefixe: string): Promise<void> {
    await p.executer('INSERT INTO vm(id,nom,adresse) VALUES(?,?,?)', [
        vmId,
        `vm-${vmId}`,
        '192.168.3.2',
    ]);
    await enroler(p, vmId, await hacher('un-secret-d-enrolement-de-la-vraie-longueur'), prefixe);
}

describe('la colonne session.vm_id', () => {
    // 🔴 C'EST LE LEGS N°3 DE P2 QUI SE FERME ICI : « `session.vm_id` reste
    // entièrement NULL ». Le préfixe du nom de session désigne la VM, et
    // c'est la TRACE qui le résout — jamais le relais, dont `apparie` reste
    // synchrone et sans retour (E10). La résolution est donc éprouvée au
    // niveau de l'observateur, là où elle vit.

    it('une session préfixée par une VM ENRÔLÉE inscrit son vm_id', async () => {
        base = await baseNeuve('trace-vm-connue');
        await enrolerUneVm(base, 'v-1', P);
        const obs = observateurDeSession(base, () => 5_000_000_000);

        obs.apparie(`${P}:bureau`);
        const ligne = await attendreLigne(base, `${P}:bureau`, () => true, 'ouverte');
        expect(ligne.vm_id).toBe('v-1');
    });

    it('une session SANS préfixe laisse vm_id à `null`', async () => {
        // Le mode d'essai local que la spec §10 pose comme LÉGITIME : un
        // agent lancé sans `AGENT_VM` nomme sa session `bureau`, tout court.
        // Lever, ou inscrire une chaîne vide, casserait ce mode — et une
        // chaîne vide mentirait en prétendant connaître une VM.
        base = await baseNeuve('trace-vm-sans-prefixe');
        await enrolerUneVm(base, 'v-1', P);
        const obs = observateurDeSession(base, () => 5_000_000_000);

        obs.apparie('bureau');
        const ligne = await attendreLigne(base, 'bureau', () => true, 'ouverte');
        expect(ligne.vm_id).toBeNull();
    });

    it('🔴 l’instant est lu SYNCHRONEMENT à l’appariement, PAS après la lecture de base', async () => {
        // 🔴 CE TEST EXISTE PARCE QU'UNE MUTATION EST RESTÉE VERTE SANS LUI.
        // La résolution du préfixe intercale une lecture de base entre
        // `apparie()` et l'INSERT : lire l'horloge dans l'appel à
        // `ouvrirSession` daterait donc `ouverte_a` de la FIN D'UNE REQUÊTE et
        // non de l'appariement. Aucun des trois tests ci-dessus ne le voyait —
        // ils ne bougent pas leur horloge —, et le commentaire de `trace.ts`
        // affirmait la règle sans que rien ne la tienne.
        base = await baseNeuve('trace-instant-synchrone');
        await enrolerUneVm(base, 'v-1', P);
        let instant = 5_000_000_000;
        const obs = observateurDeSession(base, () => instant);

        obs.apparie(`${P}:bureau`);
        // 🔴 CETTE LIGNE COURT AVANT QUE LA RÉSOLUTION N'AIT ABOUTI, et c'est
        // ce qui rend le test décidable : `resoudreVm` est asynchrone, donc
        // `apparie` a rendu la main ici sans que la base ait répondu. Une
        // horloge lue plus tard verrait 9 000 000 000.
        instant = 9_000_000_000;

        const ligne = await attendreLigne(base, `${P}:bureau`, () => true, 'ouverte');
        expect(Number(ligne.ouverte_a)).toBe(5_000_000_000);
        expect(ligne.vm_id).toBe('v-1');
    });

    it('🔴 une session à préfixe INCONNU laisse vm_id à `null`, et le JOURNALISE', async () => {
        // 🔴 LES DEUX MOITIÉS COMPTENT. Inscrire quand même ferait MENTIR la
        // colonne — elle nommerait une VM que la base ne connaît pas. Et se
        // taire rendrait le cas indiscernable du précédent : un agent dont
        // l'enrôlement a été révoqué apparierait des sessions sans que rien,
        // nulle part, ne le signale.
        base = await baseNeuve('trace-vm-inconnue');
        const journal = vi.spyOn(console, 'warn').mockImplementation(() => {});
        const inconnu = 'Zz9QmRhH1x2kLpVbNc7dAw';
        const obs = observateurDeSession(base, () => 5_000_000_000);

        obs.apparie(`${inconnu}:bureau`);
        const ligne = await attendreLigne(base, `${inconnu}:bureau`, () => true, 'ouverte');
        expect(ligne.vm_id).toBeNull();

        const lignes = journal.mock.calls.map((c) => String(c[0])).join('\n');
        expect(lignes).toContain(inconnu);
        journal.mockRestore();
    });
});

describe('le service entier', () => {
    it('écrit une ligne à l’appariement, et la clôt à la déconnexion des deux pairs', async () => {
        base = await baseNeuve('trace-service');
        service = await demarrerServeur(CONFIG, base);
        const url = `ws://127.0.0.1:${service.port}/`;

        const agent = await connecter(url, 'agent', `${P}:trace-1`);
        const client = await connecter(url, 'client', `${P}:trace-1`);

        const ouverte = await attendreLigne(base, `${P}:trace-1`, () => true, 'ouverte');
        expect(Number(ouverte.ouverte_a)).toBeGreaterThan(0);
        expect(ouverte.fermee_a).toBeNull();

        await fermer(agent);
        await fermer(client);

        const close = await attendreLigne(base, `${P}:trace-1`, (l) => l.fermee_a !== null, 'close');
        expect(Number(close.fermee_a)).toBeGreaterThanOrEqual(Number(close.ouverte_a));
        expect(await lireParNom(base, `${P}:trace-1`)).toHaveLength(1);
    });

    it('n’écrit rien quand un seul pair s’est déclaré', async () => {
        // 🔴 Le premier test serait VERT avec une écriture posée trop tôt —
        // dès la première déclaration. C'est ce test-ci qui distingue
        // « appariement » de « connexion », et le superviseur se déclare seul
        // sur `bureau` des heures durant au démarrage de la VM.
        base = await baseNeuve('trace-solitaire');
        service = await demarrerServeur(CONFIG, base);
        const url = `ws://127.0.0.1:${service.port}/`;

        const agent = await connecter(url, 'agent', `${P}:trace-2`);
        await new Promise((r) => setTimeout(r, 300));
        expect(await lireParNom(base, `${P}:trace-2`)).toHaveLength(0);

        await fermer(agent);
        await new Promise((r) => setTimeout(r, 300));
        expect(await lireParNom(base, `${P}:trace-2`)).toHaveLength(0);
    });

    it('inscrit en base l’utilisateur du client authentifié qui apparie', async () => {
        // ⚠️ CE TEST NE PEUT PAS ÊTRE CELUI D'UN CLIENT ANONYME : depuis que
        // la garde est câblée, un client anonyme n'atteint jamais
        // l'appariement. Un test qui en supposerait un mesurerait un état que
        // le produit ne peut plus produire — vacueux par construction. Le cas
        // réel est celui-ci : la session de contrôle `bureau`, où l'`agent`
        // arrive seul et sans identité, et où le CLIENT, lui, est authentifié.
        base = await baseNeuve('trace-appartenance');
        service = await demarrerServeur(CONFIG, base);
        const url = `ws://127.0.0.1:${service.port}/`;

        const agent = await connecter(url, 'agent', `${P}:bureau`);
        const client = await connecter(url, 'client', `${P}:bureau`, 'u-proprietaire');

        const ligne = await attendreLigne(base, `${P}:bureau`, () => true, 'ouverte');
        expect(ligne.utilisateur_id).toBe('u-proprietaire');

        await fermer(agent);
        await fermer(client);
    });
});
