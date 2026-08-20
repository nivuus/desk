// Le DISCRIMINANT du critère ① de P5 (décision D15).
//
// 🔴 POURQUOI IL EXISTE. Les deux instances Postgres du dépôt — celle de TEST
// (127.0.0.1:5433) et celle de DÉPLOIEMENT (127.0.0.1:5434) — portent la MÊME
// image `postgres:16-alpine`, à dessein : le mot « diffère » de l'énoncé du
// critère doit porter sur la CONFIGURATION, pas sur le moteur. Conséquence
// directe : une suite verte ne dit pas LAQUELLE elle a touchée. Ce programme
// le dit.
//
// 🔴 IL PASSE PAR LE PILOTE DU SERVICE, jamais par `psql`. Un `psql` mesurerait
// que la base répond ; celui-ci mesure que LE CHEMIN QUE LA SUITE EMPRUNTE
// aboutit sur cette instance-là.
//
// 🔴 ET IL NE SE CONTENTE PAS DE CE QUE D15 PRESCRIVAIT, PARCE QUE LA MESURE A
// MONTRÉ QUE CELA NE DISCRIMINE PAS. La décision D15 du plan nomme
// `SELECT current_database(), inet_server_port(), version()`. Mesuré le
// 20 août 2026 : `inet_server_port()` rend **5432 des DEUX côtés** — c'est le
// port du serveur DANS son conteneur, jamais le port publié sur l'hôte (5433
// ou 5434). Et `version()` est identique mot pour mot, les deux instances
// portant la même image, ce qui est justement voulu. Le seul champ prescrit
// qui distingue est `current_database()`, et il ne distingue que parce que les
// deux bases ont reçu des NOMS différents — une convention, pas une identité.
//
// Deux champs sont donc AJOUTÉS, et ce sont eux qui portent le critère :
//
//   `pg_control_system().system_identifier` — l'identité de la GRAPPE, posée
//   par `initdb` et unique par instance. Rien ne la falsifie sans refaire la
//   grappe. C'est le discriminant fort.
//
//   `pg_postmaster_start_time()` — corrobore : les deux instances n'ont pas
//   été démarrées au même instant.
//
// ⚠️ `system_identifier` EST COULÉ EN `text`, ET C'EST LE PILOTE DU SERVICE QUI
// L'EXIGE : c'est un entier 64 bits non signé, et `base/pilote-postgres.ts`
// REFUSE tout BIGINT hors de l'entier sûr de JavaScript plutôt que de le
// convertir en silence — refus observé le 20 août 2026 sur la valeur
// 7676031821417750562. C'est une corroboration au passage : ce programme
// emprunte bien le chemin du service, garde comprise, et non un `psql`.
//
// ⚠️ IL N'IMPRIME JAMAIS LE MOT DE PASSE. L'URL est rendue avec sa partie
// secrète remplacée — même règle que `securite/secrets.test.ts`, et pour la
// même raison : ce journal est versionné.
import { ouvrirPostgres } from '../../../../../../plateforme/src/base/pilote-postgres.ts';

/// Remplace le mot de passe d'une URL `postgres://user:motdepasse@hôte/...`.
/// ⚠️ Elle ne masque QUE ce champ : une URL portant un secret ailleurs (une
/// option de connexion, par exemple) ne serait pas couverte. Aucune de celles
/// employées ici n'en porte.
function masquer(url: string): string {
    return url.replace(/(postgres:\/\/[^:@/]+:)[^@]*(@)/, '$1***MASQUE***$2');
}

const url = process.env.PLATEFORME_BASE_URL;
if (!url) throw new Error('PLATEFORME_BASE_URL manquante : le discriminant ne devine pas la cible.');
const etiquette = process.argv[2] ?? '(sans étiquette)';

const pilote = ouvrirPostgres(url, 1);
try {
    const lignes = await pilote.interroger<{
        current_database: string;
        inet_server_port: number;
        version: string;
        system_identifier: string;
        demarre_a: Date;
    }>(
        'SELECT current_database(), inet_server_port(), version(), ' +
            'CAST((SELECT system_identifier FROM pg_control_system()) AS text) AS system_identifier, ' +
            'pg_postmaster_start_time() AS demarre_a',
        [],
    );
    const l = lignes[0]!;
    console.log(`DISCRIMINANT [${etiquette}]`);
    console.log(`  url visee           = ${masquer(url)}`);
    console.log(`  base                = ${l.current_database}`);
    console.log(`  identite de GRAPPE  = ${l.system_identifier}   <- LE discriminant`);
    console.log(`  demarree a          = ${new Date(l.demarre_a).toISOString()}`);
    console.log(`  inet_server_port()  = ${l.inet_server_port}   <- NE DISCRIMINE PAS (port interne au conteneur)`);
    console.log(`  version             = ${l.version}   <- NE DISCRIMINE PAS (meme image)`);
} finally {
    await pilote.fermer();
}
