// L'interface que `protocole.ts` consomme pour muter, et la fabrique qui la
// relie à `mutation.ts`.
//
// ⚠️ **POURQUOI UN MODULE À PART, ET NON UN `Mutateur` DANS `mutation.ts`** :
// `protocole.ts` ne doit dépendre d'AUCUNE poignée du navigateur — c'est ce
// qui lui permet d'être testé avec un adaptateur factice, sans jamais décrire
// un système de fichiers. Le type qu'il importe ne porte donc que des chaînes
// et des booléens ; la racine, elle, est capturée par la fabrique.
//
// C'est le même découpage que `Ecrivain` (`ecriture.ts`) : une interface de
// verbes d'un côté, une fabrique qui capture la racine de l'autre.

import { renommer, supprimer, type RacineMutable, type TraceRenommage } from './mutation';

export interface Mutateur {
    renommer(de: string, vers: string, repertoire: boolean): Promise<TraceRenommage>;
    supprimer(chemin: string, repertoire: boolean): Promise<void>;
}

export function creerMutateur(racine: RacineMutable): Mutateur {
    return {
        renommer: (de, vers, repertoire) => renommer(racine, de, vers, repertoire),
        supprimer: (chemin, repertoire) => supprimer(racine, chemin, repertoire),
    };
}
