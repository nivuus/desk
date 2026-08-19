// L'unique implémentation d'`Orchestrateur` de la v1, sous les DEUX moteurs.
//
// 🔴 L'HORLOGE EST INJECTÉE, et c'est ce qui rend la borne du critère ④
// assiégeable des deux côtés. Un `Date.now()` lu dans le module ne laisserait
// qu'un seul instant observable, et le seuil ne serait jamais franchi dans une
// exécution de test.
//
// ⚠️ SEIZE TESTS ET NON LES QUINZE DU PLAN, annoncé avant d'être lu : le plan
// ne donne aucune ligne à `lister`, alors que la conversion `LigneVm` -> `Vm`
// (snake_case vers camelCase) est du code qui peut casser en silence.

import { afterEach, describe, expect, it, vi } from 'vitest';
import { baseNeuve, MOTEUR } from '../base/harnais';
import type { Pilote } from '../base/pilote';
import { SEUIL_INJOIGNABLE_MS } from '../agents/fraicheur';
import { enroler, marquerVu } from '../depot/agent';
import { lireParId } from '../depot/vm';
import { creerUtilisateur } from '../depot/utilisateur';
import { BACKEND_STATIQUE } from './refus';
import { inventaireStatique } from './inventaire-statique';

const MS = 1_787_136_773_742;

let base: Pilote | undefined;

afterEach(async () => {
    await base?.fermer();
    base = undefined;
    vi.restoreAllMocks();
});

async function poserVm(p: Pilote, id: string, nom: string): Promise<void> {
    await p.executer('INSERT INTO vm(id, nom, adresse) VALUES(?, ?, ?)', [id, nom, '192.168.3.2']);
}

async function poserUtilisateur(p: Pilote, email: string): Promise<string> {
    return creerUtilisateur(p, email, 'empreinte-opaque-de-test', MS);
}

/// Une horloge FIGÉE mais réglable : chaque test pose l'instant qu'il éprouve.
function horlogeA(instant: number): () => number {
    return () => instant;
}

