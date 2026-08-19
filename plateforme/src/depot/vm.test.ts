// Le dépôt `vm`, sous les DEUX moteurs — même harnais que `pilotes.test.ts`.
//
// 🔴 LES HORODATAGES SONT DE LA MAGNITUDE D'UNE ÉPOQUE EN MILLISECONDES,
// jamais des petits nombres commodes. C'est la leçon la plus chère de P1 : la
// double passe n'écrivait que des `1_000`, qui tiennent dans un entier de 4
// octets, et déclarait ainsi portable un schéma que Postgres refusait pour
// toute écriture réelle.

import { afterEach, describe, expect, it } from 'vitest';
import { baseNeuve, MOTEUR } from '../base/harnais';
import type { Pilote } from '../base/pilote';
import { enroler, marquerVu } from './agent';
import { creerUtilisateur } from './utilisateur';
import { attribuerSiLibre, detacher, lireParId, lireParNom, lister } from './vm';

/// Une époque réelle, pas un petit nombre : voir l'en-tête.
const MS = 1_787_136_773_742;

let base: Pilote | undefined;

afterEach(async () => {
    await base?.fermer();
    base = undefined;
});

/// Crée une VM. ⚠️ `depot/vm.ts` n'écrit AUCUNE VM : le seul chemin de
/// création est `admin/enroler-agent.ts`, et P4 n'en ajoute pas d'autre (D1).
/// Les tests posent donc la ligne eux-mêmes, comme le fait cette commande.
async function poserVm(p: Pilote, id: string, nom: string): Promise<void> {
    await p.executer('INSERT INTO vm(id, nom, adresse) VALUES(?, ?, ?)', [
        id,
        nom,
        '192.168.3.2',
    ]);
}

/// `vm.utilisateur_id` RÉFÉRENCE `utilisateur(id)`, et SQLite applique bien la
/// contrainte (`pilote-sqlite.ts` pose `PRAGMA foreign_keys`). Un identifiant
/// inventé ferait donc échouer l'attribution pour une raison étrangère au
/// test.
async function poserUtilisateur(p: Pilote, email: string): Promise<string> {
    return creerUtilisateur(p, email, 'empreinte-opaque-de-test', MS);
}

