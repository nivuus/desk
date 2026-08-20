// La fusion de catalogue, éprouvée SANS BASE ET SANS HORLOGE.
//
// 🔴 C'est ce que le module achète : les trois règles qui décident ce qu'on
// écrit — marquer disparue, ressusciter, mettre à jour — se rougissent ici,
// sur un tableau et un objet, sans ouvrir de base ni monter de serveur. Une
// règle qui vivrait dans le dépôt ou dans le canal ne serait éprouvable que
// par un test qui traverse un moteur SQL.

import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import path from 'node:path';
import type { Application, CatalogueMessage } from '../../../proto/ts/plateforme';
import { fusionner, type Connue } from './catalogue';

function app(cle: string, nom = cle): Application {
    return {
        cle,
        nom,
        chemin: `C:\\Bureau\\${nom}.lnk`,
        cible: `c:\\programmes\\${nom}.exe`,
        arguments: '',
        repertoire: 'c:\\programmes',
    };
}

function connue(id: string, cle: string, disparueA: number | null = null): Connue {
    return { id, cle, disparue_a: disparueA };
}

function message(
    complet: boolean,
    applications: Application[],
    disparues: string[] = [],
): CatalogueMessage {
    return { v: 2, type: 'catalogue', complet, applications, disparues };
}

describe('fusionner', () => {
    it('à `complet: true`, marque disparue TOUTE ligne connue absente du message', () => {
        // 🔴 Traiter un message complet comme un delta laisserait au catalogue,
        // POUR TOUJOURS, une application désinstallée pendant que le canal
        // était coupé : sa clé ne figurerait dans aucun `disparues`, personne
        // ne l'ayant vue partir.
        const f = fusionner(
            [connue('id-a', 'a'), connue('id-b', 'b')],
            message(true, [app('a')]),
        );
        expect(f.aMarquerDisparues).toEqual(['id-b']);
        expect(f.aMettreAJour.map((m) => m.id)).toEqual(['id-a']);
    });

    it('ne RE-marque pas une ligne déjà disparue', () => {
        // ⚠️ `disparue_a` est POSÉE, JAMAIS SUPPRIMÉE : la ré-écrire à chaque
        // réconciliation ferait avancer l'instant de disparition tant que
        // l'agent tourne, et la colonne dirait « disparue il y a 30 secondes »
        // d'une application partie depuis un mois.
        const f = fusionner([connue('id-b', 'b', 1_787_136_773_742)], message(true, []));
        expect(f.aMarquerDisparues).toEqual([]);
    });

    it("à `complet: false`, n'invente AUCUNE disparition et n'honore que `disparues`", () => {
        // 🔴 La rouge inverse de la première : traiter un delta comme un état
        // complet VIDERAIT le catalogue à chaque message ne portant qu'une
        // apparition.
        const f = fusionner(
            [connue('id-a', 'a'), connue('id-b', 'b'), connue('id-c', 'c')],
            message(false, [app('a')], ['c']),
        );
        expect(f.aMarquerDisparues).toEqual(['id-c']);
        expect(f.aInserer).toEqual([]);
        expect(f.aMettreAJour.map((m) => m.id)).toEqual(['id-a']);
    });

    it('à `complet: true` avec `applications: []`, VIDE le catalogue', () => {
        // Le cas `catalogue_vide` des vecteurs partagés : une VM dont on
        // désinstalle tout. Le traiter comme « rien à faire » la laisserait
        // pleine.
        const f = fusionner([connue('id-a', 'a'), connue('id-b', 'b')], message(true, []));
        expect(f.aMarquerDisparues).toEqual(['id-a', 'id-b']);
        expect(f.aInserer).toEqual([]);
        expect(f.aMettreAJour).toEqual([]);
    });

    it("apparie sur la CLÉ : une ligne connue et présente va dans aMettreAJour, jamais dans aInserer", () => {
        // 🔴 Apparier sur l'`id` serait impossible : l'agent ne les connaît
        // pas, et ne les a jamais vus. Il ne connaît que la clé.
        const f = fusionner([connue('id-a', 'a')], message(true, [app('a', 'renomme')]));
        expect(f.aInserer).toEqual([]);
        expect(f.aMettreAJour).toEqual([{ id: 'id-a', app: app('a', 'renomme') }]);
    });

    it('une ligne connue DISPARUE et de nouveau présente est RESSUSCITÉE, et mise à jour', () => {
        // 🔴 L'insérer à neuf lui donnerait un identifiant NEUF, et une PWA
        // installée depuis l'ancien pointerait dans le vide. C'est la même
        // perte d'identifiant que celle qu'un DELETE causerait, par une autre
        // porte.
        //
        // ⚠️ ELLE EST DANS LES DEUX LISTES, et c'est délibéré : ressusciter
        // remet `disparue_a` à NULL, mettre à jour rafraîchit les champs. Une
        // résurrection seule rendrait visible une ligne aux champs périmés.
        const f = fusionner(
            [connue('id-a', 'a', 1_787_136_773_742)],
            message(true, [app('a', 'revenu')]),
        );
        expect(f.aRessusciter).toEqual(['id-a']);
        expect(f.aMettreAJour).toEqual([{ id: 'id-a', app: app('a', 'revenu') }]);
        expect(f.aInserer).toEqual([]);
        expect(f.aMarquerDisparues).toEqual([]);
    });

    it('insère une clé inconnue, et ignore une `disparues` qui ne désigne personne', () => {
        // ⚠️ Une clé de `disparues` inconnue n'est pas une erreur : l'agent
        // peut annoncer la disparition d'une application que la plateforme
        // n'a jamais enregistrée — un message montant perdu suffit. Lever
        // abattrait le canal d'un agent qui va très bien.
        const f = fusionner([], message(false, [app('neuve')], ['jamais-vue']));
        expect(f.aInserer).toEqual([app('neuve')]);
        expect(f.aMarquerDisparues).toEqual([]);
        expect(f.aRessusciter).toEqual([]);
    });

    it('est PUR : ni horloge lue, ni pilote de base', () => {
        // 🔴 CE CONTRÔLE BLANCHIT LES COMMENTAIRES AVANT DE CHERCHER, et ce
        // n'est pas une précaution de style : ce fichier-ci NOMME `Date.now`
        // et `Pilote` dans sa propre prose pour expliquer pourquoi ils sont
        // absents. Un `grep` nu serait donc satisfait — ou rougi — par la
        // documentation plutôt que par le code, et ne discriminerait rien.
        // Ce dépôt a payé trois fois pour cette forme exacte de contrôle.
        const source = readFileSync(
            path.join(path.dirname(fileURLToPath(import.meta.url)), 'catalogue.ts'),
            'utf8',
        );
        const code = source
            .replace(/\/\*[\s\S]*?\*\//g, '')
            .replace(/\/\/.*$/gm, '');
        expect([...code.matchAll(/\bDate\.now\b|\bPilote\b/g)].map((m) => m[0])).toEqual([]);
    });
});
