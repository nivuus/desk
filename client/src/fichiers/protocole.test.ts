import { describe, expect, it, vi } from 'vitest';
import {
    TYPE_ATTRIBUTS,
    TYPE_DONNEES,
    TYPE_ECHEC,
    TYPE_ENTREES,
    TYPE_LIRE,
    TYPE_LISTER,
    TYPE_META,
    encoder,
} from '../../../proto/ts/fichiers';
import { parseDonnees, parseEchec, parseEntrees, parseMeta } from '../../../proto/ts/fichiers-entetes';
import { decoder } from '../../../proto/ts/fichiers';
import { EchecFichiers, type Adaptateur } from './adaptateur';
import { creerServeur } from './protocole';

/** Un adaptateur factice : le protocole ne connaît AUCUN système de fichiers. */
function fauxAdaptateur(surcharge: Partial<Adaptateur> = {}): Adaptateur {
    return {
        lister: async () => [{ nom: 'a.txt', repertoire: false, taille: 7, modifie: 42 }],
        attributs: async () => ({ repertoire: false, taille: 1234, modifie: 1_690_000_000_000 }),
        lire: async () => new Uint8Array([9, 8, 7]),
        ...surcharge,
    };
}

describe('serveur du protocole fichiers', () => {
    it('répond à LISTER par ENTREES', async () => {
        const serveur = creerServeur(fauxAdaptateur());
        const reponse = await serveur.traiter(encoder(TYPE_LISTER, 11, { chemin: 'dossier' }));
        const trame = decoder(reponse!);
        expect(trame.type).toBe(TYPE_ENTREES);
        expect(trame.correlation).toBe(11);
        expect(parseEntrees(trame.entete).entrees[0].nom).toBe('a.txt');
        // Charge binaire vide : les entrées sont dans l'en-tête.
        expect(trame.charge.length).toBe(0);
    });

    it('répond à ATTRIBUTS par META', async () => {
        const serveur = creerServeur(fauxAdaptateur());
        const trame = decoder((await serveur.traiter(encoder(TYPE_ATTRIBUTS, 3, { chemin: '' })))!);
        expect(trame.type).toBe(TYPE_META);
        expect(parseMeta(trame.entete).taille).toBe(1234);
    });

    it('🔴 répond à LIRE par DONNEES dont la longueur est celle REELLEMENT lue', async () => {
        // 🔴 Un fichier lu jusqu'à sa fin rend MOINS d'octets que demandé.
        // Recopier la longueur DEMANDÉE dans l'en-tête ferait mentir la trame,
        // et l'agent la refuserait pour incohérence en-tête/charge — le seul
        // contrôle qui existe pour empêcher d'écrire dans le tampon de ProjFS
        // une quantité que l'émetteur ne croyait pas envoyer.
        const serveur = creerServeur(fauxAdaptateur({ lire: async () => new Uint8Array([1, 2]) }));
        const trame = decoder(
            (await serveur.traiter(
                encoder(TYPE_LIRE, 5, { chemin: 'gros.bin', position: 64, longueur: 4096 }),
            ))!,
        );
        expect(trame.type).toBe(TYPE_DONNEES);
        const entete = parseDonnees(trame.entete);
        expect(entete.position).toBe(64);
        expect(entete.longueur).toBe(2);
        expect([...trame.charge]).toEqual([1, 2]);
    });

    it('🔴 une réponse à une corrélation inconnue est ignorée', async () => {
        // Le navigateur est un SERVEUR : il ne demande jamais rien, donc aucune
        // corrélation ne lui appartient. Une trame de type RÉPONSE ne peut être
        // qu'un écho, une boucle, ou un pair confus — la décoder comme une
        // requête est le défaut exact que la tâche 17 corrige côté agent, où un
        // aiguillage sur le seul drapeau binaire prenait toute trame pour une
        // entrée souris.
        const journal = vi.fn();
        const serveur = creerServeur(fauxAdaptateur(), journal);
        for (const type of [TYPE_ENTREES, TYPE_META, TYPE_DONNEES, TYPE_ECHEC]) {
            expect(await serveur.traiter(encoder(type, 77, {}))).toBeNull();
        }
        expect(journal).toHaveBeenCalledTimes(4);
        // 🔴 LE JOURNAL DOIT DIRE LAQUELLE DES DEUX CAUSES, et c'est une
        // MUTATION SURVIVANTE qui l'a exigé : retirer le bras des types de
        // réponse laissait ces trames tomber dans le catch-all « type inconnu »,
        // qui les ignore AUSSI — le test passait au vert sur un aiguillage qui
        // ne nommait plus ses cas. Or les deux causes n'appellent pas le même
        // geste : une réponse reçue dit que l'agent nous renvoie notre propre
        // trafic, un type inconnu dit que les deux bouts n'ont pas la même
        // version. Confondre les deux, c'est le bras catch-all de
        // `capteur/pont_media.rs`, que ce dépôt a payé quatre fois.
        for (const appel of journal.mock.calls) {
            expect(appel[0]).toMatch(/réponse ignorée/);
            expect(appel[0]).toMatch(/77/);
        }
    });

    it('un type inconnu est ignoré, et le journal le distingue d’une réponse', async () => {
        const journal = vi.fn();
        const serveur = creerServeur(fauxAdaptateur(), journal);
        expect(await serveur.traiter(encoder(200, 9, {}))).toBeNull();
        expect(journal.mock.calls[0][0]).toMatch(/type inconnu/);
        expect(journal.mock.calls[0][0]).toMatch(/200/);
    });

    it('une trame illisible est ignorée plutôt que de faire tomber le canal', async () => {
        const journal = vi.fn();
        const serveur = creerServeur(fauxAdaptateur(), journal);
        const mauvaise = new Uint8Array(encoder(TYPE_LISTER, 1, { chemin: '' }));
        mauvaise[0] = 2; // version 2
        expect(await serveur.traiter(mauvaise.buffer as ArrayBuffer)).toBeNull();
        expect(journal.mock.calls[0][0]).toMatch(/version/i);
    });

    it('🔴 un échec de l’adaptateur devient un CODE, jamais une chaîne', async () => {
        const serveur = creerServeur(
            fauxAdaptateur({
                attributs: async () => {
                    throw new EchecFichiers('acces-refuse', 'permission révoquée');
                },
            }),
        );
        const trame = decoder((await serveur.traiter(encoder(TYPE_ATTRIBUTS, 4, { chemin: 'x' })))!);
        expect(trame.type).toBe(TYPE_ECHEC);
        expect(trame.correlation).toBe(4);
        expect(parseEchec(trame.entete).code).toBe('acces-refuse');
    });

    it('une panne imprévue devient interne, et la corrélation est RENDUE', async () => {
        // 🔴 Ne rien répondre laisserait la commande en vol côté agent jusqu'à
        // son expiration : l'Explorateur se figerait sur une panne qui, elle,
        // est immédiate.
        const serveur = creerServeur(
            fauxAdaptateur({
                lister: async () => {
                    throw new Error('quelque chose a explosé');
                },
            }),
        );
        const trame = decoder((await serveur.traiter(encoder(TYPE_LISTER, 6, { chemin: '' })))!);
        expect(trame.type).toBe(TYPE_ECHEC);
        expect(trame.correlation).toBe(6);
        expect(parseEchec(trame.entete).code).toBe('interne');
    });

    it('un en-tête malformé est REFUSÉ, et la corrélation est rendue', async () => {
        const serveur = creerServeur(fauxAdaptateur());
        // `chemin` absent : le parseur du protocole partagé lève.
        const trame = decoder((await serveur.traiter(encoder(TYPE_LISTER, 8, { rien: 1 })))!);
        expect(trame.type).toBe(TYPE_ECHEC);
        expect(trame.correlation).toBe(8);
        expect(parseEchec(trame.entete).code).toBe('interne');
    });
});
