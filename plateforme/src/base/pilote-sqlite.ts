// Le pilote `node:sqlite` — celui du développement et de la suite de tests.
//
// 🔴 L'`ExperimentalWarning` de `node:sqlite` N'EST PAS MASQUÉ, nulle part :
// ni `NODE_NO_WARNINGS`, ni `--no-warnings`, ni `--disable-warning`, ni dans
// `package.json`, ni dans la configuration vitest, ni dans `verify-all.sh`.
// La spec §3.2 accepte ce module PARCE QUE « l'échec d'une évolution d'API est
// bruyant et immédiat » : l'éteindre retirerait exactement le bruit qui
// justifie la décision. Ce que rend la sortie lisible n'est pas la
// suppression, c'est la RARETÉ — `node:sqlite` n'est importé QUE par ce
// fichier, donc l'avertissement paraît une fois par processus, sur deux
// lignes. Un contrôle de `pilote.test.ts` interdit de le masquer plus tard.
//
// ⚠️ `DatabaseSync` est SYNCHRONE ; l'interface `Pilote` est asynchrone. Les
// méthodes ci-dessous rendent donc des promesses DÉJÀ RÉSOLUES. Il n'y a ici
// aucune concurrence, aucun parallélisme, aucune attente réelle : un
// successeur qui croirait pouvoir lancer deux requêtes de front sur ce pilote
// se tromperait. L'asynchronie est celle de l'interface, pas celle du moteur.

import { DatabaseSync } from 'node:sqlite';
import type { Pilote } from './pilote';

export function ouvrirSqlite(cheminOuMemoire: string): Pilote {
    const base = new DatabaseSync(cheminOuMemoire);
    // Sans ce pragma, SQLite N'APPLIQUE PAS les clés étrangères : elles sont
    // acceptées à la déclaration et ignorées à l'exécution. La portabilité du
    // schéma serait alors éprouvée d'un seul côté — Postgres les applique,
    // lui, sans qu'on ait rien à demander.
    base.exec('PRAGMA foreign_keys = ON');
    return pilote(base);
}

function pilote(base: DatabaseSync): Pilote {
    return {
        async executer(sql, params) {
            const r = base.prepare(sql).run(...(params as never[]));
            return { lignes: Number(r.changes) };
        },
        async interroger<T>(sql: string, params: unknown[]): Promise<T[]> {
            return base.prepare(sql).all(...(params as never[])) as T[];
        },
        async transaction<T>(corps: (p: Pilote) => Promise<T>): Promise<T> {
            base.exec('BEGIN');
            try {
                const valeur = await corps(pilote(base));
                base.exec('COMMIT');
                return valeur;
            } catch (cause) {
                base.exec('ROLLBACK');
                throw cause;
            }
        },
        async fermer() {
            base.close();
        },
    };
}
