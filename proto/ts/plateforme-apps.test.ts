/**
 * Tests du miroir TypeScript du canal plateforme — la GESTION D'APPLICATIONS.
 *
 * Extrait de `proto/ts/plateforme.test.ts` VERBATIM (sous-bloc G2, tâche 2) :
 * ce fichier était à 512 lignes et figurait au tableau de dette de `CLAUDE.md`
 * SANS POINT DE CHUTE. Le sous-bloc G2 travaille dedans, donc il l'a découpé —
 * la règle du dépôt est que le découpage rétroactif se fait au moment où l'on
 * travaille dans le fichier, pas en chantier séparé.
 *
 * **Aucun test n'a été ajouté, retiré ni réécrit par ce déplacement.** Le
 * compte de `npx vitest run` a été annoncé AVANT d'être mesuré : 142 tests,
 * 5 fichiers avant, 142 tests et 6 fichiers après. ⚠️ Le compte de FICHIERS
 * change, celui de TESTS non — dire lequel on annonce (piège de P3).
 *
 * La frontière est le miroir exacte de celle du Rust : le cycle de vie reste
 * chez `plateforme.test.ts`, la gestion d'apps vient ici. ⚠️ Le `describe`
 * « la version 2, et les listes blanches DÉRIVÉES de l'union » est RESTÉ chez
 * `plateforme.test.ts` : il éprouve la constante de version et les deux listes
 * blanches de l'union entière, pas le catalogue.
 */
import { describe, expect, it } from 'vitest';
import {
    parseDepuisLaPlateforme,
    parseVersLaPlateforme,
    type Application,
    encodeCatalogue,
    encodeLancee,
    encodeLancer,
} from './plateforme';

// ---------------------------------------------------------------------------
// Sous-bloc G1 — catalogue d'applications et lancement (v2).
// ---------------------------------------------------------------------------

const APP_TEMOIN: Application = {
    cle: 'a1b2',
    nom: 'Bloc-notes',
    chemin: 'C:\\Users\\u\\Desktop\\Bloc-notes.lnk',
    cible: 'c:\\windows\\system32\\notepad.exe',
    arguments: '',
    repertoire: 'c:\\windows\\system32',
};

const CATALOGUE_TEMOIN =
    '{"type":"catalogue","v":2,"complet":true,"applications":[{"cle":"a1b2",'
    + '"nom":"Bloc-notes","chemin":"C:\\\\Users\\\\u\\\\Desktop\\\\Bloc-notes.lnk",'
    + '"cible":"c:\\\\windows\\\\system32\\\\notepad.exe","arguments":"",'
    + '"repertoire":"c:\\\\windows\\\\system32"}],"disparues":["disparue-1"]}';

describe('le catalogue et le lancement, sens AGENT -> PLATEFORME', () => {
    it('encode `catalogue` exactement comme Rust', () => {
        // 🔴 L'ordre des champs est `type` PUIS `v` : serde émet le tag interne
        // en premier, `JSON.stringify` respecte l'ordre d'insertion, et le
        // vecteur fige la chaîne octet pour octet. Écrire `v` d'abord — ce que
        // fait `control.ts` — produirait une chaîne différente, et la
        // divergence a été trouvée par ce test en P3, pas par la relecture.
        expect(encodeCatalogue(true, [APP_TEMOIN], ['disparue-1'])).toBe(CATALOGUE_TEMOIN);
    });

    it('encode `lancee` exactement comme Rust', () => {
        expect(encodeLancee('d-7', 'raccourci')).toBe(
            '{"type":"lancee","v":2,"demande":"d-7","issue":"raccourci"}',
        );
    });

    it('lit un `catalogue` bien formé, et retrouve chacun de ses champs', () => {
        const lu = parseVersLaPlateforme(CATALOGUE_TEMOIN);
        expect(lu.ok).toBe(true);
        if (!lu.ok) return;
        expect(lu.message.type).toBe('catalogue');
        if (lu.message.type !== 'catalogue') return;
        expect(lu.message.complet).toBe(true);
        expect(lu.message.disparues).toEqual(['disparue-1']);
        expect(lu.message.applications).toEqual([APP_TEMOIN]);
    });

    it('🔴 REJETTE un `catalogue` dont `applications` n’est pas un tableau, motif `forme`', () => {
        // 🔴 C'est le SEUL parseur du fichier dont les octets viennent d'un
        // tiers. Sans cette garde, un `applications` absent ou scalaire
        // traverserait jusqu'à la requête SQL. Et rendre `enrolement` ferait
        // lire « secret faux » au pair pour un message parfaitement
        // authentifié : le motif désigne la cause, il ne la déguise pas.
        expect(
            parseVersLaPlateforme('{"type":"catalogue","v":2,"complet":true,"applications":3,"disparues":[]}'),
        ).toEqual({ ok: false, motif: 'forme' });
        expect(
            parseVersLaPlateforme('{"type":"catalogue","v":2,"complet":true,"disparues":[]}'),
        ).toEqual({ ok: false, motif: 'forme' });
    });

    it('🔴 REJETTE un `catalogue` dont une application est incomplète, motif `forme`', () => {
        expect(
            parseVersLaPlateforme(
                '{"type":"catalogue","v":2,"complet":true,"applications":[{"cle":"a"}],"disparues":[]}',
            ),
        ).toEqual({ ok: false, motif: 'forme' });
    });

    it('🔴 REJETTE une `lancee` dont l’issue est inconnue, motif `forme`', () => {
        expect(
            parseVersLaPlateforme('{"type":"lancee","v":2,"demande":"d","issue":"peut-etre"}'),
        ).toEqual({ ok: false, motif: 'forme' });
    });

    it('lit une `lancee` bien formée', () => {
        const lu = parseVersLaPlateforme('{"type":"lancee","v":2,"demande":"d-7","issue":"echec"}');
        expect(lu).toEqual({
            ok: true,
            message: { type: 'lancee', v: 2, demande: 'd-7', issue: 'echec' },
        });
    });
});

describe('le lancement, sens PLATEFORME -> AGENT', () => {
    it('encode `lancer` exactement comme Rust', () => {
        expect(encodeLancer('d-7', 'a1b2')).toBe(
            '{"type":"lancer","v":2,"demande":"d-7","cle":"a1b2"}',
        );
    });

    it('relit un `lancer`', () => {
        // 🔴 La rouge : l'omettre de `TYPES_DEPUIS`. Le parseur lèverait
        // « type de message de plateforme inconnu » sur un ordre valide.
        const lu = parseDepuisLaPlateforme(
            '{"type":"lancer","v":2,"demande":"d-7","cle":"a1b2"}',
        ) as unknown as Record<string, unknown>;
        expect(lu.type).toBe('lancer');
        expect(lu.demande).toBe('d-7');
        expect(lu.cle).toBe('a1b2');
    });
});
