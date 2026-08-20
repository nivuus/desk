// Les deux routes d'icône, éprouvées À TRAVERS un serveur HTTP réel.
//
// 🔴 LE CONTRÔLE DE CHEMIN COMPARE LE CORPS, JAMAIS LE SEUL STATUT, et ce
// n'est pas une précaution : G1 a MESURÉ qu'un `startsWith('/application')`
// laissait ses tests VERTS — la route mangeait toute la famille et rendait SON
// PROPRE 404 typé, indiscernable du 404 générique tant qu'on ne lisait que le
// statut (commit `a97f902`).

import { createHash } from 'node:crypto';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, describe, expect, it } from 'vitest';
import { ouvrirMagasin, type Magasin } from '../apps/icones';
import { MOTEUR } from '../base/harnais';
import { creerUtilisateur } from '../depot/utilisateur';
import {
    attribuer,
    avec,
    demonter,
    jetonDe,
    monterRoute,
    MS,
    poserApp,
    poserVm,
    SECRET,
    type Montage,
} from './routes-harnais';
import { ICONE_MAX_OCTETS, servirIcone } from './routes-icone';

const PNG = Buffer.from('\x89PNG\r\n\x1a\n-des-octets-d-icone');
const EMPREINTE = createHash('sha256').update(PNG).digest('hex');
const AUTRE = createHash('sha256').update('autre').digest('hex');

let montage: Montage | undefined;
let magasin: Magasin;
let racines: string[] = [];

afterEach(async () => {
    await demonter(montage);
    montage = undefined;
    for (const r of racines) rmSync(r, { recursive: true, force: true });
    racines = [];
});

async function servir(nom: string): Promise<string> {
    const r = mkdtempSync(join(tmpdir(), 'g2-routes-icone-'));
    racines.push(r);
    magasin = ouvrirMagasin(join(r, 'icones'), () => {});
    montage = await monterRoute(nom, (req, rep, base) =>
        servirIcone(req, rep, {
            base,
            secretJeton: SECRET,
            magasin,
            maintenant: () => MS,
        }),
    );
    return montage.url;
}

