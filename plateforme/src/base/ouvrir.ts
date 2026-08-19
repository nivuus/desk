// Le choix du pilote, en un seul endroit.
//
// ⚠️ Une valeur inconnue de `PLATEFORME_BASE` LÈVE — jamais de repli
// silencieux sur sqlite. `lireConfig` la refuse déjà en amont ; ce second
// refus existe pour le jour où un appelant construirait une `Config` à la
// main, et parce qu'un `switch` sans cas par défaut se dégraderait en
// `undefined` sans rien dire.

import type { Config } from '../config';
import type { Pilote } from './pilote';
import { ouvrirSqlite } from './pilote-sqlite';

export async function ouvrirBase(config: Config): Promise<Pilote> {
    switch (config.base) {
        case 'sqlite':
            return ouvrirSqlite(config.urlBase);
        default:
            throw new Error(
                `moteur de base inconnu : ${String(config.base)} — aucun repli n'est fait`,
            );
    }
}
