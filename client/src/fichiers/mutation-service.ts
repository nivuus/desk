// The interface `protocole.ts` consumes to mutate, and the factory that
// wires it to `mutation.ts`.
//
// ⚠️ **WHY A SEPARATE MODULE, AND NOT A `Mutateur` IN `mutation.ts`**:
// `protocole.ts` must depend on NO browser handle — that is what
// lets it be tested with a fake adapter, without ever describing
// a file system. The type it imports therefore only carries strings
// and booleans; the root, for its part, is captured by the factory.
//
// It is the same split as `Ecrivain` (`ecriture.ts`): an interface of
// verbs on one side, a factory capturing the root on the other.

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
