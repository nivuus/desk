// Les deux routes d'authentification, éprouvées À TRAVERS le serveur HTTP réel.
//
// 🔴 LE CRITÈRE ④ EST DANS CE FICHIER, et son contrôle du contrôle a été joué :
// un `console.log(JSON.stringify(corps))` délibérément ajouté dans la route le
// fait rougir. Un test de balayage qui ne capturerait pas réellement la console
// serait vert quoi qu'il arrive — le patron du contrôle vacueux, payé quatre
// fois par ce dépôt.

import { afterEach, describe, expect, it, vi } from 'vitest';
import { baseNeuve } from '../base/harnais';
import type { Pilote } from '../base/pilote';
import type { Config } from '../config';
import { hacher } from '../identite/mot-de-passe';
import { verifierJeton } from '../identite/jeton';
import { creerUtilisateur } from '../depot/utilisateur';
import { demarrerServeur, type ServicePlateforme } from './serveur';

const SECRET = 'un-secret-de-plateforme-de-quarante-octets';
const ORIGINE = 'http://127.0.0.1:5173';
const MOT_DE_PASSE = 'un-mot-de-passe-ordinaire-42';
const MS = 1_787_136_773_742;

function config(origineClient?: string): Config {
    return {
        hote: '127.0.0.1',
        port: 0,
        base: 'sqlite',
        urlBase: ':memory:',
        secretJeton: SECRET,
        origineClient,
    };
}

let base: Pilote | undefined;
let service: ServicePlateforme | undefined;

afterEach(async () => {
    await service?.close();
    service = undefined;
    await base?.fermer();
    base = undefined;
    vi.restoreAllMocks();
});

async function servir(nom: string, origineClient?: string): Promise<string> {
    base = await baseNeuve(nom);
    await creerUtilisateur(base, 'ada@exemple.test', await hacher(MOT_DE_PASSE), MS);
    service = await demarrerServeur(config(origineClient), base);
    return `http://127.0.0.1:${service.port}`;
}

/// `Response.json()` rend `unknown` : ce petit typage évite d'éparpiller des
/// assertions de type dans les tests, sans rien affirmer sur le contenu.
interface Paire { acces: string; rafraichissement: string; expire_a: number; refus?: string }
async function corpsDe(r: Response): Promise<Paire & Record<string, unknown>> {
    return (await r.json()) as Paire & Record<string, unknown>;
}

function poster(url: string, corps: unknown, entetes: Record<string, string> = {}) {
    return fetch(url, {
        method: 'POST',
        headers: { 'content-type': 'application/json', ...entetes },
        body: typeof corps === 'string' ? corps : JSON.stringify(corps),
    });
}

