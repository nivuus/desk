// Les deux routes d'authentification, éprouvées À TRAVERS le serveur HTTP réel.
//
// 🔴 LE CRITÈRE ④ EST DANS CE FICHIER, et son contrôle du contrôle a été joué :
// un `console.log(JSON.stringify(corps))` délibérément ajouté dans la route le
// fait rougir. Un test de balayage qui ne capturerait pas réellement la console
// serait vert quoi qu'il arrive — le patron du contrôle vacueux, payé quatre
// fois par ce dépôt.

import { afterEach, describe, expect, it, vi } from 'vitest';
import { baseNeuve, piloteCompteur } from '../base/harnais';
import type { Pilote } from '../base/pilote';
import type { Config } from '../config';
import { hacher } from '../identite/mot-de-passe';
import { verifierJeton } from '../identite/jeton';
import { creerUtilisateur } from '../depot/utilisateur';
import { demarrerServeur, type ServicePlateforme } from './serveur';
import { ECHECS_MAX_ADRESSE, ECHECS_MAX_COMPTE } from '../securite/frein';

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
        // Aucun proxy declare : voir `config.ts`, l'ensemble vide est le defaut
        // et signifie « ne croire l'adresse annoncee par personne ».
        proxyDeConfiance: new Set(),
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

describe('le frein des routes d’authentification', () => {
    /// Comme `servir`, mais rend AUSSI le compteur d'accès à la base.
    async function servirCompte(
        nom: string,
        origineClient?: string,
    ): Promise<{ url: string; acces: () => number; remettre: () => void }> {
        const reel = await baseNeuve(nom);
        base = reel;
        await creerUtilisateur(reel, 'ada@exemple.test', await hacher(MOT_DE_PASSE), MS);
        const compteur = piloteCompteur(reel);
        service = await demarrerServeur(config(origineClient), compteur.pilote);
        return { url: `http://127.0.0.1:${service.port}`, acces: compteur.acces, remettre: compteur.remettre };
    }

    function echouer(url: string, email: string) {
        return poster(`${url}/auth/connexion`, { email, motdepasse: 'ce-n-est-pas-le-bon' });
    }

    it('(a) la n+1ᵉ tentative sur le MÊME compte est refusée par le frein', async () => {
        const url = await servir('frein-compte');
        for (let i = 0; i < ECHECS_MAX_COMPTE; i++) {
            expect((await echouer(url, 'ada@exemple.test')).status).toBe(401);
        }
        // 🔴 La n+1ᵉ ne coûte plus rien au service : elle est refusée AVANT
        // toute vérification.
        const refus = await echouer(url, 'ada@exemple.test');
        expect(refus.status).toBe(429);
        expect((await corpsDe(refus)).refus).toBe('trop-de-tentatives');
    }, 30000);

    it('(a bis) le frein du compte mord même avec le BON mot de passe', async () => {
        // ⚠️ C'est l'arbitrage assumé de D1, et il se retourne contre
        // l'utilisateur légitime : un attaquant peut brûler le budget d'un
        // compte qu'il vise et en refuser l'accès à son propriétaire pendant
        // la fenêtre. Il est éprouvé ici plutôt que laissé implicite.
        const url = await servir('frein-compte-legitime');
        for (let i = 0; i < ECHECS_MAX_COMPTE; i++) await echouer(url, 'ada@exemple.test');
        const legitime = await poster(`${url}/auth/connexion`, {
            email: 'ada@exemple.test',
            motdepasse: MOT_DE_PASSE,
        });
        expect(legitime.status).toBe(429);
    }, 30000);

    it('(a ter) la casse du courriel ne donne PAS un budget neuf', async () => {
        // Sans normalisation en minuscules, `ADA@exemple.test` serait une
        // seconde clé, et le budget du compte se multiplierait par le nombre
        // de casses que l'attaquant sait écrire.
        const url = await servir('frein-compte-casse');
        for (let i = 0; i < ECHECS_MAX_COMPTE; i++) await echouer(url, 'ada@exemple.test');
        expect((await echouer(url, 'ADA@Exemple.TEST')).status).toBe(429);
    }, 30000);

    it('(b) la n+1ᵉ depuis la MÊME adresse, comptes tous DISTINCTS, est refusée', async () => {
        // Aucun budget de compte ne peut mordre ici : chaque courriel est
        // essayé UNE seule fois. Seule la clé d'adresse peut refuser — c'est
        // le seul frein qui ferme le BALAYAGE de comptes.
        const url = await servir('frein-adresse');
        for (let i = 0; i < ECHECS_MAX_ADRESSE; i++) {
            expect((await echouer(url, `n${i}@exemple.test`)).status).toBe(401);
        }
        expect((await echouer(url, 'encore-un-autre@exemple.test')).status).toBe(429);
    }, 60000);

    it('(c) 🔴 le refus freiné NE TOUCHE PAS la base — donc aucun scrypt', async () => {
        // 🔴 C'EST L'ASSERTION QUI DONNE SON SENS AU FREIN. Un frein posté
        // APRÈS le hachage compterait des échecs qu'il a déjà payés au prix
        // fort : `scrypt` est à mémoire dure et coûte délibérément cher
        // (mesuré ce jour : 68 ms par hachage), et un attaquant qui le
        // déclenche à volonté épuise le service sans jamais deviner un secret.
        const { url, acces, remettre } = await servirCompte('frein-sans-base');
        for (let i = 0; i < ECHECS_MAX_COMPTE; i++) await echouer(url, 'ada@exemple.test');
        // Le compteur est remis à zéro APRÈS les tentatives payées, pour que
        // la mesure ne porte QUE sur la requête freinée.
        remettre();
        const refus = await echouer(url, 'ada@exemple.test');
        expect(refus.status).toBe(429);
        expect(acces()).toBe(0);
    }, 30000);

    it('(d) un SUCCÈS remet le compteur du COMPTE à zéro', async () => {
        const url = await servir('frein-succes-compte');
        for (let i = 0; i < ECHECS_MAX_COMPTE - 1; i++) await echouer(url, 'ada@exemple.test');
        expect((await poster(`${url}/auth/connexion`, {
            email: 'ada@exemple.test',
            motdepasse: MOT_DE_PASSE,
        })).status).toBe(200);
        // Sans la remise à zéro, la `max`-ième de cette seconde série
        // franchirait le budget et rendrait 429.
        for (let i = 0; i < ECHECS_MAX_COMPTE - 1; i++) {
            expect((await echouer(url, 'ada@exemple.test')).status).toBe(401);
        }
    }, 30000);

    it("(e) 🔴 un succès ne remet PAS le compteur de l'ADRESSE à zéro", async () => {
        // 🔴 Le passer sur la clé d'adresse BLANCHIRAIT un attaquant qui
        // possède un compte valide : il lui suffirait de s'y connecter entre
        // deux rafales pour rendre son budget d'adresse à zéro.
        const url = await servir('frein-succes-adresse');
        for (let i = 0; i < ECHECS_MAX_ADRESSE - 1; i++) await echouer(url, `m${i}@exemple.test`);
        expect((await poster(`${url}/auth/connexion`, {
            email: 'ada@exemple.test',
            motdepasse: MOT_DE_PASSE,
        })).status).toBe(200);
        // L'adresse est à `max - 1` ; cette tentative la porte à `max`…
        expect((await echouer(url, 'avant-dernier@exemple.test')).status).toBe(401);
        // …et la suivante est refusée. Si le succès avait effacé l'adresse,
        // elle serait à 1 et celle-ci passerait.
        expect((await echouer(url, 'dernier@exemple.test')).status).toBe(429);
    }, 60000);

    it('(f) le 429 porte `Retry-After` ET les en-têtes CORS', async () => {
        // 🔴 Sans les en-têtes CORS, le NAVIGATEUR ne peut pas lire le refus,
        // et l'utilisateur voit un échec opaque au lieu de « réessayez dans
        // n minutes ». Aucun test Node ne le verrait — `fetch` Node n'applique
        // pas la politique d'origine — d'où l'assertion sur l'en-tête lui-même.
        const url = await servir('frein-entetes', ORIGINE);
        for (let i = 0; i < ECHECS_MAX_COMPTE; i++) {
            await poster(`${url}/auth/connexion`, { email: 'ada@exemple.test', motdepasse: 'faux' },
                { origin: ORIGINE });
        }
        const refus = await poster(`${url}/auth/connexion`,
            { email: 'ada@exemple.test', motdepasse: 'faux' }, { origin: ORIGINE });
        expect(refus.status).toBe(429);
        const retry = refus.headers.get('retry-after');
        expect(retry).not.toBeNull();
        // En SECONDES, un entier positif — jamais 0, qui inviterait le
        // demandeur à revenir immédiatement.
        expect(Number(retry)).toBeGreaterThan(0);
        expect(Number.isInteger(Number(retry))).toBe(true);
        expect(refus.headers.get('access-control-allow-origin')).toBe(ORIGINE);
    }, 30000);

    it("(g) la trace NOMME l'adresse retenue", async () => {
        // ⚠️ C'est le SEUL remède au mode de défaillance nommé dans
        // `http/adresse-source.ts` : un exploitant qui pose un proxy sans
        // déclarer sa confiance verra son frein par adresse dégénérer en frein
        // GLOBAL, et la seule chose qui le lui dira est cette ligne, où il
        // reconnaîtra l'adresse de son proxy.
        const avertir = vi.spyOn(console, 'warn').mockImplementation(() => {});
        const url = await servir('frein-trace');
        for (let i = 0; i < ECHECS_MAX_COMPTE; i++) await echouer(url, 'ada@exemple.test');
        await echouer(url, 'ada@exemple.test');
        const lignes = avertir.mock.calls.map((c) => String(c[0]));
        const freinees = lignes.filter((l) => l.startsWith('frein '));
        expect(freinees.length).toBeGreaterThanOrEqual(1);
        expect(freinees[0]).toContain('adresse=');
        expect(freinees[0]).toContain('route=/auth/connexion');
    }, 30000);

    it("(h) `/auth/rafraichir` est freiné par l'ADRESSE seule", async () => {
        // ⚠️ Le demandeur n'y présente AUCUN courriel, seulement un jeton
        // opaque : prendre ce jeton pour clé reviendrait à indexer une table
        // sur un secret.
        const url = await servir('frein-rafraichir');
        for (let i = 0; i < ECHECS_MAX_ADRESSE; i++) {
            expect((await poster(`${url}/auth/rafraichir`, { rafraichissement: `faux-${i}` })).status)
                .toBe(401);
        }
        const refus = await poster(`${url}/auth/rafraichir`, { rafraichissement: 'encore-faux' });
        expect(refus.status).toBe(429);
    }, 60000);

    it("(h bis) 🔴 un échec sur `/auth/rafraichir` ne consomme AUCUN budget de compte", async () => {
        // Sinon, un attaquant qui ne connaît aucun courriel pourrait tout de
        // même verrouiller des comptes — ou, plus subtil, la clé de compte
        // serait le jeton lui-même.
        const url = await servir('frein-rafraichir-compte');
        for (let i = 0; i < ECHECS_MAX_COMPTE + 2; i++) {
            await poster(`${url}/auth/rafraichir`, { rafraichissement: `faux-${i}` });
        }
        // Le compte d'Ada n'a jamais été nommé : sa connexion doit passer.
        expect((await poster(`${url}/auth/connexion`, {
            email: 'ada@exemple.test',
            motdepasse: MOT_DE_PASSE,
        })).status).toBe(200);
    }, 30000);
});