describe(`inventaireStatique, moteur=${MOTEUR}`, () => {
    it('🔴 `instantane` REFUSE par un type — motif, opération, backend', async () => {
        // 🔴 La rouge : le remplacer par un `return` silencieux. C'est
        // LITTÉRALEMENT le critère ① — « une opération que le backend ne sait
        // pas faire rend un refus typé, jamais un silence ».
        base = await baseNeuve('inv-instantane');
        await poserVm(base, 'v1', 'w1');
        const o = inventaireStatique(base, horlogeA(MS));
        expect(await o.instantane('v1', 'avant-mise-a-jour')).toEqual({
            ok: false,
            motif: 'non-supporte',
            operation: 'instantane',
            backend: BACKEND_STATIQUE,
        });
    });

    it('🔴 `instantane` JOURNALISE son refus', async () => {
        // 🔴 La rouge : retirer le `console.warn`. ⚠️ `it()` DISTINCT du
        // précédent, et c'est la leçon ①A/①A-bis de P2 : `expect` interrompt
        // un test à sa première assertion fausse, si bien qu'une seconde
        // assertion placée ici ne serait éprouvée par RIEN.
        base = await baseNeuve('inv-instantane-journal');
        await poserVm(base, 'v1', 'w1');
        const avertir = vi.spyOn(console, 'warn').mockImplementation(() => {});
        const o = inventaireStatique(base, horlogeA(MS));
        await o.instantane('v1', 'avant-mise-a-jour');
        expect(avertir).toHaveBeenCalledTimes(1);
        // La ligne nomme l'opération ET le backend : un refus qu'on lit dans un
        // journal sans savoir de quoi il parle n'informe pas.
        const ligne = String(avertir.mock.calls[0][0]);
        expect(ligne).toContain('instantane');
        expect(ligne).toContain(BACKEND_STATIQUE);
    });

    it('`demarrer` refuse de même', async () => {
        // ⚠️ Les trois verbes partagent UNE SEULE fonction `refuser…`, si bien
        // que ce test éprouve la même ligne que le précédent. Il existe quand
        // même : un verbe qui cesserait un jour de passer par elle ne se
        // verrait pas autrement. Le coût est deux tests presque identiques, et
        // il est payé.
        base = await baseNeuve('inv-demarrer');
        await poserVm(base, 'v1', 'w1');
        const o = inventaireStatique(base, horlogeA(MS));
        expect(await o.demarrer('v1')).toEqual({
            ok: false,
            motif: 'non-supporte',
            operation: 'demarrer',
            backend: BACKEND_STATIQUE,
        });
    });

    it('`arreter` refuse de même', async () => {
        // ⚠️ `arreter` n'est nommé par AUCUN critère de la spec — `instantane`
        // seul l'est. Il est refusé quand même, pour la raison de §3.6 : une
        // opération que le backend ne sait pas faire rend un refus typé. Le
        // dire ici évite qu'un lecteur croie à un oubli.
        base = await baseNeuve('inv-arreter');
        await poserVm(base, 'v1', 'w1');
        const o = inventaireStatique(base, horlogeA(MS));
        expect(await o.arreter('v1')).toEqual({
            ok: false,
            motif: 'non-supporte',
            operation: 'arreter',
            backend: BACKEND_STATIQUE,
        });
    });

    it('🔴 `lister` convertit la ligne en `Vm`, sans perdre un champ', async () => {
        // 🔴 La rouge : oublier un champ dans la conversion `LigneVm` -> `Vm`.
        // `prefixe_session` -> `prefixe` et `vu_a` -> `vuA` sont deux
        // renommages, donc deux occasions de rendre `undefined` en silence —
        // et `undefined` n'est pas `null` : `etatDe` distingue le second.
        base = await baseNeuve('inv-lister');
        await poserVm(base, 'v1', 'w1');
        await enroler(base, 'v1', 'empreinte-opaque', 'PREFIXEv1');
        await marquerVu(base, 'v1', MS);
        const alice = await poserUtilisateur(base, 'alice@exemple.test');
        await inventaireStatique(base, horlogeA(MS)).attribuer('v1', alice);

        const [vm] = await inventaireStatique(base, horlogeA(MS)).lister();
        expect(vm).toEqual({
            id: 'v1',
            nom: 'w1',
            adresse: '192.168.3.2',
            utilisateurId: alice,
            prefixe: 'PREFIXEv1',
            vuA: MS,
        });
    });

    it('🔴 `etat` : `prete` à vu_a + SEUIL EXACTEMENT, `injoignable` une ms plus tard', async () => {
        // 🔴 La rouge : figer l'horloge injectée (par exemple lire `Date.now()`
        // dans le module). La borne ne serait plus assiégée des deux côtés, et
        // un seuil jamais atteint ne prouve rien. La borne est FRANCHE et du
        // côté de `prete` — `agents/fraicheur.ts` l'écrit ainsi précisément
        // pour cela.
        base = await baseNeuve('inv-etat-borne');
        await poserVm(base, 'v1', 'w1');
        await enroler(base, 'v1', 'empreinte-opaque', 'PREFIXEv1');
        await marquerVu(base, 'v1', MS);

        expect(await inventaireStatique(base, horlogeA(MS + SEUIL_INJOIGNABLE_MS)).etat('v1'))
            .toBe('prete');
        expect(await inventaireStatique(base, horlogeA(MS + SEUIL_INJOIGNABLE_MS + 1)).etat('v1'))
            .toBe('injoignable');
    });

    it('🔴 `etat` d’une VM JAMAIS VUE rend `injoignable`', async () => {
        // 🔴 La rouge : rendre `prete`. `vu_a` naît `null` à l'enrôlement
        // (`depot/agent.ts`) : une VM enrôlée mais jamais démarrée serait
        // annoncée prête, et l'erreur ne se verrait qu'au moment d'ouvrir une
        // session sur une machine éteinte.
        base = await baseNeuve('inv-etat-jamais-vue');
        await poserVm(base, 'v1', 'w1');
        await enroler(base, 'v1', 'empreinte-opaque', 'PREFIXEv1');
        expect(await inventaireStatique(base, horlogeA(MS)).etat('v1')).toBe('injoignable');
    });

    it('`etat` d’une VM INCONNUE rend `injoignable`, jamais une exception', async () => {
        // 🔴 La rouge : lever. Une exception remonterait en 500 là où il n'y a
        // rien d'anormal — et sur une route ce serait un oracle d'énumération.
        base = await baseNeuve('inv-etat-inconnue');
        expect(await inventaireStatique(base, horlogeA(MS)).etat('v-inexistante'))
            .toBe('injoignable');
    });

    it('🔴 `attribuer` sur une VM libre rend `{ok:true}`', async () => {
        base = await baseNeuve('inv-attrib-ok');
        await poserVm(base, 'v1', 'w1');
        const alice = await poserUtilisateur(base, 'alice@exemple.test');
        expect(await inventaireStatique(base, horlogeA(MS)).attribuer('v1', alice))
            .toEqual({ ok: true });
    });

    it('🔴 …et la LIGNE porte le propriétaire', async () => {
        // ⚠️ `it()` DISTINCT : le verdict et l'état relu sont deux assertions,
        // et un `attribuer` qui rendrait `{ok:true}` sans rien écrire passerait
        // la première.
        base = await baseNeuve('inv-attrib-ok-etat');
        await poserVm(base, 'v1', 'w1');
        const alice = await poserUtilisateur(base, 'alice@exemple.test');
        await inventaireStatique(base, horlogeA(MS)).attribuer('v1', alice);
        expect((await lireParId(base, 'v1'))?.utilisateur_id).toBe(alice);
    });

    it('🔴 `attribuer` sur une VM DÉJÀ PRISE rend `vm-deja-attribuee`', async () => {
        // 🔴 La rouge : retirer la clause `AND utilisateur_id IS NULL` de
        // `depot/vm.ts`. MESURÉ sur les deux moteurs : le vol passe alors.
        base = await baseNeuve('inv-attrib-prise');
        await poserVm(base, 'v1', 'w1');
        const alice = await poserUtilisateur(base, 'alice@exemple.test');
        const bob = await poserUtilisateur(base, 'bob@exemple.test');
        const o = inventaireStatique(base, horlogeA(MS));
        await o.attribuer('v1', alice);
        expect(await o.attribuer('v1', bob)).toEqual({
            ok: false,
            motif: 'vm-deja-attribuee',
            operation: 'attribuer',
            backend: BACKEND_STATIQUE,
        });
    });

    it('🔴 …et le propriétaire N’A PAS changé', async () => {
        // ⚠️ `it()` DISTINCT : c'est la propriété ②a elle-même, et elle porte
        // sur l'ÉTAT, pas sur le verdict.
        base = await baseNeuve('inv-attrib-prise-etat');
        await poserVm(base, 'v1', 'w1');
        const alice = await poserUtilisateur(base, 'alice@exemple.test');
        const bob = await poserUtilisateur(base, 'bob@exemple.test');
        const o = inventaireStatique(base, horlogeA(MS));
        await o.attribuer('v1', alice);
        await o.attribuer('v1', bob);
        expect((await lireParId(base, 'v1'))?.utilisateur_id).toBe(alice);
    });

    it('🔴 `attribuer` à qui a DÉJÀ une VM rend `utilisateur-servi` et NE LÈVE PAS', async () => {
        // 🔴 La rouge : supprimer la traduction. L'exception d'unicité
        // remonterait, et la couche HTTP rendrait **500** — c'est la troisième
        // assertion du critère ②, « violation d'index traduite en refus typé,
        // jamais en 500 ».
        base = await baseNeuve('inv-attrib-servi');
        await poserVm(base, 'v1', 'w1');
        await poserVm(base, 'v2', 'w2');
        const alice = await poserUtilisateur(base, 'alice@exemple.test');
        const o = inventaireStatique(base, horlogeA(MS));
        await o.attribuer('v1', alice);
        expect(await o.attribuer('v2', alice)).toEqual({
            ok: false,
            motif: 'utilisateur-servi',
            operation: 'attribuer',
            backend: BACKEND_STATIQUE,
        });
        expect((await lireParId(base, 'v2'))?.utilisateur_id).toBeNull();
    });

    it('`attribuer` sur une VM INCONNUE rend `vm-inconnue`', async () => {
        base = await baseNeuve('inv-attrib-inconnue');
        const alice = await poserUtilisateur(base, 'alice@exemple.test');
        expect(await inventaireStatique(base, horlogeA(MS)).attribuer('v-inexistante', alice))
            .toEqual({
                ok: false,
                motif: 'vm-inconnue',
                operation: 'attribuer',
                backend: BACKEND_STATIQUE,
            });
    });

    it('🔴 une exception ÉTRANGÈRE à l’unicité est RELANCÉE, jamais traduite', async () => {
        // 🔴 La rouge : traduire TOUTE exception en `utilisateur-servi`. Une
        // base injoignable serait alors présentée comme un refus métier — la
        // panne muette exacte que la spec §6 interdit (« il ne démarre pas
        // dégradé »). C'est le SEUL endroit du service où une exception est
        // rattrapée, et ce `catch` ne doit jamais devenir muet.
        //
        // Le pilote factice répond aux lectures et LÈVE à l'écriture.
        const factice: Pilote = {
            async interroger<T>(): Promise<T[]> {
                // Une VM libre, et l'utilisateur n'en a aucune : les trois
                // contrôles préalables passent, et l'on atteint l'écriture.
                return [
                    {
                        id: 'v1',
                        nom: 'w1',
                        adresse: '192.168.3.2',
                        utilisateur_id: null,
                        prefixe_session: 'PREFIXEv1',
                        vu_a: MS,
                    },
                ] as unknown as T[];
            },
            async executer() {
                throw new Error('base injoignable');
            },
            async transaction<T>(corps: (p: Pilote) => Promise<T>): Promise<T> {
                return corps(factice);
            },
            async fermer() {},
        };
        await expect(
            inventaireStatique(factice, horlogeA(MS)).attribuer('v1', 'u-ada'),
        ).rejects.toThrow(/base injoignable/);
    });

    it('🔴 la VIOLATION D’INDEX est traduite en `utilisateur-servi`, jamais relancée', async () => {
        // 🔴 CE TEST EXISTE PARCE QU'UNE MUTATION EST RESTÉE VERTE SANS LUI, et
        // c'est le seul qui atteigne le `catch`. Le test « attribuer à qui a
        // déjà une VM » passe par la LECTURE PRÉALABLE et n'arrive jamais à
        // l'écriture : rendre le `catch` entièrement relançant laissait donc
        // les seize tests verts, et la troisième assertion du critère ② —
        // « violation d'index traduite en refus typé, JAMAIS en 500 » —
        // n'était éprouvée par RIEN. MESURÉ, puis réparé ici.
        //
        // Le cas est celui de la COURSE : entre la lecture et l'écriture,
        // l'utilisateur a acquis une autre VM ailleurs. La lecture préalable ne
        // peut structurellement pas le voir ; c'est l'index partiel qui LÈVE, et
        // c'est la relecture d'après `ROLLBACK` qui l'explique.
        //
        // 🔴 La rouge : faire relancer le `catch` sans traduire. La couche HTTP
        // rendrait alors 500 sur ce qui est un refus métier.
        let lectures = 0;
        const enCourse: Pilote = {
            async interroger<T>(): Promise<T[]> {
                lectures += 1;
                const v1 = {
                    id: 'v1', nom: 'w1', adresse: '192.168.3.2',
                    utilisateur_id: null, prefixe_session: 'PREFIXEv1', vu_a: MS,
                };
                // La PREMIÈRE lecture, celle de la transaction, ne voit rien à
                // l'utilisateur : les trois contrôles passent.
                // La SECONDE, celle d'après le `ROLLBACK`, voit la VM qu'il
                // vient d'acquérir — et c'est elle qui explique l'exception.
                const v2 = {
                    id: 'v2', nom: 'w2', adresse: '192.168.3.2',
                    utilisateur_id: lectures === 1 ? null : 'u-ada',
                    prefixe_session: 'PREFIXEv2', vu_a: MS,
                };
                return [v1, v2] as unknown as T[];
            },
            async executer() {
                // Le texte imite un moteur, et AUCUN code ne le lit : les deux
                // moteurs n'écrivent pas le même, et c'est l'état relu qui
                // tranche.
                throw new Error('UNIQUE constraint failed: vm.utilisateur_id');
            },
            async transaction<T>(corps: (p: Pilote) => Promise<T>): Promise<T> {
                return corps(enCourse);
            },
            async fermer() {},
        };

        expect(await inventaireStatique(enCourse, horlogeA(MS)).attribuer('v1', 'u-ada')).toEqual({
            ok: false,
            motif: 'utilisateur-servi',
            operation: 'attribuer',
            backend: BACKEND_STATIQUE,
        });
        // La relecture a bien EU LIEU : sans elle, le motif serait deviné.
        expect(lectures).toBe(2);
    });

    it('le PERDANT d’une course séquentielle reçoit `vm-deja-attribuee`', async () => {
        // ⚠️ CE TEST EST SÉQUENTIEL ET NE MESURE PAS LA SÉRIALISATION : le
        // perdant joue APRÈS le gagnant. Il éprouve la TRADUCTION du refus, pas
        // le verrou. La sérialisation, elle, a été mesurée hors suite, sur
        // PostgreSQL 16.15 seul, sur UNE paire de transactions — B bloque sur
        // le verrou de ligne de A puis rend `0 ligne` après son `COMMIT`. Rien
        // n'est établi au-delà de deux concurrents, et le cas ne peut pas être
        // mesuré utilement sur `node:sqlite`, ouvert en `:memory:` dans un
        // processus unique.
        base = await baseNeuve('inv-course');
        await poserVm(base, 'v1', 'w1');
        const alice = await poserUtilisateur(base, 'alice@exemple.test');
        const bob = await poserUtilisateur(base, 'bob@exemple.test');
        const gagnant = inventaireStatique(base, horlogeA(MS));
        const perdant = inventaireStatique(base, horlogeA(MS));
        expect(await gagnant.attribuer('v1', alice)).toEqual({ ok: true });
        const issue = await perdant.attribuer('v1', bob);
        expect(issue).toEqual({
            ok: false,
            motif: 'vm-deja-attribuee',
            operation: 'attribuer',
            backend: BACKEND_STATIQUE,
        });
    });
});
