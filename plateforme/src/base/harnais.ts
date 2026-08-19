// Le harnais de la double passe : il ouvre le pilote que `PLATEFORME_BASE`
// désigne, sur une base NEUVE, et applique les migrations.
//
// 🔴 UN SAUT EST UN ÉCHEC (spec §7.1). Si `PLATEFORME_BASE=postgres` et que
// l'instance est injoignable, les tests ÉCHOUENT avec la cause. Il n'y a ici
// ni `it.skipIf`, ni `describe.skip`, ni `if (!disponible) return`. Ce dépôt a
// payé plusieurs fois pour un contrôle qui ne pouvait pas échouer ; un test qui
// DISPARAÎT quand sa dépendance manque est la même erreur sous une autre
// forme — il rend vert un état qu'il n'a pas mesuré.

import type { Pilote } from './pilote';
import { ouvrirPostgres } from './pilote-postgres';
import { ouvrirSqlite } from './pilote-sqlite';
import { appliquerMigrations, REPERTOIRE_MIGRATIONS } from './migrations';

export const MOTEUR = (process.env.PLATEFORME_BASE ?? 'sqlite') as 'sqlite' | 'postgres';

/// URL de l'instance de test, celle de `docker-compose.plateforme.yml`.
/// Surchargeable par `PLATEFORME_BASE_URL` pour viser une autre instance.
const URL_POSTGRES =
    process.env.PLATEFORME_BASE_URL ??
    'postgres://plateforme:plateforme-test@127.0.0.1:5433/plateforme_test';

/// L'instant auquel les migrations de test sont appliquées.
///
/// 🔴 C'est une valeur de la MAGNITUDE D'UNE ÉPOQUE EN MILLISECONDES, et non
/// un petit nombre commode, parce qu'un petit nombre ne mesure rien.
/// `1_000` tenait dans un entier 4 octets ; `Date.now()` n'y tient pas. Le
/// service de production n'écrit que des `Date.now()` : une suite qui n'écrit
/// que des `1_000` déclare portable un schéma qui refuse toute écriture réelle
/// sur l'un des deux moteurs, sans qu'aucun test ne rougisse.
export const INSTANT_MIGRATION = 1_700_000_000_000;

/// Ouvre une base VIERGE et y applique les migrations.
///
/// SQLite : une base en mémoire, donc neuve par construction.
/// Postgres : un SCHÉMA jetable, propre à l'appel, posé en tête du
/// `search_path`. C'est ce qui permet à plusieurs fichiers de test de tourner
/// de front sans se marcher dessus, là où un `DROP SCHEMA public` global les
/// ferait s'entre-détruire.
export async function baseNeuve(nom: string): Promise<Pilote> {
    if (MOTEUR === 'sqlite') {
        const p = ouvrirSqlite(':memory:');
        await appliquerMigrations(p, REPERTOIRE_MIGRATIONS, INSTANT_MIGRATION);
        return p;
    }

    const schema = `t_${nom.replace(/[^a-z0-9]/gi, '_')}_${process.pid}_${compteur()}`;
    // Le schéma est interpolé, jamais paramétré : un identifiant SQL ne peut
    // pas être un paramètre de requête, sur aucun moteur. Il est construit
    // ici, à partir de valeurs que seul ce fichier fournit, et filtré sur
    // [a-z0-9_] — aucune entrée extérieure n'y arrive.
    const admin = ouvrirPostgres(URL_POSTGRES);
    await admin.executer(`DROP SCHEMA IF EXISTS ${schema} CASCADE`, []);
    await admin.executer(`CREATE SCHEMA ${schema}`, []);
    await admin.fermer();

    const p = ouvrirPostgres(`${URL_POSTGRES}?options=-c%20search_path%3D${schema}`);
    await appliquerMigrations(p, REPERTOIRE_MIGRATIONS, INSTANT_MIGRATION);
    return p;
}

let n = 0;
function compteur(): number {
    return ++n;
}
