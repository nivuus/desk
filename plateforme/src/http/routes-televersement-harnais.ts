// Les fixtures communes aux deux fichiers de test du téléversement : un magasin
// de tranches jetable, un serveur qui ne porte QUE cette route, et le fichier
// témoin qu'ils déposent tous les deux.
//
// 🔴 EXTRAIT AVANT L'ADDITION, ET C'EST LA RÈGLE DU DÉPÔT, PAS UN GOÛT.
// `routes-televersement.test.ts` a atteint 504 lignes pour un plafond de 500 ;
// le dépôt a payé DEUX FOIS en D9 pour avoir rattrapé un franchissement par une
// COMPRESSION qu'il interdit nommément, et la revue a exigé l'extraction
// ensuite. Précédents de forme, tous deux existants : `http/routes-harnais.ts`
// et `agents/canal-harnais.ts`.
//
// 🔴 CE MODULE N'EST PAS UN `.test.ts`, ET C'EST STRUCTUREL : un fichier de test
// qui en importerait un autre RE-EXÉCUTERAIT ses `it()`. L'état ci-dessous est
// néanmoins par FICHIER, vitest donnant à chacun son propre registre de modules.
//
// ⚠️ AUCUNE ASSERTION ICI. Le harnais monte et démonte ; il ne juge de rien.

import { createHash } from 'node:crypto';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { ouvrirMagasinTranches, type MagasinTranches } from '../apps/magasin-tranches';
import type { Pilote } from '../base/pilote';
import { creer } from '../depot/televersement';
import { creerUtilisateur } from '../depot/utilisateur';
import { avec, demonter, monterRoute, MS, SECRET, type Montage } from './routes-harnais';
import { servirTeleversement } from './routes-televersement';

/// Dix octets, un pas de quatre : trois tranches (4 + 4 + 2). La dernière est
/// PLUS COURTE que le pas — seul cas où la borne du `PUT` et le verdict
/// pourraient être confondus.
export const CONTENU = Buffer.from('0123456789');
export const PAS = 4;
export const SHA = createHash('sha256').update(CONTENU).digest('hex');
export const TRANCHES = [CONTENU.subarray(0, 4), CONTENU.subarray(4, 8), CONTENU.subarray(8, 10)];

/// L'identifiant bien formé d'un téléversement qui n'existe pas — le témoin
/// auquel se compare le refus d'un téléversement d'autrui.
export const INCONNU = '00000000-0000-4000-8000-000000000000';

let montage: Montage | undefined;
let racines: string[] = [];

/// Le magasin du montage courant, et sa racine sur disque. ⚠️ Réaffectés à
/// chaque `monter` : un test qui les lirait avant serait fautif, pas eux.
export let magasin: MagasinTranches;
export let racine: string;

export async function monter(nom: string): Promise<{ url: string; base: Pilote }> {
    const r = mkdtempSync(join(tmpdir(), 'g3-routes-tel-'));
    racines.push(r);
    racine = join(r, 'televersements');
    magasin = ouvrirMagasinTranches(racine, () => {});
    montage = await monterRoute(nom, (req, rep, base) =>
        servirTeleversement(req, rep, {
            base,
            secretJeton: SECRET,
            tranches: magasin,
            maintenant: () => MS,
        }),
    );
    return { url: montage.url, base: montage.base };
}

export async function nettoyer(): Promise<void> {
    await demonter(montage);
    montage = undefined;
    for (const r of racines) rmSync(r, { recursive: true, force: true });
    racines = [];
}

export function utilisateur(base: Pilote, courriel: string): Promise<string> {
    return creerUtilisateur(base, courriel, 'empreinte-opaque-de-test', MS);
}

/// Pose une ligne à pas court, SANS passer par la route de création — celle-ci
/// pose `TAILLE_TRANCHE` (8 Mio), et une rouge qu'on n'ose plus rejouer parce
/// qu'elle coûte huit mébioctets n'en est plus une.
export async function poser(
    base: Pilote,
    proprietaire: string,
    sha256 = SHA,
    taille = CONTENU.length,
): Promise<string> {
    const ligne = await creer(
        base,
        { utilisateurId: proprietaire, nom: 'installeur.exe', taille, sha256, tailleTranche: PAS },
        MS,
    );
    return ligne.id;
}

export function deposer(
    url: string,
    id: string,
    n: number | string,
    corps: Uint8Array,
    jeton: string,
) {
    return fetch(`${url}/televersement/${id}/tranche/${n}`, {
        method: 'PUT',
        headers: avec(jeton, { 'content-type': 'application/octet-stream' }),
        body: corps,
    });
}

export const sceller = (url: string, id: string, jeton: string) =>
    fetch(`${url}/televersement/${id}/sceller`, { method: 'POST', headers: avec(jeton) });

/// ⚠️ `corps` passe TEL QUEL si c'est une chaîne : sinon le harnais rendrait
/// valide, en le sérialisant, le non-JSON qu'on veut justement envoyer.
export const declarerChez = (url: string, jeton: string, corps: unknown) =>
    fetch(`${url}/televersement`, {
        method: 'POST',
        headers: avec(jeton, { 'content-type': 'application/json' }),
        body: typeof corps === 'string' ? corps : JSON.stringify(corps),
    });
