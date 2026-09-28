// The interface that the two engines present to the rest of the service, and the only
// PURE function of the layer: the conversion of `?` markers into `$1..$n`.
//
// What this layer trades (spec §3.2, §7.1): a library for a
// discipline. No compiler checks a hand-written SQL string.
// The two guards that replace the compiler are the static lint of the
// `.sql` files and the double execution pass against the two drivers; it is
// MEASURED that neither one is enough on its own.

export interface Pilote {
    executer(sql: string, params: unknown[]): Promise<{ lignes: number }>;
    interroger<T>(sql: string, params: unknown[]): Promise<T[]>;
    transaction<T>(corps: (p: Pilote) => Promise<T>): Promise<T>;
    fermer(): Promise<void>;
}

/// Converts `?` markers into `$1..$n`. THROWS if the SQL carries a string
/// literal: the conversion is safe ONLY under the rule "every value goes
/// in as a parameter".
///
/// 🔴 This refusal is not decorative, and it is not guaranteed by the engine.
/// Measured on 19 August 2026 on SQLite 3.50.4: `SELECT '?' AS x` is perfectly
/// VALID SQL. A naive conversion would turn it into `SELECT '$1' AS x` and
/// would silently change the meaning of the query. The refusal is what makes
/// the discipline MECHANICAL instead of documentary.
///
/// The cost is named: no migration, no repository query can
/// carry a string literal — not even a default value. The v1 schema
/// contains none. The day one becomes necessary, it is this
/// decision that will have to be reopened, not worked around.
export function rendreMarqueurs(sql: string): string {
    if (/['"]/.test(sql)) {
        throw new Error(
            "SQL refusé : il porte une chaîne littérale (apostrophe ou guillemet). " +
                'Toute valeur passe en paramètre, sans exception — sans quoi la ' +
                "conversion des marqueurs changerait le sens de la requête. SQL : " +
                sql,
        );
    }
    let n = 0;
    return sql.replace(/\?/g, () => `$${++n}`);
}
