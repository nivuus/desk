// Le lanceur de migrations : transactionnel, ordonné NUMÉRIQUEMENT, idempotent.
//
// Spec §6 : une migration en échec annule sa transaction, arrête le service et
// laisse la version inchangée. Jamais de schéma à moitié appliqué — un schéma
// partiel serait indiscernable d'un schéma correct au démarrage suivant, et
// c'est la classe de panne muette contre laquelle tout ce dépôt est écrit.

import { readdirSync, readFileSync } from 'node:fs';
import path from 'node:path';
import type { Pilote } from './pilote';

interface Migration {
    version: number;
    fichier: string;
    sql: string;
}

/// 🔴 Le tri est NUMÉRIQUE, jamais lexicographique.
///
/// Un tri de chaînes place `0010` avant `0002` : la dixième migration
/// s'appliquerait avant la deuxième, et le schéma serait faux sans qu'aucune
/// erreur ne le dise — chaque fichier serait pourtant bien exécuté, et
/// `schema_migration` bien renseignée. C'est une panne muette différée de neuf
/// migrations, et c'est pour cela que le tri est explicite ici.
function lire(repertoire: string): Migration[] {
    return readdirSync(repertoire)
        .filter((f) => f.endsWith('.sql'))
        .map((fichier) => {
            const tete = /^(\d+)/.exec(fichier);
            if (!tete) {
                throw new Error(
                    `migration sans numéro de version en tête : ${fichier} — le nom doit commencer par des chiffres`,
                );
            }
            return {
                version: Number(tete[1]),
                fichier,
                sql: readFileSync(path.join(repertoire, fichier), 'utf8'),
            };
        })
        .sort((a, b) => a.version - b.version);
}

/// Découpe un fichier en instructions.
///
/// Les commentaires `--` sont retirés d'abord : ils portent des apostrophes de
/// français, et une instruction vide ferait échouer certains moteurs.
function instructions(sql: string): string[] {
    return sql
        .replace(/--.*$/gm, '')
        .split(';')
        .map((s) => s.trim())
        .filter((s) => s.length > 0);
}

/// Applique les migrations non encore appliquées et rend leur NOMBRE.
///
/// `maintenant` est un paramètre, jamais `Date.now()` lu ici : c'est la règle
/// du sous-ensemble (les horodatages sont toujours écrits par l'application),
/// et c'est ce qui rend le résultat vérifiable sur une valeur exacte.
export async function appliquerMigrations(
    p: Pilote,
    repertoire: string,
    maintenant: number,
): Promise<number> {
    // La table de suivi doit exister avant qu'on puisse la lire. `IF NOT
    // EXISTS` la rend idempotente ; sa définition est répétée ici et dans
    // `0001-socle.sql`, à l'identique, parce qu'aucune des deux ne peut
    // s'appuyer sur l'autre : la première migration a besoin de la table pour
    // s'enregistrer.
    await p.executer(
        'CREATE TABLE IF NOT EXISTS schema_migration (version INTEGER PRIMARY KEY, applique_a INTEGER NOT NULL)',
        [],
    );

    const deja = await p.interroger<{ version: number }>(
        'SELECT version FROM schema_migration',
        [],
    );
    const appliquees = new Set(deja.map((l) => Number(l.version)));

    let compte = 0;
    for (const migration of lire(repertoire)) {
        if (appliquees.has(migration.version)) continue;
        await p.transaction(async (tx) => {
            for (const instruction of instructions(migration.sql)) {
                // `schema_migration` est déjà créée ci-dessus : une seconde
                // création dans la même transaction échouerait. Le socle la
                // déclare pour un lecteur du schéma, pas pour l'exécution.
                if (/^CREATE\s+TABLE\s+schema_migration\b/i.test(instruction)) continue;
                await tx.executer(instruction, []);
            }
            await tx.executer(
                'INSERT INTO schema_migration(version, applique_a) VALUES(?, ?)',
                [migration.version, maintenant],
            );
        });
        compte += 1;
    }
    return compte;
}

/// Le répertoire des migrations, résolu depuis ce module — pour qu'aucun
/// appelant n'ait à connaître l'arborescence.
export const REPERTOIRE_MIGRATIONS = path.join(
    path.dirname(new URL(import.meta.url).pathname),
    'migrations',
);
