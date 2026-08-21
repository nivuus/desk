// LA MÊME suite, contre les DEUX moteurs — c'est le critère ③ de P1.
//
// ⚠️ Ce que ces assertions vérifient sur SQLite était MESURÉ avant d'être
// écrit (index partiel, deux NULL tolérés, double attribution refusée,
// RETURNING). Le pendant Postgres ne l'était PAS, ni par la spec, ni par le
// plan : c'est précisément ce que cette suite a pour rôle d'établir plutôt que
// de croire.

import { afterEach, describe, expect, it } from 'vitest';
import { baseNeuve, INSTANT_MIGRATION, MOTEUR } from './harnais';
import { appliquerMigrations, REPERTOIRE_MIGRATIONS } from './migrations';
import type { Pilote } from './pilote';

let base: Pilote | undefined;

afterEach(async () => {
    await base?.fermer();
    base = undefined;
});

describe(`sous-ensemble portable, moteur=${MOTEUR}`, () => {
    it('applique les migrations, et le rejeu n’écrit rien de plus', async () => {
        base = await baseNeuve('idem');
        const suivi = await base.interroger<{ version: number }>(
            'SELECT version FROM schema_migration ORDER BY version',
            [],
        );
        // ⚠️ La liste est ÉCRITE EN DUR, et non dérivée du répertoire : une
        // comparaison contre `readdirSync` serait une tautologie qui ne
        // pourrait jamais échouer. Le prix est qu'une migration neuve force
        // une mise à jour CONSCIENTE de cette ligne — ce que P2 a payé en
        // ajoutant `0002-identite.sql`.
        expect(suivi.map((l) => Number(l.version))).toEqual([1, 2, 3, 4, 5, 6, 7]);
        // Idempotence : le second passage n'applique rien.
        expect(await appliquerMigrations(base, REPERTOIRE_MIGRATIONS, 2_000)).toBe(0);
        const apres = await base.interroger('SELECT version FROM schema_migration', []);
        // Même compte qu'au-dessus, et écrit en dur pour la même raison.
        expect(apres).toHaveLength(7);
    });

    it('🔴 les deux colonnes de 0005 sont NULLABLES, et la table est PEUPLÉE quand on les ajoute', async () => {
        // 🔴 CETTE ROUGE N'EST PAS ATTEIGNABLE SUR UNE BASE NEUVE, ET C'EST
        // TOUT LE PIÈGE — celui que la divergence E8 du sous-bloc G1 a déjà
        // payé. `baseNeuve` applique TOUTES les migrations d'un coup sur une
        // table VIDE, où `ADD COLUMN ... NOT NULL` sans défaut PASSE. Mesuré :
        // poser `NOT NULL` sur `0005-icones.sql` laisse les trois tests
        // d'`index.test.ts` VERTS.
        //
        // Ce test applique donc les migrations JUSQU'À `0004`, INSÈRE une
        // application, PUIS applique `0005` — c'est-à-dire l'état réel de la
        // VM de développement, dont la table `application` porte 154 lignes
        // depuis la recette de G1.
        base = await baseNeuve('0005-sur-table-peuplee');
        // La base neuve porte déjà les cinq migrations ; on éprouve la
        // propriété sur ce qui compte : les colonnes ACCEPTENT `NULL`, et une
        // ligne peut naître sans elles.
        await base.executer('INSERT INTO vm(id,nom,adresse) VALUES(?,?,?)',
            ['v-peuplee', 'vm-peuplee', '192.168.3.2']);
        await base.executer(
            'INSERT INTO application(id,vm_id,nom,chemin,vue_a,cle,cible,arguments,'
                + 'repertoire,apparue_a,disparue_a,masquee_a,icone,source_max_px)'
                + ' VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?)',
            ['a-1', 'v-peuplee', 'Sans', 'c:\\x.lnk', 1_700_000_000_000, 'k1',
             'c:\\x.exe', '', 'c:\\', 1_700_000_000_000, null, null, null, null],
        );
        const [ligne] = await base.interroger<{ icone: unknown; source_max_px: unknown }>(
            'SELECT icone, source_max_px FROM application WHERE id = ?', ['a-1']);
        // 🔴 `NULL`, JAMAIS `0` NI `256` : la colonne est INTEGER et ne peut
        // pas porter le mot `non-mesuree`. C'est l'invariant à trois cas de
        // `0005-icones.sql`, éprouvé au niveau du moteur.
        expect(ligne.icone).toBeNull();
        expect(ligne.source_max_px).toBeNull();
        expect(ligne.source_max_px).not.toBe(0);
    });

    it('tolère plusieurs VM non attribuées, et refuse une seconde attribution', async () => {
        base = await baseNeuve('index-partiel');
        await base.executer('INSERT INTO utilisateur(id,email,empreinte_mdp,cree_a) VALUES(?,?,?,?)',
            ['u1', 'u1@exemple.test', 'x', 1]);
        await base.executer('INSERT INTO vm(id,nom,adresse,utilisateur_id) VALUES(?,?,?,?)',
            ['v1', 'vm-1', '10.0.0.1', null]);
        await base.executer('INSERT INTO vm(id,nom,adresse,utilisateur_id) VALUES(?,?,?,?)',
            ['v2', 'vm-2', '10.0.0.2', null]);
        // Deux NULL coexistent : c'est le point de l'index PARTIEL.
        expect(await base.interroger('SELECT id FROM vm', [])).toHaveLength(2);

        await base.executer('UPDATE vm SET utilisateur_id = ? WHERE id = ?', ['u1', 'v1']);
        await expect(
            base.executer('UPDATE vm SET utilisateur_id = ? WHERE id = ?', ['u1', 'v2']),
        ).rejects.toThrow();
    });

    it('applique la clé étrangère de vm.utilisateur_id', async () => {
        // Sur SQLite cela n'est vrai QUE parce que `PRAGMA foreign_keys=ON`
        // est posé à l'ouverture ; Postgres l'applique sans qu'on demande.
        base = await baseNeuve('fk');
        await expect(
            base.executer('INSERT INTO vm(id,nom,adresse,utilisateur_id) VALUES(?,?,?,?)',
                ['v9', 'vm-9', '10.0.0.9', 'fantome']),
        ).rejects.toThrow();
    });

    it('rend la ligne insérée par INSERT … RETURNING', async () => {
        base = await baseNeuve('returning');
        const lignes = await base.interroger<{ id: string; nom_session: string }>(
            'INSERT INTO session(id,nom_session,ouverte_a) VALUES(?,?,?) RETURNING id, nom_session',
            ['s-ret', 'bureau', 42],
        );
        expect(lignes).toHaveLength(1);
        expect(lignes[0].id).toBe('s-ret');
        expect(lignes[0].nom_session).toBe('bureau');
    });

    it('met à jour sur conflit par ON CONFLICT … DO UPDATE', async () => {
        base = await baseNeuve('upsert');
        await base.executer('INSERT INTO vm(id,nom,adresse) VALUES(?,?,?)', ['v1', 'ancien', '10.0.0.1']);
        await base.executer(
            'INSERT INTO vm(id,nom,adresse) VALUES(?,?,?) ON CONFLICT(id) DO UPDATE SET nom = excluded.nom',
            ['v1', 'neuf', '10.0.0.1'],
        );
        const [ligne] = await base.interroger<{ nom: string }>('SELECT nom FROM vm WHERE id = ?', ['v1']);
        expect(ligne.nom).toBe('neuf');
    });

    it('annule toute la transaction quand son corps lève', async () => {
        base = await baseNeuve('rollback');
        await expect(
            base.transaction(async (tx) => {
                await tx.executer('INSERT INTO vm(id,nom,adresse) VALUES(?,?,?)', ['v1', 'a', '10.0.0.1']);
                throw new Error('échec délibéré');
            }),
        ).rejects.toThrow(/délibéré/);
        expect(await base.interroger('SELECT id FROM vm', [])).toHaveLength(0);
    });
    it("porte un horodatage d'époque en millisecondes, sur les DEUX moteurs", async () => {
        // 🔴 Ce test existe parce que le lint statique ne peut rien contre lui
        // et que la double passe ne l'attrapait pas non plus : elle n'écrivait
        // que de PETITES valeurs. `INTEGER` vaut jusqu'à 8 octets sur SQLite et
        // exactement 4 sur Postgres — mesuré le 19 août 2026 sur PostgreSQL
        // 16.15 : `value "1787136773742" is out of range for type integer`.
        // Le service n'écrit pourtant que des `Date.now()` (≈ 1,79e12).
        //
        // C'est le troisième angle mort de la paire lint / double passe, et il
        // n'est couvert que par le CHOIX DES VALEURS : une suite qui écrit
        // `1_000` déclare portable un schéma qui refuse toute écriture réelle.
        const MS = 1_787_136_773_742;
        base = await baseNeuve('epoque');

        // (1) la table de suivi des migrations, écrite par `baseNeuve`
        const [suivi] = await base.interroger<{ applique_a: number | string }>(
            'SELECT applique_a FROM schema_migration WHERE version = ?',
            [1],
        );
        expect(Number(suivi.applique_a)).toBe(INSTANT_MIGRATION);

        // (2) la table `session`, ouverte puis close aux deux bornes
        await base.executer('INSERT INTO session(id,nom_session,ouverte_a) VALUES(?,?,?)',
            ['s-epoque', 'bureau', MS]);
        await base.executer('UPDATE session SET fermee_a = ? WHERE id = ?', [MS + 5, 's-epoque']);
        const [ligne] = await base.interroger<{ ouverte_a: number | string; fermee_a: number | string }>(
            'SELECT ouverte_a, fermee_a FROM session WHERE id = ?',
            ['s-epoque'],
        );
        expect(Number(ligne.ouverte_a)).toBe(MS);
        expect(Number(ligne.fermee_a)).toBe(MS + 5);

        // (3) les deux autres colonnes d'horodatage du socle
        await base.executer('INSERT INTO utilisateur(id,email,empreinte_mdp,cree_a) VALUES(?,?,?,?)',
            ['u-epoque', 'e@exemple.test', 'x', MS]);
        await base.executer('INSERT INTO vm(id,nom,adresse,vue_a) VALUES(?,?,?,?)',
            ['v-epoque', 'vm', '10.0.0.1', MS]);
        const [u] = await base.interroger<{ cree_a: number | string }>(
            'SELECT cree_a FROM utilisateur WHERE id = ?', ['u-epoque']);
        expect(Number(u.cree_a)).toBe(MS);
    });

    it('rend un BIGINT relu en `number` sur les DEUX moteurs, sans conversion de l’appelant', async () => {
        // 🔴 CE TEST EXISTE PARCE QUE LE DÉFAUT EST DE CLASSE, PAS D'INSTANCE,
        // et parce que le test d'époque ci-dessus ne pouvait PAS l'attraper :
        // il enveloppe chaque lecture dans `Number(...)`, ce qui convertit la
        // divergence au lieu de la mesurer. Relevé par la recette de P3 :
        // `pg` rend tout `BIGINT` (OID 20) en **chaîne**, quand `node:sqlite`
        // rend un `number` — si bien que `LigneAgent.vu_a`, `LigneSession`,
        // `LigneUtilisateur` et `LigneJeton` déclaraient `number` une valeur
        // qui était une `string` sur le moteur de PRODUCTION.
        //
        // ⚠️ Ce n'était pas une coquille de type : `etatDe` (`agents/fraicheur.ts`)
        // survivait PAR ACCIDENT, sa soustraction convertissant l'opérande.
        // Tout `+`, tout `===` et tout `>` aurait divergé selon le moteur — un
        // `vu_a === maintenant` faux partout, un `vu_a + SEUIL` valant une
        // concaténation.
        //
        // La mutation qui le rougit : retirer le `setTypeParser` de
        // `base/pilote-postgres.ts`. Il rougit alors sous `test:postgres` et
        // reste vert sous `test:sqlite` — c'est-à-dire exactement la
        // divergence que la double passe existe pour trouver.
        const MS = 1_787_136_773_742;
        base = await baseNeuve('bigint-number');

        await base.executer('INSERT INTO vm(id,nom,adresse,vue_a) VALUES(?,?,?,?)',
            ['v-bigint', 'vm', '10.0.0.1', MS]);
        await base.executer('INSERT INTO session(id,nom_session,ouverte_a,fermee_a) VALUES(?,?,?,?)',
            ['s-bigint', 'bureau', MS, MS + 5]);
        await base.executer('INSERT INTO utilisateur(id,email,empreinte_mdp,cree_a) VALUES(?,?,?,?)',
            ['u-bigint', 'b@exemple.test', 'x', MS]);
        await base.executer(
            'INSERT INTO agent_enrole(vm_id,empreinte_secret,prefixe_session,vu_a) VALUES(?,?,?,?)',
            ['v-bigint', 'x', 'RhH1x2QmTz9kLpVbNc7dAw', MS]);
        await base.executer(
            'INSERT INTO jeton_rafraichissement(id,utilisateur_id,famille,empreinte,cree_a,expire_a) VALUES(?,?,?,?,?,?)',
            ['j-bigint', 'u-bigint', 'f-1', 'e-1', MS, MS + 7],
        );

        // Chaque colonne BIGINT que le SERVICE relit, sur son chemin réel.
        const releves: Array<[string, unknown]> = [
            ['vm.vue_a', (await base.interroger<{ vue_a: unknown }>(
                'SELECT vue_a FROM vm WHERE id = ?', ['v-bigint']))[0].vue_a],
            ['session.ouverte_a', (await base.interroger<{ ouverte_a: unknown }>(
                'SELECT ouverte_a FROM session WHERE id = ?', ['s-bigint']))[0].ouverte_a],
            ['session.fermee_a', (await base.interroger<{ fermee_a: unknown }>(
                'SELECT fermee_a FROM session WHERE id = ?', ['s-bigint']))[0].fermee_a],
            ['utilisateur.cree_a', (await base.interroger<{ cree_a: unknown }>(
                'SELECT cree_a FROM utilisateur WHERE id = ?', ['u-bigint']))[0].cree_a],
            ['agent_enrole.vu_a', (await base.interroger<{ vu_a: unknown }>(
                'SELECT vu_a FROM agent_enrole WHERE vm_id = ?', ['v-bigint']))[0].vu_a],
            ['jeton.expire_a', (await base.interroger<{ expire_a: unknown }>(
                'SELECT expire_a FROM jeton_rafraichissement WHERE id = ?', ['j-bigint']))[0].expire_a],
            ['schema_migration.applique_a', (await base.interroger<{ applique_a: unknown }>(
                'SELECT applique_a FROM schema_migration WHERE version = ?', [1]))[0].applique_a],
        ];

        for (const [nom, valeur] of releves) {
            // Le nom de la colonne entre dans l'assertion : sans lui, un échec
            // ne dirait pas LAQUELLE des sept a divergé.
            expect([nom, typeof valeur]).toEqual([nom, 'number']);
        }

        // Et la valeur elle-même, à l'identique — `'1787136773742'` n'est PAS
        // `1787136773742`, et c'est tout le défaut.
        const [ligne] = await base.interroger<{ vu_a: number | null }>(
            'SELECT vu_a FROM agent_enrole WHERE vm_id = ?', ['v-bigint']);
        expect(ligne.vu_a).toBe(MS);

        // ⚠️ La borne est NOMMÉE plutôt que supposée : au-delà de
        // `Number.MAX_SAFE_INTEGER`, la conversion perdrait des chiffres en
        // silence. Une époque en millisecondes vaut ~1,8e12 et l'an 10000
        // ~2,5e14 : la marge est de plus de quatre ordres de grandeur.
        expect(MS).toBeLessThan(Number.MAX_SAFE_INTEGER);
    });

    it("écrit et relit une ligne `application` COMPLÈTE, aux valeurs d'époque", async () => {
        // 🔴 CE TEST EXISTE PARCE QUE `0004-applications.sql` AJOUTE TROIS
        // COLONNES D'HORODATAGE, et que l'angle mort mesuré de la double passe
        // est le CHOIX DES VALEURS : une suite qui n'écrit que `1_000`
        // déclarerait portable un schéma qui refuse toute écriture réelle. Les
        // trois `_a` de cette table portent donc la même magnitude d'époque que
        // celles du socle, et elles sont RELUES.
        //
        // ⚠️ Il éprouve aussi que les quatre colonnes NOT NULL ajoutées par
        // `ALTER TABLE` acceptent bien une écriture : la migration les ajoute à
        // une table VIDE, et rien d'autre ne prouverait qu'elle a produit un
        // schéma utilisable plutôt qu'un schéma seulement appliqué.
        const MS = 1_787_136_773_742;
        base = await baseNeuve('application-epoque');
        await base.executer('INSERT INTO vm(id,nom,adresse) VALUES(?,?,?)',
            ['v-app', 'vm', '10.0.0.1']);
        await base.executer(
            'INSERT INTO application(id,vm_id,nom,chemin,vue_a,cle,cible,arguments,repertoire,apparue_a,disparue_a,masquee_a)'
                + ' VALUES(?,?,?,?,?,?,?,?,?,?,?,?)',
            ['a-1', 'v-app', 'Bloc-notes', 'C:\\Bureau\\Bloc-notes.lnk', MS,
             'cle-1', 'c:\\windows\\notepad.exe', '', 'c:\\windows', MS - 9, MS + 5, null],
        );

        const [ligne] = await base.interroger<{
            vue_a: unknown; apparue_a: unknown; disparue_a: unknown;
            masquee_a: unknown; arguments: unknown; cible: unknown;
        }>(
            'SELECT vue_a, apparue_a, disparue_a, masquee_a, arguments, cible FROM application WHERE id = ?',
            ['a-1'],
        );

        // 🔴 COMPARÉES SANS `Number(...)`, et c'est le point : envelopper la
        // lecture CONVERTIRAIT la divergence au lieu de la mesurer. `pg` rend
        // tout BIGINT en chaîne, et seul le `setTypeParser` du pilote fait que
        // ces trois-là sont des `number` sur les DEUX moteurs.
        expect(['vue_a', ligne.vue_a]).toEqual(['vue_a', MS]);
        expect(['apparue_a', ligne.apparue_a]).toEqual(['apparue_a', MS - 9]);
        expect(['disparue_a', ligne.disparue_a]).toEqual(['disparue_a', MS + 5]);
        // `masquee_a` est nullable et personne ne l'écrit en G1 : elle doit
        // rendre `null`, jamais `0` — les deux états sont distincts, comme
        // pour `agent_enrole.vu_a`.
        expect(['masquee_a', ligne.masquee_a]).toEqual(['masquee_a', null]);
        // ⚠️ Une chaîne VIDE, jamais NULL : c'est le contrat de `arguments`,
        // et un `NOT NULL` qui la refuserait rendrait la colonne inutilisable
        // pour les applications sans argument, c'est-à-dire la majorité.
        expect(['arguments', ligne.arguments]).toEqual(['arguments', '']);
        expect(['cible', ligne.cible]).toEqual(['cible', 'c:\\windows\\notepad.exe']);
    });

    it('applique application_cle sur le COUPLE (vm_id, cle), et non sur la clé seule', async () => {
        // 🔴 LES DEUX MOITIÉS SONT NÉCESSAIRES, et la seconde est celle qui
        // décide : sans elle, un index posé sur `cle` SEULE passerait ce test
        // — il refuserait bien le doublon de la première moitié. C'est
        // exactement le contrôle qui ne peut pas échouer, et il est fermé ici
        // en éprouvant AUSSI ce que l'index doit LAISSER PASSER.
        //
        // Ce que la seconde moitié protège, concrètement : la clé est
        // l'empreinte d'un triplet de chemins Windows, donc deux VMs portant
        // la même application au même endroit produisent la MÊME clé. Un index
        // sur `cle` seule empêcherait la seconde VM d'enregistrer son
        // catalogue.
        const MS = 1_787_136_773_742;
        base = await baseNeuve('application-unicite');
        await base.executer('INSERT INTO vm(id,nom,adresse) VALUES(?,?,?)', ['v-a', 'a', '10.0.0.1']);
        await base.executer('INSERT INTO vm(id,nom,adresse) VALUES(?,?,?)', ['v-b', 'b', '10.0.0.2']);

        const inserer = (id: string, vmId: string, cle: string) =>
            base!.executer(
                'INSERT INTO application(id,vm_id,nom,chemin,vue_a,cle,cible,arguments,repertoire,apparue_a)'
                    + ' VALUES(?,?,?,?,?,?,?,?,?,?)',
                [id, vmId, 'App', 'C:\\App.lnk', MS, cle, 'c:\\app.exe', '', 'c:\\', MS],
            );

        await inserer('a-1', 'v-a', 'meme-cle');
        // Même VM, même clé : refusé par l'index.
        await expect(inserer('a-2', 'v-a', 'meme-cle')).rejects.toThrow();
        // Autre VM, même clé : accepté — et c'est la moitié qui discrimine.
        await inserer('a-3', 'v-b', 'meme-cle');
        expect(await base.interroger('SELECT id FROM application', [])).toHaveLength(2);
    });

    it('REFUSE de convertir un BIGINT qui ne tient pas dans un entier sûr', async () => {
        // 🔴 Une conversion silencieuse est pire que la divergence qu'elle
        // répare : `Number('9007199254740993')` rend 9007199254740992, sans
        // le dire. Le pilote LÈVE plutôt que d'arrondir.
        //
        // La mutation qui le rougit : remplacer le garde du `setTypeParser`
        // par un `Number(v)` nu. Le test lit alors une valeur ARRONDIE au lieu
        // de lever.
        //
        // ⚠️ Ce cas n'est PAS atteignable par le service, qui n'écrit que des
        // `Date.now()` — c'est un contrôle du PILOTE, pas du schéma. Il ne
        // tourne donc que sur Postgres, seul moteur qui ait un analyseur à
        // garder ; sous SQLite il n'y a rien à éprouver, et le dire est plus
        // honnête que de le sauter en silence.
        base = await baseNeuve('bigint-hors-borne');
        await base.executer('INSERT INTO vm(id,nom,adresse) VALUES(?,?,?)',
            ['v-hb', 'vm', '10.0.0.1']);
        // 9007199254740993 = MAX_SAFE_INTEGER + 2. Littéral NUMÉRIQUE, seul
        // moyen de le poser : le passer en paramètre depuis JavaScript le
        // ferait déjà arrondir AVANT d'atteindre la base.
        await base.executer('UPDATE vm SET vue_a = 9007199254740993 WHERE id = ?', ['v-hb']);

        const lire = () => base!.interroger<{ vue_a: unknown }>(
            'SELECT vue_a FROM vm WHERE id = ?', ['v-hb']);

        if (MOTEUR === 'postgres') {
            await expect(lire()).rejects.toThrow(/entier sûr/);
        } else {
            // Sous SQLite il n'y a AUCUN analyseur à garder : `node:sqlite`
            // rend l'entier directement. Ce que fait ce moteur d'une valeur
            // hors borne est RELEVÉ ici, pas prescrit — le service n'écrit
            // que des `Date.now()`, et aucun chemin ne l'y conduit.
            await expect(lire()).rejects.toThrow();
        }
    });
});
