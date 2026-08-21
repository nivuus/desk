// Le canal `/agent`, côté INSTALLATION : la réémission à l'enrôlement, et les
// deux montantes que l'agent rapporte.
//
// 🔴 UN FICHIER À PART, PAS UNE ADDITION À `canal-apps.test.ts`. Celui-ci est à
// 373 lignes ; y ajouter cent vingt lignes l'aurait porté à portée du plafond,
// et ce dépôt écrit quatre fois que la marge regagnée par une extraction se
// reperd si on la traite comme acquise. Le harnais est PARTAGÉ
// (`canal-harnais.ts`, extrait par G2 pour cette raison exacte) : le recopier
// aurait produit deux `ouvrirUrl` qui divergeraient à la première correction
// portée sur un seul des deux.

import { afterEach, describe, expect, it, vi } from 'vitest';
import { WebSocketServer } from 'ws';
import {
    encodeEnroler,
    encodeProgression,
    encodeTermine,
    PLATEFORME_VERSION,
} from '../../../proto/ts/plateforme';
import { baseNeuve } from '../base/harnais';
import type { Pilote } from '../base/pilote';
import { Frein } from '../securite/frein';
import { creer as creerInstallation, lireParId } from '../depot/installation';
import { creer as creerTeleversement, sceller } from '../depot/televersement';
import { servirLeCanalAgent } from './canal';
import { enrolerUneVm, ouvrir, SECRET, SECRET_VM, T0, type Pair } from './canal-harnais';
import { RegistreAgents } from './registre';

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
        frein: new Frein(),
        proxyDeConfiance: new Set(),
        magasin: undefined,
    });
    const adresse = wss.address();
    return typeof adresse === 'object' && adresse ? adresse.port : 0;
}

async function enrole(pair: Pair): Promise<void> {
    const rep = await pair.dire(encodeEnroler('v-1', SECRET_VM));
    expect(rep.type).toBe('enrole');
}

/// Un téléversement scellé, et une installation `en_attente` pour la VM `v-1`.
async function unOrdreEnAttente(p: Pilote): Promise<{ installation: string; tel: string }> {
    await p.executer(
        'INSERT INTO utilisateur(id,email,empreinte_mdp,cree_a) VALUES(?,?,?,?)',
        ['u-1', 'a@b.c', 'scrypt$1$1$1$x$y', T0],
    );
    const tel = await creerTeleversement(
        p,
        {
            utilisateurId: 'u-1',
            nom: 'Firefox Setup 130.0.exe',
            taille: 3_221_225_472,
            sha256: 'a'.repeat(64),
            tailleTranche: 8 * 1024 * 1024,
        },
        T0,
    );
    await sceller(p, tel.id, T0 + 1);
    const inst = await creerInstallation(p, { vmId: 'v-1', televersementId: tel.id }, T0 + 2);
    return { installation: inst.id, tel: tel.id };
}

