// The startup sequence of the service, and its refusal to start degraded.
//
// 🔴 THE ORDER IS NON-NEGOTIABLE:
//     lireConfig -> ouvrirBase -> appliquerMigrations -> balayerLesOuvertes
//     -> startServer
//
// THE PORT OPENS ONLY LAST. A peer must never reach a
// service whose database is not ready: spec §6, "a signaling that pairs
// without recording anything would be indistinguishable from working correctly". The
// service REFUSES to start, with the cause, rather than serving halfway.
//
// This module is separate from `index.ts` to be testable: `index.ts` reads
// `process.env` and runs on import, which a test cannot do
// several times.

import type { Config } from './config';
import { appliquerMigrations, REPERTOIRE_MIGRATIONS } from './base/migrations';
import { ouvrirBase } from './base/ouvrir';
import type { Pilote } from './base/pilote';
import { balayerLesOuvertes } from './depot/session';
import { startServer, type ServicePlateforme } from './http/serveur';

export interface Service {
    port: number;
    base: Pilote;
    arreter(): Promise<void>;
}

export async function start(config: Config, maintenant = Date.now()): Promise<Service> {
    let base: Pilote;
    try {
        base = await ouvrirBase(config);
        await appliquerMigrations(base, REPERTOIRE_MIGRATIONS, maintenant);
    } catch (cause) {
        // No port has been opened at this stage, and that is the point: the rejection
        // leaves the service ENTIRELY absent, never half present.
        throw new Error(`database unreachable or migrations failed: ${String(cause)}`, { cause });
    }

    const balayees = await balayerLesOuvertes(base, maintenant);
    if (balayees > 0) {
        console.log(`${balayees} session(s) left open closed at startup`);
    }

    // THE PORT OPENS ONLY HERE, after the database and its migrations.
    let service: ServicePlateforme;
    try {
        service = await startServer(config, base);
    } catch (cause) {
        // The database is already open: close it again rather than leaving a
        // dangling connection behind an aborted startup.
        await base.fermer();
        throw cause;
    }

    return {
        port: service.port,
        base,
        async arreter() {
            await service.close();
            await base.fermer();
        },
    };
}
