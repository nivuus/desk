// The choice of driver, in a single place.
//
// ⚠️ An unknown value of `PLATEFORME_BASE` THROWS — never a silent
// fallback to sqlite. `lireConfig` already refuses it upstream; this second
// refusal exists for the day a caller builds a `Config` by
// hand, and because a `switch` with no default case would degrade to
// `undefined` without saying anything.

import type { Config } from '../config';
import type { Pilote } from './pilote';
import { ouvrirPostgres } from './pilote-postgres';
import { ouvrirSqlite } from './pilote-sqlite';

export async function ouvrirBase(config: Config): Promise<Pilote> {
    switch (config.base) {
        case 'sqlite':
            return ouvrirSqlite(config.urlBase);
        case 'postgres':
            return ouvrirPostgres(config.urlBase);
        default:
            throw new Error(
                `moteur de base inconnu : ${String(config.base)} — aucun repli n'est fait`,
            );
    }
}
