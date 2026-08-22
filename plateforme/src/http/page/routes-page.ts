// Le servant de la page bâtie. Il n'applique que le verdict de `resolution.ts`
// et ne décide rien lui-même — toute la sécurité vit dans la règle pure.

import { createReadStream } from 'node:fs';
import { stat } from 'node:fs/promises';
import type { IncomingMessage, ServerResponse } from 'node:http';
import { resolve, sep } from 'node:path';
import { ENTETES_DOCUMENT, ENTETES_RESSOURCE } from './entetes-page';
import { resoudre } from './resolution';

export interface DependancesPage {
    /// Absente ⇒ le servant se retire, et le 404 générique reprend la main.
    racinePage?: string;
}

export async function servirPage(
    req: IncomingMessage,
    rep: ServerResponse,
    deps: DependancesPage,
): Promise<boolean> {
    if (deps.racinePage === undefined) return false;
    // 🔴 HORS GET/HEAD, ON SE RETIRE — jamais un 405. Voir le § 4.3 de la spec :
    // le repli SPA résout n'importe quel chemin, donc un 405 masquerait la
    // faute de frappe d'un appel d'API au lieu de la nommer.
    if (req.method !== 'GET' && req.method !== 'HEAD') return false;

    const chemin = new URL(req.url ?? '/', 'http://placeholder').pathname;
    const verdict = resoudre(chemin);
    // Un refus rend `false` : la chaîne se termine sur le 404 générique, plutôt
    // que d'inventer une SECONDE forme de 404 que rien ne testerait.
    if (!verdict.ok) return false;

    const racine = resolve(deps.racinePage);
    const fichier = resolve(racine, verdict.fichier);
    // Ceinture. La garantie est la règle pure ; ceci la redouble sur le
    // chemin RÉSOLU du système de fichiers, et ne coûte rien.
    if (fichier !== racine && !fichier.startsWith(racine + sep)) return false;

    try {
        if (!(await stat(fichier)).isFile()) return false;
    } catch {
        return false;
    }

    rep.writeHead(200, {
        'content-type': verdict.mime,
        ...(verdict.document ? ENTETES_DOCUMENT : ENTETES_RESSOURCE),
    });
    if (req.method === 'HEAD') {
        rep.end();
        return true;
    }
    createReadStream(fichier).pipe(rep);
    return true;
}