describe('la réémission des installations à l’enrôlement', () => {
    // 🔴 LA ROUGE DU CRITÈRE ④, ET ELLE EST GRATUITE SUR LE BINAIRE DE G1 —
    // qui n'a aucune variante `installer` du tout. Un `push` WebSocket n'a
    // AUCUNE garantie de livraison : sans cette réémission, un ordre émis
    // pendant une coupure serait perdu SANS TERME, et l'utilisateur
    // attendrait une installation que personne ne relancerait jamais.
    //
    // ⚠️ CE TEST N'ENVOIE RIEN APRÈS L'ENRÔLEMENT, et c'est le point : il
    // attend un message que le canal POUSSE de lui-même. `recevoir()` existe
    // pour cela — l'attendre par un `dire('')` serait une course, et
    // provoquerait un refus `forme` une fois sur deux.
    it('🔴 POUSSE l’ordre en attente, sans que le pair ait rien demandé', async () => {
        base = await baseNeuve('canal-inst-reemission');
        await enrolerUneVm(base, 'v-1');
        const { installation, tel } = await unOrdreEnAttente(base);
        const pair = await ouvrir(await demarrer(base));

        await enrole(pair);
        const ordre = await pair.recevoir();
        expect(ordre).toEqual({
            type: 'installer',
            v: PLATEFORME_VERSION,
            installation,
            // ⚠️ L'URL EST RELATIVE, et c'est délibéré : l'agent la résout
            // contre l'adresse de son propre canal, comme il dérive déjà celle
            // du téléversement d'icônes. Deux variables pour la même adresse
            // divergeraient le jour où l'une des deux serait changée.
            url: `/televersement/${tel}/contenu`,
            nom: 'Firefox Setup 130.0.exe',
            taille: 3_221_225_472,
            sha256: 'a'.repeat(64),
        });
        pair.socket.terminate();
    });

    // 🔴 LA MOITIÉ QUI EMPÊCHE LA DOUBLE EXÉCUTION, et sans elle le test
    // ci-dessus serait vrai d'un service qui rejoue INDÉFINIMENT. Dès qu'un
    // agent a rapporté une progression, la ligne passe `en_cours` et cesse
    // d'être réémise. C'est la PREMIÈRE des deux ceintures ; la seconde est le
    // marqueur sur le disque de la VM, et elle protège du cas où la première a
    // perdu sa base.
    it('🔴 NE RÉÉMET PLUS une installation déjà commencée', async () => {
        base = await baseNeuve('canal-inst-pas-deux-fois');
        await enrolerUneVm(base, 'v-1');
        const { installation } = await unOrdreEnAttente(base);
        const premier = await ouvrir(await demarrer(base));
        await enrole(premier);
        // Le premier enrôlement le reçoit bien — c'est le témoin sans lequel
        // l'assertion suivante serait vraie d'un service entièrement muet.
        expect((await premier.recevoir()).installation).toBe(installation);

        // L'agent rapporte une progression : la ligne quitte `en_attente`.
        // ⚠️ `send`, JAMAIS `dire` : une progression n'appelle AUCUNE réponse,
        // et `dire` l'attendrait deux secondes avant de rougir pour la mauvaise
        // raison. C'est ce que fait déjà le test du catalogue, pour la même
        // raison — on attend l'EFFET en base, pas un accusé qui n'existe pas.
        premier.socket.send(encodeProgression(installation, 'transfert', 8_388_608, 3_221_225_472, 1_200));
        // ⚠️ L'ÉCRITURE N'EST PAS ATTENDUE PAR LE CANAL (c'est la règle du
        // fichier), donc on attend l'EFFET plutôt qu'une durée.
        await attendreEtat(base, installation, 'en_cours');
        premier.socket.terminate();

        // Un second enrôlement ne doit RIEN recevoir.
        const second = await ouvrir(await demarrer(base));
        await enrole(second);
        await expect(second.recevoir()).rejects.toThrow(/aucun message poussé/);
        second.socket.terminate();
    });
});

