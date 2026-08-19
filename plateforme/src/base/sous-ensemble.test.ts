// Le LINT STATIQUE du sous-ensemble SQL portable.
//
// 🔴 Il n'est PAS redondant avec la double passe d'exécution, et le fait est
// MESURÉ. Le 19 août 2026, sur SQLite 3.50.4 :
//     AUTOINCREMENT sqlite : ACCEPTE
//     SERIAL sqlite        : ACCEPTE (type libre)
// SQLite accepte n'importe quel nom de type par affinité. `SERIAL` y passe
// donc sans bruit — et il passe aussi sur Postgres, où il SIGNIFIE AUTRE
// CHOSE. Deux passes vertes, deux schémas différents : le test d'exécution ne
// peut pas attraper `SERIAL`. Seul ce lint le peut.
//
// Inversement, ce lint ne peut rien contre une construction syntaxiquement
// licite des deux côtés mais de sémantique divergente — c'est le rôle de la
// double passe. Chacun couvre l'angle mort de l'autre.

import { readdirSync, readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import path from 'node:path';
import { describe, expect, it } from 'vitest';

const REPERTOIRE = path.join(path.dirname(fileURLToPath(import.meta.url)), 'migrations');

const INTERDITS: Array<[RegExp, string]> = [
    [/\bSERIAL\b/i, 'SERIAL : accepté par affinité sur SQLite, autre chose sur Postgres'],
    [/\bAUTOINCREMENT\b/i, 'AUTOINCREMENT : spécifique à SQLite'],
    [/\bdatetime\s*\(/i, 'datetime() : les horodatages sont écrits par l’application'],
    [/\bnow\s*\(/i, 'now() : les horodatages sont écrits par l’application'],
    [/\bCURRENT_TIMESTAMP\b/i, 'CURRENT_TIMESTAMP : idem'],
    [/\bUUID\b/i, 'UUID : type Postgres, absent de SQLite — les identifiants sont TEXT'],
    [/\bTIMESTAMPTZ\b/i, 'TIMESTAMPTZ : type Postgres'],
    [/\bJSONB\b/i, 'JSONB : type Postgres'],
    [/\bBOOLEAN\b/i, 'BOOLEAN : les booléens sont INTEGER 0/1'],
    // 🔴 MESURÉ, pas prudentiel. `INTEGER` vaut jusqu'à 8 octets sur SQLite et
    // exactement 4 sur Postgres : le 19 août 2026, sur PostgreSQL 16.15, un
    // `Date.now()` dans une colonne INTEGER rendait
    //     value "1787136773742" is out of range for type integer
    // et le service ne pouvait pas appliquer ses PROPRES migrations, tandis
    // que SQLite l'acceptait sans un mot.
    //
    // Ce que ce motif suppose, et qu'il faut savoir : la CONVENTION DE NOMMAGE
    // `_a` pour un horodatage (cree_a, vue_a, ouverte_a, fermee_a,
    // applique_a). Une colonne d'horodatage nommée autrement échapperait à ce
    // lint — il ne remplace donc pas le test de magnitude de
    // `pilotes.test.ts`, il en est le pendant lexical.
    [
        /\b\w+_a\s+INTEGER\b/i,
        'un horodatage `_a` en INTEGER : 4 octets sur Postgres, où un Date.now() déborde — BIGINT',
    ],
];

/// Retire les commentaires `--` AVANT tout contrôle.
///
/// ⚠️ Sans ce retrait, le lint serait rouge sur SA PROPRE DOCUMENTATION : les
/// commentaires SQL sont en français et portent des apostrophes, que le moteur
/// ne voit jamais. Ils nomment aussi les jetons interdits pour expliquer
/// pourquoi ils le sont.
function corps(texte: string): string {
    return texte.replace(/--.*$/gm, '');
}

const fichiers = readdirSync(REPERTOIRE).filter((f) => f.endsWith('.sql')).sort();

describe('sous-ensemble SQL portable', () => {
    it('lit au moins un fichier de migration', () => {
        // ⚠️ Sans cette assertion, un lint qui ne lit AUCUN fichier passerait
        // trivialement — et serait vert le jour où le répertoire serait
        // renommé. C'est le patron du « contrôle qui ne peut pas échouer »,
        // que ce dépôt a payé quatre fois.
        expect(fichiers.length).toBeGreaterThan(0);
    });

    for (const fichier of fichiers) {
        const sql = corps(readFileSync(path.join(REPERTOIRE, fichier), 'utf8'));

        it(`${fichier} n'emploie aucun jeton hors du sous-ensemble`, () => {
            const trouves = INTERDITS.filter(([motif]) => motif.test(sql)).map(([, raison]) => raison);
            // Comparé comme une CHAÎNE et non comme un tableau : vitest tronque
            // un tableau à `[ Array(1) ]`, message qui ne nomme pas le jeton
            // fautif — un diagnostic qui n'aide en rien celui qui le lira.
            expect(trouves.join(' | ')).toBe('');
        });

        it(`${fichier} ne porte aucune chaîne littérale`, () => {
            // Contrainte de `rendreMarqueurs` : toute valeur passe en
            // paramètre, pas même un DEFAULT littéral. Une apostrophe ici
            // ferait lever la conversion des marqueurs côté Postgres.
            expect(sql).not.toMatch(/['"]/);
        });
    }
});

describe('la définition dupliquée de schema_migration', () => {
    // ⚠️ `migrations.ts` recrée `schema_migration` en dur, parce que la
    // première migration a besoin de la table pour s'enregistrer. Le
    // commentaire de ce fichier-là affirme que les deux définitions sont
    // « à l'identique » — affirmation que RIEN ne vérifiait, et qui aurait
    // divergé en silence : une base créée par la ligne en dur n'aurait plus
    // ressemblé au schéma que le socle décrit.
    const source = readFileSync(
        path.join(path.dirname(fileURLToPath(import.meta.url)), 'migrations.ts'),
        'utf8',
    );
    const socle = readFileSync(path.join(REPERTOIRE, '0001-socle.sql'), 'utf8');

    /// Réduit une définition de table à `colonne type` séparés par des
    /// virgules, pour comparer la STRUCTURE et non la mise en page.
    function colonnes(ddl: string): string {
        const corps = /schema_migration\s*\(([^)]*)\)/i.exec(ddl);
        if (!corps) throw new Error(`aucune définition de schema_migration dans ce texte`);
        return corps[1]
            .split(',')
            .map((c) => c.replace(/\s+/g, ' ').trim().toUpperCase())
            .join(', ');
    }

    it('est la même dans migrations.ts et dans 0001-socle.sql', () => {
        expect(colonnes(source)).toBe(colonnes(corps(socle)));
    });

    it("porte bien l'horodatage en BIGINT", () => {
        // Sans cette seconde assertion, deux définitions FAUSSES et identiques
        // passeraient la première — c'est le contrôle qui ne peut pas échouer.
        expect(colonnes(source)).toContain('APPLIQUE_A BIGINT');
    });
});
