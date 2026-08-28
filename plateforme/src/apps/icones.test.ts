import { createHash, randomUUID } from 'node:crypto';
import { existsSync, mkdtempSync, readdirSync, rmSync, utimesSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, describe, expect, it } from 'vitest';
import { AGE_EVICTION_ICONE_MS, empreinteValide, ouvrirMagasin } from './icones';

let racines: string[] = [];
function magasinNeuf() {
    const r = mkdtempSync(join(tmpdir(), 'g2-icones-'));
    racines.push(r);
    return ouvrirMagasin(join(r, 'icones'), () => {});
}
afterEach(() => {
    for (const r of racines) rmSync(r, { recursive: true, force: true });
    racines = [];
});

const OCTETS = Buffer.from('\x89PNG\r\n\x1a\n-des-octets');
const EMPREINTE = createHash('sha256').update(OCTETS).digest('hex');

describe('le magasin d’icônes sur disque', () => {
    it('écrit, relit, et se sait posséder', () => {
        const m = magasinNeuf();
        expect(m.possede(EMPREINTE)).toBe(false);
        m.ecrire(EMPREINTE, OCTETS);
        expect(m.possede(EMPREINTE)).toBe(true);
        expect(m.lire(EMPREINTE)).toEqual(OCTETS);
    });

    it('🔴 REFUSE des octets qui ne correspondent pas à leur empreinte', () => {
        // 🔴 SANS CE RECALCUL, L'ADRESSAGE PAR CONTENU N'EN SERAIT PAS UN : un
        // agent fautif empoisonnerait le magasin d'un fichier qui ne
        // correspond pas à son nom, et `Cache-Control: immutable` rendrait
        // l'empoisonnement PERMANENT dans les caches.
        const m = magasinNeuf();
        const mensonge = createHash('sha256').update('autre chose').digest('hex');
        expect(() => m.ecrire(mensonge, OCTETS)).toThrow(/empreinte annoncée/);
        // Et le fichier partiel n'est JAMAIS écrit.
        expect(m.possede(mensonge)).toBe(false);
        expect(readdirSync(m.repertoire)).toEqual([]);
    });

    it('🔴 REFUSE une empreinte qui pourrait sortir du magasin', () => {
        // 🔴 SANS `empreinteValide`, `:sha256` EST UN COMPOSANT DE CHEMIN
        // FOURNI PAR LE RÉSEAU, et `..` y est significatif.
        const m = magasinNeuf();
        for (const mauvaise of [
            '../../../etc/passwd',
            '..%2f..%2fx',
            'a'.repeat(63),
            'a'.repeat(65),
            'A'.repeat(64), // majuscules : refusées, voir le commentaire
            `${'a'.repeat(63)}/`,
            '',
            '.',
            '..',
        ]) {
            expect(empreinteValide(mauvaise)).toBe(false);
            expect(() => m.ecrire(mauvaise, OCTETS)).toThrow(/invalide/);
            expect(m.possede(mauvaise)).toBe(false);
            expect(m.lire(mauvaise)).toBeUndefined();
        }
        expect(readdirSync(m.repertoire)).toEqual([]);
        expect(empreinteValide(EMPREINTE)).toBe(true);
    });

    it('l’écriture est ATOMIQUE : aucun fichier partiel ne reste', () => {
        const m = magasinNeuf();
        m.ecrire(EMPREINTE, OCTETS);
        // 🔴 UN FICHIER TRONQUÉ SOUS UN NOM QUI PROMET SON CONTENU serait
        // servi sans jamais être relu. Le seul fichier du répertoire est
        // l'empreinte elle-même — aucun `.part` résiduel.
        expect(readdirSync(m.repertoire)).toEqual([EMPREINTE]);
    });

    it('🔴 `manquantes` interroge le DISQUE, pas une liste en mémoire', () => {
        const m = magasinNeuf();
        m.ecrire(EMPREINTE, OCTETS);
        expect(m.manquantes([EMPREINTE])).toEqual([]);

        // 🔴 LE FICHIER EST SUPPRIMÉ SOUS LES PIEDS DU MAGASIN. Une table de
        // comptabilité ne le verrait pas, et l'icône serait perdue POUR
        // TOUJOURS. C'est la propriété du critère ⑦ : le magasin se
        // reconstruit tout seul.
        rmSync(join(m.repertoire, EMPREINTE));
        expect(m.manquantes([EMPREINTE])).toEqual([EMPREINTE]);
        expect(m.possede(EMPREINTE)).toBe(false);
    });

    it('`manquantes` préserve l’ordre d’annonce et fond les doublons', () => {
        const m = magasinNeuf();
        const a = createHash('sha256').update('a').digest('hex');
        const b = createHash('sha256').update('b').digest('hex');
        const c = createHash('sha256').update('c').digest('hex');
        m.ecrire(b, Buffer.from('b'));
        expect(m.manquantes([c, a, c, b, a])).toEqual([c, a]);
    });

    it('une empreinte MAL FORMÉE n’est pas « manquante » : elle est ignorée', () => {
        // La redemander ferait boucler l'agent sur une valeur que la route
        // refuserait de toute façon.
        const m = magasinNeuf();
        expect(m.manquantes(['../x', 'ZZZ'])).toEqual([]);
    });

    it('un magasin VIDE fait tout redemander — le premier démarrage, et la perte', () => {
        const m = magasinNeuf();
        const a = createHash('sha256').update('a').digest('hex');
        const b = createHash('sha256').update('b').digest('hex');
        expect(m.manquantes([a, b])).toEqual([a, b]);
    });

    it('le répertoire est CRÉÉ s’il manque, et son chemin est JOURNALISÉ', () => {
        const r = mkdtempSync(join(tmpdir(), 'g2-icones-'));
        racines.push(r);
        const vu: string[] = [];
        const cible = join(r, 'profond', randomUUID());
        expect(existsSync(cible)).toBe(false);
        ouvrirMagasin(cible, (c) => vu.push(c));
        expect(existsSync(cible)).toBe(true);
        // ⚠️ LA LIGNE DE JOURNAL N'EST PAS DÉCORATIVE : la variable étant
        // facultative, un opérateur peut se tromper de répertoire sans que
        // rien ne casse — le magasin se reconstruirait ailleurs, en silence.
        expect(vu).toEqual([cible]);
    });

    it('un fichier étranger déjà présent est LU tel quel, sans être revalidé', () => {
        // ⚠️ PROPRIÉTÉ DÉCLARÉE, PAS UNE LACUNE CACHÉE : `lire` ne recalcule
        // rien. La vérification vit à l'ÉCRITURE, qui est le seul chemin par
        // lequel un pair peut déposer quelque chose. Un fichier posé à la main
        // dans le magasin est la responsabilité de qui l'a posé.
        const m = magasinNeuf();
        writeFileSync(join(m.repertoire, EMPREINTE), 'pas le bon contenu');
        expect(m.lire(EMPREINTE)?.toString()).toBe('pas le bon contenu');
    });
});