describe('les deux montantes de l’installation', () => {
    // 🔴 NI PROGRESSION NI ISSUE SANS ENRÔLEMENT. Les accepter laisserait un
    // pair anonyme écrire dans la table `installation` d'une VM qui n'est pas
    // la sienne — donc DÉCLARER RÉUSSIE, OU REFUSÉE, L'INSTALLATION D'AUTRUI.
    // C'est le trou que le refus `sequence` du battement ferme déjà, par deux
    // autres portes.
    it('🔴 une `progression` avant tout enrôlement est refusée, motif `sequence`', async () => {
        base = await baseNeuve('canal-inst-seq-progression');
        await enrolerUneVm(base, 'v-1');
        const { installation } = await unOrdreEnAttente(base);
        const pair = await ouvrir(await demarrer(base));

        const rep = await pair.dire(encodeProgression(installation, 'transfert', 1, 2, 3));
        expect(rep).toEqual({ type: 'refus', v: PLATEFORME_VERSION, motif: 'sequence' });
        // Et RIEN n'a été écrit : c'est la moitié qui décide. Un refus qui
        // aurait quand même laissé passer l'écriture serait une porte ouverte
        // avec un panneau « fermé ».
        expect((await lireParId(base, installation))?.etat).toBe('en_attente');
        pair.socket.terminate();
    });

    it('🔴 un `termine` avant tout enrôlement est refusé, motif `sequence`', async () => {
        base = await baseNeuve('canal-inst-seq-termine');
        await enrolerUneVm(base, 'v-1');
        const { installation } = await unOrdreEnAttente(base);
        const pair = await ouvrir(await demarrer(base));

        const rep = await pair.dire(
            encodeTermine(installation, 'reussie', null, 0, '', false),
        );
        expect(rep).toEqual({ type: 'refus', v: PLATEFORME_VERSION, motif: 'sequence' });
        expect((await lireParId(base, installation))?.issue).toBeNull();
        pair.socket.terminate();
    });

    it('un `termine` valide écrit l’issue, le motif et le code de sortie', async () => {
        base = await baseNeuve('canal-inst-termine');
        await enrolerUneVm(base, 'v-1');
        const { installation } = await unOrdreEnAttente(base);
        const pair = await ouvrir(await demarrer(base));
        await enrole(pair);
        await pair.recevoir(); // l'ordre réémis

        maintenant = T0 + 90_000;
        pair.socket.send(
            // ⚠️ 3010 EST UN SUCCÈS QUI DEMANDE UN REDÉMARRAGE, et il est
            // RAPPORTÉ à côté de l'issue sans que rien n'en déduise quoi que ce
            // soit : c'est l'agent, et lui seul, qui a compté les applications
            // apparues pendant sa fenêtre.
            encodeTermine(installation, 'reussie', null, 3010, 'Redemarrage requis.', false),
        );
        await attendreEtat(base, installation, 'terminee');
        const ligne = await lireParId(base, installation);
        expect(ligne?.issue).toBe('reussie');
        expect(ligne?.code_sortie).toBe(3010);
        expect(ligne?.terminee_a).toBe(T0 + 90_000);
        pair.socket.terminate();
    });

    // ⚠️ UN `termine` DONT L'ÉCRITURE ÉCHOUE NE DOIT PAS ABATTRE LA CONNEXION.
    // Un `await` sur le chemin d'un message ferait qu'une base momentanément
    // indisponible tuerait le canal d'un agent qui va très bien, et une
    // promesse rejetée sans `catch` abattrait tout le process Node.
    it("🔴 l'écriture de l'issue n'est PAS ATTENDUE, et son échec n'abat pas la connexion", async () => {
        base = await baseNeuve('canal-inst-echec-ecriture');
        await enrolerUneVm(base, 'v-1');
        const { installation } = await unOrdreEnAttente(base);
        const pair = await ouvrir(await demarrer(base));
        await enrole(pair);
        await pair.recevoir();

        vi.spyOn(base, 'executer').mockRejectedValue(new Error('base indisponible'));
        vi.spyOn(console, 'error').mockImplementation(() => {});
        pair.socket.send(encodeTermine(installation, 'reussie', null, 0, '', false));
        // On laisse le gestionnaire courir : l'écriture est lancée sans être
        // attendue, et c'est précisément la propriété qu'on éprouve.
        await new Promise((r) => setTimeout(r, 50));

        // La connexion vit toujours : le battement suivant obtient sa réponse.
        vi.restoreAllMocks();
        const rep = await pair.dire(JSON.stringify({ type: 'battement', v: PLATEFORME_VERSION }));
        expect(rep.type).toBe('battement-recu');
        pair.socket.terminate();
    });
});

/// Attend que l'état d'une installation atteigne `attendu`, BORNÉ.
///
/// 🔴 ON ATTEND LE FAIT, JAMAIS UNE DURÉE. Les écritures de ce canal sont
/// délibérément NON ATTENDUES (`void … .catch(…)`), donc un `await` sur une
/// durée arbitraire serait une course : trop court il rougirait sur une base
/// lente, trop long il ralentirait toute la suite.
async function attendreEtat(p: Pilote, id: string, attendu: string): Promise<void> {
    for (let i = 0; i < 200; i += 1) {
        const ligne = await lireParId(p, id);
        if (ligne?.etat === attendu) return;
        await new Promise((r) => setTimeout(r, 10));
    }
    throw new Error(`l'installation ${id} n'a pas atteint l'état ${attendu} en 2000 ms`);
}
