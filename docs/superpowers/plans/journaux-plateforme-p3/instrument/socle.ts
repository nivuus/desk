// Le socle commun des sondes de la recette P3 : démarrer un service réel,
// ouvrir le canal `/agent`, ouvrir une poignée de main sur le relais.
//
// 🔴 AUCUN `ws` N'EST IMPORTÉ ICI. Node 24 porte `WebSocket` en global, et
// c'est délibéré : la sonde vit hors de `plateforme/`, et importer `ws`
// depuis ce répertoire résoudrait le paquet de la RACINE du dépôt — une
// SECONDE instance de la bibliothèque, de version potentiellement autre que
// celle que le service emploie. Le client global de Node ne partage rien avec
// le serveur : c'est du protocole sur le fil, et rien d'autre.
//
// 🔴 LE SERVICE EST LE VRAI, monté par `demarrer()` — la même séquence que
// `src/index.ts`. Rien n'est simulé côté plateforme : ce sont les pairs
// `agent` et `client` qui sont des sockets scriptés, comme la spec §4 P3
// l'exige.

import { lireConfig } from '../../../../../plateforme/src/config';
import { demarrer, type Service } from '../../../../../plateforme/src/demarrage';
import { ouvrirPostgres } from '../../../../../plateforme/src/base/pilote-postgres';

export type Moteur = 'sqlite' | 'postgres';

/// Le secret de signature des jetons. FIXE et long : `lireConfig` refuse
/// en dessous de `LONGUEUR_SECRET_MIN`, et un secret tiré au sort rendrait
/// les journaux non comparables d'une exécution à l'autre pour rien.
export const SECRET_JETON = '***RETIRE-DE-L-HISTORIQUE***';

const URL_POSTGRES =
    process.env.PLATEFORME_BASE_URL ??
    'postgres://plateforme:plateforme-test@127.0.0.1:5433/plateforme_test';

/// Démarre un service NEUF sur le moteur demandé, port éphémère.
///
/// Postgres : un SCHÉMA jetable, comme `base/harnais.ts` le fait pour les
/// tests — deux exécutions successives ne doivent pas se marcher dessus.
export async function demarrerService(moteur: Moteur, nom: string): Promise<Service> {
    let urlBase = ':memory:';
    if (moteur === 'postgres') {
        const schema = `p3_${nom.replace(/[^a-z0-9]/gi, '_')}_${process.pid}`;
        const admin = ouvrirPostgres(URL_POSTGRES);
        await admin.executer(`DROP SCHEMA IF EXISTS ${schema} CASCADE`, []);
        await admin.executer(`CREATE SCHEMA ${schema}`, []);
        await admin.fermer();
        urlBase = `${URL_POSTGRES}?options=-c%20search_path%3D${schema}`;
    }
    const config = lireConfig({
        PLATEFORME_HOTE: '127.0.0.1',
        // 0 = port éphémère. `demarrerServeur` relit l'adresse réelle.
        PLATEFORME_PORT: '0',
        PLATEFORME_BASE: moteur,
        PLATEFORME_BASE_URL: urlBase,
        PLATEFORME_SECRET_JETON: SECRET_JETON,
    });
    return demarrer(config);
}

/// Une trame reçue, TELLE QUELLE — la chaîne brute, jamais un objet reparsé :
/// le critère ② compare deux refus CARACTÈRE POUR CARACTÈRE, ce qu'un objet
/// reparsé rendrait impossible à établir.
export interface Trame {
    brut: string;
}

export interface Fermeture {
    code: number;
    raison: string;
}

/// Un socket scripté : il collecte tout ce qui arrive, et sa fermeture.
export class Pair {
    readonly recues: Trame[] = [];
    fermeture: Fermeture | undefined;
    private readonly socket: WebSocket;

    private constructor(socket: WebSocket) {
        this.socket = socket;
        socket.addEventListener('message', (e) => {
            this.recues.push({ brut: String((e as MessageEvent).data) });
        });
        socket.addEventListener('close', (e) => {
            const ferme = e as CloseEvent;
            this.fermeture = { code: ferme.code, raison: ferme.reason };
        });
    }

    static async ouvrir(url: string): Promise<Pair> {
        const socket = new WebSocket(url);
        const pair = new Pair(socket);
        await new Promise<void>((resolve, reject) => {
            socket.addEventListener('open', () => resolve(), { once: true });
            socket.addEventListener('error', () => reject(new Error(`connexion refusée : ${url}`)), {
                once: true,
            });
        });
        return pair;
    }

    envoyer(brut: string): void {
        this.socket.send(brut);
    }

    /// Attend qu'au moins `n` trames soient arrivées, ou que le délai expire.
    /// ⚠️ NE LÈVE PAS sur expiration : l'ABSENCE de réponse est elle-même un
    /// relevé (« a reçu ice-config : false »), et une exception la
    /// transformerait en panne de sonde.
    async attendre(n: number, delaiMs = 2000): Promise<void> {
        const fin = Date.now() + delaiMs;
        while (this.recues.length < n && Date.now() < fin && this.fermeture === undefined) {
            await new Promise((r) => setTimeout(r, 20));
        }
        // Une trame peut encore arriver juste avant une fermeture : on laisse
        // un dernier battement de boucle d'évènements passer.
        await new Promise((r) => setTimeout(r, 60));
    }

    fermer(): void {
        try {
            this.socket.close();
        } catch {
            /* déjà fermé */
        }
    }
}

/// La poignée de main du RELAIS, telle que `agent/src/signaling.rs:68` la
/// compose. `jeton` à `undefined` part en `null`, exactement comme l'agent
/// sans identité — c'est le cas de la sonde E2.
export function poignee(role: string, session: string, jeton?: string): string {
    return JSON.stringify({ role, session, jeton: jeton ?? null });
}

export function ligne(cle: string, valeur: unknown): string {
    return `${cle.padEnd(34)}: ${typeof valeur === 'string' ? valeur : JSON.stringify(valeur)}`;
}

/// L'en-tête que TOUT journal de cette recette porte.
export function entete(titre: string, moteur: Moteur, commit: string): string {
    return [
        `# ${titre}`,
        `# Exécution ${moteur === 'sqlite' ? '1' : '2'} — moteur ${moteur}`,
        `# Jouée le ${new Date().toISOString()}, commit ${commit}`,
        '#',
    ].join('\n');
}