describe('l’éviction par âge, avec plancher', () => {
    // Le temps est INJECTÉ, jamais lu de l'horloge : un test qui attendrait
    // réellement l'âge d'éviction serait un test qu'on désactive au premier
    // ralentissement de la machine.
    const JOUR_MS = 24 * 60 * 60_000;

    /// Une icône dont le contenu et l'empreinte se correspondent, comme
    /// `ecrire` l'exige.
    function icone(texte: string): { empreinte: string; octets: Buffer } {
        const octets = Buffer.from(texte);
        return { empreinte: createHash('sha256').update(octets).digest('hex'), octets };
    }

    /// Dépose une icône, puis FORCE sa date de dernière modification : c'est
    /// l'équivalent, sur le magasin RÉEL, du `deposer(cle, octets, quand)` de
    /// la tâche — `ecrire` seul n'a aucune prise sur l'horloge du disque.
    function deposerA(m: ReturnType<typeof magasinNeuf>, texte: string, quandMs: number): string {
        const { empreinte, octets } = icone(texte);
        m.ecrire(empreinte, octets);
        utimesSync(join(m.repertoire, empreinte), new Date(quandMs), new Date(quandMs));
        return empreinte;
    }

    it('évince une icône vieille et NON référencée', async () => {
        const m = magasinNeuf();
        const orpheline = deposerA(m, 'orpheline', 0);
        await m.evincer({ maintenant: 400 * JOUR_MS, referencees: new Set() });
        expect(m.possede(orpheline)).toBe(false);
    });

    // 🔴 LE SEUL TEST QUI DISTINGUE UNE ÉVICTION D'UNE CORRUPTION. Sans lui,
    // une éviction qui emporte TOUT passerait le test précédent.
    it('NE PEUT PAS évincer une icône vieille mais RÉFÉRENCÉE par une entrée vivante', async () => {
        const m = magasinNeuf();
        const enService = deposerA(m, 'en-service', 0);
        await m.evincer({ maintenant: 400 * JOUR_MS, referencees: new Set([enService]) });
        expect(m.possede(enService)).toBe(true);
    });

    it('n’évince pas une icône jeune', async () => {
        const m = magasinNeuf();
        const recente = deposerA(m, 'recente', 0);
        await m.evincer({ maintenant: 1 * JOUR_MS, referencees: new Set() });
        expect(m.possede(recente)).toBe(true);
    });

    it('🔴 la constante N n’est PAS calibrée : le plancher, lui, tient à n’importe quelle valeur', () => {
        // Contrôle de cohérence du montage lui-même : si `AGE_EVICTION_ICONE_MS`
        // dérivait un jour hors de l'intervalle [1 jour, 400 jours], les deux
        // tests ci-dessus perdraient leur sens sans qu'aucune rouge ne le dise.
        expect(AGE_EVICTION_ICONE_MS).toBeGreaterThan(1 * JOUR_MS);
        expect(AGE_EVICTION_ICONE_MS).toBeLessThan(400 * JOUR_MS);
    });

    it('un nom qui n’est pas une empreinte valide n’est jamais touché', async () => {
        const m = magasinNeuf();
        writeFileSync(join(m.repertoire, 'etranger'), 'pas une empreinte');
        utimesSync(join(m.repertoire, 'etranger'), new Date(0), new Date(0));
        await m.evincer({ maintenant: 400 * JOUR_MS, referencees: new Set() });
        expect(existsSync(join(m.repertoire, 'etranger'))).toBe(true);
    });
});
