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
            expect(trouves).toEqual([]);
        });

        it(`${fichier} ne porte aucune chaîne littérale`, () => {
            // Contrainte de `rendreMarqueurs` : toute valeur passe en
            // paramètre, pas même un DEFAULT littéral. Une apostrophe ici
            // ferait lever la conversion des marqueurs côté Postgres.
            expect(sql).not.toMatch(/['"]/);
        });
    }
});
