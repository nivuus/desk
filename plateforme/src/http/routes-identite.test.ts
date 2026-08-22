import { afterEach, describe, expect, it } from 'vitest';
import { baseNeuve } from '../base/harnais';
import type { Pilote } from '../base/pilote';
import { demarrerServeur, type ServicePlateforme } from './serveur';
import type { Config } from '../config';
import { mkdtempSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { lireIdentitePomerium, ENTETE_IDENTITE } from './routes-identite';

const SECRET = 'un-secret-de-plateforme-de-quarante-octets';

const CONFIG: Config = {
    hote: '127.0.0.1',
    port: 0,
    base: 'sqlite',
    urlBase: ':memory:',
    secretJeton: SECRET,
    auth: 'pomerium',
    // Les tests de ce fichier se connectent en boucle locale : c'est
    // l'adresse que `req.socket.remoteAddress` va réellement porter.
    proxyDeConfiance: new Set(['127.0.0.1']),
    repertoireIcones: join(mkdtempSync(join(tmpdir(), 'moi-icones-')), 'icones'),
    repertoireTeleversements: join(mkdtempSync(join(tmpdir(), 'moi-tranches-')), 'televersements'),
};

describe('lireIdentitePomerium', () => {
    it('rend le courriel', () => {
        const v = lireIdentitePomerium({ [ENTETE_IDENTITE]: 'a@b.c' });
        expect(v).toEqual({ ok: true, email: 'a@b.c' });
    });

    it('rogne les espaces', () => {
        const v = lireIdentitePomerium({ [ENTETE_IDENTITE]: '  a@b.c  ' });
        expect(v).toEqual({ ok: true, email: 'a@b.c' });
    });

    // 🔴 AUCUN REPLI SUR UN UTILISATEUR PAR DÉFAUT : une mauvaise
    // configuration du proxy doit être bruyante et refusante.
    it("refuse l'en-tête absent", () => {
        expect(lireIdentitePomerium({})).toEqual({ ok: false, motif: 'identite-absente' });
    });

    it("refuse l'en-tête vide", () => {
        expect(lireIdentitePomerium({ [ENTETE_IDENTITE]: '   ' })).toEqual({
            ok: false,
            motif: 'identite-absente',
        });
    });

    // Précédent littéral de `porteur.ts` : en choisir un serait prendre une
    // décision qu'un attaquant exploite dès que deux couches n'en prennent pas
    // la même.
    it("REFUSE un en-tête RÉPÉTÉ, jamais ne le désambiguïse", () => {
        expect(lireIdentitePomerium({ [ENTETE_IDENTITE]: ['a@b.c', 'mechant@x.y'] })).toEqual({
            ok: false,
            motif: 'identite-absente',
        });
    });
});

let base: Pilote | undefined;
let service: ServicePlateforme | undefined;

afterEach(async () => {
    await service?.close();
    service = undefined;
    await base?.fermer();
    base = undefined;
});

/// Compte les comptes. C'est LE COMPTE qui dit si l'upsert a créé une fois ou
/// deux — jamais la seule absence d'erreur.
async function combienDeComptes(p: Pilote): Promise<number> {
    return (await p.interroger<{ id: string }>('SELECT id FROM utilisateur', [])).length;
}

describe('GET /auth/moi', () => {
    it('① compte inconnu ⇒ 200, et le compte est CRÉÉ', async () => {
        base = await baseNeuve('moi-inconnu');
        service = await demarrerServeur(CONFIG, base);
        expect(await combienDeComptes(base)).toBe(0);

        const r = await fetch(`http://127.0.0.1:${service.port}/auth/moi`, {
            headers: { 'x-pomerium-claim-email': 'a@b.c' },
        });

        expect(r.status).toBe(200);
        const corps = (await r.json()) as { acces?: unknown };
        expect(typeof corps.acces).toBe('string');
        expect(await combienDeComptes(base)).toBe(1);
    });

    it('② compte connu ⇒ 200, et AUCUN compte de plus', async () => {
        base = await baseNeuve('moi-connu');
        service = await demarrerServeur(CONFIG, base);
        const url = `http://127.0.0.1:${service.port}/auth/moi`;
        const en = { 'x-pomerium-claim-email': 'a@b.c' };

        await fetch(url, { headers: en });
        const r = await fetch(url, { headers: en });

        expect(r.status).toBe(200);
        expect(await combienDeComptes(base)).toBe(1);
    });

    // 🔴 LA ROUGE DU CRITÈRE ① : sans `pass_identity_headers` chez Pomerium,
    // le service REFUSE — il ne se replie sur aucun utilisateur par défaut.
    it('③ en-tête absent ⇒ 401 identite-absente', async () => {
        base = await baseNeuve('moi-absent');
        service = await demarrerServeur(CONFIG, base);

        const r = await fetch(`http://127.0.0.1:${service.port}/auth/moi`);

        expect(r.status).toBe(401);
        expect(await r.json()).toEqual({ refus: 'identite-absente' });
        expect(await combienDeComptes(base)).toBe(0);
    });

    // 🔴 LA GARDE QUI FERME LE CONTOURNEMENT : LES DEUX BRAS, SINON LE 401
    // SEUL NE PROUVERAIT RIEN — il serait indiscernable d'une route entièrement
    // en panne. Le bras VERT est ① et ② ci-dessus, qui se connectent en boucle
    // locale et réussissent précisément parce que `CONFIG.proxyDeConfiance`
    // déclare `127.0.0.1`.
    it("REFUSE (401) l'en-tête d'identité venu d'un pair non déclaré", async () => {
        base = await baseNeuve('moi-pair-etranger-statut');
        service = await demarrerServeur({ ...CONFIG, proxyDeConfiance: new Set(['10.9.9.9']) }, base);
        const r = await fetch(`http://127.0.0.1:${service.port}/auth/moi`, {
            headers: { [ENTETE_IDENTITE]: 'a@b.c' },
        });
        expect(r.status).toBe(401);
    });

    it("REFUSE l'en-tête d'identité venu d'un pair non déclaré, avec le motif nommé", async () => {
        base = await baseNeuve('moi-pair-etranger-motif');
        service = await demarrerServeur({ ...CONFIG, proxyDeConfiance: new Set(['10.9.9.9']) }, base);
        const r = await fetch(`http://127.0.0.1:${service.port}/auth/moi`, {
            headers: { [ENTETE_IDENTITE]: 'a@b.c' },
        });
        const corps = (await r.json()) as { refus?: unknown };
        expect(corps.refus).toBe('pair-non-de-confiance');
    });

    // 🔴 LES ROUGES DES CRITÈRES ② ET ③ EN UN SEUL TEST, ET L'EN-TÊTE EST
    // PRÉSENT À DESSEIN : c'est ce qui prouve qu'un en-tête FORGÉ est ignoré
    // en mode `motdepasse`, et pas seulement que la route est absente.
    it('④ mode motdepasse ⇒ 404, en-tête forgé IGNORÉ', async () => {
        base = await baseNeuve('moi-motdepasse');
        service = await demarrerServeur({ ...CONFIG, auth: 'motdepasse' }, base);

        const r = await fetch(`http://127.0.0.1:${service.port}/auth/moi`, {
            headers: { 'x-pomerium-claim-email': 'forge@x.y' },
        });

        expect(r.status).toBe(404);
        expect(await combienDeComptes(base)).toBe(0);
    });
});