describe(`routes d'icône, moteur=${MOTEUR}`, () => {
    it('🔴 ne mange QUE ses deux chemins — le corps du 404 est comparé', async () => {
        // 🔴 SI CETTE MUTATION SURVIT, LE TEST EST FAUX, PAS LA MUTATION.
        // Quatre chemins déclinés, dont deux qui n'existaient dans aucune
        // première rédaction.
        const url = await servir('icone-chemins');
        for (const chemin of [
            '/iconedetournee',
            '/icone/',
            '/icone/a/b',
            '/application/x/icone/y',
            '/applications',
            '/application/x/lancer',
        ]) {
            const r = await fetch(`${url}${chemin}`);
            expect(r.status, chemin).toBe(404);
            // Le 404 GÉNÉRIQUE de `serveur.ts`, mot pour mot — pas un 404 typé
            // que ce routeur aurait rendu.
            expect(await r.text(), chemin).toBe('introuvable\n');
        }
    });

    it('sert `/application/:id/icone` SANS `?e=` par un 400 typé', async () => {
        const url = await servir('icone-sans-e');
        const jeton = jetonDe('u1');
        const r = await fetch(`${url}/application/x/icone`, { headers: avec(jeton) });
        // Le chemin EST reconnu — donc pas le 404 générique.
        expect(r.status).toBe(400);
        expect(await r.json()).toEqual({ refus: 'empreinte-absente' });
    });

    describe('PUT /icone/:sha256 — l’agent dépose', () => {
        it('accepte un jeton d’AGENT et rend 204', async () => {
            const url = await servir('icone-put');
            const r = await fetch(`${url}/icone/${EMPREINTE}`, {
                method: 'PUT',
                headers: avec(jetonDe('vm-1', 'agent')),
                body: PNG,
            });
            expect(r.status).toBe(204);
            expect(magasin.lire(EMPREINTE)).toEqual(PNG);
        });

        it('🔴 REFUSE un jeton d’UTILISATEUR en 403, jamais 401', async () => {
            // 🔴 SYMÉTRIQUE DU `jeton-agent` de `porteur.ts`, et argumenté de
            // la même façon : le jeton est VALIDE, il n'est simplement pas
            // celui d'un agent. Un 401 inviterait à se reconnecter pour rien.
            const url = await servir('icone-put-humain');
            const r = await fetch(`${url}/icone/${EMPREINTE}`, {
                method: 'PUT',
                headers: avec(jetonDe('u1')),
                body: PNG,
            });
            expect(r.status).toBe(403);
            expect(await r.json()).toEqual({ refus: 'jeton-utilisateur' });
            expect(magasin.lire(EMPREINTE)).toBeUndefined();
        });

        it('refuse sans jeton, et avec un jeton illisible', async () => {
            const url = await servir('icone-put-nu');
            const sans = await fetch(`${url}/icone/${EMPREINTE}`, { method: 'PUT', body: PNG });
            expect(sans.status).toBe(401);
            expect(await sans.json()).toEqual({ refus: 'jeton-absent' });
            const faux = await fetch(`${url}/icone/${EMPREINTE}`, {
                method: 'PUT',
                headers: { authorization: 'Bearer pas-un-jeton' },
                body: PNG,
            });
            expect(faux.status).toBe(401);
            expect(await faux.json()).toEqual({ refus: 'jeton-invalide' });
        });

        it('🔴 RECALCULE l’empreinte et REFUSE si elle ment', async () => {
            // 🔴 SANS CE RECALCUL, L'ADRESSAGE PAR CONTENU N'EN SERAIT PAS UN,
            // et `Cache-Control: immutable` rendrait l'empoisonnement PERMANENT
            // dans les caches. Éprouvé au magasin, REJOUÉ ICI de bout en bout.
            const url = await servir('icone-put-menteur');
            const r = await fetch(`${url}/icone/${AUTRE}`, {
                method: 'PUT',
                headers: avec(jetonDe('vm-1', 'agent')),
                body: PNG,
            });
            expect(r.status).toBe(400);
            expect(await r.json()).toEqual({ refus: 'empreinte' });
            expect(magasin.lire(AUTRE)).toBeUndefined();
        });

        it('🔴 REFUSE une empreinte qui pourrait sortir du magasin', async () => {
            const url = await servir('icone-put-chemin');
            // `..%2f..%2fx` : le chemin décodé sortirait du répertoire.
            const r = await fetch(`${url}/icone/..%2f..%2fx`, {
                method: 'PUT',
                headers: avec(jetonDe('vm-1', 'agent')),
                body: PNG,
            });
            expect(r.status).toBe(400);
            expect(await r.json()).toEqual({ refus: 'empreinte-invalide' });
        });

        it('refuse un corps au-delà du plafond, en 413 TYPÉ', async () => {
            const url = await servir('icone-put-gros');
            const gros = Buffer.alloc(ICONE_MAX_OCTETS + 1, 7);
            const r = await fetch(`${url}/icone/${createHash('sha256').update(gros).digest('hex')}`, {
                method: 'PUT',
                headers: avec(jetonDe('vm-1', 'agent')),
                body: gros,
            });
            expect(r.status).toBe(413);
            expect(await r.json()).toEqual({ refus: 'taille' });
        });

        it('refuse une autre méthode que PUT en 405', async () => {
            const url = await servir('icone-put-methode');
            const r = await fetch(`${url}/icone/${EMPREINTE}`, { headers: avec(jetonDe('u1')) });
            expect(r.status).toBe(405);
            expect(await r.json()).toEqual({ refus: 'methode' });
        });
    });

    describe('GET /application/:id/icone?e= — l’utilisateur lit', () => {
        async function poser(nom: string): Promise<{ url: string; id: string; u: string }> {
            const url = await servir(nom);
            const base = montage!.base;
            await poserVm(base, 'vm-1');
            const u = await attribuer(base, 'vm-1', 'ada@exemple.test');
            const id = await poserApp(base, 'vm-1', 'Bloc-notes', 'a1b2', EMPREINTE, {
                pixels: 256,
            });
            magasin.ecrire(EMPREINTE, PNG);
            return { url, id, u };
        }

        it('sert le PNG, avec un `Cache-Control` immuable', async () => {
            const { url, id, u } = await poser('icone-get');
            const r = await fetch(`${url}/application/${id}/icone?e=${EMPREINTE}`, {
                headers: avec(jetonDe(u)),
            });
            expect(r.status).toBe(200);
            expect(r.headers.get('content-type')).toBe('image/png');
            // ⚠️ `private`, JAMAIS `public` : la réponse est authentifiée par
            // le porteur, et un cache partagé n'a rien à faire d'une icône
            // servie sous un jeton.
            expect(r.headers.get('cache-control')).toBe('private, max-age=31536000, immutable');
            // 🔴 L'EXCEPTION NE S'ÉLARGIT PAS : `cache-control` est écrasé,
            // `nosniff` NE L'EST PAS. C'est le seul des deux en-têtes de
            // sécurité qui soit une garde — sans lui, un navigateur pourrait
            // deviner un type autre que `image/png` sur des octets qu'un pair
            // a déposés.
            expect(r.headers.get('x-content-type-options')).toBe('nosniff');
            expect(Buffer.from(await r.arrayBuffer())).toEqual(PNG);
        });

        it('🔴 un `?e=` PÉRIMÉ rend 404, et c’est ce qui rend `immutable` HONNÊTE', async () => {
            // 🔴 SANS CE REFUS, une vieille URL servirait l'icône COURANTE
            // sous un en-tête immuable — le cache serait empoisonné POUR UN AN
            // avec une image qui n'est pas celle que l'URL nomme.
            const { url, id, u } = await poser('icone-get-perime');
            magasin.ecrire(AUTRE, Buffer.from('autre'));
            const r = await fetch(`${url}/application/${id}/icone?e=${AUTRE}`, {
                headers: avec(jetonDe(u)),
            });
            expect(r.status).toBe(404);
            expect(await r.json()).toEqual({ refus: 'icone-inconnue' });
        });

        it('🔴 l’icône d’une VM D’AUTRUI répond comme une VM INCONNUE', async () => {
            // 🔴 LE REFUS EST INDISTINGUABLE — `404 vm-inconnue`, jamais un
            // `403` qui dirait « celle-là existe, mais pas pour vous ». C'est
            // l'oracle d'énumération que le propriétaire du dépôt a RETIRÉ, et
            // le réintroduire ici serait le rouvrir par une porte de derrière.
            const { url, id } = await poser('icone-get-autrui');
            const autre = await creerUtilisateur(montage!.base, 'autre@exemple.test', 'x', MS);
            const etrangere = await fetch(`${url}/application/${id}/icone?e=${EMPREINTE}`, {
                headers: avec(jetonDe(autre)),
            });
            const inconnue = await fetch(`${url}/application/pas-un-id/icone?e=${EMPREINTE}`, {
                headers: avec(jetonDe(autre)),
            });
            expect(etrangere.status).toBe(404);
            expect(inconnue.status).toBe(404);
            // Le CORPS aussi : c'est là que l'oracle se lirait.
            expect(await etrangere.json()).toEqual({ refus: 'vm-inconnue' });
            expect(await inconnue.json()).toEqual({ refus: 'application-inconnue' });
        });

        it('🔴 REFUSE un jeton d’AGENT sur le GET — le symétrique du PUT', async () => {
            const { url, id } = await poser('icone-get-agent');
            const r = await fetch(`${url}/application/${id}/icone?e=${EMPREINTE}`, {
                headers: avec(jetonDe('vm-1', 'agent')),
            });
            expect(r.status).toBe(403);
            expect(await r.json()).toEqual({ refus: 'jeton-agent' });
        });

        it('une application SANS icône rend 404, pas une image vide', async () => {
            const url = await servir('icone-get-sans');
            const base = montage!.base;
            await poserVm(base, 'vm-1');
            const u = await attribuer(base, 'vm-1', 'ada@exemple.test');
            const id = await poserApp(base, 'vm-1', 'Sans', 'c3d4');
            const r = await fetch(`${url}/application/${id}/icone?e=${EMPREINTE}`, {
                headers: avec(jetonDe(u)),
            });
            expect(r.status).toBe(404);
            expect(await r.json()).toEqual({ refus: 'icone-inconnue' });
        });

        it('la base connaît l’empreinte, le DISQUE ne l’a pas : 404, et cela se répare seul', async () => {
            const { url, id, u } = await poser('icone-get-disque-vide');
            rmSync(join(magasin.repertoire, EMPREINTE));
            const r = await fetch(`${url}/application/${id}/icone?e=${EMPREINTE}`, {
                headers: avec(jetonDe(u)),
            });
            expect(r.status).toBe(404);
            // Et l'inventaire la redemande : c'est l'auto-reconstruction.
            expect(magasin.manquantes([EMPREINTE])).toEqual([EMPREINTE]);
        });

        it('sert la réponse préalable OPTIONS', async () => {
            // ⚠️ LES DEUX ROUTES EXIGENT `Authorization`, DONC LA REQUÊTE EST
            // NON SIMPLE : le navigateur envoie d'abord un `OPTIONS` et
            // ABANDONNE sans jamais envoyer la vraie requête si la réponse ne
            // lui convient pas. C'est le défaut exact que la corroboration
            // navigateur de P4 a trouvé, et qu'aucun test de Node ne voyait.
            const url = await servir('icone-options');
            for (const chemin of [`/icone/${EMPREINTE}`, '/application/x/icone']) {
                const r = await fetch(`${url}${chemin}`, { method: 'OPTIONS' });
                expect(r.status, chemin).toBe(204);
            }
        });
    });
});
