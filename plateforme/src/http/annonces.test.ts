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
import { demarrerServeur, type ServicePlateforme } from './serveur';
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

describe('la racine de page annoncée', () => {
    // 🔴 « AUCUNE PAGE SERVIE » EST UNE INFORMATION, PAS UN SILENCE : c'est
    // elle qui distingue le montage nginx nominal d'une racine fausse.
    it("annonce l'ABSENCE de page servie, plutôt que de se taire", () => {
        expect(annonceRacinePage({ arme: false }).texte).toContain('racine=aucune');
    });

    it("l'absence de page servie n'est PAS une erreur", () => {
        expect(annonceRacinePage({ arme: false }).niveau).toBe('info');
    });

    it('annonce le chemin retenu quand la racine est lisible', () => {
        const annonce = annonceRacinePage({ arme: true, chemin: '/srv/page', lisible: true });
        expect(annonce.texte).toContain('racine=/srv/page');
    });

    // 🔴 BRUYANTE, JAMAIS `info` : c'est le cas exact que la revue finale a
    // classé Critique — une racine posée mais inexistante, qui répondait `404`
    // à chaque requête sans rien dire nulle part.
    it('une racine ILLISIBLE est annoncée au niveau ERREUR', () => {
        const annonce = annonceRacinePage({
            arme: true,
            chemin: '/srv/absente',
            lisible: false,
            cause: 'ENOENT',
        });
        expect(annonce.niveau).toBe('erreur');
    });

    it("une racine ILLISIBLE dit l'effet, pas seulement la cause", () => {
        const annonce = annonceRacinePage({
            arme: true,
            chemin: '/srv/absente',
            lisible: false,
            cause: 'ENOENT',
        });
        expect(annonce.texte).toContain('404');
    });
});

describe("l'état de la racine de page", () => {
    it("une valeur vide vaut l'absence, jamais le répertoire courant", async () => {
        expect(await etatRacinePage('')).toEqual({ arme: false });
    });

    // 🔴 LE CHEMIN RÉSOLU, JAMAIS LA VALEUR BRUTE. Un chemin relatif au
    // journal serait ambigu : son ancrage dépend du répertoire courant du
    // processus, que l'exploitant ne lit nulle part.
    it('résout la racine en chemin ABSOLU', async () => {
        const etat = await etatRacinePage('client/dist', async () => {});
        expect(etat).toEqual({ arme: true, chemin: join(process.cwd(), 'client/dist'), lisible: true });
    });

    // 🔴 LE TÉMOIN NÉGATIF DU SONDAGE : sans lui, le `lisible: true` ci-dessus
    // serait rendu par un état qui ne sonde RIEN.
    it("une racine que le sondage refuse est rendue ILLISIBLE, avec sa cause", async () => {
        const etat = await etatRacinePage('/srv/page', async () => {
            throw new Error('ENOENT : rien ici');
        });
        expect(etat).toMatchObject({ arme: true, lisible: false });
    });
});

describe('le sondage réel du disque', () => {
    it('accepte un répertoire lisible', async () => {
        await expect(sonderRepertoire(mkdtempSync(join(tmpdir(), 'annonce-ok-')))).resolves
            .toBeUndefined();
    });

    it('refuse un chemin inexistant', async () => {
        const absent = join(mkdtempSync(join(tmpdir(), 'annonce-absent-')), 'jamais-cree');
        await expect(sonderRepertoire(absent)).rejects.toThrow();
    });

    // ⚠️ UN FICHIER ORDINAIRE POSÉ COMME RACINE REND LE MÊME `404` MUET qu'une
    // racine absente : c'est une faute de configuration plausible (pointer
    // `index.html` au lieu de `client/dist`), et elle doit être nommée.
    it("refuse un chemin qui n'est pas un répertoire", async () => {
        const fichier = join(mkdtempSync(join(tmpdir(), 'annonce-fichier-')), 'page.html');
        writeFileSync(fichier, 'x');
        await expect(sonderRepertoire(fichier)).rejects.toThrow();
    });
});

