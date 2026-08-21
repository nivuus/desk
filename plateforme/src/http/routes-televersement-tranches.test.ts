// La famille « DÉPOSER » des routes de téléversement : le rang, la borne dure
// du `PUT`, et l'idempotence du redépôt.
//
// 🔴 EXTRAITE DE `routes-televersement.test.ts`, QUI A ATTEINT 504 LIGNES pour
// un plafond de 500. Le dépôt a payé DEUX FOIS en D9 pour avoir rattrapé un
// franchissement par une COMPRESSION qu'il interdit nommément ; l'extraction
// est le geste que sa doctrine prescrit, et CHAQUE CAS EMPORTE AVEC LUI LE
// COMMENTAIRE QUI LE JUSTIFIE — aucune ligne d'assertion n'a été reformulée.
//
// 🔴 LES FIXTURES VIENNENT DE `routes-televersement-harnais.ts`, JAMAIS D'UNE
// COPIE : deux montages divergeraient, et le jour où l'un des deux changerait
// de pas, l'autre éprouverait un contrat que le produit n'a plus.

import { readdirSync } from 'node:fs';
import { join } from 'node:path';
import { afterEach, describe, expect, it } from 'vitest';
import { MOTEUR } from '../base/harnais';
import { avec, jetonDe } from './routes-harnais';
import {
    deposer,
    magasin,
    monter,
    nettoyer,
    PAS,
    poser,
    racine,
    TRANCHES,
    utilisateur,
} from './routes-televersement-harnais';

afterEach(nettoyer);

describe(`routes de téléversement — déposer, moteur=${MOTEUR}`, () => {
    /* ── ③ DÉPOSER ───────────────────────────────────────────────────── */

    it('dépose, relit l’état, et REDÉPOSER écrase (idempotence)', async () => {
        const { url, base } = await monter('tel-deposer');
        const ada = await utilisateur(base, 'ada@exemple.test');
        const jeton = jetonDe(ada);
        const id = await poser(base, ada);

        const un = await deposer(url, id, 1, TRANCHES[1], jeton);
        expect(un.status).toBe(200);
        expect(await un.json()).toEqual({ n: 1, octets: 4 });

        const etat = await fetch(`${url}/televersement/${id}`, { headers: avec(jeton) });
        expect(((await etat.json()) as { tranches_presentes: unknown }).tranches_presentes).toEqual([
            { n: 1, octets: 4 },
        ]);

        // Redéposer le MÊME rang, plus court : la seconde écriture gagne.
        const bis = await deposer(url, id, 1, Buffer.from('xy'), jeton);
        expect(bis.status).toBe(200);
        expect(magasin.lister(id)).toEqual([{ n: 1, octets: 2 }]);
    });

    it('🔴 refuse une tranche au-delà du pas, et ne laisse AUCUN fichier', async () => {
        // 🔴 LA ROUGE : sans la borne, on rend 200 et l'on écrit une tranche de
        // cinq octets là où le pas en vaut quatre.
        const { url, base } = await monter('tel-borne');
        const ada = await utilisateur(base, 'ada@exemple.test');
        const id = await poser(base, ada);

        const r = await deposer(url, id, 0, Buffer.alloc(PAS + 1, 0x41), jetonDe(ada));
        expect(r.status).toBe(413);
        expect(await r.json()).toEqual({ refus: 'tranche-trop-grande', maximum: PAS });

        // ⚠️ AUCUNE TRANCHE : le `rename` n'a jamais eu lieu, donc rien n'existe
        // sous le nom définitif. C'est CE contrôle qui tue la rouge — sans la
        // borne, le fichier `0` existe et `lister` le rend.
        expect(magasin.lister(id)).toEqual([]);

        // 🔴 LE RÉPERTOIRE EST VIDE, ET CETTE ASSERTION A ÉTÉ RENDUE À SA
        // FORME FORTE APRÈS QUE LE DÉFAUT QU'ELLE CONTOURNAIT A ÉTÉ CORRIGÉ.
        //
        // Elle s'est d'abord arrêtée à « ce qui reste ne peut être qu'un
        // `.part` », sur un défaut MESURÉ du magasin : `magasin-tranches.ts`
        // affirmait que « LE FICHIER PARTIEL EST SUPPRIMÉ, quelle que soit la
        // cause », et c'était faux — une COURSE entre le `rmSync` du chemin
        // d'erreur et l'`open(2)` ASYNCHRONE de `createWriteStream`, qui créait
        // le fichier juste après sa suppression. C'est ce qui faisait tomber ce
        // test par intermittence sous la charge de la suite complète, et jamais
        // isolé.
        //
        // ✅ LA COURSE EST SUPPRIMÉE À SA SOURCE — le descripteur est ouvert par
        // `openSync` AVANT le `pipeline`, donc l'inode existe déjà quand le
        // `catch` supprime. Différentiel mesuré par sonde directe sur `ecrire`,
        // hors HTTP, **une exécution de 400 dépassements par bras** :
        // **100 répertoires non vides sur 400 AVANT, 0 sur 400 APRÈS**.
        // ⚠️ Le taux dépend de la charge — une mesure antérieure sous une autre
        // charge relevait 42 sur 400 —, donc AUCUN taux n'est revendiqué ; ce
        // qui est établi est la disparition, pas une fréquence.
        //
        // ⚠️ Ce n'était PAS un trou de protocole : `lister` ignore les noms non
        // numériques, donc aucune fausse tranche n'a jamais été comptée et le
        // scellement n'en voyait rien. C'était une FUITE DE DISQUE, sur un
        // service qui accepte 4 Gio.
        expect(readdirSync(join(racine, id)), 'aucun résidu, .part compris').toEqual([]);
    });

    it('refuse un rang qui n’est pas un entier, ou qui sort du plan', async () => {
        const { url, base } = await monter('tel-rang');
        const ada = await utilisateur(base, 'ada@exemple.test');
        const jeton = jetonDe(ada);
        const id = await poser(base, ada);

        for (const rang of ['%2B1', '1e3', '01x', '-1']) {
            const r = await deposer(url, id, rang, Buffer.from("a"), jeton);
            expect(r.status, rang).toBe(400);
            expect(await r.json(), rang).toEqual({ refus: 'rang-invalide' });
        }
        // Trois tranches (0, 1, 2) : le rang 3 ne deviendra jamais cohérent.
        const hors = await deposer(url, id, 3, Buffer.from('a'), jeton);
        expect(hors.status).toBe(409);
        expect(await hors.json()).toEqual({ refus: 'rang-hors-plan', tranches: 3 });
        expect(magasin.lister(id)).toEqual([]);
    });
});
