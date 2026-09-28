// The startup operating traces — ONE TEST PER TRACE.
//
// 🔴 WHAT THESE TESTS EXIST TO PREVENT: a badly set page root
// that returns `404` on everything, without a log line — hence strictly
// indistinguishable from the absent variable —, and a trust set that
// refuses everyone silently. Both failures are SILENT, and a silent failure
// cannot be caught by reading the code.

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
    // 🔴 "NO PAGE SERVED" IS INFORMATION, NOT SILENCE: it is
    // what distinguishes the nominal nginx setup from a wrong root.
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

    // 🔴 LOUD, NEVER `info`: it is the exact case the final review
    // classified Critical — a root set but nonexistent, which answered `404`
    // to every request without saying anything anywhere.
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

    // 🔴 THE RESOLVED PATH, NEVER THE RAW VALUE. A relative path in the
    // log would be ambiguous: its anchoring depends on the current directory of the
    // process, which the operator reads nowhere.
    it('resolves the root to an ABSOLUTE path', async () => {
        const etat = await etatRacinePage('client/dist', async () => {});
        expect(etat).toEqual({ arme: true, chemin: join(process.cwd(), 'client/dist'), lisible: true });
    });

    // 🔴 THE NEGATIVE WITNESS OF THE PROBE: without it, the `lisible: true` above
    // would be returned by a state that probes NOTHING.
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

    // ⚠️ AN ORDINARY FILE SET AS ROOT GIVES THE SAME SILENT `404` as an
    // absent root: it is a plausible configuration mistake (pointing to
    // `index.html` instead of `client/dist`), and it must be named.
    it("refuses a path that is not a directory", async () => {
        const file = join(mkdtempSync(join(tmpdir(), 'announce-file-')), 'page.html');
        writeFileSync(file, 'x');
        await expect(sonderRepertoire(file)).rejects.toThrow();
    });
});

describe("the announced trust set", () => {
    // 🔴 WHAT THE SERVICE RETAINED, NEVER WHAT IT WAS GIVEN — and it is what
    // makes a host name VISIBLE. A host name matches no
    // `remoteAddress`, so `pairDeConfiance` refuses everyone, and the
    // service still answers: the only thing that says so is this line.
    it('names each retained entry', () => {
        const annonce = annonceProxyDeConfiance(new Set(['172.18.0.5', 'pomerium.interne']));
        expect(annonce.texte).toContain('pomerium.interne');
    });

    it('announces an EMPTY set rather than staying silent', () => {
        expect(annonceProxyDeConfiance(new Set()).texte).toContain('retained=none');
    });

    // ⚠️ IT IS NOT BECAUSE THE EMPTY SET WOULD BE THE "SAFE DEFAULT" —
    // that justification was FALSIFIED (review, correction round 3 of
    // `frein(pont)`): in THE SETUP this repository ships
    // (`docker-compose.plateforme.yml`, nginx in front of the platform even in
    // `motdepasse` mode), an empty set makes the brake degenerate into a
    // budget SHARED by all traffic, without any attacker having to
    // forge anything — see the doc of `annonceProxyDeConfiance`,
    // corrected in its place. `info` remains the expected level because this
    // PURE function has no way of knowing whether the caller runs behind
    // a proxy — an unconditional `error` would wrongly alarm the setup where
    // the empty set is legitimately safe (direct exposure). `config.ts`
    // already refuses to start without it in `pomerium` mode.
    it("an empty set is NOT an error", () => {
        expect(annonceProxyDeConfiance(new Set()).niveau).toBe('info');
    });
});

// 🔴 THE FOUR TESTS ABOVE ARE PURE: THEY DO NOT PROVE THAT
// `startServer` CALLS THEM. It is the trap `CLAUDE.md` names for
// `scripts/run-agent.sh` — "the check that matters is reading the line in the
// GENERATED script, never tracing the code" —, and it replays here: a fully
// tested announcements module NEVER WIRED would give exactly the
// silence it exists to remove. These tests mount the REAL service and
// read the console.
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

    // 🔴 THE CRITICAL C1, MEASURED ON THE REAL SERVICE: a root set but
    // nonexistent returned `404` on every page, without a line anywhere.
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