describe('routes d’authentification', () => {
    it('connexion valide : l’accès est un jeton que le service accepte, et le rafraîchissement vaut', async () => {
        const base_ = await servir('auth-ok');
        const r = await poster(`${base_}/auth/connexion`, {
            email: 'ada@exemple.test',
            motdepasse: MOT_DE_PASSE,
        });
        expect(r.status).toBe(200);
        const corps = await corpsDe(r);
        expect(typeof corps.acces).toBe('string');
        expect(typeof corps.rafraichissement).toBe('string');
        expect(corps.expire_a).toBeGreaterThan(Date.now());
        // Le jeton rendu est réellement vérifiable par ce service.
        expect(verifierJeton(corps.acces, SECRET, Date.now()).ok).toBe(true);

        // Et le rafraîchissement tourne.
        const r2 = await poster(`${base_}/auth/rafraichir`, {
            rafraichissement: corps.rafraichissement,
        });
        expect(r2.status).toBe(200);
        const corps2 = await corpsDe(r2);
        expect(corps2.rafraichissement).not.toBe(corps.rafraichissement);
    });

    it('mot de passe faux et courriel inconnu rendent le MÊME message', async () => {
        // 🔴 Un message qui les distingue est un ORACLE d'énumération de
        // comptes : l'attaquant apprend quelles adresses existent.
        const base_ = await servir('auth-refus');
        const faux = await poster(`${base_}/auth/connexion`, {
            email: 'ada@exemple.test',
            motdepasse: 'ce-n-est-pas-le-bon-mot-de-passe',
        });
        const inconnu = await poster(`${base_}/auth/connexion`, {
            email: 'personne@exemple.test',
            motdepasse: MOT_DE_PASSE,
        });
        expect(faux.status).toBe(401);
        expect(inconnu.status).toBe(401);
        expect(await corpsDe(faux)).toEqual(await corpsDe(inconnu));
        const troisieme = await corpsDe(await poster(`${base_}/auth/connexion`, {
            email: 'ada@exemple.test',
            motdepasse: 'encore-faux',
        }));
        expect(troisieme.refus).toBe('identifiants');
    });

    it('REJEU : rafraîchir deux fois le même jeton rend 401, et la famille tombe', async () => {
        const base_ = await servir('auth-rejeu');
        const { rafraichissement } = await corpsDe(await poster(`${base_}/auth/connexion`, {
            email: 'ada@exemple.test',
            motdepasse: MOT_DE_PASSE,
        }));
        const premier = await corpsDe(await poster(`${base_}/auth/rafraichir`, { rafraichissement }));

        const rejeu = await poster(`${base_}/auth/rafraichir`, { rafraichissement });
        expect(rejeu.status).toBe(401);
        expect((await corpsDe(rejeu)).refus).toBe('rejeu');

        // Le jeton NEUF, que le voleur détiendrait, est mort lui aussi.
        const neuf = await poster(`${base_}/auth/rafraichir`, {
            rafraichissement: premier.rafraichissement,
        });
        expect(neuf.status).toBe(401);
    });

    it('borne le corps à 4 KiB : 5 KiB rend 413', async () => {
        // Sans borne, un pair anonyme fait grossir la mémoire à volonté.
        const base_ = await servir('auth-gros');
        const r = await poster(`${base_}/auth/connexion`, JSON.stringify({
            email: 'ada@exemple.test',
            motdepasse: 'x'.repeat(5 * 1024),
        })).catch(() => undefined);
        // La connexion peut être coupée avant la réponse : les deux issues
        // sont acceptables, l'important est qu'aucun 200 ne sorte.
        if (r) expect(r.status).toBe(413);
    });

    it('refuse une méthode autre que POST par 405, et un corps non-JSON par 400', async () => {
        const base_ = await servir('auth-methode');
        const g = await fetch(`${base_}/auth/connexion`);
        expect(g.status).toBe(405);
        const mauvais = await poster(`${base_}/auth/connexion`, 'ceci-n-est-pas-du-json');
        expect(mauvais.status).toBe(400);
        expect((await corpsDe(mauvais)).refus).toBe('forme');
    });

    it('OPTIONS rend 204, avec les en-têtes CORS SEULEMENT si une origine est autorisée', async () => {
        const avec = await servir('auth-cors-oui', ORIGINE);
        const r = await fetch(`${avec}/auth/connexion`, {
            method: 'OPTIONS',
            headers: { origin: ORIGINE },
        });
        expect(r.status).toBe(204);
        expect(r.headers.get('access-control-allow-origin')).toBe(ORIGINE);
        expect(r.headers.get('vary')).toBe('Origin');
        await service!.close();
        service = undefined;
        await base!.fermer();
        base = undefined;

        const sans = await servir('auth-cors-non');
        const r2 = await fetch(`${sans}/auth/connexion`, {
            method: 'OPTIONS',
            headers: { origin: ORIGINE },
        });
        expect(r2.status).toBe(204);
        // Le défaut est le refus : aucun en-tête, jamais `*`.
        expect(r2.headers.get('access-control-allow-origin')).toBeNull();
    });

    it('laisse le 404 de P1 intact sur un chemin qui n’est pas d’authentification', async () => {
        // ⚠️ Rien ne testait ce 404 : le changer serait un effet de bord non
        // déclaré.
        const base_ = await servir('auth-404');
        const r = await fetch(`${base_}/autre-chose`);
        expect(r.status).toBe(404);
        expect(await r.text()).toBe('introuvable\n');
    });

    it('CRITÈRE ④ : aucune trace ni aucune réponse ne porte le CHAMP du mot de passe', async () => {
        // 🔴 Le balayage cherche LE CHAMP, pas la valeur : un journal du CORPS
        // ENTIER de la requête ferait apparaître le mot de passe sans que la
        // chaîne exacte soit cherchée nulle part.
        const base_ = await servir('auth-trace');
        const capture: string[] = [];
        for (const canal of ['log', 'warn', 'error'] as const) {
            vi.spyOn(console, canal).mockImplementation((...a: unknown[]) => {
                capture.push(a.map(String).join(' '));
            });
        }

        const reussie = await poster(`${base_}/auth/connexion`, {
            email: 'ada@exemple.test',
            motdepasse: MOT_DE_PASSE,
        });
        const echouee = await poster(`${base_}/auth/connexion`, {
            email: 'ada@exemple.test',
            motdepasse: 'ce-n-est-pas-le-bon',
        });

        const balai = /motdepasse|mot_de_passe|empreinte_mdp/i;
        expect(capture.join('\n')).not.toMatch(balai);
        expect(await reussie.text()).not.toMatch(balai);
        expect(await echouee.text()).not.toMatch(balai);
        // Et le contrôle PEUT échouer : la capture attrape bien la console.
        console.log('témoin de capture');
        expect(capture.join('\n')).toContain('témoin de capture');
    });
});
