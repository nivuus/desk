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

/// 🔴 `pg` REND LES `BIGINT` EN CHAÎNE, ET C'EST MESURÉ, PAS SUPPOSÉ.
///
/// Relevé par la recette du sous-bloc P3, le 19 août 2026 : `typeof` d'un
/// `vu_a` relu vaut `number` sous `node:sqlite` et `string` sous `pg`. La
/// raison est que le protocole de PostgreSQL rend un `int8` en texte et que
/// `pg` refuse par défaut de le convertir, un `int8` pouvant dépasser
/// l'entier sûr de JavaScript.
///
/// **Sans cette ligne, le défaut est de CLASSE et non d'instance** :
/// `LigneAgent.vu_a`, `LigneSession.ouverte_a` / `.fermee_a`,
/// `LigneUtilisateur.cree_a` et les deux colonnes de `LigneJeton` déclarent
/// toutes `number` une valeur qui est une `string` sur le moteur de
/// PRODUCTION. Le typage ne le voit pas — `interroger<T>` fait un `as T[]`,
/// donc l'affirmation est prise pour argent comptant.
///
/// ⚠️ CE QUE CE DÉFAUT NE FAISAIT PAS ÉCHOUER, et pourquoi c'est le pire cas :
/// `agents/fraicheur.ts::etatDe` survivait PAR ACCIDENT, sa soustraction
/// convertissant l'opérande. `depot/jeton.ts` s'en était tiré par un
/// `Number(...)` local et un type `number | string`. Rien ne rougissait, et
/// pourtant tout `+`, tout `===` et tout `>` aurait divergé selon le moteur.
///
/// La conversion LÈVE au-delà de `Number.MAX_SAFE_INTEGER` plutôt que
/// d'arrondir en silence : `Number('9007199254740993')` rend
/// `9007199254740992` sans le dire, et une seconde perdue sur un horodatage
/// serait exactement le genre de faute qu'aucun test ne rattraperait. Le
/// service n'écrit que des `Date.now()` (~1,8e12, soit quatre ordres de
/// grandeur sous la borne) : ce chemin n'est pas atteignable par lui, et il
/// est gardé quand même.
///
/// ⚠️ `setTypeParser` est GLOBAL AU PROCESSUS, et c'est déclaré : il n'existe
/// aucun autre consommateur de `pg` ici, et un réglage par `Pool` se
/// perdrait pour le pool d'administration que `base/harnais.ts` ouvre.
pg.types.setTypeParser(pg.types.builtins.INT8, (texte: string) => {
    const valeur = Number(texte);
    if (!Number.isSafeInteger(valeur)) {
        throw new Error(
            `BIGINT hors de l'entier sûr de JavaScript, converti nulle part : ${texte}`,
        );
    }
    return valeur;
});

/// `maxClients` borne le nombre de connexions que CE pilote garde ouvertes.
///
/// 🔴 IL EXISTE POUR LES TESTS, ET LE DÉFAUT EST CELUI DE `pg` (dix), qui est
/// le bon pour un SERVICE : une instance ouvre exactement UN pilote, pour
/// toute sa vie, et lui rogner sa concurrence n'aurait aucun sens.
///
/// ⚠️ MESURÉ LE 20 AOÛT 2026, ET CE N'EST PAS UNE PRÉCAUTION THÉORIQUE : la
/// suite ouvre CENT VINGT-HUIT bases (`grep -c 'baseNeuve('`), réparties sur
/// vingt-deux fichiers que vitest exécute EN PARALLÈLE, et chaque `baseNeuve`
/// ouvre DEUX pilotes (un d'administration, un de travail). À dix clients
/// chacun, dix bases concurrentes atteignent exactement le
/// `max_connections = 100` de l'instance de test. Le jour où P5 a ajouté ses
/// trois fichiers de test, l'instance a rendu
/// `FATAL: sorry, too many clients already` PUIS un backend a été
/// `terminated by signal 11: Segmentation fault` en pleine migration : la
/// suite entière est repassée en 181 échecs, sur un code parfaitement sain.
/// La cause était la SUITE, pas le service — mais une suite qui fait tomber
/// son instance ne mesure plus rien.
export function ouvrirPostgres(url: string, maxClients?: number): Pilote {
    const pool = new pg.Pool({
        connectionString: url,
        ...(maxClients === undefined ? {} : { max: maxClients }),
    });
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