describe(`dépôt vm, moteur=${MOTEUR}`, () => {
    it('🔴 `lister` rend la VM AVEC son préfixe et son `vu_a`, joints depuis agent_enrole', async () => {
        // 🔴 La rouge : remplacer le `LEFT JOIN` par un `JOIN`. Une VM enrôlée
        // mais qui n'a jamais battu resterait visible (son `vu_a` est nul, pas
        // sa ligne) — mais une VM créée SANS enrôlement disparaîtrait de
        // l'inventaire, sans qu'aucune erreur ne le dise.
        base = await baseNeuve('vm-lister');
        await poserVm(base, 'v1', 'w1');
        await enroler(base, 'v1', 'empreinte-opaque', 'PREFIXEv1');
        await marquerVu(base, 'v1', MS);
        // La seconde VM n'est PAS enrôlée : c'est elle qui éprouve le LEFT.
        await poserVm(base, 'v2', 'w2');

        const lignes = await lister(base);
        expect(lignes.map((l) => l.id).sort()).toEqual(['v1', 'v2']);

        const v1 = lignes.find((l) => l.id === 'v1')!;
        expect(v1.nom).toBe('w1');
        expect(v1.adresse).toBe('192.168.3.2');
        expect(v1.prefixe_session).toBe('PREFIXEv1');
        expect(v1.utilisateur_id).toBeNull();

        const v2 = lignes.find((l) => l.id === 'v2')!;
        expect(v2.prefixe_session).toBeNull();
        expect(v2.vu_a).toBeNull();
    });

    it('🔴 `vu_a` est un NOMBRE, pas une chaîne, après une époque réelle', async () => {
        // 🔴 C'est le défaut de CLASSE que P3 a payé : `pg` rend tout `BIGINT`
        // en CHAÎNE, et `interroger<T>` fait un `as T[]` — aucun typage ne
        // pouvait l'attraper. Le remède est au pilote
        // (`base/pilote-postgres.ts::setTypeParser`) ; cette assertion est le
        // témoin AU POINT D'USAGE, et elle rougit sous `test:postgres` si le
        // `setTypeParser` disparaît.
        //
        // ⚠️ Elle ne rougirait PAS sous `test:sqlite`, où le type est juste
        // depuis toujours. Un test vert sur un seul moteur ne dit rien ici.
        base = await baseNeuve('vm-type-vua');
        await poserVm(base, 'v1', 'w1');
        await enroler(base, 'v1', 'empreinte-opaque', 'PREFIXEv1');
        await marquerVu(base, 'v1', MS);

        const [ligne] = await lister(base);
        expect(typeof ligne.vu_a).toBe('number');
        // Et la VALEUR EXACTE : un `Number()` posé dans le dépôt masquerait le
        // type sans que rien ne le dise, mais une troncature, elle, se verrait.
        expect(ligne.vu_a).toBe(MS);
    });

    it('`lireParId` et `lireParNom` rendent la même ligne, `undefined` sur inconnu', async () => {
        base = await baseNeuve('vm-lire');
        await poserVm(base, 'v1', 'w1');
        await enroler(base, 'v1', 'empreinte-opaque', 'PREFIXEv1');

        const parId = await lireParId(base, 'v1');
        const parNom = await lireParNom(base, 'w1');
        expect(parId).toEqual(parNom);
        expect(parId?.prefixe_session).toBe('PREFIXEv1');
        // JAMAIS une exception sur un inconnu : le motif serait indiscernable
        // d'un défaut de base, et sur une route ce serait un oracle.
        expect(await lireParId(base, 'v-inexistante')).toBeUndefined();
        expect(await lireParNom(base, 'w-inexistante')).toBeUndefined();
    });

    it('🔴 `attribuerSiLibre` sur une VM LIBRE rend 1', async () => {
        base = await baseNeuve('vm-attrib-libre');
        await poserVm(base, 'v1', 'w1');
        const alice = await poserUtilisateur(base, 'alice@exemple.test');
        expect(await attribuerSiLibre(base, 'v1', alice)).toBe(1);
        expect((await lireParId(base, 'v1'))?.utilisateur_id).toBe(alice);
    });

    it('🔴 `attribuerSiLibre` sur une VM DÉJÀ PRISE rend 0, sans lever', async () => {
        // 🔴 La rouge : retirer `AND utilisateur_id IS NULL`. MESURÉ sur les
        // DEUX moteurs (SQLite 3.50.4, PostgreSQL 16.15) : l'`UPDATE` nu rend
        // alors `1 ligne` et la VM d'alice est VOLÉE. L'index partiel
        // `vm_un_utilisateur` n'interdit RIEN à ce vol — il rend
        // `utilisateur_id` unique À TRAVERS LES LIGNES, donc il interdit qu'un
        // utilisateur ait deux VMs, jamais qu'une VM change de main.
        base = await baseNeuve('vm-attrib-prise');
        await poserVm(base, 'v1', 'w1');
        const alice = await poserUtilisateur(base, 'alice@exemple.test');
        const bob = await poserUtilisateur(base, 'bob@exemple.test');
        await attribuerSiLibre(base, 'v1', alice);
        expect(await attribuerSiLibre(base, 'v1', bob)).toBe(0);
    });

    it('🔴 …et la VM DÉJÀ PRISE n’a PAS changé de propriétaire', async () => {
        // ⚠️ `it()` DISTINCT du précédent, et c'est la propriété ②a
        // elle-même : `expect` interrompt un test à sa première assertion
        // fausse, si bien qu'une seule des deux serait éprouvée. Sous la
        // mutation, le compte rendu ET l'état de la ligne changent tous les
        // deux, et les deux doivent se voir.
        base = await baseNeuve('vm-attrib-prise-etat');
        await poserVm(base, 'v1', 'w1');
        const alice = await poserUtilisateur(base, 'alice@exemple.test');
        const bob = await poserUtilisateur(base, 'bob@exemple.test');
        await attribuerSiLibre(base, 'v1', alice);
        await attribuerSiLibre(base, 'v1', bob);
        expect((await lireParId(base, 'v1'))?.utilisateur_id).toBe(alice);
    });

    it('🔴 `attribuerSiLibre` pour un utilisateur qui a DÉJÀ une VM LÈVE', async () => {
        // 🔴 La rouge : retirer l'index `vm_un_utilisateur` de
        // `0001-socle.sql`. MESURÉ sur les deux moteurs : l'`UPDATE` passe
        // alors, et l'utilisateur se retrouve avec deux VMs. C'est la
        // propriété ②b, et elle est distincte de ②a — celle-ci LÈVE, l'autre
        // rend `0 ligne` sans exception.
        //
        // ⚠️ LE TEXTE DE L'EXCEPTION DIFFÈRE D'UN MOTEUR À L'AUTRE
        // (`UNIQUE constraint failed: vm.utilisateur_id` contre
        // `duplicate key value violates unique constraint "vm_un_utilisateur"`)
        // : ce test asserte QU'ELLE LÈVE, jamais ce qu'elle dit. Aucun code du
        // service ne compare ce texte non plus.
        base = await baseNeuve('vm-attrib-servi');
        await poserVm(base, 'v1', 'w1');
        await poserVm(base, 'v2', 'w2');
        const alice = await poserUtilisateur(base, 'alice@exemple.test');
        await attribuerSiLibre(base, 'v1', alice);
        await expect(attribuerSiLibre(base, 'v2', alice)).rejects.toThrow();
        // Et `v2` est restée au vivier.
        expect((await lireParId(base, 'v2'))?.utilisateur_id).toBeNull();
    });

    it('`attribuerSiLibre` sur une VM INCONNUE rend 0 sans lever', async () => {
        // 🔴 La rouge : lever. Le motif serait alors indiscernable d'un défaut
        // de base — et `changes = 0` confond bien TROIS causes (E8), ce qui
        // est précisément pourquoi l'appelant lit d'abord la ligne.
        base = await baseNeuve('vm-attrib-inconnue');
        const alice = await poserUtilisateur(base, 'alice@exemple.test');
        expect(await attribuerSiLibre(base, 'v-inexistante', alice)).toBe(0);
    });

    it('🔴 `detacher` rend la VM au vivier, et une seconde attribution redevient possible', async () => {
        // 🔴 La rouge : ne pas remettre `null`. La VM resterait prise à vie, et
        // `--detacher` de `admin:attribuer` ne servirait à rien.
        base = await baseNeuve('vm-detacher');
        await poserVm(base, 'v1', 'w1');
        const alice = await poserUtilisateur(base, 'alice@exemple.test');
        const bob = await poserUtilisateur(base, 'bob@exemple.test');
        await attribuerSiLibre(base, 'v1', alice);
        expect(await detacher(base, 'v1')).toBe(1);
        expect((await lireParId(base, 'v1'))?.utilisateur_id).toBeNull();
        expect(await attribuerSiLibre(base, 'v1', bob)).toBe(1);
        expect((await lireParId(base, 'v1'))?.utilisateur_id).toBe(bob);
    });
});
