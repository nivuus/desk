// The common base of acceptance run P4's probes: start a REAL service,
// populate its database as an administrator would, and hit its routes.
//
// 🔴 THE SERVICE IS THE REAL ONE, mounted by `start()` — the same sequence as
// `src/index.ts`. Nothing is simulated on the platform side: it is HTTP over a
// socket, against a real database, on both engines. What the probe
// fabricates are the PEERS (a browser scripted through `fetch`) and
// the administrator (who creates accounts and enrols VMs) — never the
// service itself.
//
// 🔴 NO DEPENDENCY IS IMPORTED FROM THIS DIRECTORY. `fetch` has been global
// since Node 18; importing anything from here would resolve the package of the
// repository's ROOT, hence a SECOND instance of a version potentially different
// from the one the service uses.

import { lireConfig } from '../../../../../plateforme/src/config';
import { start, type Service } from '../../../../../plateforme/src/demarrage';
import { ouvrirPostgres } from '../../../../../plateforme/src/base/pilote-postgres';
import type { Pilote } from '../../../../../plateforme/src/base/pilote';
import { createUser } from '../../../../../plateforme/src/depot/utilisateur';
import { enrolerLaVm } from '../../../../../plateforme/src/admin/enroler-agent';
import { signer } from '../../../../../plateforme/src/identite/jeton';

export type Moteur = 'sqlite' | 'postgres';

/// FIXED and long: `lireConfig` refuses below `MIN_SECRET_LENGTH`, and
/// a randomly drawn secret would make the logs non-comparable from one
/// run to the next for nothing. It protects nothing — the service only lives
/// for the duration of the probe, on an ephemeral loopback port.
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
        const schema = `p4_${nom.replace(/[^a-z0-9]/gi, '_')}_${process.pid}`;
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

/// The order of magnitude of an epoch in milliseconds. It is P1's most
/// expensive lesson: the double pass only wrote `1_000`s and declared
/// portable a schema Postgres refused for any real write.
export const INSTANT = 1_700_000_000_000;

/// Creates an account. The hash is a non-derivable LITERAL: this probe
/// never signs in by password, it signs its tokens directly.
export async function createAccount(p: Pilote, email: string): Promise<string> {
    return createUser(p, email, 'scrypt$16384$8$1$sel-de-recette$aucune-connexion-par-ce-chemin', INSTANT);
}

export interface TestVm {
    vmId: string;
    prefixe: string;
    nom: string;
}

/// Enrols a VM as `npm run admin:agent` would — the SAME function.
export async function enroler(p: Pilote, nom: string): Promise<TestVm> {
    const e = await enrolerLaVm(p, nom, '192.168.3.2', INSTANT);
    return { vmId: e.vmId, prefixe: e.prefixe, nom };
}

/// A user access token, signed with the SAME secret as the service.
export function jetonDe(userId: string): string {
    return signer(userId, SECRET_JETON, Date.now());
}

/// Writes `vu_a` at the desired value. That is how the probe besieges the
/// freshness bound through the REAL service: `Date.now` is not injectable there
/// (`http/serveur.ts` passes it as is to the three routers), but the
/// DATA is, and it is the same subtraction that is tested.
export async function poserVuA(p: Pilote, vmId: string, vuA: number | null): Promise<void> {
    await p.executer('UPDATE agent_enrole SET vu_a = ? WHERE vm_id = ?', [vuA, vmId]);
}

export interface Reponse {
    code: number;
    corps: any;
    dureeMs: number;
}

/// A REAL HTTP request against the service, timed.
export async function appeler(
    port: number,
    chemin: string,
    methode: string,
    jeton?: string,
): Promise<Reponse> {
    const debut = performance.now();
    const r = await fetch(`http://127.0.0.1:${port}${chemin}`, {
        method: methode,
        headers: jeton === undefined ? {} : { authorization: `Bearer ${jeton}` },
    });
    const corps = await r.json().catch(() => undefined);
    return { code: r.status, corps, dureeMs: performance.now() - debut };
}

export function ligne(cle: string, value: unknown): string {
    return `${cle.padEnd(46)}: ${typeof value === 'string' ? value : JSON.stringify(value)}`;
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

/// The verdict collector. It only does one thing more than a `console.log`:
/// it REMEMBERS whether one of them failed, which gives the probe its exit code.
export class Verdicts {
    private toutTenu = true;
    dire(texte: string): void {
        process.stdout.write(`${texte}\n`);
    }
    /// ⚠️ A VERDICT CARRIES ITS EXPECTED AND ITS OBTAINED, ALWAYS. A bare "HELD"
    /// cannot be reread: one would not know what was compared.
    juger(nom: string, obtenu: unknown, attendu: unknown): void {
        const ok = JSON.stringify(obtenu) === JSON.stringify(attendu);
        if (!ok) this.toutTenu = false;
        this.dire(ligne(nom, ok ? 'TENU' : `🔴 NON TENU — obtenu ${JSON.stringify(obtenu)}, attendu ${JSON.stringify(attendu)}`));
    }
    /// For what is judged by an INEQUALITY (a duration under a bound).
    judgeBelow(nom: string, obtenu: number, borne: number): void {
        const ok = obtenu < borne;
        if (!ok) this.toutTenu = false;
        this.dire(ligne(nom, ok ? `TENU (${obtenu.toFixed(2)} < ${borne})` : `🔴 NON TENU — ${obtenu.toFixed(2)} ≥ ${borne}`));
    }
    conclure(): number {
        this.dire('');
        this.dire(ligne('VERDICT', this.toutTenu ? 'TOUT TENU' : '🔴 AU MOINS UNE ASSERTION NON TENUE'));
        return this.toutTenu ? 0 : 1;
    }
}
