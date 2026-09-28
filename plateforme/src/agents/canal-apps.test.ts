// Le canal `/agent`, côté APPLICATIONS : le catalogue qu'un agent pousse, et
// l'issue de lancement qu'il rapporte.
//
// 🔴 CE FICHIER EST NÉ D'UNE EXTRACTION, PAS D'UNE DUPLICATION : le harnais
// qu'il partage avec `canal.test.ts` vit dans `canal-harnais.ts`, extrait avant
// que ces cas ne soient écrits. Les recopier aurait produit deux `ouvrirUrl`
// qui divergeraient à la première correction portée sur un seul des deux.
//
// 🔴 LES DEUX REFUS `sequence` SONT LA MOITIÉ QUI COMPTE. Un pair qui n'a
// présenté aucun secret pourrait sinon écrire dans la table `application`
// d'une VM qu'il n'a pas authentifiée — c'est le trou exact que le refus du
// battement ferme déjà, par une autre porte.

import { createHash } from 'node:crypto';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { WebSocketServer } from 'ws';
import {
    PLATEFORME_VERSION,
    encodeCatalogue,
    encodeEnroler,
    encodeLancee,
    type Application,
} from '../../../proto/ts/plateforme';
import { baseNeuve } from '../base/harnais';
import type { Pilote } from '../base/pilote';
import { lireParVm } from '../depot/application';
import { servirLeCanalAgent } from './canal';
import { enrolerUneVm, ouvrir, SECRET, SECRET_VM, T0, type Pair } from './canal-harnais';
import { RegistreAgents } from './registre';
import { Frein } from '../securite/frein';
import { ouvrirMagasin, type Magasin } from '../apps/icones';

let base: Pilote | undefined;
let wss: WebSocketServer | undefined;
let registre = new RegistreAgents();
let maintenant = T0;

afterEach(async () => {
    if (wss) {
        for (const socket of wss.clients) socket.terminate();
        await new Promise<void>((r) => wss!.close(() => r()));
        wss = undefined;
    }
    await base?.fermer();
    base = undefined;
    vi.restoreAllMocks();
});

/// Le magasin du montage courant — `undefined` tant qu'aucun test n'en demande.
let magasin: Magasin | undefined;
let racinesIcones: string[] = [];

async function start(p: Pilote, withStore = false): Promise<number> {
    maintenant = T0;
    registre = new RegistreAgents();
    magasin = undefined;
    if (withStore) {
        const r = mkdtempSync(join(tmpdir(), 'g2-canal-icones-'));
        racinesIcones.push(r);
        magasin = ouvrirMagasin(join(r, 'icones'), () => {});
    }
    wss = new WebSocketServer({ port: 0, host: '127.0.0.1' });
    await new Promise<void>((r) => wss!.once('listening', () => r()));
    servirLeCanalAgent(wss, {
        base: p,
        secretJeton: SECRET,
        maintenant: () => maintenant,
        registre,
        // Un frein NEUF par montage : ce fichier eprouve le catalogue et le
        // lancement, pas le freinage, et un frein partage entre tests ferait
        // deborder les budgets d'ADRESSE (127.0.0.1 est la meme pour tous).
        frein: new Frein(),
        // Aucun proxy declare : la cle d'adresse est celle du pair reel.
        proxyDeConfiance: new Set(),
        magasin,
    });
    const adresse = wss.address();
    return typeof adresse === 'object' && adresse ? adresse.port : 0;
}

function app(nom: string, cle: string, icone: string | null = null): Application {
    return {
        cle,
        nom,
        chemin: `C:\\Users\\guacamole\\Desktop\\${nom}.lnk`,
        cible: `c:\\program files\\${nom}\\${nom}.exe`,
        arguments: '',
        repertoire: `c:\\program files\\${nom}`,
        icone,
        source_max: icone === null ? 'non-mesuree' : { pixels: 256 },
        accent: null,
        associations: [],
    };
}

/// Attend que le catalogue de la VM satisfasse `predicat`, ou ÉCHOUE.
///
/// ⚠️ BORNÉE, ET ÉCHOUANT SUR EXPIRATION — même figure qu'`attendreVu`, et pour
/// la même raison : l'écriture du catalogue est délibérément lancée SANS être
/// attendue, donc une écriture perdue ne se manifeste que par un état qui
/// n'arrive pas. Une boucle sans borne pendrait au lieu de rougir.
async function attendreCatalogue(
    p: Pilote,
    vmId: string,
    predicat: (noms: string[]) => boolean,
    quoi: string,
    borneMs = 2000,
): Promise<string[]> {
    const fin = Date.now() + borneMs;
    for (;;) {
        const noms = (await lireParVm(p, vmId)).map((l) => l.nom);
        if (predicat(noms)) return noms;
        if (Date.now() > fin) {
            throw new Error(`catalogue ${quoi} never reached for ${vmId} (seen=${noms.join(',')})`);
        }
        await new Promise((r) => setTimeout(r, 25));
    }
}