describe("l'ensemble de confiance annoncé", () => {
    // 🔴 CE QUE LE SERVICE A RETENU, JAMAIS CE QU'ON LUI A DONNÉ — et c'est ce
    // qui rend un nom d'hôte VISIBLE. Un nom d'hôte ne correspond à aucune
    // `remoteAddress`, donc `pairDeConfiance` refuse tout le monde, et le
    // service répond quand même : la seule chose qui le dise est cette ligne.
    it('nomme chaque entrée retenue', () => {
        const annonce = annonceProxyDeConfiance(new Set(['172.18.0.5', 'pomerium.interne']));
        expect(annonce.texte).toContain('pomerium.interne');
    });

    it('annonce un ensemble VIDE plutôt que de se taire', () => {
        expect(annonceProxyDeConfiance(new Set()).texte).toContain('retenus=aucun');
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
    // un proxy — un `erreur` inconditionnel alarmerait à tort le montage où
    // l'ensemble vide est légitimement sûr (exposition directe). `config.ts`
    // refuse déjà de démarrer sans lui en mode `pomerium`.
    it("un ensemble vide n'est PAS une erreur", () => {
        expect(annonceProxyDeConfiance(new Set()).niveau).toBe('info');
    });
});

// 🔴 LES QUATRE TESTS CI-DESSUS SONT PURS : ILS NE PROUVENT PAS QUE
// `demarrerServeur` LES APPELLE. C'est le piège que `CLAUDE.md` nomme pour
// `scripts/run-agent.sh` — « le contrôle qui vaut est de lire la ligne dans le
// script GÉNÉRÉ, jamais de tracer le code » —, et il se rejoue ici : un module
// d'annonces entièrement testé et JAMAIS BRANCHÉ rendrait exactement le
// silence qu'il existe pour supprimer. Ces tests-ci montent le VRAI service et
// lisent la console.
describe('le service annonce au démarrage', () => {
    let base: Pilote | undefined;
    let service: ServicePlateforme | undefined;

    afterEach(async () => {
        await service?.close();
        service = undefined;
        await base?.fermer();
        base = undefined;
    });

    async function demarrerEtCapturer(
        surcharge: Partial<Config>,
        nom: string,
    ): Promise<{ infos: string[]; erreurs: string[] }> {
        const infos: string[] = [];
        const erreurs: string[] = [];
        const espionInfo = vi.spyOn(console, 'info').mockImplementation((m) => infos.push(String(m)));
        const espionErreur = vi
            .spyOn(console, 'error')
            .mockImplementation((m) => erreurs.push(String(m)));
        try {
            base = await baseNeuve(nom);
            service = await demarrerServeur({ ...CONFIG, ...surcharge }, base);
        } finally {
            espionInfo.mockRestore();
            espionErreur.mockRestore();
        }
        return { infos, erreurs };
    }

    it('annonce la racine de page RETENUE, résolue', async () => {
        const racine = mkdtempSync(join(tmpdir(), 'annonce-service-'));
        const { infos } = await demarrerEtCapturer({ racinePage: racine }, 'annonce-page-armee');
        expect(infos.some((l) => l.startsWith('page servie') && l.includes(racine))).toBe(true);
    });

    it("annonce l'absence de page servie quand la variable n'est pas posée", async () => {
        const { infos } = await demarrerEtCapturer({ racinePage: undefined }, 'annonce-page-absente');
        expect(infos.some((l) => l.startsWith('page servie') && l.includes('racine=aucune'))).toBe(
            true,
        );
    });

    // 🔴 LE CRITIQUE C1, MESURÉ SUR LE VRAI SERVICE : une racine posée mais
    // inexistante rendait `404` sur toute page, sans une ligne nulle part.
    it('une racine posée mais INEXISTANTE est annoncée sur console.error', async () => {
        const absente = join(mkdtempSync(join(tmpdir(), 'annonce-absente-')), 'jamais-batie');
        const { erreurs } = await demarrerEtCapturer({ racinePage: absente }, 'annonce-page-morte');
        expect(erreurs.some((l) => l.startsWith('page servie') && l.includes('lisible=non'))).toBe(
            true,
        );
    });

    it("annonce l'ensemble de confiance retenu", async () => {
        const { infos } = await demarrerEtCapturer(
            { proxyDeConfiance: new Set(['172.18.0.5']) },
            'annonce-proxy',
        );
        expect(
            infos.some((l) => l.startsWith('proxys de confiance') && l.includes('172.18.0.5')),
        ).toBe(true);
    });
});
