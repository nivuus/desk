import { afterEach, describe, expect, it } from 'vitest';
import { baseNeuve, piloteCompteur } from '../base/harnais';
import type { Pilote } from '../base/pilote';
import { CacheSante, PERIODE_SANTE_MS, servirSante } from './routes-sante';
import { demarrerServeur, type ServicePlateforme } from './serveur';
import type { Config } from '../config';
import { mkdtempSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const SECRET = 'un-secret-de-plateforme-de-quarante-octets';
/// Une époque réelle, sur le patron de `base/harnais.ts` : les petites valeurs
/// ne mesurent rien.
const T0 = 1_787_000_000_000;

const CONFIG: Config = {
    hote: '127.0.0.1',
    port: 0,
    base: 'sqlite',
    urlBase: ':memory:',
    secretJeton: SECRET,
    proxyDeConfiance: new Set(),
    repertoireIcones: join(mkdtempSync(join(tmpdir(), 'g2-icones-')), 'icones'),
};

let base: Pilote | undefined;
let service: ServicePlateforme | undefined;

afterEach(async () => {
    await service?.close();
    service = undefined;
    await base?.fermer();
    base = undefined;
});

/// Un pilote dont TOUT lever — pour éprouver le 503 sans casser une vraie base.
function piloteMort(): Pilote {
    const mort: Pilote = {
        async interroger() {
            throw new Error('base injoignable');
        },
        async executer() {
            throw new Error('base injoignable');
        },
        async transaction<T>(corps: (p: Pilote) => Promise<T>): Promise<T> {
            return corps(mort);
        },
        async fermer() {},
    };
    return mort;
}

describe('GET /sante', () => {
    it('(a) base saine ⇒ 200 et `{"etat":"ok"}`', async () => {
        base = await baseNeuve('sante-ok');
        service = await demarrerServeur(CONFIG, base);
        const r = await fetch(`http://127.0.0.1:${service.port}/sante`);
        expect(r.status).toBe(200);
        expect(await r.json()).toEqual({ etat: 'ok' });
    });

    it('(b) base en échec ⇒ 503 et `{"etat":"degrade"}`', async () => {
        service = await demarrerServeur(CONFIG, piloteMort());
        const r = await fetch(`http://127.0.0.1:${service.port}/sante`);
        expect(r.status).toBe(503);
        expect(await r.json()).toEqual({ etat: 'degrade' });
    });

    it("(c) 🔴 la réponse ne porte RIEN D'AUTRE", async () => {
        // 🔴 COMPARAISON DE L'OBJET ENTIER, jamais un `toContain` : une page de
        // santé bavarde est un INVENTAIRE offert à un anonyme, et c'est par
        // construction la seule route non authentifiée qui reste après P2.
        // Un ajout futur de version, de compte de sessions, d'URL de base ou
        // de nom de moteur doit faire ROUGIR ce test.
        base = await baseNeuve('sante-rien-d-autre');
        service = await demarrerServeur(CONFIG, base);
        const corps = (await (await fetch(`http://127.0.0.1:${service.port}/sante`)).json()) as Record<string, unknown>;
        expect(Object.keys(corps)).toEqual(['etat']);
    });

    it('(d) 🔴 N appels dans la période ne font QU’UNE requête', async () => {
        // 🔴 SANS LE CACHE, `/sante` TRADUIT UNE REQUÊTE HTTP ANONYME EN
        // REQUÊTE SQL, À VOLONTÉ : c'est une amplification, sur la route même
        // qu'un équilibreur de charge appelle en boucle. Le cache est LE POINT
        // de cette route, pas un raffinement.
        const reel = await baseNeuve('sante-cache');
        base = reel;
        const compteur = piloteCompteur(reel);
        const cache = new CacheSante();
        let horloge = T0;
        const deps = { base: compteur.pilote, maintenant: () => horloge, cache };
        const N = 5;
        for (let i = 0; i < N; i++) {
            expect(await cache.verdict(deps.base, deps.maintenant())).toBe(true);
        }
        expect(compteur.acces()).toBe(1);
    });

    it('(e) après la période, une NOUVELLE requête a lieu', async () => {
        const reel = await baseNeuve('sante-cache-expire');
        base = reel;
        const compteur = piloteCompteur(reel);
        const cache = new CacheSante();
        expect(await cache.verdict(compteur.pilote, T0)).toBe(true);
        expect(compteur.acces()).toBe(1);
        // Juste sous la période : toujours le verdict en cache.
        expect(await cache.verdict(compteur.pilote, T0 + PERIODE_SANTE_MS - 1)).toBe(true);
        expect(compteur.acces()).toBe(1);
        // Au-delà : on redemande.
        expect(await cache.verdict(compteur.pilote, T0 + PERIODE_SANTE_MS + 1)).toBe(true);
        expect(compteur.acces()).toBe(2);
    });

    it('(d bis) 🔴 N appels CONCURRENTS ne font QU’UNE requête non plus', async () => {
        // Le cas réel d'un équilibreur de charge : plusieurs sondes en vol au
        // même instant. Sans déduplication de la requête EN COURS, chacune
        // lancerait la sienne — et le cache ne servirait qu'après coup,
        // c'est-à-dire jamais sous la charge qu'il existe pour absorber.
        const reel = await baseNeuve('sante-cache-concurrent');
        base = reel;
        const compteur = piloteCompteur(reel);
        const cache = new CacheSante();
        const tous = await Promise.all(
            Array.from({ length: 8 }, () => cache.verdict(compteur.pilote, T0)),
        );
        expect(tous).toEqual(Array.from({ length: 8 }, () => true));
        expect(compteur.acces()).toBe(1);
    });

    it('(f) une méthode autre que GET rend 405', async () => {
        base = await baseNeuve('sante-methode');
        service = await demarrerServeur(CONFIG, base);
        const r = await fetch(`http://127.0.0.1:${service.port}/sante`, { method: 'POST' });
        expect(r.status).toBe(405);
        expect(await r.json()).toEqual({ refus: 'methode' });
    });

    it("(g) `/sante` n'est PAS authentifiée — aucun jeton n'est exigé", async () => {
        // ⚠️ Délibéré, et c'est ce qui rend le cache obligatoire : une sonde
        // d'équilibreur ne présente aucun jeton, et une sonde FREINÉE
        // déclarerait le service mort. Le cache est ce qui la rend sûre SANS
        // frein — les deux décisions vivent dans le même paragraphe.
        base = await baseNeuve('sante-anonyme');
        service = await demarrerServeur(CONFIG, base);
        const r = await fetch(`http://127.0.0.1:${service.port}/sante`);
        expect(r.status).toBe(200);
    });

    it("(h) un chemin voisin n'est PAS servi par la santé", async () => {
        // Comparaison EXACTE, jamais un `startsWith` : `/santelle` n'est pas
        // `/sante`, et un préfixe ouvrirait une famille de chemins que
        // personne n'a décidés.
        base = await baseNeuve('sante-chemin');
        service = await demarrerServeur(CONFIG, base);
        expect((await fetch(`http://127.0.0.1:${service.port}/santelle`)).status).toBe(404);
    });

    it('(i) `servirSante` rend `false` sur un chemin qui n’est pas le sien', async () => {
        // La convention des quatre routeurs : rendre `false` laisse le suivant
        // essayer, et le 404 générique conclut.
        const req = { url: '/autre', method: 'GET', headers: {} } as never;
        const rep = { writeHead() {}, end() {}, setHeader() {} } as never;
        base = await baseNeuve('sante-faux');
        expect(await servirSante(req, rep, { base, maintenant: () => T0, cache: new CacheSante() }))
            .toBe(false);
    });
});
