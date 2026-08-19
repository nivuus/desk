// Le pilote `pg` — celui de la production, et la seconde moitié de la double
// passe qui éprouve le sous-ensemble SQL portable.
//
// `pg` est purement JavaScript : la propriété « aucune dépendance native »
// (§4.2 du cadrage, écrit après le naufrage de `fuse-native`) tient.
//
// Chaque `executer`/`interroger` passe son SQL par `rendreMarqueurs` AVANT de
// l'envoyer : les requêtes du service sont écrites une seule fois, en style
// `?`, et c'est ce pilote-ci qui les traduit. Le refus des chaînes littérales
// que porte `rendreMarqueurs` est donc appliqué à toute requête qui passe par
// Postgres, pas seulement documenté.

import pg from 'pg';
import { type Pilote, rendreMarqueurs } from './pilote';

export function ouvrirPostgres(url: string): Pilote {
    const pool = new pg.Pool({ connectionString: url });
    return {
        async executer(sql, params) {
            const r = await pool.query(rendreMarqueurs(sql), params);
            return { lignes: r.rowCount ?? 0 };
        },
        async interroger<T>(sql: string, params: unknown[]): Promise<T[]> {
            const r = await pool.query(rendreMarqueurs(sql), params);
            return r.rows as T[];
        },
        async transaction<T>(corps: (p: Pilote) => Promise<T>): Promise<T> {
            // 🔴 LE PIÈGE DE `pg`, nommé plutôt que subi : un `BEGIN` émis sur
            // le POOL et un `COMMIT` émis ensuite sur le pool prendraient deux
            // clients DIFFÉRENTS, donc deux transactions différentes — et le
            // tout SILENCIEUSEMENT, sans erreur, la première restant ouverte
            // jusqu'à expiration. Le client est donc pris une fois et gardé
            // pour toute la durée du corps.
            const client = await pool.connect();
            try {
                await client.query('BEGIN');
                const valeur = await corps(surClient(client));
                await client.query('COMMIT');
                return valeur;
            } catch (cause) {
                await client.query('ROLLBACK');
                throw cause;
            } finally {
                client.release();
            }
        },
        async fermer() {
            await pool.end();
        },
    };
}

/// Un `Pilote` restreint au client déjà emprunté : c'est ce qui garantit que
/// tout le corps d'une transaction parle bien à la MÊME connexion.
function surClient(client: pg.PoolClient): Pilote {
    return {
        async executer(sql, params) {
            const r = await client.query(rendreMarqueurs(sql), params);
            return { lignes: r.rowCount ?? 0 };
        },
        async interroger<T>(sql: string, params: unknown[]): Promise<T[]> {
            const r = await client.query(rendreMarqueurs(sql), params);
            return r.rows as T[];
        },
        async transaction<T>(corps: (p: Pilote) => Promise<T>): Promise<T> {
            // Pas de transaction imbriquée en P1 : `SAVEPOINT` serait un
            // mécanisme de plus à éprouver des deux côtés, et rien ne
            // l'emploie. Refus explicite plutôt qu'un `BEGIN` imbriqué que
            // Postgres accepterait en avertissant, et SQLite en échouant.
            throw new Error('transaction imbriquée non prise en charge');
        },
        async fermer() {
            throw new Error('un pilote de transaction ne se ferme pas : il est relâché');
        },
    };
}
