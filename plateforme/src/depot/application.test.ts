// Ces tests tournent sous `test:sqlite` ET sous `test:postgres`, par le même
// harnais que `pilotes.test.ts` et `agent.test.ts`.
//
// 🔴 LES VALEURS SONT RÉALISTES, JAMAIS COMMODES : les horodatages portent une
// MAGNITUDE D'ÉPOQUE, les chemins sont des chemins Windows, et les clés ont la
// longueur d'une empreinte. C'est la leçon la plus chère de P1 — la double
// passe n'écrivait que des `1_000`, et déclarait portable un schéma que
// Postgres refusait pour toute écriture réelle.

import { afterEach, describe, expect, it } from 'vitest';
import { baseNeuve, MOTEUR } from '../base/harnais';
import type { Pilote } from '../base/pilote';
import type { Application } from '../../../proto/ts/plateforme';
import type { Fusion } from '../apps/catalogue';
import {
    appliquer,
    lireConnues,
    lireParId,
    lireParVm,
    pxDepuisSourceMax,
    sourceMaxDepuis,
} from './application';

let base: Pilote | undefined;

/// La magnitude qui a réellement cassé Postgres en P1.
const MS = 1_787_136_773_742;

afterEach(async () => {
    await base?.fermer();
    base = undefined;
});

async function avecVm(p: Pilote, id: string): Promise<void> {
    await p.executer('INSERT INTO vm(id,nom,adresse) VALUES(?,?,?)', [id, `vm-${id}`, '192.168.3.2']);
}

function app(nom: string, cle: string): Application {
    return {
        cle,
        nom,
        chemin: `C:\\Users\\guacamole\\Desktop\\${nom}.lnk`,
        cible: `c:\\program files\\${nom}\\${nom}.exe`,
        arguments: '',
        repertoire: `c:\\program files\\${nom}`,
        icone: null,
        source_max: 'non-mesuree',
    };
}

/// Une empreinte de la VRAIE longueur d'un SHA-256 hexadécimal.
function cle(n: number): string {
    return `${n}`.padStart(64, 'a');
}

const RIEN: Fusion = { aInserer: [], aMettreAJour: [], aMarquerDisparues: [], aRessusciter: [] };

