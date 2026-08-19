// L'interface que les deux moteurs présentent au reste du service, et la seule
// fonction PURE de la couche : la conversion des marqueurs `?` en `$1..$n`.
//
// Ce que cette couche échange (spec §3.2, §7.1) : une bibliothèque contre une
// discipline. Aucun compilateur ne vérifie une chaîne SQL écrite à la main.
// Les deux gardes qui remplacent le compilateur sont le lint statique des
// `.sql` et la double passe d'exécution contre les deux pilotes ; il est
// MESURÉ que ni l'un ni l'autre ne suffit seul.

export interface Pilote {
    executer(sql: string, params: unknown[]): Promise<{ lignes: number }>;
    interroger<T>(sql: string, params: unknown[]): Promise<T[]>;
    transaction<T>(corps: (p: Pilote) => Promise<T>): Promise<T>;
    fermer(): Promise<void>;
}

/// Convertit les marqueurs `?` en `$1..$n`. LÈVE si le SQL porte une chaîne
/// littérale : la conversion n'est sûre QUE sous la règle « toute valeur passe
/// en paramètre ».
///
/// 🔴 Ce refus n'est pas décoratif, et il n'est pas garanti par le moteur.
/// Mesuré le 19 août 2026 sur SQLite 3.50.4 : `SELECT '?' AS x` est du SQL
/// parfaitement VALIDE. Une conversion naïve le rendrait `SELECT '$1' AS x` et
/// changerait silencieusement le sens de la requête. Le refus est ce qui rend
/// la discipline MÉCANIQUE au lieu de documentaire.
///
/// Le coût est nommé : aucune migration, aucune requête de dépôt ne peut
/// porter de chaîne littérale — pas même une valeur par défaut. Le schéma v1
/// n'en contient aucune. Le jour où l'une devient nécessaire, c'est cette
/// décision qu'il faudra rouvrir, et non la contourner.
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
