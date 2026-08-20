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

async function demarrer(p: Pilote): Promise<number> {
    maintenant = T0;
    registre = new RegistreAgents();
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
    });
    const adresse = wss.address();
    return typeof adresse === 'object' && adresse ? adresse.port : 0;
}

function app(nom: string, cle: string): Application {
    return {
        cle,
        nom,
        chemin: `C:\\Users\\guacamole\\Desktop\\${nom}.lnk`,
        cible: `c:\\program files\\${nom}\\${nom}.exe`,
        arguments: '',
        repertoire: `c:\\program files\\${nom}`,
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
            throw new Error(`catalogue ${quoi} jamais atteint pour ${vmId} (vu=${noms.join(',')})`);
        }
        await new Promise((r) => setTimeout(r, 25));
    }
}

/// Enrôle le pair et attend la réponse — préalable de tous les cas nominaux.
async function enrole(pair: Pair): Promise<void> {
    const rep = await pair.dire(encodeEnroler('v-1', SECRET_VM));
    expect(rep.type).toBe('enrole');
}

describe('le canal /agent, côté applications', () => {
    it('🔴 un `catalogue` AVANT tout enrôlement est refusé, motif `sequence`', async () => {
        // 🔴 L'ACCEPTER LAISSERAIT UN PAIR ANONYME ÉCRIRE DANS LA TABLE
        // `application` D'UNE VM QU'IL N'A PAS AUTHENTIFIÉE. C'est le trou
        // exact que le refus `sequence` du battement ferme déjà, par une autre
        // porte — et celle-ci écrit en base, là où celle-là ne délivrait qu'un
        // jeton.
        base = await baseNeuve('canal-apps-sequence-catalogue');
        await enrolerUneVm(base, 'v-1');
        const pair = await ouvrir(await demarrer(base));

        const rep = await pair.dire(encodeCatalogue(true, [app('Intrus', 'cle-intrus')], []));
        expect(rep).toEqual({ type: 'refus', v: PLATEFORME_VERSION, motif: 'sequence' });
        // Et RIEN n'a été écrit : c'est la moitié qui décide. Un refus qui
        // aurait quand même laissé passer l'écriture serait une porte ouverte
        // avec un panneau « fermé ».
        expect(await lireParVm(base, 'v-1')).toEqual([]);
        pair.socket.terminate();
    });

    it('🔴 un `lancee` avant tout enrôlement est refusé, motif `sequence`', async () => {
        // Même raison : un anonyme pourrait sinon résoudre la demande d'un
        // autre, et faire croire à un lancement réussi qui n'a pas eu lieu.
        base = await baseNeuve('canal-apps-sequence-lancee');
        await enrolerUneVm(base, 'v-1');
        const pair = await ouvrir(await demarrer(base));

        const rep = await pair.dire(encodeLancee('d-1', 'raccourci'));
        expect(rep).toEqual({ type: 'refus', v: PLATEFORME_VERSION, motif: 'sequence' });
        pair.socket.terminate();
    });

    it('un `catalogue` valide est FUSIONNÉ et ÉCRIT', async () => {
        base = await baseNeuve('canal-apps-ecriture');
        await enrolerUneVm(base, 'v-1');
        const pair = await ouvrir(await demarrer(base));
        await enrole(pair);

        pair.socket.send(encodeCatalogue(true, [app('Firefox', 'c-1'), app('Excel', 'c-2')], []));
        expect(await attendreCatalogue(base, 'v-1', (n) => n.length === 2, 'à deux entrées'))
            .toEqual(['Excel', 'Firefox']);

        // Et un second message COMPLET sans Excel l'en retire — c'est la
        // fusion qui décide, et elle est branchée sur le chemin réel.
        maintenant = T0 + 30_000;
        pair.socket.send(encodeCatalogue(true, [app('Firefox', 'c-1')], []));
        expect(await attendreCatalogue(base, 'v-1', (n) => n.length === 1, 'à une entrée'))
            .toEqual(['Firefox']);
        pair.socket.terminate();
    });

    it("🔴 l'écriture du catalogue n'est PAS ATTENDUE, et son échec n'abat pas la connexion", async () => {
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
        const pair = await ouvrir(await demarrer(base));
        await enrole(pair);

        await base.fermer();
        pair.socket.send(encodeCatalogue(true, [app('Firefox', 'c-1')], []));

        // La connexion vit toujours, et répond encore : le refus d'un message
        // mal formé est la preuve la moins ambiguë que la boucle tourne.
        const rep = await pair.dire('pas du json');
        expect(rep).toEqual({ type: 'refus', v: PLATEFORME_VERSION, motif: 'forme' });
        // Et l'échec est AU JOURNAL, jamais silencieux : une écriture perdue
        // ne se voit nulle part ailleurs.
        expect(journal.mock.calls.map((c) => String(c[0])).join('\n')).toContain('v-1');

        pair.socket.terminate();
        base = undefined;
    });

    it("l'enrôlement INSCRIT la VM au registre, et la fermeture l'en RETIRE", async () => {
        // 🔴 Oublier l'inscription ferait rendre `agent-injoignable` à tout
        // lancement, pour une VM parfaitement connectée. Oublier le retrait
        // ferait l'inverse : une VM morte resterait « joignable » jusqu'au
        // prochain enrôlement, et chaque lancement coûterait cinq secondes
        // d'attente avant d'échouer.
        base = await baseNeuve('canal-apps-registre');
        await enrolerUneVm(base, 'v-1');
        const pair = await ouvrir(await demarrer(base));

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

    it('un `lancee` RÉSOUT la demande en vol, avec son issue', async () => {
        base = await baseNeuve('canal-apps-lancee');
        await enrolerUneVm(base, 'v-1');
        const pair = await ouvrir(await demarrer(base));
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