/// Enrôle le pair et attend la réponse — préalable de tous les cas nominaux.
async function enrole(pair: Pair): Promise<void> {
    const rep = await pair.dire(encodeEnroler('v-1', SECRET_VM));
    expect(rep.type).toBe('enrole');
}

describe('the /agent channel, applications side', () => {
    it('🔴 a `catalogue` BEFORE any enrolment is refused, reason `sequence`', async () => {
        // 🔴 L'ACCEPTER LAISSERAIT UN PAIR ANONYME ÉCRIRE DANS LA TABLE
        // `application` D'UNE VM QU'IL N'A PAS AUTHENTIFIÉE. C'est le trou
        // exact que le refus `sequence` du battement ferme déjà, par une autre
        // porte — et celle-ci écrit en base, là où celle-là ne délivrait qu'un
        // jeton.
        base = await baseNeuve('canal-apps-sequence-catalogue');
        await enrolerUneVm(base, 'v-1');
        const pair = await ouvrir(await start(base));

        const rep = await pair.dire(encodeCatalogue(true, [app('Intrus', 'cle-intrus')], []));
        expect(rep).toEqual({ type: 'refus', v: PLATEFORME_VERSION, motif: 'sequence' });
        // Et RIEN n'a été écrit : c'est la moitié qui décide. Un refus qui
        // aurait quand même laissé passer l'écriture serait une porte ouverte
        // avec un panneau « fermé ».
        expect(await lireParVm(base, 'v-1')).toEqual([]);
        pair.socket.terminate();
    });

    it('🔴 a `lancee` before any enrolment is refused, reason `sequence`', async () => {
        // Même raison : un anonyme pourrait sinon résoudre la demande d'un
        // autre, et faire croire à un lancement réussi qui n'a pas eu lieu.
        base = await baseNeuve('canal-apps-sequence-lancee');
        await enrolerUneVm(base, 'v-1');
        const pair = await ouvrir(await start(base));

        const rep = await pair.dire(encodeLancee('d-1', 'raccourci'));
        expect(rep).toEqual({ type: 'refus', v: PLATEFORME_VERSION, motif: 'sequence' });
        pair.socket.terminate();
    });

    it('a valid `catalogue` is MERGED and WRITTEN', async () => {
        base = await baseNeuve('canal-apps-ecriture');
        await enrolerUneVm(base, 'v-1');
        const pair = await ouvrir(await start(base));
        await enrole(pair);

        pair.socket.send(encodeCatalogue(true, [app('Firefox', 'c-1'), app('Excel', 'c-2')], []));
        expect(await attendreCatalogue(base, 'v-1', (n) => n.length === 2, 'with two entries'))
            .toEqual(['Excel', 'Firefox']);

        // Et un second message COMPLET sans Excel l'en retire — c'est la
        // fusion qui décide, et elle est branchée sur le chemin réel.
        maintenant = T0 + 30_000;
        pair.socket.send(encodeCatalogue(true, [app('Firefox', 'c-1')], []));
        expect(await attendreCatalogue(base, 'v-1', (n) => n.length === 1, 'with one entry'))
            .toEqual(['Firefox']);
        pair.socket.terminate();
    });

    it("🔴 writing the catalogue is NOT AWAITED, and its failure does not bring the connection down", async () => {
        // 🔴 UN `await` DANS LE GESTIONNAIRE `message` FERAIT QU'UNE BASE
        // MOMENTANÉMENT INDISPONIBLE ABATTRAIT LA CONNEXION D'UN AGENT QUI VA
        // TRÈS BIEN — et une promesse rejetée SANS `catch` abattrait tout le
        // process Node. C'est la règle que ce canal s'impose depuis P3 pour
        // `marquerVu`.
        //
        // La base est FERMÉE sous les pieds du canal : toute écriture lève.
        // Le pair, lui, doit continuer d'être servi.
        base = await baseNeuve('canal-apps-echec-ecriture');
        await enrolerUneVm(base, 'v-1');
        const journal = vi.spyOn(console, 'error').mockImplementation(() => {});
        const pair = await ouvrir(await start(base));
        await enrole(pair);

        await base.fermer();
        pair.socket.send(encodeCatalogue(true, [app('Firefox', 'c-1')], []));

        // La connexion vit toujours, et répond encore : le refus d'un message
        // mal formé est la preuve la moins ambiguë que la boucle tourne.
        const rep = await pair.dire('not json');
        expect(rep).toEqual({ type: 'refus', v: PLATEFORME_VERSION, motif: 'forme' });
        // Et l'échec est AU JOURNAL, jamais silencieux : une écriture perdue
        // ne se voit nulle part ailleurs.
        expect(journal.mock.calls.map((c) => String(c[0])).join('\n')).toContain('v-1');

        pair.socket.terminate();
        base = undefined;
    });

    it("enrolment REGISTERS the VM in the registry, and closing REMOVES it", async () => {
        // 🔴 Oublier l'inscription ferait rendre `agent-injoignable` à tout
        // lancement, pour une VM parfaitement connectée. Oublier le retrait
        // ferait l'inverse : une VM morte resterait « joignable » jusqu'au
        // prochain enrôlement, et chaque lancement coûterait cinq secondes
        // d'attente avant d'échouer.
        base = await baseNeuve('canal-apps-registre');
        await enrolerUneVm(base, 'v-1');
        const pair = await ouvrir(await start(base));

        // AVANT l'enrôlement : personne. C'est le témoin sans lequel
        // l'assertion suivante serait vraie d'un registre qui accepterait tout.
        await expect(registre.lancer('v-1', 'c-1', 'd-0')).resolves.toBe('agent-injoignable');

        await enrole(pair);
        const enVol = registre.lancer('v-1', 'c-1', 'd-1');
        // L'ordre est bien PARTI sur le socket : le pair le reçoit.
        const ordre = await pair.recevoir();
        expect(ordre).toEqual({
            type: 'lancer',
            v: PLATEFORME_VERSION,
            demande: 'd-1',
            cle: 'c-1',
        });

        // La fermeture retire la VM — et rejette la demande en vol.
        pair.socket.close();
        expect(await enVol).toBe('agent-injoignable');
        await expect(registre.lancer('v-1', 'c-1', 'd-2')).resolves.toBe('agent-injoignable');
    });

    it('a `lancee` RESOLVES the request in flight, with its outcome', async () => {
        base = await baseNeuve('canal-apps-lancee');
        await enrolerUneVm(base, 'v-1');
        const pair = await ouvrir(await start(base));
        await enrole(pair);

        const enVol = registre.lancer('v-1', 'c-1', 'd-1');
        const ordre = await pair.recevoir();
        expect(ordre.demande).toBe('d-1');

        pair.socket.send(encodeLancee('d-1', 'raccourci'));
        // 🔴 `raccourci` ET NON `true` : c'est le CHEMIN emprunté qui rend le
        // critère de recette décidable — lancer par la cible reconstruite au
        // lieu du `.lnk` passerait un critère qui ne dirait que « quelque chose
        // s'est lancé ».
        expect(await enVol).toBe('raccourci');
        pair.socket.terminate();
    });
});

