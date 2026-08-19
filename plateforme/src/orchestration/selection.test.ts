// Quelles VMs d'un inventaire sont à cet utilisateur. PUR, donc éprouvable
// sans base ni socket.
//
// 🔴 POURQUOI CE MODULE EXISTE PLUTÔT QU'UN `filter` DANS LA ROUTE : un défaut
// de filtre qui vivrait dans `http/routes-vm.ts` fuiterait l'inventaire
// ENTIER, et il n'y aurait aucun endroit où le rougir sans monter un serveur.
// Ici, la mutation « rendre l'inventaire entier » tombe en quelques
// microsecondes.
//
// ⚠️ CINQ TESTS ET NON QUATRE, ET C'EST ANNONCÉ : le plan range « rend
// l'unique VM » et « lève sur un doublon » dans une seule ligne de tableau,
// alors que sa propre doctrine exige un `it()` par assertion (leçon ①A/①A-bis
// de P2). Les deux sont donc séparés.

import { describe, expect, it } from 'vitest';
import type { Vm } from './interface';
import { laVmDe, vmsDe } from './selection';

const MS = 1_787_136_773_742;

function vm(id: string, utilisateurId: string | null): Vm {
    return {
        id,
        nom: `w-${id}`,
        adresse: '192.168.3.2',
        utilisateurId,
        prefixe: `prefixe-${id}`,
        // Une époque en millisecondes, jamais un petit nombre commode : c'est
        // la leçon la plus chère de P1.
        vuA: MS,
    };
}

const INVENTAIRE: readonly Vm[] = [
    vm('v1', 'alice'),
    vm('v2', 'bob'),
    vm('v3', null),
    vm('v4', 'alice-bis'),
];

describe('sélection des VMs d’un utilisateur', () => {
    it('🔴 `vmsDe` ne rend QUE les VMs du demandeur', () => {
        // 🔴 La rouge : rendre l'inventaire entier. C'est la mutation exacte
        // que ce module existe pour rendre visible — et elle est indétectable
        // si le filtre vit dans la route.
        expect(vmsDe(INVENTAIRE, 'alice').map((v) => v.id)).toEqual(['v1']);
        expect(vmsDe(INVENTAIRE, 'bob').map((v) => v.id)).toEqual(['v2']);
        // Et une comparaison par PRÉFIXE rendrait `v4` à alice.
        expect(vmsDe(INVENTAIRE, 'alice').map((v) => v.id)).not.toContain('v4');
    });

    it('🔴 une VM à `utilisateurId` nul n’est rendue à PERSONNE', () => {
        // 🔴 La rouge : traiter `null` comme « libre pour tous ». Tout
        // utilisateur authentifié verrait alors chaque VM non attribuée.
        // ⚠️ C'est le comportement que le sous-bloc G1 retient délibérément
        // pour SES routes (« servie, et journalisée ») ; P4 ne le retient pas
        // — voir D8. Les deux chantiers divergent ici, et c'est déclaré.
        for (const qui of ['alice', 'bob', 'personne', '']) {
            expect(vmsDe(INVENTAIRE, qui).map((v) => v.id)).not.toContain('v3');
        }
        expect(laVmDe(INVENTAIRE, '')).toBeUndefined();
    });

    it('🔴 `laVmDe` rend `undefined` quand l’utilisateur n’en a aucune', () => {
        // 🔴 La rouge : rendre `inventaire[0]`. Le critère ③ — « un
        // utilisateur sans VM reçoit 409 `aucune-vm` » — deviendrait
        // invérifiable, la route croyant toujours en tenir une.
        expect(laVmDe(INVENTAIRE, 'carole')).toBeUndefined();
        expect(laVmDe([], 'alice')).toBeUndefined();
    });

    it('`laVmDe` rend l’unique VM quand il y en a une', () => {
        const trouvee = laVmDe(INVENTAIRE, 'bob');
        expect(trouvee?.id).toBe('v2');
    });

    it('🔴 `laVmDe` LÈVE si l’inventaire en porte DEUX pour le même utilisateur', () => {
        // 🔴 La rouge : rendre la première en silence. L'index partiel
        // `vm_un_utilisateur` rend ce cas impossible EN BASE — mais cette
        // fonction reçoit un tableau, et rien dans sa signature ne dit d'où il
        // vient. Le jour où l'index serait retiré (ce que la rouge ②b de la
        // recette fait exprès), c'est cette exception qui dirait où regarder ;
        // un silence, lui, ferait choisir une VM au hasard.
        const double: readonly Vm[] = [vm('v1', 'alice'), vm('v9', 'alice')];
        // Le message NOMME l'index dont la disparition expliquerait le doublon :
        // c'est ce qui dit où regarder, et c'est ce qu'on asserte.
        expect(() => laVmDe(double, 'alice')).toThrow(/vm_un_utilisateur/);
        // `vmsDe`, elle, ne lève PAS : elle rend ce qu'elle voit. C'est
        // `laVmDe` qui porte l'invariant « au plus une ».
        expect(vmsDe(double, 'alice')).toHaveLength(2);
    });
});
