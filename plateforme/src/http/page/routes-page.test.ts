import { afterEach, describe, expect, it } from 'vitest';
import { mkdirSync, mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { baseNeuve } from '../../base/harnais';
import type { Pilote } from '../../base/pilote';
import { demarrerServeur, type ServicePlateforme } from '../serveur';
import type { Config } from '../../config';

const SECRET = 'un-secret-de-plateforme-de-quarante-octets';

const CONFIG: Config = {
    hote: '127.0.0.1',
    port: 0,
    base: 'sqlite',
    urlBase: ':memory:',
    secretJeton: SECRET,
    proxyDeConfiance: new Set(),
    repertoireIcones: join(mkdtempSync(join(tmpdir(), 'page-icones-')), 'icones'),
    repertoireTeleversements: join(mkdtempSync(join(tmpdir(), 'page-tranches-')), 'televersements'),
    auth: 'pomerium',
};

let base: Pilote | undefined;
let service: ServicePlateforme | undefined;

afterEach(async () => {
    await service?.close();
    service = undefined;
    await base?.fermer();
    base = undefined;
});

// Une racine temporaire, bâtie à la main : le test ne dépend pas de
// `client/dist`, qui peut ne pas être bâti.
function racineJetable(): string {
    const racine = mkdtempSync(join(tmpdir(), 'page-'));
    mkdirSync(join(racine, 'assets'));
    writeFileSync(join(racine, 'index.html'), '<!doctype html><title>page</title>');
    writeFileSync(join(racine, 'assets', 'index-a1b2c3.js'), 'export const x = 1;\n');
    // Le PIÈGE que le critère ⑥ éprouve : un fichier homonyme d'une route.
    writeFileSync(join(racine, 'sante'), 'ceci ne doit JAMAIS etre servi');
    writeFileSync(join(racine, 'secret.env'), 'MOT_DE_PASSE=x');
    return racine;
}

describe('GET /', () => {
    // 🔴 LE TÉMOIN NÉGATIF. Sans lui, le 200 du test suivant ne prouve pas que
    // PLATEFORME_PAGE a servi à quelque chose.
    it("sans PLATEFORME_PAGE, GET / rend le 404 d'hier, mot pour mot", async () => {
        base = await baseNeuve('page-temoin-negatif');
        service = await demarrerServeur({ ...CONFIG, racinePage: undefined }, base);
        const r = await fetch(`http://127.0.0.1:${service.port}/`);
        expect(r.status).toBe(404);
        expect(await r.text()).toBe('introuvable\n');
    });

    it('avec PLATEFORME_PAGE, GET / rend index.html avec ses en-têtes de document', async () => {
        base = await baseNeuve('page-index');
        service = await demarrerServeur({ ...CONFIG, racinePage: racineJetable() }, base);
        const r = await fetch(`http://127.0.0.1:${service.port}/`);
        expect(r.status).toBe(200);
        // Comparer les OCTETS, jamais le seul code 200.
        expect(await r.text()).toBe('<!doctype html><title>page</title>');
        expect(r.headers.get('content-security-policy')).toContain("default-src 'self'");
        expect(r.headers.get('cache-control')).toBe('no-store');
    });

    it('un actif porte immutable, JAMAIS no-store', async () => {
        base = await baseNeuve('page-actif');
        service = await demarrerServeur({ ...CONFIG, racinePage: racineJetable() }, base);
        const r = await fetch(`http://127.0.0.1:${service.port}/assets/index-a1b2c3.js`);
        expect(r.headers.get('cache-control')).toBe('public, max-age=31536000, immutable');
    });

    // 🔴 LA ROUGE DE L'ORDRE DE CHAÎNAGE. Un fichier nommé `sante` déposé dans la
    // racine ne doit JAMAIS supplanter le routeur de santé — la panne la plus
    // discrète possible, le service répondant 200 avec un corps plausible.
    it('/sante reste servi par SON routeur, malgré un fichier homonyme', async () => {
        base = await baseNeuve('page-homonyme');
        service = await demarrerServeur({ ...CONFIG, racinePage: racineJetable() }, base);
        const r = await fetch(`http://127.0.0.1:${service.port}/sante`);
        expect(r.headers.get('content-type')).toContain('application/json');
        expect(await r.text()).not.toContain('JAMAIS');
    });

    it('refuse un fichier hors de la liste MIME', async () => {
        base = await baseNeuve('page-mime-refuse');
        service = await demarrerServeur({ ...CONFIG, racinePage: racineJetable() }, base);
        const r = await fetch(`http://127.0.0.1:${service.port}/secret.env`);
        expect(r.status).toBe(404);
    });

    // 🔴 HORS GET/HEAD, LE COMPORTEMENT EST CELUI D'HIER À L'OCTET PRÈS. Rendre
    // 405 ferait qu'un POST sur un chemin mal orthographié — le repli SPA résout
    // n'importe quoi — obtiendrait « méthode » au lieu du 404 qui le désigne.
    it('un POST sur un chemin inconnu rend toujours 404, jamais 405', async () => {
        base = await baseNeuve('page-post-inconnu');
        service = await demarrerServeur({ ...CONFIG, racinePage: racineJetable() }, base);
        const r = await fetch(`http://127.0.0.1:${service.port}/aplication/x`, { method: 'POST' });
        expect(r.status).toBe(404);
    });

    it('sert un HEAD sans corps', async () => {
        base = await baseNeuve('page-head');
        service = await demarrerServeur({ ...CONFIG, racinePage: racineJetable() }, base);
        const r = await fetch(`http://127.0.0.1:${service.port}/`, { method: 'HEAD' });
        expect(r.status).toBe(200);
        expect(await r.text()).toBe('');
    });
});