// ---------------------------------------------------------------------------
// Sous-bloc G2 — l'inventaire des icônes manquantes.
// ---------------------------------------------------------------------------

/// 🔴 LES EMPREINTES SONT DÉRIVÉES DE LEUR CONTENU, JAMAIS INVENTÉES. Une
/// première rédaction posait `'a'.repeat(64)` et déposait des octets
/// quelconques dessous : `write` l'a REFUSÉ — c'est la garde de recalcul
/// faisant exactement son travail, sur le test qui l'ignorait.
const OCTETS_1 = Buffer.from('\x89PNG-un');
const OCTETS_2 = Buffer.from('\x89PNG-deux');
const E1 = createHash('sha256').update(OCTETS_1).digest('hex');
const E2 = createHash('sha256').update(OCTETS_2).digest('hex');

/// Attend le prochain message poussé, ou rend `undefined` s'il n'en vient
/// aucun.
///
/// 🔴 IL FAUT UNE ATTENTE POSITIVE POUR POUVOIR CONCLURE À L'ABSENCE. Le
/// message est poussé APRÈS l'écriture du catalogue, qui est délibérément
/// lancée SANS être attendue : conclure trop tôt rendrait le critère ⑤ vert
/// sur un produit qui pousse bel et bien un message. `recevoir()` est bornée à
/// 2 000 ms et LÈVE sur expiration — c'est cette levée qui vaut « aucun ».
async function pousseOuRien(pair: Pair): Promise<Record<string, unknown> | undefined> {
    try {
        return await pair.recevoir();
    } catch {
        return undefined;
    }
}

