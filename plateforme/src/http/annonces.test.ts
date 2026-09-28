// Les traces d'exploitation du démarrage — UN TEST PAR TRACE.
//
// 🔴 CE QUE CES TESTS EXISTENT POUR EMPÊCHER : une racine de page mal posée
// qui rend `404` sur tout, sans une ligne de journal — donc strictement
// indiscernable de la variable absente —, et un ensemble de confiance qui
// refuse tout le monde en silence. Les deux pannes sont MUETTES, et une panne
// muette ne se rattrape pas à la lecture du code.

import { mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { baseNeuve } from '../base/harnais';
import type { Pilote } from '../base/pilote';
import type { Config } from '../config';
import { startServer, type ServicePlateforme } from './serveur';
import {
    annonceProxyDeConfiance,
    annonceRacinePage,
    etatRacinePage,
    sonderRepertoire,
} from './annonces';

const CONFIG: Config = {
    hote: '127.0.0.1',
    port: 0,
    base: 'sqlite',
    urlBase: ':memory:',
    secretJeton: 'un-secret-de-plateforme-de-quarante-octets',
    proxyDeConfiance: new Set(),
    repertoireIcones: join(mkdtempSync(join(tmpdir(), 'annonce-icones-')), 'icones'),
    repertoireTeleversements: join(
        mkdtempSync(join(tmpdir(), 'annonce-tranches-')),
        'televersements',
    ),
    auth: 'pomerium',
};

describe('the announced page root', () => {
    // 🔴 « AUCUNE PAGE SERVIE » EST UNE INFORMATION, PAS UN SILENCE : c'est
    // elle qui distingue le montage nginx nominal d'une racine fausse.
    it("announces the ABSENCE of a served page, rather than staying silent", () => {
        expect(annonceRacinePage({ arme: false }).texte).toContain('root=none');
    });

    it("the absence of a served page is NOT an error", () => {
        expect(annonceRacinePage({ arme: false }).niveau).toBe('info');
    });

    it('announces the retained path when the root is readable', () => {
        const annonce = annonceRacinePage({ arme: true, chemin: '/srv/page', lisible: true });
        expect(annonce.texte).toContain('root=/srv/page');
    });

    // 🔴 BRUYANTE, JAMAIS `info` : c'est le cas exact que la revue finale a
    // classé Critique — une racine posée mais inexistante, qui répondait `404`
    // à chaque requête sans rien dire nulle part.
    it('an UNREADABLE root is announced at ERROR level', () => {
        const annonce = annonceRacinePage({
            arme: true,
            chemin: '/srv/absente',
            lisible: false,
            cause: 'ENOENT',
        });
        expect(annonce.niveau).toBe('error');
    });

    it("an UNREADABLE root states the effect, not only the cause", () => {
        const annonce = annonceRacinePage({
            arme: true,
            chemin: '/srv/absente',
            lisible: false,
            cause: 'ENOENT',
        });
        expect(annonce.texte).toContain('404');
    });
});

describe("the state of the page root", () => {
    it("an empty value means absence, never the current directory", async () => {
        expect(await etatRacinePage('')).toEqual({ arme: false });
    });

    // 🔴 LE CHEMIN RÉSOLU, JAMAIS LA VALEUR BRUTE. Un chemin relatif au
    // journal serait ambigu : son ancrage dépend du répertoire courant du
    // processus, que l'exploitant ne lit nulle part.
    it('resolves the root to an ABSOLUTE path', async () => {
        const etat = await etatRacinePage('client/dist', async () => {});
        expect(etat).toEqual({ arme: true, chemin: join(process.cwd(), 'client/dist'), lisible: true });
    });

    // 🔴 LE TÉMOIN NÉGATIF DU SONDAGE : sans lui, le `lisible: true` ci-dessus
    // serait rendu par un état qui ne sonde RIEN.
    it("a root the probe refuses is returned UNREADABLE, with its cause", async () => {
        const etat = await etatRacinePage('/srv/page', async () => {
            throw new Error('ENOENT: nothing here');
        });
        expect(etat).toMatchObject({ arme: true, lisible: false });
    });
});

describe('the real disk probe', () => {
    it('accepts a readable directory', async () => {
        await expect(sonderRepertoire(mkdtempSync(join(tmpdir(), 'annonce-ok-')))).resolves
            .toBeUndefined();
    });

    it('refuses a non-existent path', async () => {
        const absent = join(mkdtempSync(join(tmpdir(), 'annonce-absent-')), 'jamais-cree');
        await expect(sonderRepertoire(absent)).rejects.toThrow();
    });

    // ⚠️ UN FICHIER ORDINAIRE POSÉ COMME RACINE REND LE MÊME `404` MUET qu'une
    // racine absente : c'est une faute de configuration plausible (pointer
    // `index.html` au lieu de `client/dist`), et elle doit être nommée.
    it("refuses a path that is not a directory", async () => {
        const file = join(mkdtempSync(join(tmpdir(), 'announce-file-')), 'page.html');
        writeFileSync(file, 'x');
        await expect(sonderRepertoire(file)).rejects.toThrow();
    });
});

describe("the announced trust set", () => {
    // 🔴 CE QUE LE SERVICE A RETENU, JAMAIS CE QU'ON LUI A DONNÉ — et c'est ce
    // qui rend un nom d'hôte VISIBLE. Un nom d'hôte ne correspond à aucune
    // `remoteAddress`, donc `pairDeConfiance` refuse tout le monde, et le
    // service répond quand même : la seule chose qui le dise est cette ligne.
    it('names each retained entry', () => {
        const annonce = annonceProxyDeConfiance(new Set(['172.18.0.5', 'pomerium.interne']));
        expect(annonce.texte).toContain('pomerium.interne');
    });

    it('announces an EMPTY set rather than staying silent', () => {
        expect(annonceProxyDeConfiance(new Set()).texte).toContain('retained=none');
    });

    // ⚠️ CE N'EST PAS PARCE QUE L'ENSEMBLE VIDE SERAIT LE « DÉFAUT SÛR » —
    // cette justification a été FALSIFIÉE (revue, round de correction 3 de
    // `frein(pont)`) : dans LE MONTAGE que ce dépôt livre
    // (`docker-compose.plateforme.yml`, nginx devant la plateforme même en
    // mode `motdepasse`), un ensemble vide fait dégénérer le frein en un
    // budget PARTAGÉ par tout le trafic, sans qu'aucun attaquant n'ait à
    // forger quoi que ce soit — voir la doc de `annonceProxyDeConfiance`,
    // corrigée à sa place. `info` reste le niveau attendu parce que cette
    // fonction PURE n'a aucun moyen de savoir si l'appelant tourne derrière
    // un proxy — un `error` inconditionnel alarmerait à tort le montage où
    // l'ensemble vide est légitimement sûr (exposition directe). `config.ts`
    // refuse déjà de démarrer sans lui en mode `pomerium`.
    it("an empty set is NOT an error", () => {
        expect(annonceProxyDeConfiance(new Set()).niveau).toBe('info');
    });
});

// 🔴 LES QUATRE TESTS CI-DESSUS SONT PURS : ILS NE PROUVENT PAS QUE
// `startServer` LES APPELLE. C'est le piège que `CLAUDE.md` nomme pour
// `scripts/run-agent.sh` — « le contrôle qui vaut est de lire la ligne dans le
// script GÉNÉRÉ, jamais de tracer le code » —, et il se rejoue ici : un module
// d'annonces entièrement testé et JAMAIS BRANCHÉ rendrait exactement le
// silence qu'il existe pour supprimer. Ces tests-ci montent le VRAI service et
// lisent la console.
describe('the service announces at startup', () => {
    let base: Pilote | undefined;
    let service: ServicePlateforme | undefined;

    afterEach(async () => {
        await service?.close();
        service = undefined;
        await base?.fermer();
        base = undefined;
    });

    async function startAndCapture(
        surcharge: Partial<Config>,
        nom: string,
    ): Promise<{ infos: string[]; errors: string[] }> {
        const infos: string[] = [];
        const errors: string[] = [];
        const espionInfo = vi.spyOn(console, 'info').mockImplementation((m) => infos.push(String(m)));
        const errorSpy = vi
            .spyOn(console, 'error')
            .mockImplementation((m) => errors.push(String(m)));
        try {
            base = await baseNeuve(nom);
            service = await startServer({ ...CONFIG, ...surcharge }, base);
        } finally {
            espionInfo.mockRestore();
            errorSpy.mockRestore();
        }
        return { infos, errors };
    }

    it('announces the RETAINED page root, resolved', async () => {
        const racine = mkdtempSync(join(tmpdir(), 'annonce-service-'));
        const { infos } = await startAndCapture({ racinePage: racine }, 'annonce-page-armee');
        expect(infos.some((l) => l.startsWith('page served') && l.includes(racine))).toBe(true);
    });

    it("announces the absence of a served page when the variable is not set", async () => {
        const { infos } = await startAndCapture({ racinePage: undefined }, 'annonce-page-absente');
        expect(infos.some((l) => l.startsWith('page served') && l.includes('root=none'))).toBe(
            true,
        );
    });

    // 🔴 LE CRITIQUE C1, MESURÉ SUR LE VRAI SERVICE : une racine posée mais
    // inexistante rendait `404` sur toute page, sans une ligne nulle part.
    it('a root set but NON-EXISTENT is announced on console.error', async () => {
        const absente = join(mkdtempSync(join(tmpdir(), 'annonce-absente-')), 'jamais-batie');
        const { errors } = await startAndCapture({ racinePage: absente }, 'annonce-page-morte');
        expect(errors.some((l) => l.startsWith('page served') && l.includes('readable=no'))).toBe(
            true,
        );
    });

    it("announces the retained trust set", async () => {
        const { infos } = await startAndCapture(
            { proxyDeConfiance: new Set(['172.18.0.5']) },
            'annonce-proxy',
        );
        expect(
            infos.some((l) => l.startsWith('trusted proxies') && l.includes('172.18.0.5')),
        ).toBe(true);
    });
});
