// The fixtures common to the two upload test files: a disposable slice
// store, a server that carries ONLY this route, and the sample
// file they both upload.
//
// 🔴 EXTRACTED BEFORE THE ADDITION, AND THAT IS THE REPOSITORY RULE, NOT A TASTE.
// `routes-televersement.test.ts` reached 504 lines for a ceiling of 500;
// the repository paid TWICE in D9 for having caught up with a crossing through a
// COMPRESSION it forbids by name, and the review demanded the extraction
// afterwards. Precedents of shape, both existing: `http/routes-harnais.ts`
// and `agents/canal-harnais.ts`.
//
// 🔴 THIS MODULE IS NOT A `.test.ts`, AND THAT IS STRUCTURAL: a test file
// that imported another one would RE-RUN its `it()`. The state below is
// nonetheless per FILE, vitest giving each one its own module registry.
//
// ⚠️ NO ASSERTION HERE. The harness sets up and tears down; it judges nothing.

import { createHash } from 'node:crypto';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { ouvrirMagasinTranches, type MagasinTranches } from '../apps/magasin-tranches';
import type { Pilote } from '../base/pilote';
import { create } from '../depot/televersement';
import { createUser } from '../depot/utilisateur';
import { withIt, demonter, monterRoute, MS, SECRET, type Montage } from './routes-harnais';
import { servirTeleversement } from './routes-televersement';

/// Ten bytes, a step of four: three slices (4 + 4 + 2). The last one is
/// SHORTER than the step — the only case where the bound of the `PUT` and the verdict
/// could be confused.
export const CONTENU = Buffer.from('0123456789');
export const PAS = 4;
export const SHA = createHash('sha256').update(CONTENU).digest('hex');
export const TRANCHES = [CONTENU.subarray(0, 4), CONTENU.subarray(4, 8), CONTENU.subarray(8, 10)];

/// The well-formed id of an upload that does not exist — the control
/// against which the refusal of someone else's upload is compared.
export const INCONNU = '00000000-0000-4000-8000-000000000000';

let montage: Montage | undefined;
let racines: string[] = [];

/// The store of the current setup, and its root on disk. ⚠️ Reassigned on
/// each `monter`: a test that read them before would be at fault, not them.
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

export function user(base: Pilote, courriel: string): Promise<string> {
    return createUser(base, courriel, 'empreinte-opaque-de-test', MS);
}

/// Sets up a short-step row, WITHOUT going through the creation route — that one
/// sets `CHUNK_SIZE` (8 MiB), and a red one no longer dares to replay because
/// it costs eight mebibytes is no longer one.
export async function poser(
    base: Pilote,
    proprietaire: string,
    sha256 = SHA,
    size = CONTENU.length,
): Promise<string> {
    const ligne = await create(
        base,
        { userId: proprietaire, nom: 'installeur.exe', taille: size, sha256, chunkSize: PAS }, // policy: allow-fr - frozen wire key or SQLite column
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
        headers: withIt(jeton, { 'content-type': 'application/octet-stream' }),
        body: corps,
    });
}

export const sceller = (url: string, id: string, jeton: string) =>
    fetch(`${url}/televersement/${id}/sceller`, { method: 'POST', headers: withIt(jeton) });

/// ⚠️ `corps` goes AS IS if it is a string: otherwise the harness would make
/// valid, by serialising it, the non-JSON one precisely wants to send.
export const declarerChez = (url: string, jeton: string, corps: unknown) =>
    fetch(`${url}/televersement`, {
        method: 'POST',
        headers: withIt(jeton, { 'content-type': 'application/json' }),
        body: typeof corps === 'string' ? corps : JSON.stringify(corps),
    });
