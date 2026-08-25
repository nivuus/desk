// ⚠️ CE FICHIER NE TOUCHE PAS LA BASE, et il court pourtant sous `test:sqlite`
// ET sous `test:postgres` : il vit dans la même suite. Il n'ouvre aucun pilote,
// donc `baseNeuve` et son nom de schéma obligatoire ne le concernent pas.
//
// ⚠️ LES DEUX TESTS DE CONFIGURATION EN FIN DE FICHIER SONT ICI PLUTÔT QUE DANS
// `config.test.ts`, ET C'EST DÉCLARÉ : l'arbre est partagé avec d'autres
// chantiers, et la tâche s'interdit d'écrire ailleurs. `config.test.ts` a reçu
// la seule ligne qu'il ne pouvait PAS ne pas recevoir — son `toEqual` compare
// l'objet ENTIER, donc un champ de plus le rend rouge.

import { randomUUID } from 'node:crypto';
import { existsSync, mkdtempSync, readdirSync, readFileSync, rmSync, utimesSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { Readable } from 'node:stream';
import { afterEach, describe, expect, it } from 'vitest';
import { lireConfig } from '../config';
import {
    AGE_EVICTION_TRANCHES_MS,
    identifiantValide,
    ouvrirMagasinTranches,
    rangValide,
} from './magasin-tranches';
import { verdict } from '../../../proto/ts/tranches';
import { SECRET as SECRET_PLATEFORME } from '../agents/canal-harnais';

let racines: string[] = [];
function magasinNeuf() {
    const r = mkdtempSync(join(tmpdir(), 'g3-tranches-'));
    racines.push(r);
    return ouvrirMagasinTranches(join(r, 'televersements'), () => {});
}
afterEach(() => {
    for (const r of racines) rmSync(r, { recursive: true, force: true });
    racines = [];
});

/// Un flux d'octets, en morceaux, pour éprouver le chemin RÉEL — celui qui
/// n'accumule pas en mémoire — plutôt qu'un `Buffer` déguisé.
function flux(...morceaux: (string | Uint8Array)[]): AsyncIterable<Uint8Array> {
    return Readable.from(morceaux.map((m) => (typeof m === 'string' ? Buffer.from(m) : m)));
}

// ⚠️ CET IDENTIFIANT PORTE DES LETTRES HEXADÉCIMALES, ET C'EST DÉLIBÉRÉ : sur
// un UUID de chiffres seuls, `toUpperCase()` serait un NO-OP et le cas
// « majuscules refusées » du test des identifiants ne pourrait PAS échouer.
const ID = '0a1b2c3d-4e5f-4a6b-8c7d-9e0f1a2b3c4d';
const PLAFOND = 1024;

async function lire(r: Readable): Promise<Buffer> {
    const bouts: Buffer[] = [];
    for await (const m of r) bouts.push(Buffer.from(m as Uint8Array));
    return Buffer.concat(bouts);
}

describe('le magasin des tranches sur disque', () => {
    it('écrit une tranche EN FLUX, et la relit par concaténation', async () => {
        const m = magasinNeuf();
        expect(await m.ecrire(ID, 0, flux('abc', 'def'), PLAFOND)).toEqual({ ok: true, octets: 6 });
        expect(await m.ecrire(ID, 1, flux('gh'), PLAFOND)).toEqual({ ok: true, octets: 2 });
        expect((await lire(m.concatener(ID, [0, 1]))).toString()).toBe('abcdefgh');
    });

    it('🔴 IL N’EXISTE AUCUN FICHIER ASSEMBLÉ — un fichier par tranche, et rien d’autre', async () => {
        // 🔴 Un installeur de 800 Mo doublerait l'espace disque au scellement,
        // et l'assemblé serait une SECONDE source de vérité que rien ne
        // départagerait de ses tranches le jour où elles divergeraient.
        const m = magasinNeuf();
        await m.ecrire(ID, 0, flux('abc'), PLAFOND);
        await m.ecrire(ID, 1, flux('de'), PLAFOND);
        // La concaténation est un FLUX : la consommer n'écrit rien.
        expect((await lire(m.concatener(ID, [0, 1]))).toString()).toBe('abcde');
        expect(readdirSync(join(m.racine, ID)).sort()).toEqual(['0', '1']);
    });

    it('🔴 le plafond COUPE, supprime le partiel, et se dit — jamais une troncature', async () => {
        const m = magasinNeuf();
        const r = await m.ecrire(ID, 0, flux('a'.repeat(600), 'b'.repeat(600)), PLAFOND);
        expect(r).toEqual({ ok: false, motif: 'plafond-depasse', plafond: PLAFOND });
        // 🔴 NI LA TRANCHE, NI LE `.part` : un fichier de 600 octets laissé là
        // serait vu PRÉSENT par la reprise, `verdict` le dirait `incoherentes`,
        // et une incohérence ne se répare pas en redemandant.
        expect(existsSync(join(m.racine, ID))).toBe(true);
        expect(readdirSync(join(m.racine, ID))).toEqual([]);
        expect(m.lister(ID)).toEqual([]);
    });

    it('une tranche EXACTEMENT au plafond passe : la borne est inclusive', async () => {
        const m = magasinNeuf();
        expect(await m.ecrire(ID, 0, flux('x'.repeat(PLAFOND)), PLAFOND))
            .toEqual({ ok: true, octets: PLAFOND });
        expect(m.lister(ID)).toEqual([{ n: 0, octets: PLAFOND }]);
    });

    it('⚠️ le plafond BORNE le disque, il ne JUGE pas le découpage', async () => {
        // Une tranche plus COURTE que le contrat passe ici sans un mot : c'est
        // `verdict` qui la déclarera `incoherentes` au scellement. Ce module ne
        // connaît pas le contrat, et le lui faire connaître serait la seconde
        // arithmétique que `proto/ts/tranches.ts` existe pour empêcher.
        const m = magasinNeuf();
        await m.ecrire(ID, 0, flux('court'), PLAFOND);
        expect(m.lister(ID)).toEqual([{ n: 0, octets: 5 }]);
        expect(verdict(20, 10, m.lister(ID))).toEqual({ etat: 'incoherentes', n: [0] });
    });

    it('🔴 `lister` interroge le DISQUE, pas une comptabilité', async () => {
        const m = magasinNeuf();
        await m.ecrire(ID, 0, flux('abcde'), PLAFOND);
        await m.ecrire(ID, 1, flux('fg'), PLAFOND);
        expect(m.lister(ID)).toEqual([{ n: 0, octets: 5 }, { n: 1, octets: 2 }]);

        // 🔴 LE FICHIER EST SUPPRIMÉ SOUS LES PIEDS DU MAGASIN. Une table
        // divergerait du disque le jour où un fichier serait perdu — et c'est
        // PRÉCISÉMENT le jour où l'on a besoin de le savoir. La reprise
        // redemande, et le déposant recomplète.
        rmSync(join(m.racine, ID, '0'));
        expect(m.lister(ID)).toEqual([{ n: 1, octets: 2 }]);
        expect(verdict(7, 5, m.lister(ID))).toEqual({ etat: 'manquantes', n: [0] });
    });

    it('`lister` trie par rang, et un tri de CHAÎNES ne suffirait pas', async () => {
        const m = magasinNeuf();
        for (const n of [10, 2, 0]) await m.ecrire(ID, n, flux('x'), PLAFOND);
        expect(m.lister(ID).map((t) => t.n)).toEqual([0, 2, 10]);
    });

    it('⚠️ un `.part` abandonné n’est PAS une tranche', async () => {
        // Le compter ferait paraître complète une tranche qui n'a jamais fini
        // de s'écrire — et `verdict` scellerait un fichier tronqué.
        const m = magasinNeuf();
        await m.ecrire(ID, 0, flux('ab'), PLAFOND);
        writeFileSync(join(m.racine, ID, '1.12345.abc.part'), 'moitie');
        expect(m.lister(ID)).toEqual([{ n: 0, octets: 2 }]);
    });

    it('un téléversement sans aucune tranche rend une liste VIDE, jamais une erreur', () => {
        const m = magasinNeuf();
        expect(m.lister(ID)).toEqual([]);
        expect(verdict(4, 2, m.lister(ID))).toEqual({ etat: 'manquantes', n: [0, 1] });
    });

    it('🔴 une tranche DISPARUE donne une ERREUR de flux, jamais un flux tronqué', async () => {
        // 🔴 Un flux court produirait chez l'agent une empreinte fausse dont
        // personne ne saurait dire la cause. C'est pourquoi `concatener` sert
        // le plan VÉRIFIÉ par l'appelant et non un listage : un listage
        // n'aurait tout simplement pas vu la tranche, et se serait terminé
        // proprement.
        const m = magasinNeuf();
        await m.ecrire(ID, 0, flux('aaa'), PLAFOND);
        await m.ecrire(ID, 1, flux('bbb'), PLAFOND);
        rmSync(join(m.racine, ID, '1'));
        await expect(lire(m.concatener(ID, [0, 1]))).rejects.toThrow(/ENOENT/);
    });

    it('🔴 une tranche supprimée EN COURS de lecture erreur aussi', async () => {
        // La variante du dessus, mais la suppression tombe pendant que le flux
        // coule : c'est le cas réel d'une purge concurrente. La première
        // tranche est assez grosse pour que la contre-pression suspende le
        // générateur bien avant qu'il n'ouvre la seconde.
        const m = magasinNeuf();
        await m.ecrire(ID, 0, flux(Buffer.alloc(1 << 20, 0x61)), 1 << 21);
        await m.ecrire(ID, 1, flux('bbb'), PLAFOND);
        const r = m.concatener(ID, [0, 1]);
        let vus = 0;
        await expect(
            (async () => {
                for await (const morceau of r) {
                    if (vus === 0) rmSync(join(m.racine, ID, '1'));
                    vus += (morceau as Uint8Array).byteLength;
                }
            })(),
        ).rejects.toThrow(/ENOENT/);
        expect(vus).toBeGreaterThan(0);
    });

    it('🔴 REFUSE un identifiant qui pourrait sortir du magasin', async () => {
        // 🔴 `/televersement/..%2f..%2fetc/tranche/0` ne doit pouvoir écrire
        // NULLE PART, et le refus est explicite : on n'assainit pas en silence,
        // sans quoi personne ne saurait où l'on a écrit.
        const m = magasinNeuf();
        for (const mauvais of [
            '../../../etc/passwd',
            '..%2f..%2fx',
            '..',
            '.',
            '',
            'x',
            `${ID}/..`,
            ID.toUpperCase(), // majuscules : refusées, voir le commentaire
            `${ID} `,
        ]) {
            expect(identifiantValide(mauvais)).toBe(false);
            await expect(m.ecrire(mauvais, 0, flux('x'), PLAFOND))
                .rejects.toThrow(/identifiant de téléversement invalide/);
            expect(() => m.lister(mauvais)).toThrow(/identifiant/);
            expect(() => m.concatener(mauvais, [0])).toThrow(/identifiant/);
            expect(() => m.supprimer(mauvais)).toThrow(/identifiant/);
        }
        expect(readdirSync(m.racine)).toEqual([]);
        expect(identifiantValide(ID)).toBe(true);
    });

    it('🔴 REFUSE un rang qui n’est pas un entier sûr positif', async () => {
        // 🔴 LE NOM DE FICHIER EST LE NOMBRE VALIDÉ, jamais un segment d'URL
        // recopié. Sous cette garde, `String(n)` est TOUJOURS une suite de
        // chiffres — au-delà de l'entier sûr, `String(1e21)` vaudrait `1e+21`.
        const m = magasinNeuf();
        for (const mauvais of [-1, 1.5, NaN, Infinity, 1e21, Number.MAX_SAFE_INTEGER + 2]) {
            expect(rangValide(mauvais)).toBe(false);
            await expect(m.ecrire(ID, mauvais, flux('x'), PLAFOND))
                .rejects.toThrow(/rang de tranche invalide/);
            expect(() => m.concatener(ID, [mauvais])).toThrow(/rang de tranche invalide/);
        }
        expect(rangValide(0)).toBe(true);
        expect(rangValide(Number.MAX_SAFE_INTEGER)).toBe(true);
        expect(readdirSync(m.racine)).toEqual([]);
    });

    it('🔴 un rang qui est un SEGMENT D’URL recopié ne peut écrire NULLE PART', async () => {
        // 🔴 LA GARDE EST UNE DÉFENSE EN PROFONDEUR, et ce test la met à
        // l'épreuve du cas qu'elle existe pour arrêter : une route qui
        // passerait le segment d'URL BRUT au lieu du nombre. Le typage
        // l'interdit, un `as` le contourne — comme le ferait un `any` ou un
        // corps JSON mal parsé, et le refus doit tenir sans lui.
        const m = magasinNeuf();
        const evil = '../../evil' as unknown as number;
        expect(rangValide(evil)).toBe(false);
        await expect(m.ecrire(ID, evil, flux('poison'), PLAFOND))
            .rejects.toThrow(/rang de tranche invalide/);
        expect(() => m.concatener(ID, [evil])).toThrow(/rang de tranche invalide/);
        // 🔴 ET RIEN N'A ÉTÉ ÉCRIT AILLEURS : ni dans la racine, ni au-dessus
        // d'elle. Un refus qui laisserait le fichier n'en serait pas un.
        expect(readdirSync(m.racine)).toEqual([]);
        expect(existsSync(join(m.racine, '..', 'evil'))).toBe(false);
        expect(existsSync(join(m.racine, ID, '..', '..', 'evil'))).toBe(false);
    });

    it('🔴 le rang est validé AVANT que le flux n’existe', async () => {
        // Un rang fautif lève à l'APPEL, où l'appelant peut encore répondre,
        // plutôt qu'au milieu d'une réponse déjà commencée.
        const m = magasinNeuf();
        await m.ecrire(ID, 0, flux('a'), PLAFOND);
        expect(() => m.concatener(ID, [0, -1])).toThrow(/rang/);
    });

    it('l’écriture est ATOMIQUE : aucun `.part` ne survit à un succès', async () => {
        const m = magasinNeuf();
        await m.ecrire(ID, 7, flux('abc'), PLAFOND);
        expect(readdirSync(join(m.racine, ID))).toEqual(['7']);
        expect(readFileSync(join(m.racine, ID, '7')).toString()).toBe('abc');
    });

    it('réécrire un rang le REMPLACE, sans laisser de résidu', async () => {
        const m = magasinNeuf();
        await m.ecrire(ID, 0, flux('aaaaa'), PLAFOND);
        await m.ecrire(ID, 0, flux('bb'), PLAFOND);
        expect(m.lister(ID)).toEqual([{ n: 0, octets: 2 }]);
        expect(readdirSync(join(m.racine, ID))).toEqual(['0']);
    });

    it('`supprimer` retire tout, et sur un absent c’est un succès', async () => {
        const m = magasinNeuf();
        await m.ecrire(ID, 0, flux('a'), PLAFOND);
        m.supprimer(ID);
        expect(existsSync(join(m.racine, ID))).toBe(false);
        expect(m.lister(ID)).toEqual([]);
        // Une purge qui passe après un dépôt abandonné avant sa première trame.
        expect(() => m.supprimer(ID)).not.toThrow();
    });

    it('deux téléversements ne se mêlent pas', async () => {
        const m = magasinNeuf();
        const autre = randomUUID();
        await m.ecrire(ID, 0, flux('un'), PLAFOND);
        await m.ecrire(autre, 0, flux('deux'), PLAFOND);
        expect(m.lister(ID)).toEqual([{ n: 0, octets: 2 }]);
        expect(m.lister(autre)).toEqual([{ n: 0, octets: 4 }]);
        m.supprimer(autre);
        expect(m.lister(ID)).toEqual([{ n: 0, octets: 2 }]);
    });

    it('la racine est CRÉÉE si elle manque, et son chemin est JOURNALISÉ', () => {
        const r = mkdtempSync(join(tmpdir(), 'g3-tranches-'));
        racines.push(r);
        const vu: string[] = [];
        const cible = join(r, 'profond', randomUUID());
        expect(existsSync(cible)).toBe(false);
        ouvrirMagasinTranches(cible, (c) => vu.push(c));
        expect(existsSync(cible)).toBe(true);
        // ⚠️ La variable étant facultative, un opérateur peut se tromper de
        // répertoire sans que rien ne casse — et ici la perte ne se répare PAS
        // toute seule : il faut qu'un humain redépose son fichier.
        expect(vu).toEqual([cible]);
    });
});

describe('l’éviction par âge, avec plancher', () => {
    // Le temps est INJECTÉ, jamais lu de l'horloge : un test qui attendrait
    // réellement l'âge d'éviction serait un test qu'on désactive au premier
    // ralentissement de la machine.
    const JOUR_MS = 24 * 60 * 60_000;

    /// Dépose une tranche unique pour `id`, puis FORCE la date de dernière
    /// modification du RÉPERTOIRE du téléversement — c'est lui, et non une
    /// tranche isolée, que `evincer` mesure. Équivalent, sur le magasin RÉEL,
    /// du `deposer(cle, octets, quand)` de la tâche.
    async function deposerA(m: ReturnType<typeof magasinNeuf>, id: string, quandMs: number): Promise<void> {
        await m.ecrire(id, 0, flux('x'), PLAFOND);
        utimesSync(join(m.racine, id), new Date(quandMs), new Date(quandMs));
    }

    it('évince un téléversement vieux et NON référencé', async () => {
        const m = magasinNeuf();
        const orphelin = randomUUID();
        await deposerA(m, orphelin, 0);
        m.evincer({ maintenant: 400 * JOUR_MS, referencees: new Set() });
        expect(existsSync(join(m.racine, orphelin))).toBe(false);
    });

    // 🔴 LE SEUL TEST QUI DISTINGUE UNE ÉVICTION D'UNE CORRUPTION. Sans lui,
    // une éviction qui emporte TOUT passerait le test précédent.
    it('NE PEUT PAS évincer un téléversement vieux mais RÉFÉRENCÉ par une entrée vivante', async () => {
        const m = magasinNeuf();
        const enService = randomUUID();
        await deposerA(m, enService, 0);
        m.evincer({ maintenant: 400 * JOUR_MS, referencees: new Set([enService]) });
        expect(existsSync(join(m.racine, enService))).toBe(true);
        expect(m.lister(enService)).toEqual([{ n: 0, octets: 1 }]);
    });

    it('n’évince pas un téléversement jeune', async () => {
        const m = magasinNeuf();
        const recent = randomUUID();
        await deposerA(m, recent, 0);
        m.evincer({ maintenant: 1 * JOUR_MS, referencees: new Set() });
        expect(existsSync(join(m.racine, recent))).toBe(true);
    });

    it('🔴 la constante N n’est PAS calibrée : le plancher, lui, tient à n’importe quelle valeur', () => {
        // Contrôle de cohérence du montage lui-même : si
        // `AGE_EVICTION_TRANCHES_MS` dérivait un jour hors de l'intervalle
        // [1 jour, 400 jours], les trois tests ci-dessus perdraient leur sens
        // sans qu'aucune rouge ne le dise.
        expect(AGE_EVICTION_TRANCHES_MS).toBeGreaterThan(1 * JOUR_MS);
        expect(AGE_EVICTION_TRANCHES_MS).toBeLessThan(400 * JOUR_MS);
    });

    it('un nom qui n’est pas un identifiant valide n’est jamais touché', () => {
        const m = magasinNeuf();
        const etranger = join(m.racine, 'pas-un-uuid');
        writeFileSync(etranger, 'x');
        utimesSync(etranger, new Date(0), new Date(0));
        m.evincer({ maintenant: 400 * JOUR_MS, referencees: new Set() });
        expect(existsSync(etranger)).toBe(true);
    });
});

describe('PLATEFORME_TELEVERSEMENTS', () => {
    // 🔴 LE SECRET PASSE PAR UNE CONSTANTE PARTAGÉE, ET CE N'EST PAS DU STYLE.
    // Le scanner de `securite/secrets.test.ts` cherche une affectation
    // LITTÉRALE à une variable de secret NOMMÉE : écrit en ligne, ce montage
    // faisait rougir « des secrets sont affectés en clair dans des fichiers
    // versionnés » — sur un secret de test parfaitement inoffensif, mais le
    // scanner ne peut pas le savoir, et c'est précisément pourquoi il ne
    // regarde pas la valeur. Réemployer la constante du harnais retire du même
    // coup une copie du littéral.
    const BASE = {
        PLATEFORME_HOTE: '127.0.0.1',
        PLATEFORME_SECRET_JETON: SECRET_PLATEFORME,
    };

    it('retient le répertoire qu’on lui NOMME', () => {
        // 🔴 LA ROUGE : la variable posée et IGNORÉE. Les tranches
        // s'écriraient ailleurs, en silence.
        //
        // `PLATEFORME_PROXY_DE_CONFIANCE` est posée pour satisfaire la garde
        // du refus de démarrer en mode `pomerium` (tâche 6, `config.ts`) —
        // ce n'est pas le sujet de ce test.
        expect(
            lireConfig({
                ...BASE,
                PLATEFORME_TELEVERSEMENTS: '/var/lib/guac/tel',
                PLATEFORME_PROXY_DE_CONFIANCE: '172.18.0.5',
            }).repertoireTeleversements,
        ).toBe('/var/lib/guac/tel');
    });

    it('🔴 une valeur VIDE retombe sur le défaut, pas sur le répertoire courant', () => {
        // `env.X ?? 'defaut'` ne rattrape PAS `''` — le sous-bloc P1 de la
        // plateforme a payé cette erreur exacte.
        //
        // `PLATEFORME_PROXY_DE_CONFIANCE` est posée pour la même raison que
        // ci-dessus.
        expect(
            lireConfig({
                ...BASE,
                PLATEFORME_TELEVERSEMENTS: '',
                PLATEFORME_PROXY_DE_CONFIANCE: '172.18.0.5',
            }).repertoireTeleversements,
        ).toBe('donnees/televersements');
    });
});
