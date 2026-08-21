// Le harnais commun des tests de routes HTTP : une base neuve, un serveur qui
// ne porte QUE la route sous test, des jetons, et les fixtures d'application.
//
// 🔴 EXTRAIT AVANT L'ADDITION, ET C'EST LA RÈGLE DU DÉPÔT, PAS UN GOÛT.
// `routes-applications.test.ts` était à 480 lignes pour un plafond de 500 :
// la famille de tests de la route d'icône l'aurait fait FRANCHIR. Le dépôt a
// franchi ce plafond trois fois au sous-bloc D10 et deux fois en D9, et l'a
// rattrapé DEUX FOIS PAR UNE COMPRESSION qu'il interdit nommément. L'extraction
// se joue donc AVANT, jamais après.
//
// Modèle : `base/harnais.ts` et `agents/canal-harnais.ts`, tous deux existants.
//
// ⚠️ AUCUNE LIGNE DE COMPORTEMENT N'A ÉTÉ AJOUTÉE, RETIRÉE NI REFORMULÉE par
// cette extraction. Le compte de tests a été ANNONCÉ avant d'être mesuré :
// 19 avant, 19 après.
//
// ⚠️ **DIVERGENCE RELEVÉE AVEC LE PLAN DE G2 (E10), QUI ANNONCE « DIX-SEPT
// tests, chiffre que G1 a annoncé puis mesuré ».** Mesuré le 20 août 2026 :
// il y en a **DIX-NEUF**. Le nombre a dérivé depuis la clôture de G1 sans que
// personne ne le reprenne — c'est le naufrage du « 487 » sous sa forme la plus
// ordinaire. Le compte qui fait foi est celui de la commande.

import { createServer, type IncomingMessage, type Server, type ServerResponse } from 'node:http';
import { baseNeuve } from '../base/harnais';
import type { Pilote } from '../base/pilote';
import { appliquer, lireParVm } from '../depot/application';
import { creerUtilisateur } from '../depot/utilisateur';
import { signer } from '../identite/jeton';
import type { Application } from '../../../proto/ts/plateforme';

export const SECRET = 'un-secret-de-plateforme-de-quarante-octets';
export const ORIGINE = 'http://127.0.0.1:5173';
export const MS = 1_787_136_773_742;

/// Ce qu'un test doit fermer quand il a fini.
export interface Montage {
    base: Pilote;
    http: Server;
    url: string;
}

/// Monte un serveur qui ne porte QUE la route donnée, plus le 404 générique de
/// `serveur.ts` REPRODUIT MOT POUR MOT.
///
/// 🔴 CE 404 N'EST PAS DÉCORATIF : c'est lui qui rend observable un `false`
/// rendu par le routeur. Sans lui, une route qui mangerait toute une famille de
/// chemins et rendrait SON PROPRE 404 typé serait indiscernable du 404
/// générique — G1 a mesuré qu'un `startsWith('/application')` laissait ses
/// tests VERTS pour cette raison exacte. **Le contrôle compare donc le CORPS,
/// jamais le seul statut.**
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
    const u = await creerUtilisateur(p, email, 'empreinte-opaque-de-test', MS);
    await p.executer('UPDATE vm SET utilisateur_id = ? WHERE id = ?', [u, vmId]);
    return u;
}

/// Une application témoin. `icone`/`source_max` prennent l'état SANS ICÔNE par
/// défaut — celui d'une extraction qui a échoué, qui n'est pas une erreur.
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
            aMettreAJour: [],
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

export function avec(
    jeton?: string,
    autres: Record<string, string> = {},
): Record<string, string> {
    return jeton === undefined ? autres : { authorization: `Bearer ${jeton}`, ...autres };
}
