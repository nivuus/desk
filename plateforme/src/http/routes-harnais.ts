// The common harness of the HTTP route tests: a fresh database, a server that
// carries ONLY the route under test, tokens, and the application fixtures.
//
// 🔴 EXTRACTED BEFORE THE ADDITION, AND THAT IS THE REPOSITORY RULE, NOT A TASTE.
// `routes-applications.test.ts` was at 480 lines for a ceiling of 500:
// the family of tests of the icon route would have made it CROSS it. The repository
// crossed that ceiling three times in sub-block D10 and twice in D9, and
// caught up with it TWICE THROUGH A COMPRESSION it forbids by name. The extraction
// therefore happens BEFORE, never after.
//
// Model: `base/harnais.ts` and `agents/canal-harnais.ts`, both existing.
//
// ⚠️ NO LINE OF BEHAVIOUR WAS ADDED, REMOVED OR REWORDED by
// this extraction. The test count was ANNOUNCED before being measured:
// 19 before, 19 after.
//
// ⚠️ **DIVERGENCE NOTED FROM THE G2 PLAN (E10), WHICH ANNOUNCES « SEVENTEEN
// tests, a figure G1 announced and then measured ».** Measured on 20 August 2026:
// there are **NINETEEN**. The number drifted since G1 was closed without
// anybody picking it up — it is the « 487 » shipwreck in its most
// ordinary form. The count that counts is the one of the command.

import { createServer, type IncomingMessage, type Server, type ServerResponse } from 'node:http';
import { baseNeuve } from '../base/harnais';
import type { Pilote } from '../base/pilote';
import { appliquer, lireParVm } from '../depot/application';
import { createUser } from '../depot/utilisateur';
import { signer } from '../identite/jeton';
import type { Application } from '../../../proto/ts/plateforme';

export const SECRET = 'un-secret-de-plateforme-de-quarante-octets';
export const ORIGINE = 'http://127.0.0.1:5173';
export const MS = 1_787_136_773_742;

/// What a test must close when it is done.
export interface Montage {
    base: Pilote;
    http: Server;
    url: string;
}

/// Stands up a server that carries ONLY the given route, plus the generic 404 of
/// `serveur.ts` REPRODUCED WORD FOR WORD.
///
/// 🔴 THIS 404 IS NOT DECORATIVE: it is what makes a `false`
/// returned by the router observable. Without it, a route that swallowed a whole family of
/// paths and returned ITS OWN typed 404 would be indistinguishable from the generic
/// 404 — G1 measured that a `startsWith('/application')` left its
/// tests GREEN for this exact reason. **The check therefore compares the BODY,
/// never the status alone.**
export async function monterRoute(
    nom: string,
    routeur: (req: IncomingMessage, rep: ServerResponse, base: Pilote) => Promise<boolean>,
): Promise<Montage> {
    const base = await baseNeuve(nom);
    const http = createServer((req, rep) => {
        void routeur(req, rep, base)
            .then((servie) => {
                if (servie) return;
                rep.writeHead(404, { 'content-type': 'text/plain; charset=utf-8' });
                rep.end('introuvable\n');
            })
            .catch((cause) => {
                rep.writeHead(500, { 'content-type': 'application/json; charset=utf-8' });
                rep.end(JSON.stringify({ refus: 'interne', cause: String(cause) }));
            });
    });
    await new Promise<void>((r) => http.listen(0, '127.0.0.1', () => r()));
    const a = http.address();
    return { base, http, url: `http://127.0.0.1:${typeof a === 'object' && a ? a.port : 0}` };
}

export async function demonter(m: Montage | undefined): Promise<void> {
    if (!m) return;
    await new Promise<void>((r) => m.http.close(() => r()));
    await m.base.fermer();
}

export async function poserVm(p: Pilote, id: string): Promise<void> {
    await p.executer('INSERT INTO vm(id, nom, adresse) VALUES(?, ?, ?)', [
        id,
        `vm-${id}`,
        '192.168.3.2',
    ]);
}

export async function attribuer(p: Pilote, vmId: string, email: string): Promise<string> {
    const u = await createUser(p, email, 'empreinte-opaque-de-test', MS);
    await p.executer('UPDATE vm SET utilisateur_id = ? WHERE id = ?', [u, vmId]);
    return u;
}

/// A sample application. `icone`/`source_max` take the NO ICON state by
/// default — that of an extraction that failed, which is not an error.
export function app(nom: string, cle: string, icone: string | null = null,
                    source: Application['source_max'] = 'non-mesuree'): Application {
    return {
        cle,
        nom,
        chemin: `C:\\Users\\guacamole\\Desktop\\${nom}.lnk`,
        cible: `c:\\program files\\${nom}\\${nom}.exe`,
        arguments: '',
        repertoire: `c:\\program files\\${nom}`,
        icone,
        source_max: source,
        accent: null,
        associations: [],
    };
}

export async function poserApp(
    p: Pilote,
    vmId: string,
    nom: string,
    cle: string,
    icone: string | null = null,
    source: Application['source_max'] = 'non-mesuree',
): Promise<string> {
    await appliquer(
        p,
        vmId,
        {
            aInserer: [app(nom, cle, icone, source)],
            toUpdate: [],
            aMarquerDisparues: [],
            aRessusciter: [],
        },
        MS,
    );
    return (await lireParVm(p, vmId)).find((l) => l.nom === nom)!.id;
}

export function jetonDe(sujet: string, type: 'utilisateur' | 'agent' = 'utilisateur'): string {
    return signer(sujet, SECRET, MS, undefined, type);
}

export function withIt(
    jeton?: string,
    autres: Record<string, string> = {},
): Record<string, string> {
    return jeton === undefined ? autres : { authorization: `Bearer ${jeton}`, ...autres };
}
