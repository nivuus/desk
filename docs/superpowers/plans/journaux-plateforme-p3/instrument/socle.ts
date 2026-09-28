// The common base of acceptance run P3's probes: start a real service,
// open the `/agent` channel, open a handshake on the relay.
//
// 🔴 NO `ws` IS IMPORTED HERE. Node 24 carries `WebSocket` as a global, and
// it is deliberate: the probe lives outside `plateforme/`, and importing `ws`
// from this directory would resolve the package of the repository's ROOT — a
// SECOND instance of the library, of a version potentially different from
// the one the service uses. Node's global client shares nothing with
// the server: it is protocol on the wire, and nothing else.
//
// 🔴 THE SERVICE IS THE REAL ONE, mounted by `start()` — the same sequence as
// `src/index.ts`. Nothing is simulated on the platform side: it is the
// `agent` and `client` peers that are scripted sockets, as spec §4 P3
// requires.

import { lireConfig } from '../../../../../plateforme/src/config';
import { start, type Service } from '../../../../../plateforme/src/demarrage';
import { ouvrirPostgres } from '../../../../../plateforme/src/base/pilote-postgres';

export type Moteur = 'sqlite' | 'postgres';

/// The token signing secret. FIXED and long: `lireConfig` refuses
/// below `MIN_SECRET_LENGTH`, and a randomly drawn secret would make
/// the logs non-comparable from one run to the next for nothing.
export const SECRET_JETON = '***RETIRE-DE-L-HISTORIQUE***';

const URL_POSTGRES =
    process.env.PLATEFORME_BASE_URL ??
    'postgres://plateforme:plateforme-test@127.0.0.1:5433/plateforme_test';

/// Starts a FRESH service on the requested engine, ephemeral port.
///
/// Postgres: a throwaway SCHEMA, as `base/harnais.ts` does for the
/// tests — two successive runs must not step on each other.
export async function startService(moteur: Moteur, nom: string): Promise<Service> {
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
        // 0 = ephemeral port. `startServer` rereads the real address.
        PLATEFORME_PORT: '0',
        PLATEFORME_BASE: moteur,
        PLATEFORME_BASE_URL: urlBase,
        PLATEFORME_SECRET_JETON: SECRET_JETON,
    });
    return start(config);
}

/// A received frame, AS IS — the raw string, never a reparsed object:
/// criterion ② compares two refusals CHARACTER FOR CHARACTER, which a reparsed
/// object would make impossible to establish.
export interface Trame {
    brut: string;
}

export interface Fermeture {
    code: number;
    raison: string;
}

/// A scripted socket: it collects everything that arrives, and its closing.
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

    /// Waits for at least `n` frames to have arrived, or for the delay to expire.
    /// ⚠️ DOES NOT THROW on expiry: the ABSENCE of an answer is itself a
    /// reading ("received ice-config: false"), and an exception would
    /// turn it into a probe failure.
    async attendre(n: number, delaiMs = 2000): Promise<void> {
        const fin = Date.now() + delaiMs;
        while (this.recues.length < n && Date.now() < fin && this.fermeture === undefined) {
            await new Promise((r) => setTimeout(r, 20));
        }
        // A frame may still arrive right before a closing: we let
        // one last event loop beat go by.
        await new Promise((r) => setTimeout(r, 60));
    }

    fermer(): void {
        try {
            this.socket.close();
        } catch {
            /* already closed */
        }
    }
}

/// The RELAY's handshake, as `agent/src/signaling.rs:68`
/// composes it. `jeton` at `undefined` goes out as `null`, exactly like the agent
/// without an identity — that is the case of probe E2.
export function poignee(role: string, session: string, jeton?: string): string {
    return JSON.stringify({ role, session, jeton: jeton ?? null });
}

export function ligne(cle: string, value: unknown): string {
    return `${cle.padEnd(34)}: ${typeof value === 'string' ? value : JSON.stringify(value)}`;
}

/// The header EVERY log of this acceptance run carries.
export function entete(titre: string, moteur: Moteur, commit: string): string {
    return [
        `# ${titre}`,
        `# Exécution ${moteur === 'sqlite' ? '1' : '2'} — moteur ${moteur}`,
        `# Jouée le ${new Date().toISOString()}, commit ${commit}`,
        '#',
    ].join('\n');
}