describe(`dépôt application, moteur=${MOTEUR}`, () => {
    it('lireParVm ne rend QUE les applications de cette VM', async () => {
        // 🔴 Omettre le `WHERE vm_id = ?` ferait voir à un utilisateur le
        // catalogue de toutes les VMs du service.
        base = await baseNeuve('app-par-vm');
        await avecVm(base, 'v-1');
        await avecVm(base, 'v-2');
        await appliquer(base, 'v-1', { ...RIEN, aInserer: [app('Firefox', cle(1))] }, MS);
        await appliquer(base, 'v-2', { ...RIEN, aInserer: [app('Excel', cle(2))] }, MS);

        expect((await lireParVm(base, 'v-1')).map((l) => l.nom)).toEqual(['Firefox']);
        expect((await lireParVm(base, 'v-2')).map((l) => l.nom)).toEqual(['Excel']);
    });

    it('lireParVm EXCLUT les disparues et les masquées', async () => {
        // 🔴 Les inclure ferait porter au catalogue affiché des applications
        // qui n'existent plus sur la VM — et le hub proposerait de lancer un
        // raccourci supprimé.
        base = await baseNeuve('app-exclusions');
        await avecVm(base, 'v-1');
        await appliquer(
            base,
            'v-1',
            { ...RIEN, aInserer: [app('Vivante', cle(1)), app('Partie', cle(2)), app('Masquee', cle(3))] },
            MS,
        );
        const partie = (await lireParVm(base, 'v-1')).find((l) => l.nom === 'Partie')!;
        const masquee = (await lireParVm(base, 'v-1')).find((l) => l.nom === 'Masquee')!;
        await appliquer(base, 'v-1', { ...RIEN, aMarquerDisparues: [partie.id] }, MS + 10);
        // `masquee_a` n'a aucun écrivain en G1 : le geste de masquage n'existe
        // pas encore. La colonne est posée à la main, exactement comme un test
        // de P4 pose `vm.utilisateur_id` que rien ne remplit avant lui.
        await base.executer('UPDATE application SET masquee_a = ? WHERE id = ?', [MS + 20, masquee.id]);

        expect((await lireParVm(base, 'v-1')).map((l) => l.nom)).toEqual(['Vivante']);
        // ⚠️ Mais elles sont TOUJOURS LÀ : c'est la moitié qui décide, et sans
        // elle un `DELETE` passerait ce test.
        expect(await lireParId(base, partie.id)).toBeDefined();
        expect(await lireParId(base, masquee.id)).toBeDefined();
    });

    it('lireParId rend `undefined` sur un identifiant inconnu, JAMAIS une exception', async () => {
        // Précédent : `depot/agent.ts::lireParVm` et `depot/vm.ts::lireParId`.
        // Une exception qui remonterait en 500 serait un oracle d'énumération.
        base = await baseNeuve('app-inconnue');
        await expect(lireParId(base, 'jamais-vu')).resolves.toBeUndefined();
    });

    it("génère l'identifiant DANS LE DÉPÔT, et pose apparue_a", async () => {
        // 🔴 Le laisser venir de l'agent ferait que deux VMs pourraient en
        // produire le même — la clé, elle, est l'empreinte d'un triplet de
        // chemins, et deux VMs portant la même application la partagent.
        base = await baseNeuve('app-identifiant');
        await avecVm(base, 'v-1');
        await appliquer(base, 'v-1', { ...RIEN, aInserer: [app('Firefox', cle(1))] }, MS);
        const [ligne] = await lireParVm(base, 'v-1');

        // La forme d'un `randomUUID()` : 36 caractères, cinq groupes, version 4.
        expect(ligne.id).toMatch(/^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/);
        expect([ligne.apparue_a, ligne.vue_a, ligne.disparue_a, ligne.masquee_a])
            .toEqual([MS, MS, null, null]);
    });

    it("met à jour les champs et avance vue_a, sans toucher ni l'id ni apparue_a", async () => {
        base = await baseNeuve('app-maj');
        await avecVm(base, 'v-1');
        await appliquer(base, 'v-1', { ...RIEN, aInserer: [app('Ancien', cle(1))] }, MS);
        const avant = (await lireParVm(base, 'v-1'))[0];

        await appliquer(
            base,
            'v-1',
            { ...RIEN, aMettreAJour: [{ id: avant.id, app: app('Neuf', cle(1)) }] },
            MS + 30,
        );
        const apres = (await lireParVm(base, 'v-1'))[0];
        expect(apres.id).toBe(avant.id);
        expect(apres.nom).toBe('Neuf');
        expect(apres.cible).toBe('c:\\program files\\Neuf\\Neuf.exe');
        expect(apres.vue_a).toBe(MS + 30);
        // 🔴 `apparue_a` est la PREMIÈRE vue : l'avancer à chaque mise à jour
        // en ferait un doublon de `vue_a`, et le verdict d'installation qui
        // la lira un jour ne verrait plus jamais une apparition.
        expect(apres.apparue_a).toBe(MS);
    });

    it('pose disparue_a SANS SUPPRIMER LA LIGNE', async () => {
        // 🔴 LE TEST LIT LES DEUX, et c'est ce qui le rend discriminant : un
        // `DELETE` poserait bien « plus dans le catalogue », et ferait perdre
        // son identifiant à une application installée côté navigateur.
        base = await baseNeuve('app-disparue');
        await avecVm(base, 'v-1');
        await appliquer(base, 'v-1', { ...RIEN, aInserer: [app('Partante', cle(1))] }, MS);
        const avant = (await lireParVm(base, 'v-1'))[0];

        await appliquer(base, 'v-1', { ...RIEN, aMarquerDisparues: [avant.id] }, MS + 40);

        expect(await lireParVm(base, 'v-1')).toEqual([]);
        const ligne = await lireParId(base, avant.id);
        expect(ligne).toBeDefined();
        expect(ligne!.disparue_a).toBe(MS + 40);
        expect(ligne!.id).toBe(avant.id);
    });

    it("ressuscite : disparue_a repasse à NULL, et l'identifiant NE CHANGE PAS", async () => {
        // 🔴 Insérer une ligne neuve serait la même perte d'identifiant, par
        // une autre porte.
        base = await baseNeuve('app-resurrection');
        await avecVm(base, 'v-1');
        await appliquer(base, 'v-1', { ...RIEN, aInserer: [app('Revenante', cle(1))] }, MS);
        const avant = (await lireParVm(base, 'v-1'))[0];
        await appliquer(base, 'v-1', { ...RIEN, aMarquerDisparues: [avant.id] }, MS + 50);

        await appliquer(
            base,
            'v-1',
            {
                ...RIEN,
                aRessusciter: [avant.id],
                aMettreAJour: [{ id: avant.id, app: app('Revenante', cle(1)) }],
            },
            MS + 60,
        );

        const [apres] = await lireParVm(base, 'v-1');
        expect(apres.id).toBe(avant.id);
        expect(apres.disparue_a).toBeNull();
        expect(apres.apparue_a).toBe(MS);
    });

    it('lireConnues rend AUSSI les disparues, avec leur instant de disparition', async () => {
        // 🔴 C'est ce qui distingue ce lecteur de `lireParVm`, et l'omission
        // serait grave : la fusion qui ne verrait pas les disparues les
        // RÉINSÉRERAIT à leur retour, avec un identifiant neuf. Le lecteur du
        // catalogue affiché et le lecteur de la fusion ne peuvent donc PAS
        // être le même.
        base = await baseNeuve('app-connues');
        await avecVm(base, 'v-1');
        await appliquer(base, 'v-1', { ...RIEN, aInserer: [app('A', cle(1)), app('B', cle(2))] }, MS);
        const b = (await lireParVm(base, 'v-1')).find((l) => l.nom === 'B')!;
        await appliquer(base, 'v-1', { ...RIEN, aMarquerDisparues: [b.id] }, MS + 70);

        const connues = await lireConnues(base, 'v-1');
        expect(connues.map((c) => c.cle).sort()).toEqual([cle(1), cle(2)].sort());
        expect(connues.find((c) => c.cle === cle(2))!.disparue_a).toBe(MS + 70);
        expect(connues.find((c) => c.cle === cle(1))!.disparue_a).toBeNull();
    });

    it("n'écrit RIEN quand une seule écriture de la fusion échoue", async () => {
        // 🔴 UNE FUSION S'APPLIQUE EN ENTIER OU PAS DU TOUT. Un catalogue à
        // moitié écrit est indiscernable d'un catalogue correct au tour
        // suivant : la réconciliation suivante le prendrait pour l'état de la
        // VM, et les lignes manquantes ne reviendraient qu'au prochain envoi
        // complet — ou jamais, si l'agent n'en émet plus.
        //
        // 🔴 L'ÉCHEC EST PLACÉ APRÈS UNE ÉCRITURE QUI RÉUSSIT, et ce n'est
        // pas un détail de mise en scène : une première rédaction faisait
        // échouer la TOUTE PREMIÈRE écriture, et la mutation « retirer la
        // transaction » lui SURVIVAIT — rien n'avait été écrit avant l'échec,
        // les deux versions rendaient donc le même état. Le contrôle ne
        // pouvait pas échouer.
        //
        // Ici, la fusion insère deux applications de MÊME clé pour la MÊME
        // VM : la première passe, la seconde est refusée par l'index unique
        // `application_cle`. Sans transaction, la première resterait.
        base = await baseNeuve('app-transaction');
        await avecVm(base, 'v-1');

        await expect(
            appliquer(
                base,
                'v-1',
                { ...RIEN, aInserer: [app('Premiere', cle(1)), app('Doublon', cle(1))] },
                MS + 80,
            ),
        ).rejects.toThrow();

        // NI L'UNE NI L'AUTRE : la première a bien été écrite, puis annulée.
        expect(await lireParVm(base, 'v-1')).toEqual([]);
        expect(await lireConnues(base, 'v-1')).toEqual([]);
    });

    it('ne porte AUCUNE valeur littérale dans ses requêtes', async () => {
        // 🔴 CE CONTRÔLE NE ROUGIT QUE SOUS `test:postgres`, et c'est mesuré :
        // `rendreMarqueurs` (`base/pilote.ts`) n'est appelée que par
        // `pilote-postgres.ts`, et c'est elle seule qui LÈVE sur une
        // apostrophe. Sous SQLite la requête fautive passerait sans un mot.
        // La rouge se joue donc sur LES DEUX moteurs, et n'est visible que sur
        // un — c'est exactement l'angle mort que la double passe existe pour
        // couvrir, et le lint statique de `sous-ensemble.test.ts` ne balaie
        // que les `.sql`.
        //
        // Ce cas exerce les QUATRE écritures et les TROIS lectures du module
        // en une fois : c'est le seul moyen de faire passer chaque requête par
        // le convertisseur de marqueurs.
        base = await baseNeuve('app-marqueurs');
        await avecVm(base, 'v-1');
        await appliquer(base, 'v-1', { ...RIEN, aInserer: [app('Une', cle(1))] }, MS);
        const une = (await lireParVm(base, 'v-1'))[0];
        await appliquer(
            base,
            'v-1',
            {
                aInserer: [],
                aMettreAJour: [{ id: une.id, app: app('Une', cle(1)) }],
                aMarquerDisparues: [une.id],
                aRessusciter: [une.id],
            },
            MS + 90,
        );
        await lireConnues(base, 'v-1');
        await expect(lireParId(base, une.id)).resolves.toBeDefined();
    });

    it('🔴 `NULL` se relit `non-mesuree`, JAMAIS `{pixels:0}` — le critère ④', async () => {
        // 🔴 REPRÉSENTER `NonMesuree` PAR UN NOMBRE FERAIT DIRE À UNE
        // PROVENANCE INCONNUE QU'ELLE VAUT QUELQUE CHOSE, et c'est tout ce que
        // le sous-bloc G2 existe pour empêcher. La règle est écrite UNE SEULE
        // FOIS, au dépôt, précisément pour qu'elle ne puisse pas diverger.
        expect(sourceMaxDepuis(null)).toBe('non-mesuree');
        expect(sourceMaxDepuis(null)).not.toEqual({ pixels: 0 });
        expect(sourceMaxDepuis(256)).toEqual({ pixels: 256 });
        expect(sourceMaxDepuis(48)).toEqual({ pixels: 48 });
        // Et l'inverse, qui doit refermer l'aller-retour.
        expect(pxDepuisSourceMax('non-mesuree')).toBeNull();
        expect(pxDepuisSourceMax({ pixels: 256 })).toBe(256);
    });

    it('🔴 les deux champs d’icône font l’ALLER-RETOUR par la base', async () => {
        base = await baseNeuve('app-icones');
        const p = base;
        await avecVm(p, 'v-ico');
        await appliquer(p, 'v-ico', {
            aInserer: [
                { ...app('Avec', 'k-avec'), icone: 'f'.repeat(64), source_max: { pixels: 256 } },
                { ...app('Sans', 'k-sans'), icone: null, source_max: 'non-mesuree' },
            ],
            aMettreAJour: [],
            aMarquerDisparues: [],
            aRessusciter: [],
        }, 1_700_000_000_000);
        const lignes = await lireParVm(p, 'v-ico');
        const avec = lignes.find((l) => l.nom === 'Avec')!;
        const sans = lignes.find((l) => l.nom === 'Sans')!;
        expect(avec.icone).toBe('f'.repeat(64));
        expect(sourceMaxDepuis(avec.source_max_px)).toEqual({ pixels: 256 });
        // 🔴 LA COMBINAISON INTERDITE — `icone` nul et une taille mesurée —
        // N'EST ÉCRITE PAR AUCUN CHEMIN. Le test la NOMME pour qu'elle ne
        // naisse pas d'une inattention.
        expect(sans.icone).toBeNull();
        expect(sans.source_max_px).toBeNull();
        expect(sourceMaxDepuis(sans.source_max_px)).toBe('non-mesuree');

        // Une icône qui CHANGE atteint bien la base : c'est le cas nominal
        // d'une application qui se met à jour, pas l'exception.
        const id = avec.id;
        await appliquer(p, 'v-ico', {
            aInserer: [],
            aMettreAJour: [
                { id, app: { ...app('Avec', 'k-avec'), icone: 'e'.repeat(64), source_max: { pixels: 48 } } },
            ],
            aMarquerDisparues: [],
            aRessusciter: [],
        }, 1_700_000_001_000);
        const relu = (await lireParVm(p, 'v-ico')).find((l) => l.id === id)!;
        expect(relu.icone).toBe('e'.repeat(64));
        expect(sourceMaxDepuis(relu.source_max_px)).toEqual({ pixels: 48 });
    });
});
