// Le nettoyage de fond des deux magasins, sous `test:sqlite` ET sous
// `test:postgres` — comme tout ce qui touche la base.
//
// 🔴 CE FICHIER EST LA RÉPONSE AU ROUND DE CORRECTION 1 : `evincer` des deux
// magasins était un mécanisme écrit, testé, documenté — et appelé par
// PERSONNE. Ici, l'ensemble des références vient de la BASE RÉELLE, jamais
// d'un paramètre construit à la main : un `unTour` qui recevrait un ensemble
// vide évincerait tout ce qui est vieux, y compris ce qui sert — la
// corruption exacte que le plancher existe pour empêcher. Le test qui prouve
// que PERSONNE N'APPELLE PLUS `evincer` — le défaut précis du round 1 — vit
// dans `http/serveur.test.ts`, parce que c'est `demarrerServeur` qui doit le
// faire, pas ce fichier-ci.

import { createHash } from 'node:crypto';
import { existsSync, mkdtempSync, rmSync, utimesSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { baseNeuve, MOTEUR } from '../base/harnais';
import type { Pilote } from '../base/pilote';
import { appliquer } from '../depot/application';
import {
    creer as creerTeleversement,
    lireParId as lireTeleversement,
} from '../depot/televersement';
import { creer as creerInstallation } from '../depot/installation';
import type { Application } from '../../../proto/ts/plateforme-apps';
import { ouvrirMagasin, type Magasin } from './icones';
import { ouvrirMagasinTranches, type MagasinTranches } from './magasin-tranches';
import { demarrerNettoyage, referencesIcones, referencesTranches, unTour } from './nettoyage';

let base: Pilote | undefined;
let racines: string[] = [];

afterEach(async () => {
    await base?.fermer();
    base = undefined;
    for (const r of racines) rmSync(r, { recursive: true, force: true });
    racines = [];
});

/// La magnitude qui a réellement cassé Postgres en P1 — même valeur que
/// `depot/application.test.ts` et `depot/installation.test.ts`, pour rester
/// dans la même famille de fixtures.
const MS = 1_787_136_773_742;
const JOUR_MS = 24 * 60 * 60_000;

function magasinIconesNeuf(): Magasin {
    const r = mkdtempSync(join(tmpdir(), 'nettoyage-icones-'));
    racines.push(r);
    return ouvrirMagasin(join(r, 'icones'), () => {});
}

function magasinTranchesNeuf(): MagasinTranches {
    const r = mkdtempSync(join(tmpdir(), 'nettoyage-tranches-'));
    racines.push(r);
    return ouvrirMagasinTranches(join(r, 'televersements'), () => {});
}

async function avecVm(p: Pilote, id: string): Promise<void> {
    await p.executer('INSERT INTO vm(id,nom,adresse) VALUES(?,?,?)', [id, `vm-${id}`, '192.168.3.2']);
}

async function avecUtilisateur(p: Pilote, id: string): Promise<void> {
    await p.executer('INSERT INTO utilisateur(id,email,empreinte_mdp,cree_a) VALUES(?,?,?,?)', [
        id,
        `${id}@exemple.test`,
        'scrypt$1$1$1$x$y',
        MS,
    ]);
}

function icone(texte: string): { empreinte: string; octets: Buffer } {
    const octets = Buffer.from(texte);
    return { empreinte: createHash('sha256').update(octets).digest('hex'), octets };
}

function appAvecIcone(nom: string, cle: string, empreinteIcone: string | null): Application {
    return {
        cle,
        nom,
        chemin: `C:\\Users\\guacamole\\Desktop\\${nom}.lnk`,
        cible: `c:\\program files\\${nom}\\${nom}.exe`,
        arguments: '',
        repertoire: `c:\\program files\\${nom}`,
        icone: empreinteIcone,
        source_max: 'non-mesuree',
        accent: null,
        associations: [],
    };
}

/// Force la date de dernière modification d'un chemin — le temps est INJECTÉ
/// dans la mesure d'âge, jamais lu de l'horloge : même règle qu'`icones.test.ts`
/// et `magasin-tranches.test.ts`.
function vieillir(chemin: string, quandMs: number): void {
    utimesSync(chemin, new Date(quandMs), new Date(quandMs));
}

/// Un flux d'un seul morceau — suffisant pour ce fichier, qui n'éprouve pas
/// le découpage lui-même (voir `magasin-tranches.test.ts` pour ça).
function flux(s: string): AsyncIterable<Uint8Array> {
    return (async function* () {
        yield Buffer.from(s);
    })();
}

describe(`nettoyage de fond, moteur=${MOTEUR}`, () => {
    it('🔴 referencesIcones VIENT DE LA BASE : une entrée vivante rend un ensemble NON VIDE', async () => {
        // Le garde du danger nommé par le round 2 : un ensemble VIDE alors
        // qu'une application vivante existe évincerait cette icône — c'est
        // exactement la corruption que le plancher existe pour empêcher.
        base = await baseNeuve('nettoyage-refs-icones-non-vide');
        await avecVm(base, 'v1');
        const { empreinte } = icone('vivante');
        await appliquer(
            base,
            'v1',
            { aInserer: [appAvecIcone('X', 'a'.repeat(64), empreinte)], aMettreAJour: [], aMarquerDisparues: [], aRessusciter: [] },
            MS,
        );
        const refs = await referencesIcones(base);
        expect(refs.size).toBeGreaterThan(0);
        expect(refs).toEqual(new Set([empreinte]));
    });

    it('une entrée DISPARUE ne référence plus son icône', async () => {
        base = await baseNeuve('nettoyage-refs-icones-disparue');
        await avecVm(base, 'v1');
        const { empreinte } = icone('feu-vivante');
        const cle = 'b'.repeat(64);
        await appliquer(
            base,
            'v1',
            { aInserer: [appAvecIcone('X', cle, empreinte)], aMettreAJour: [], aMarquerDisparues: [], aRessusciter: [] },
            MS,
        );
        const [{ id }] = await base.interroger<{ id: string }>(
            'SELECT id FROM application WHERE cle = ?',
            [cle],
        );
        await appliquer(base, 'v1', { aInserer: [], aMettreAJour: [], aMarquerDisparues: [id], aRessusciter: [] }, MS + 1);
        expect(await referencesIcones(base)).toEqual(new Set());
    });

    it('une entrée MASQUÉE référence ENCORE son icône — masquer n’est pas disparaître', async () => {
        base = await baseNeuve('nettoyage-refs-icones-masquee');
        await avecVm(base, 'v1');
        const { empreinte } = icone('masquable');
        const cle = 'c'.repeat(64);
        await appliquer(
            base,
            'v1',
            { aInserer: [appAvecIcone('X', cle, empreinte)], aMettreAJour: [], aMarquerDisparues: [], aRessusciter: [] },
            MS,
        );
        const [{ id }] = await base.interroger<{ id: string }>(
            'SELECT id FROM application WHERE cle = ?',
            [cle],
        );
        await base.executer('UPDATE application SET masquee_a = ? WHERE id = ?', [MS + 1, id]);
        // ⚠️ SI CE TEST ROUGIT PARCE QUE L'ENSEMBLE EST VIDE, LA RÈGLE EST
        // FAUSSE DANS LE SENS DANGEREUX : elle évincerait l'icône d'une
        // application que l'utilisateur peut encore démasquer.
        expect(await referencesIcones(base)).toEqual(new Set([empreinte]));
    });

    it('un TOUR RÉEL, contre la base : vieille+référencée reste, vieille+orpheline part, jeune reste', async () => {
        base = await baseNeuve('nettoyage-tour-icones');
        await avecVm(base, 'v1');
        const enService = icone('en-service-tour');
        await appliquer(
            base,
            'v1',
            {
                aInserer: [appAvecIcone('X', 'd'.repeat(64), enService.empreinte)],
                aMettreAJour: [],
                aMarquerDisparues: [],
                aRessusciter: [],
            },
            MS,
        );

        const magasin = magasinIconesNeuf();
        const orpheline = icone('orpheline-tour');
        const recente = icone('recente-tour');
        magasin.ecrire(enService.empreinte, enService.octets);
        magasin.ecrire(orpheline.empreinte, orpheline.octets);
        magasin.ecrire(recente.empreinte, recente.octets);
        vieillir(join(magasin.repertoire, enService.empreinte), 0);
        vieillir(join(magasin.repertoire, orpheline.empreinte), 0);
        // `recente` garde sa date d'écriture réelle (maintenant) : jeune par
        // construction, sans qu'il faille la forcer.

        const tranches = magasinTranchesNeuf();
        await unTour({ base, magasin, tranches, maintenant: 400 * JOUR_MS });

        expect(magasin.possede(enService.empreinte)).toBe(true);
        expect(magasin.possede(orpheline.empreinte)).toBe(false);
        expect(magasin.possede(recente.empreinte)).toBe(true);
    });

    it('🔴 referencesTranches VIENT DE LA BASE : une ligne existante rend un ensemble NON VIDE', async () => {
        base = await baseNeuve('nettoyage-refs-tranches-non-vide');
        await avecUtilisateur(base, 'u1');
        const t = await creerTeleversement(
            base,
            { utilisateurId: 'u1', nom: 'x.exe', taille: 1, sha256: 'e'.repeat(64), tailleTranche: 8 },
            MS,
        );
        const refs = await referencesTranches(base);
        expect(refs.size).toBeGreaterThan(0);
        expect(refs).toEqual(new Set([t.id]));
    });

    it(
        '🔴 un TOUR RÉEL, contre la base : la clé étrangère d’installation PROTÈGE ' +
            'la ligne — et donc le disque —, un téléversement vraiment orphelin perd les DEUX',
        async () => {
            base = await baseNeuve('nettoyage-tour-tranches');
            await avecUtilisateur(base, 'u1');
            await avecVm(base, 'v1');

            // A : vieux, mais une installation le référence encore.
            const a = await creerTeleversement(
                base,
                { utilisateurId: 'u1', nom: 'a.exe', taille: 1, sha256: 'f'.repeat(64), tailleTranche: 8 },
                MS - 400 * JOUR_MS,
            );
            await creerInstallation(base, { vmId: 'v1', televersementId: a.id }, MS);

            // B : vieux, et personne ne le référence — le cas ORPHELIN.
            const b = await creerTeleversement(
                base,
                { utilisateurId: 'u1', nom: 'b.exe', taille: 1, sha256: 'a'.repeat(64), tailleTranche: 8 },
                MS - 400 * JOUR_MS,
            );

            // C : jeune — protégé par son âge, indépendamment de toute référence.
            const c = await creerTeleversement(
                base,
                { utilisateurId: 'u1', nom: 'c.exe', taille: 1, sha256: 'b'.repeat(64), tailleTranche: 8 },
                MS,
            );

            const tranches = magasinTranchesNeuf();
            await tranches.ecrire(a.id, 0, flux('a'), 1024);
            await tranches.ecrire(b.id, 0, flux('b'), 1024);
            await tranches.ecrire(c.id, 0, flux('c'), 1024);
            vieillir(join(tranches.racine, a.id), 0);
            vieillir(join(tranches.racine, b.id), 0);
            // `c` garde sa date d'écriture réelle : jeune par construction.

            const magasin = magasinIconesNeuf();
            await unTour({ base, magasin, tranches, maintenant: MS });

            // A : la ligne SURVIT (refus de la clé étrangère), donc le disque aussi.
            expect(await lireTeleversement(base, a.id)).toBeDefined();
            expect(existsSync(join(tranches.racine, a.id))).toBe(true);

            // B : ligne ET disque disparaissent — l'orphelin réel.
            expect(await lireTeleversement(base, b.id)).toBeUndefined();
            expect(existsSync(join(tranches.racine, b.id))).toBe(false);

            // C : trop jeune pour être même candidat à la purge de ligne.
            expect(await lireTeleversement(base, c.id)).toBeDefined();
            expect(existsSync(join(tranches.racine, c.id))).toBe(true);
        },
    );

    // 🔴 LE CRITIQUE DU ROUND DE CORRECTION 2 — LES DEUX PLANCHERS NE
    // MESURAIENT PAS LE MÊME ÂGE. La purge de LIGNE filtrait sur `cree_a` ;
    // l'éviction du DISQUE filtre sur `mtime`, la DERNIÈRE ACTIVITÉ — ce
    // n'est PAS la même chose, et l'écart est DÉTERMINISTE, pas une course :
    // un téléversement créé il y a 31 jours dont une tranche vient
    // d'arriver À L'INSTANT voyait sa ligne supprimée (candidate par
    // `cree_a`, aucune installation ne la référençant) alors que son
    // répertoire restait — la reprise meurt, le disque n'est même pas
    // libéré. Ce test rejoue EXACTEMENT ce scénario.
    it(
        '🔴 CRITIQUE : un téléversement CRÉÉ il y a 31 jours, dont une tranche ' +
            'vient d’ARRIVER, conserve SA LIGNE ET SON DISQUE',
        async () => {
            base = await baseNeuve('nettoyage-critique-planchers');
            await avecUtilisateur(base, 'u1');

            const maintenant = MS;
            const t = await creerTeleversement(
                base,
                {
                    utilisateurId: 'u1',
                    nom: 'actif.exe',
                    taille: 1,
                    sha256: 'c'.repeat(64),
                    tailleTranche: 8,
                },
                maintenant - 31 * JOUR_MS,
            );

            const tranches = magasinTranchesNeuf();
            await tranches.ecrire(t.id, 0, flux('x'), 1024);
            // La tranche « vient d'arriver » : sa date d'activité est FORCÉE
            // à `maintenant`, jamais laissée à vieillir — c'est elle qui
            // distingue ce scénario du cas orphelin réel (le test au-dessus).
            vieillir(join(tranches.racine, t.id), maintenant);

            const magasin = magasinIconesNeuf();
            await unTour({ base, magasin, tranches, maintenant });

            // AVANT LE REMÈDE : la ligne aurait disparu (31 jours > les 30
            // du plancher de LIGNE) alors que le disque restait intact (le
            // plancher de DISQUE, lui, la voit jeune) — la corruption
            // déterministe. APRÈS : les deux planchers s'accordent, et les
            // DEUX survivent.
            expect(await lireTeleversement(base, t.id)).toBeDefined();
            expect(existsSync(join(tranches.racine, t.id))).toBe(true);
        },
    );

    // ⚠️ Important ① (round de correction 2) : le refus de clé étrangère —
    // le SEUL échec ATTENDU de la purge de ligne — reste MUET ; tout le
    // reste se journalise. Les deux bras du même mécanisme, dans deux tests
    // séparés pour qu'on ne les confonde pas.
    it('un refus de clé étrangère (ATTENDU) ne journalise RIEN', async () => {
        base = await baseNeuve('nettoyage-fk-muet');
        await avecUtilisateur(base, 'u1');
        await avecVm(base, 'v1');
        const t = await creerTeleversement(
            base,
            { utilisateurId: 'u1', nom: 'ref.exe', taille: 1, sha256: 'e'.repeat(64), tailleTranche: 8 },
            MS - 400 * JOUR_MS,
        );
        await creerInstallation(base, { vmId: 'v1', televersementId: t.id }, MS);

        const tranches = magasinTranchesNeuf();
        await tranches.ecrire(t.id, 0, flux('x'), 1024);
        vieillir(join(tranches.racine, t.id), 0);

        const espion = vi.spyOn(console, 'error').mockImplementation(() => {});
        try {
            const magasin = magasinIconesNeuf();
            await unTour({ base, magasin, tranches, maintenant: MS });
            expect(espion).not.toHaveBeenCalled();
        } finally {
            espion.mockRestore();
        }
    });

    it('🔴 un échec de purge qui N’EST PAS une clé étrangère SE JOURNALISE', async () => {
        base = await baseNeuve('nettoyage-fk-pas-muet');
        await avecUtilisateur(base, 'u1');
        // Vraiment orphelin — rien ne le référence, la suppression de sa
        // ligne RÉUSSIRAIT normalement (voir le test « un TOUR RÉEL… » plus
        // haut) : c'est ce qui rend la faute injectée ci-dessous imputable
        // au `catch`, jamais à la clé étrangère.
        const t = await creerTeleversement(
            base,
            { utilisateurId: 'u1', nom: 'orph.exe', taille: 1, sha256: 'f'.repeat(64), tailleTranche: 8 },
            MS - 400 * JOUR_MS,
        );
        const tranches = magasinTranchesNeuf();
        await tranches.ecrire(t.id, 0, flux('x'), 1024);
        vieillir(join(tranches.racine, t.id), 0);

        // Un pilote qui fait échouer SPÉCIFIQUEMENT le DELETE, d'une cause
        // SANS RAPPORT avec une clé étrangère — la mutation jouée par la
        // revue elle-même (« base coupée »), reproduite ici comme fixture.
        const basePannee: Pilote = {
            ...base,
            async executer(sql, params) {
                if (sql.startsWith('DELETE FROM televersement')) {
                    throw new Error('base coupee — rien a voir avec une cle etrangere');
                }
                return base!.executer(sql, params);
            },
        };

        const espion = vi.spyOn(console, 'error').mockImplementation(() => {});
        try {
            const magasin = magasinIconesNeuf();
            await unTour({ base: basePannee, magasin, tranches, maintenant: MS });
            expect(espion).toHaveBeenCalledTimes(1);
            expect(espion.mock.calls[0][0]).toContain(t.id);
            expect(espion.mock.calls[0][0]).toContain('base coupee');
            // La ligne n'a PAS été supprimée : l'échec a bien empêché la
            // suppression, il ne l'a pas seulement rendue muette.
            expect(await lireTeleversement(base, t.id)).toBeDefined();
        } finally {
            espion.mockRestore();
        }
    });

    // ⚠️ Important ③ (round de correction 2) : `arreter()` empêche
    // réellement les tours SUIVANTS. `clearInterval` n'interrompt pas un
    // tour déjà en vol — ce test ne prouve QUE l'absence de tours après
    // l'appel, pas l'interruption d'un tour en cours, exactement ce que le
    // commentaire corrigé d'`arreter()` promet et rien de plus.
    it('🔴 arreter() empêche VRAIMENT les tours SUIVANTS', async () => {
        base = await baseNeuve('nettoyage-arret-reel');
        let tours = 0;
        const magasinReel = magasinIconesNeuf();
        const magasin: typeof magasinReel = {
            ...magasinReel,
            async evincer(opts) {
                tours += 1;
                return magasinReel.evincer(opts);
            },
        };
        const tranches = magasinTranchesNeuf();

        const nettoyage = await demarrerNettoyage(
            { base, magasin, tranches, maintenant: () => MS },
            20, // periodeMs minuscule — un test, jamais une valeur livrée.
        );
        expect(tours).toBe(1); // le premier tour, ATTENDU.

        nettoyage.arreter();
        // Largement plus long que `periodeMs` (20 ms) : si `arreter()`
        // n'empêchait rien, plusieurs tours de plus auraient eu le temps de
        // s'exécuter dans cette fenêtre.
        await new Promise((resolve) => setTimeout(resolve, 150));
        expect(tours).toBe(1);
    });

    // 🔴 DURCISSEMENT (round de correction 3) : une ligne MALFORMÉE (id qui
    // n'est pas un UUID — NON ATTEIGNABLE aujourd'hui, `creer` étant le seul
    // `INSERT`, mais provoquée ici directement en SQL) ne doit PAS faire
    // abandonner le tour entier. L'ordre de retour de `lirePlusVieuxQue`
    // n'étant pas garanti, ce test ne suppose AUCUN ordre : que la ligne
    // fautive soit vue avant ou après l'orpheline légitime, celle-ci doit,
    // dans tous les cas, finir purgée — ligne ET disque.
    it('une ligne malformée n’abandonne pas le tour : l’orpheline voisine est quand même évincée', async () => {
        base = await baseNeuve('nettoyage-durcissement-id-malforme');
        await avecUtilisateur(base, 'u1');

        const vieux = MS - 400 * JOUR_MS;
        // La ligne FAUTIVE, posée directement en SQL — jamais par `creer`.
        await base.executer(
            'INSERT INTO televersement(id,utilisateur_id,nom,taille,sha256,taille_tranche,cree_a,scelle_a)'
                + ' VALUES(?,?,?,?,?,?,?,?)',
            ['pas-un-uuid', 'u1', 'fautif.exe', 1, 'a'.repeat(64), 8, vieux, null],
        );
        // L'orpheline LÉGITIME, par le chemin normal.
        const orpheline = await creerTeleversement(
            base,
            { utilisateurId: 'u1', nom: 'orph.exe', taille: 1, sha256: 'b'.repeat(64), tailleTranche: 8 },
            vieux,
        );

        const tranches = magasinTranchesNeuf();
        await tranches.ecrire(orpheline.id, 0, flux('x'), 1024);
        vieillir(join(tranches.racine, orpheline.id), 0);

        const magasin = magasinIconesNeuf();
        await unTour({ base, magasin, tranches, maintenant: MS });

        expect(await lireTeleversement(base, orpheline.id)).toBeUndefined();
        expect(existsSync(join(tranches.racine, orpheline.id))).toBe(false);
    });
});