describe("the inventory of missing icons", () => {
    afterEach(() => {
        for (const r of racinesIcones) rmSync(r, { recursive: true, force: true });
        racinesIcones = [];
    });

    it('asks for the fingerprints the store does NOT have', async () => {
        base = await baseNeuve('canal-icones-manque');
        const port = await start(base, true);
        await enrolerUneVm(base, 'v-1');
        const pair = await ouvrir(port);
        await enrole(pair);
        pair.socket.send(encodeCatalogue(true, [app('A', 'c-a', E1), app('B', 'c-b', E2)], []));
        const message = await pousseOuRien(pair);
        expect(message, "the inventory must be pushed").toBeDefined();
        expect(message!.type).toBe('icones-manquantes');
        expect(message!.empreintes).toEqual([E1, E2]);
        expect(message!.v).toBe(PLATEFORME_VERSION);
        pair.socket.close();
    });

    it('🔴 PUSHES NOTHING when the store already has everything — criterion ⑤', async () => {
        // 🔴 UNE LISTE VIDE COÛTERAIT UN MESSAGE PAR RÉCONCILIATION SUR UN
        // DISQUE AU REPOS, c'est-à-dire toutes les trente secondes, pour
        // toujours. C'est très exactement ce que le diff de G1 existe pour
        // éviter, et c'est là que le critère ⑤ se juge.
        base = await baseNeuve('canal-icones-rien');
        const port = await start(base, true);
        magasin!.write(E1, OCTETS_1);
        magasin!.write(E2, OCTETS_2);
        await enrolerUneVm(base, 'v-1');
        const pair = await ouvrir(port);
        await enrole(pair);
        pair.socket.send(encodeCatalogue(true, [app('A', 'c-a', E1), app('B', 'c-b', E2)], []));
        // 🔴 LE CATALOGUE EST BIEN ARRIVÉ — sans quoi ce « rien » serait celui
        // d'un produit EN PANNE, et ne dirait rien du tout.
        await attendreCatalogue(base, 'v-1', (n) => n.includes('A'), 'written');
        expect(await pousseOuRien(pair)).toBeUndefined();
        pair.socket.close();
    });

    it('asks ONLY for what is missing, and ignores the applications WITHOUT an icon', async () => {
        base = await baseNeuve('canal-icones-partiel');
        const port = await start(base, true);
        magasin!.write(E1, OCTETS_1);
        await enrolerUneVm(base, 'v-1');
        const pair = await ouvrir(port);
        await enrole(pair);
        pair.socket.send(
            encodeCatalogue(true, [app('A', 'c-a', E1), app('B', 'c-b', E2), app('C', 'c-c')], []),
        );
        const message = await pousseOuRien(pair);
        expect(message!.empreintes).toEqual([E2]);
        pair.socket.close();
    });

    it('🔴 ASKS AGAIN for an icon whose FILE has disappeared — criterion ⑦', async () => {
        // 🔴 L'INVENTAIRE INTERROGE LE DISQUE, PAS UNE TABLE. Une table de
        // comptabilité ne verrait pas la perte, et l'icône serait perdue POUR
        // TOUJOURS. C'est ce qui rend le magasin AUTO-RECONSTRUCTIBLE, et donc
        // le disque acceptable.
        base = await baseNeuve('canal-icones-perdu');
        const port = await start(base, true);
        magasin!.write(E1, OCTETS_1);
        rmSync(join(magasin!.repertoire, E1));
        await enrolerUneVm(base, 'v-1');
        const pair = await ouvrir(port);
        await enrole(pair);
        pair.socket.send(encodeCatalogue(true, [app('A', 'c-a', E1)], []));
        const message = await pousseOuRien(pair);
        expect(message!.empreintes).toEqual([E1]);
        pair.socket.close();
    });

    it('without a store, no inventory — and the catalogue is written anyway', async () => {
        base = await baseNeuve('canal-icones-sans-magasin');
        const port = await start(base, false);
        await enrolerUneVm(base, 'v-1');
        const pair = await ouvrir(port);
        await enrole(pair);
        pair.socket.send(encodeCatalogue(true, [app('A', 'c-a', E1)], []));
        await attendreCatalogue(base, 'v-1', (n) => n.includes('A'), 'written');
        expect(await pousseOuRien(pair)).toBeUndefined();
        pair.socket.close();
    });
});
