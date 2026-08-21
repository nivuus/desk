// L'ordre de démarrage, et le refus de démarrer dégradé.
//
// Spec §6, premier cas : le service REFUSE de démarrer, avec la cause. Il ne
// démarre pas dégradé — « un signaling qui apparie sans rien enregistrer
// serait indiscernable du bon fonctionnement ». C'est la classe de défaut
// contre laquelle tout ce dépôt est écrit.

import net from 'node:net';
import { afterEach, describe, expect, it } from 'vitest';
import type { Config } from './config';
import { demarrer, type Service } from './demarrage';
import { baseNeuve } from './base/harnais';
import { ouvrirSession } from './depot/session';
import { mkdtempSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

let service: Service | undefined;

afterEach(async () => {
    await service?.arreter();
    service = undefined;
});

/// Tente une connexion TCP nue, et dit si quelque chose écoute.
function connecterA(port: number): Promise<void> {
    return new Promise((resolve, reject) => {
        const s = net.connect({ host: '127.0.0.1', port });
        s.once('connect', () => {
            s.destroy();
            resolve();
        });
        s.once('error', (e) => {
            s.destroy();
            reject(e);
        });
    });
}

const PORT_MORT = 45_137;

// Un secret de test EXPLICITE, jamais `''` : `lireConfig` refuse la chaîne
// vide, et un littéral `Config` construit à la main doit porter une valeur
// qu'un service accepterait réellement.
const SECRET = 'un-secret-de-plateforme-de-quarante-octets';

describe('démarrage du service', () => {
    it("refuse de démarrer quand la base est injoignable, et n'ouvre aucun port", async () => {
        const config: Config = {
            hote: '127.0.0.1',
            port: PORT_MORT,
            base: 'postgres',
            // Un port sur lequel rien n'écoute : la connexion est refusée.
            urlBase: 'postgres://x:y@127.0.0.1:1/x',
            secretJeton: SECRET,
            // Aucun proxy declare : voir `config.ts`, l'ensemble vide est le
            // defaut et signifie « ne croire l'adresse annoncee par personne ».
            proxyDeConfiance: new Set(),
            repertoireIcones: join(mkdtempSync(join(tmpdir(), 'g2-icones-')), 'icones'),
            repertoireTeleversements: join(mkdtempSync(join(tmpdir(), 'g3-tranches-')), 'televersements'),
        };
        // Deux assertions DISTINCTES, et la seconde est le point de ce test.
        await expect(demarrer(config)).rejects.toThrow(/base/i);
        // 🔴 Sans celle-ci, un service qui ouvre son port PUIS meurt passerait
        // pour correct. C'est elle qui exerce l'ordre de démarrage.
        await expect(connecterA(PORT_MORT)).rejects.toThrow(/ECONNREFUSED/);
    });

    it('clôt au démarrage les sessions restées ouvertes, et dit combien', async () => {
        // Une base qui porte deux sessions ouvertes, comme après un arrêt
        // brutal du service.
        const base = await baseNeuve('demarrage-balai');
        await ouvrirSession(base, 'survivante-1', 1_000);
        await ouvrirSession(base, 'survivante-2', 2_000);

        const balayees = await import('./depot/session').then((m) =>
            m.balayerLesOuvertes(base, 9_000),
        );
        expect(balayees).toBe(2);
        await base.fermer();
    });

    it('ouvre le port et sert le relais quand la base est prête', async () => {
        service = await demarrer({
            hote: '127.0.0.1',
            port: 0,
            base: 'sqlite',
            urlBase: ':memory:',
            secretJeton: SECRET,
            // Aucun proxy déclaré : le service ne croira l'en-tête
            // `X-Forwarded-For` de personne, ce qui est le défaut de
            // `lireConfig` et l'état d'un déploiement sans proxy inverse.
            proxyDeConfiance: new Set(),
            repertoireIcones: join(mkdtempSync(join(tmpdir(), 'g2-icones-')), 'icones'),
            repertoireTeleversements: join(mkdtempSync(join(tmpdir(), 'g3-tranches-')), 'televersements'),
        });
        expect(service.port).toBeGreaterThan(0);
        await expect(connecterA(service.port)).resolves.toBeUndefined();
        // Les migrations sont appliquées : la table existe et se lit. Le
        // compte est écrit en dur pour la raison donnée dans
        // `base/pilotes.test.ts` — P2 l'a porté de 1 à 2 en ajoutant
        // `0002-identite.sql`, P3 de 2 à 3 en ajoutant `0003-agents.sql`, et
        // G1 de 3 à 4 en ajoutant `0004-applications.sql`.
        //
        // ⚠️ C'est la SECONDE place du dépôt qui fige ce compte, et la seule
        // que `pilotes.test.ts` ne nomme pas : mettre l'une à jour sans
        // l'autre laisse une rouge dont la cause est ailleurs que là où on la
        // cherche. Les deux se trouvent par
        // `grep -rn "schema_migration" src/ | grep -i test`.
        expect(await service.base.interroger('SELECT version FROM schema_migration', []))
            .toHaveLength(7);
    });
});
