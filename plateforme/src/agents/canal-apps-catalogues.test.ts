// Two catalogues of the same VM handled in the same tick — both engines.
//
// 🔴 THE DEFECT: `ws` emits every message of one received buffer
// synchronously, so two `catalogue` frames that arrive in one TCP segment reach
// `traiter` back to back. Each read the known applications, THEN each applied
// its merge: both saw a new application as unknown, both inserted it, and the
// second transaction failed on `UNIQUE (vm_id, cle)` — "catalogue not written",
// the delta lost until the next full catalogue.
//
// `traiter` is called directly, twice, without yielding in between: that is
// exactly what the socket does, made deterministic.

import { afterEach, describe, expect, it, vi } from 'vitest';
import type { WebSocket } from 'ws';
import type { Application, CatalogueMessage } from '../../../proto/ts/plateforme';
import { PLATEFORME_VERSION } from '../../../proto/ts/plateforme';
import { baseNeuve, MOTEUR } from '../base/harnais';
import type { Pilote } from '../base/pilote';
import { lireParVm } from '../depot/application';
import { traiter } from './canal-apps';
import { RegistreAgents } from './registre';

let base: Pilote | undefined;

afterEach(async () => {
    await base?.fermer();
    base = undefined;
    vi.restoreAllMocks();
});

const MS = 1_787_136_773_742;

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
        accent: null,
        associations: [],
    };
}

function catalogue(complet: boolean, applications: Application[]): CatalogueMessage {
    return { type: 'catalogue', v: PLATEFORME_VERSION, complet, applications, disparues: [] };
}

async function attendre(p: Pilote, predicat: (noms: string[]) => boolean): Promise<string[]> {
    const fin = Date.now() + 2000;
    for (;;) {
        const noms = (await lireParVm(p, 'v1')).map((l) => l.nom).sort();
        if (predicat(noms)) return noms;
        if (Date.now() > fin) throw new Error(`catalogue never reached (seen=${noms.join(',')})`);
        await new Promise((r) => setTimeout(r, 25));
    }
}

describe(`two catalogues in the same tick, engine=${MOTEUR}`, () => {
    it('🔴 the second one is applied on top of the first, never lost', async () => {
        base = await baseNeuve('catalogues-meme-tour');
        await base.executer('INSERT INTO vm(id,nom,adresse) VALUES(?,?,?)', ['v1', 'vm-1', '192.168.3.2']);
        const erreurs = vi.spyOn(console, 'error').mockImplementation(() => {});
        const deps = {
            base,
            registre: new RegistreAgents(),
            magasin: undefined,
            socket: {} as WebSocket,
            vmId: 'v1',
            maintenant: () => MS,
            envoyer: () => {},
        };

        traiter(deps, catalogue(true, [app('Bloc-notes', 'cle-a')]));
        traiter(deps, catalogue(false, [app('Bloc-notes renomme', 'cle-a'), app('Paint', 'cle-b')]));

        expect(await attendre(base, (n) => n.includes('Paint') && n.includes('Bloc-notes renomme')))
            .toEqual(['Bloc-notes renomme', 'Paint']);
        expect(erreurs).not.toHaveBeenCalled();
    });
});
