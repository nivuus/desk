import { describe, expect, it, vi } from 'vitest';
import {
    TYPE_ATTRIBUTS,
    TYPE_CREATE,
    TYPE_DATA,
    TYPE_ECHEC,
    TYPE_WRITE,
    TYPE_ENTREES,
    TYPE_FAIT,
    TYPE_RENOMMER,
    TYPE_DELETE,
    TYPE_LIRE,
    TYPE_LISTER,
    TYPE_META,
    encoder,
} from '../../../proto/ts/fichiers';
import { parseData, parseEchec, parseEntrees, parseMeta } from '../../../proto/ts/fichiers-entetes';
import { decoder } from '../../../proto/ts/fichiers';
import { FilesError, type Adaptateur } from './adaptateur';
import { createServer } from './protocole';
import type { Ecrivain } from './ecriture';
import type { Mutateur } from './mutation-service';

/** Un écrivain factice : le protocole ne connaît AUCUN système de fichiers. */
function fauxEcrivain(surcharge: Partial<Ecrivain> = {}): Ecrivain & { vus: string[] } {
    const vus: string[] = [];
    return {
        vus,
        write: async (chemin, position, octets, premier, last) => {
            vus.push(`ecrire ${chemin} @${position} +${octets.length} ${premier}/${last}`);
        },
        create: async (chemin, repertoire) => {
            vus.push(`creer ${chemin} ${repertoire}`);
        },
        abandonner: () => vus.push('abandonner'),
        ...surcharge,
    };
}

/** Un adaptateur factice : le protocole ne connaît AUCUN système de fichiers. */
function fauxAdaptateur(surcharge: Partial<Adaptateur> = {}): Adaptateur {
    return {
        lister: async () => [{ nom: 'a.txt', repertoire: false, taille: 7, modifie: 42 }],
        attributs: async () => ({
            nom: 'Nom Stocké.txt',
            repertoire: false,
            taille: 1234,
            modifie: 1_690_000_000_000,
        }),
        lire: async () => new Uint8Array([9, 8, 7]),
        ...surcharge,
    };
}

describe('file protocol server', () => {
    it('answers LISTER with ENTREES', async () => {
        const serveur = createServer(fauxAdaptateur());
        const reponse = await serveur.traiter(encoder(TYPE_LISTER, 11, { chemin: 'dossier' }));
        const trame = decoder(reponse!);
        expect(trame.type).toBe(TYPE_ENTREES);
        expect(trame.correlation).toBe(11);
        expect(parseEntrees(trame.entete).entrees[0].nom).toBe('a.txt');
        // Charge binaire vide : les entrées sont dans l'en-tête.
        expect(trame.charge.length).toBe(0);
    });

    it('answers ATTRIBUTS with META', async () => {
        const serveur = createServer(fauxAdaptateur());
        const trame = decoder((await serveur.traiter(encoder(TYPE_ATTRIBUTS, 3, { chemin: '' })))!);
        expect(trame.type).toBe(TYPE_META);
        expect(parseMeta(trame.entete).taille).toBe(1234);
        // 🔴 **LE NOM CANONIQUE TRAVERSE LE FIL.** Sans lui, le substitut
        // serait créé sous le nom que l'application a TAPÉ, et non sous celui
        // qui existe sur le poste local.
        expect(parseMeta(trame.entete).nom).toBe('Nom Stocké.txt');
    });

    it('🔴 answers LIRE with DONNEES whose length is the one ACTUALLY read', async () => {
        // 🔴 Un fichier lu jusqu'à sa fin rend MOINS d'octets que demandé.
        // Recopier la longueur DEMANDÉE dans l'en-tête ferait mentir la trame,
        // et l'agent la refuserait pour incohérence en-tête/charge — le seul
        // contrôle qui existe pour empêcher d'écrire dans le tampon de ProjFS
        // une quantité que l'émetteur ne croyait pas envoyer.
        const serveur = createServer(fauxAdaptateur({ lire: async () => new Uint8Array([1, 2]) }));
        const trame = decoder(
            (await serveur.traiter(
                encoder(TYPE_LIRE, 5, { chemin: 'gros.bin', position: 64, longueur: 4096 }),
            ))!,
        );
        expect(trame.type).toBe(TYPE_DATA);
        const entete = parseData(trame.entete);
        expect(entete.position).toBe(64);
        expect(entete.longueur).toBe(2);
        expect([...trame.charge]).toEqual([1, 2]);
    });

    it('🔴 an answer to an unknown correlation is ignored', async () => {
        // Le navigateur est un SERVEUR : il ne demande jamais rien, donc aucune
        // corrélation ne lui appartient. Une trame de type RÉPONSE ne peut être
        // qu'un écho, une boucle, ou un pair confus — la décoder comme une
        // requête est le défaut exact que la tâche 17 corrige côté agent, où un
        // aiguillage sur le seul drapeau binaire prenait toute trame pour une
        // entrée souris.
        const journal = vi.fn();
        const serveur = createServer(fauxAdaptateur(), journal);
        for (const type of [TYPE_ENTREES, TYPE_META, TYPE_DATA, TYPE_ECHEC]) {
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
            expect(appel[0]).toMatch(/answer ignored/);
            expect(appel[0]).toMatch(/77/);
        }
    });

    it('an unknown type is ignored, and the log tells it apart from an answer', async () => {
        const journal = vi.fn();
        const serveur = createServer(fauxAdaptateur(), journal);
        expect(await serveur.traiter(encoder(200, 9, {}))).toBeNull();
        expect(journal.mock.calls[0][0]).toMatch(/unknown type/);
        expect(journal.mock.calls[0][0]).toMatch(/200/);
    });

    it('an unreadable frame is ignored rather than bringing the channel down', async () => {
        const journal = vi.fn();
        const serveur = createServer(fauxAdaptateur(), journal);
        const mauvaise = new Uint8Array(encoder(TYPE_LISTER, 1, { chemin: '' }));
        mauvaise[0] = 2; // version 2
        expect(await serveur.traiter(mauvaise.buffer as ArrayBuffer)).toBeNull();
        expect(journal.mock.calls[0][0]).toMatch(/version/i);
    });

    it('🔴 an adapter failure becomes a CODE, never a string', async () => {
        const serveur = createServer(
            fauxAdaptateur({
                attributs: async () => {
                    throw new FilesError('acces-refuse', 'permission revoked');
                },
            }),
        );
        const trame = decoder((await serveur.traiter(encoder(TYPE_ATTRIBUTS, 4, { chemin: 'x' })))!);
        expect(trame.type).toBe(TYPE_ECHEC);
        expect(trame.correlation).toBe(4);
        expect(parseEchec(trame.entete).code).toBe('acces-refuse');
    });

    it('an unexpected failure becomes internal, and the correlation is RETURNED', async () => {
        // 🔴 Ne rien répondre laisserait la commande en vol côté agent jusqu'à
        // son expiration : l'Explorateur se figerait sur une panne qui, elle,
        // est immédiate.
        const serveur = createServer(
            fauxAdaptateur({
                lister: async () => {
                    throw new Error('something blew up');
                },
            }),
        );
        const trame = decoder((await serveur.traiter(encoder(TYPE_LISTER, 6, { chemin: '' })))!);
        expect(trame.type).toBe(TYPE_ECHEC);
        expect(trame.correlation).toBe(6);
        expect(parseEchec(trame.entete).code).toBe('interne');
    });

    it('a malformed header is REFUSED, and the correlation is returned', async () => {
        const serveur = createServer(fauxAdaptateur());
        // `chemin` absent : le parseur du protocole partagé lève.
        const trame = decoder((await serveur.traiter(encoder(TYPE_LISTER, 8, { rien: 1 })))!);
        expect(trame.type).toBe(TYPE_ECHEC);
        expect(trame.correlation).toBe(8);
        expect(parseEchec(trame.entete).code).toBe('interne');
    });
});

describe('the F2 write verbs', () => {
    it('🔴 a write ALWAYS receives a FAIT or an ECHEC', async () => {
        // Ne rien rendre laisserait la commande en vol côté agent jusqu'à
        // `WRITE_TIMEOUT` — trente secondes pendant lesquelles le fil d'écriture
        // ne pousserait plus rien, et le compteur de dues ne bougerait pas.
        const ecrivain = fauxEcrivain();
        const serveur = createServer(fauxAdaptateur(), () => {}, { ecrivain });
        const trame = decoder(
            (await serveur.traiter(
                encoder(
                    TYPE_WRITE,
                    7,
                    { chemin: 'note.txt', position: 0, longueur: 3, premier: true, dernier: true },
                    new Uint8Array([1, 2, 3]),
                ),
            ))!,
        );
        expect(trame.type).toBe(TYPE_FAIT);
        expect(trame.correlation).toBe(7);
        expect(ecrivain.vus).toEqual(['ecrire note.txt @0 +3 true/true']);
    });

    it('a creation receives a FAIT', async () => {
        const ecrivain = fauxEcrivain();
        const serveur = createServer(fauxAdaptateur(), () => {}, { ecrivain });
        const trame = decoder(
            (await serveur.traiter(encoder(TYPE_CREATE, 8, { chemin: 'dossier', repertoire: true })))!,
        );
        expect(trame.type).toBe(TYPE_FAIT);
        expect(ecrivain.vus).toEqual(['creer dossier true']);
    });

    it('🔴 THE WRITER FAILURE CODE GOES THROUGH, it is not overwritten', async () => {
        // Rendre `interne` pour tout détruirait la cause à l'émission —
        // exactement le défaut de `web/index.js:669`, qui émettait
        // `JSON.stringify(e)` et rendait `"{}"` pour toute `Error`.
        const ecrivain = fauxEcrivain({
            write: async () => {
                throw new FilesError('casse-ambigue', 'homonyme');
            },
        });
        const serveur = createServer(fauxAdaptateur(), () => {}, { ecrivain });
        const trame = decoder(
            (await serveur.traiter(
                encoder(TYPE_WRITE, 9, {
                    chemin: 'a.txt',
                    position: 0,
                    longueur: 0,
                    premier: true,
                    dernier: true,
                }),
            ))!,
        );
        expect(trame.type).toBe(TYPE_ECHEC);
        expect(parseEchec(trame.entete).code).toBe('casse-ambigue');
    });

    it('🔴 without a writer, the write is refused as `protege-en-ecriture`', async () => {
        // PAS `interne` : « ce lecteur est en lecture seule » et « le lecteur
        // est en panne » n'appellent pas le même geste, et c'est tout l'objet
        // de `CodeEchec`.
        const serveur = createServer(fauxAdaptateur());
        const trame = decoder(
            (await serveur.traiter(
                encoder(TYPE_WRITE, 1, {
                    chemin: 'a.txt',
                    position: 0,
                    longueur: 0,
                    premier: true,
                    dernier: true,
                }),
            ))!,
        );
        expect(parseEchec(trame.entete).code).toBe('protege-en-ecriture');
    });

    it('🔴 refuses a frame whose header and payload contradict each other', async () => {
        // Écrire une quantité d'octets que l'émetteur ne croyait pas envoyer
        // est le genre de divergence qu'aucun contrôle en aval ne rattrape :
        // seul un condensat le dirait.
        const ecrivain = fauxEcrivain();
        const serveur = createServer(fauxAdaptateur(), () => {}, { ecrivain });
        const trame = decoder(
            (await serveur.traiter(
                encoder(
                    TYPE_WRITE,
                    2,
                    { chemin: 'a.txt', position: 0, longueur: 99, premier: true, dernier: true },
                    new Uint8Array([1, 2, 3]),
                ),
            ))!,
        );
        expect(trame.type).toBe(TYPE_ECHEC);
        // RIEN ne doit avoir été écrit : le refus vient AVANT l'écrivain.
        expect(ecrivain.vus).toEqual([]);
    });
});

describe('reporting a write failure', () => {
    it('🔴 NAMES the file and the cause to the shell page', async () => {
        // 🔴 Le navigateur est le SEUL à connaître la cause, et il n'a personne
        // à qui la dire : le code traverse bien le fil, mais il n'atteint
        // AUCUNE application Windows — le handle est refermé depuis longtemps.
        // Ce rappel est le chemin le plus court vers la seule personne que cela
        // concerne.
        const vus: Array<[string, string]> = [];
        const ecrivain = fauxEcrivain({
            write: async () => {
                throw new FilesError('disque-plein', 'no space left');
            },
        });
        const serveur = createServer(fauxAdaptateur(), () => {}, {
            ecrivain,
            onEchecEcriture: (chemin, code) => vus.push([chemin, code]),
        });
        await serveur.traiter(
            encoder(TYPE_WRITE, 4, {
                chemin: 'dossier/rapport.docx',
                position: 0,
                longueur: 0,
                premier: true,
                dernier: true,
            }),
        );
        expect(vus).toEqual([['dossier/rapport.docx', 'disque-plein']]);
    });

    it('names NOTHING when the header itself is unreadable', async () => {
        // Deviner un chemin qu'on n'a pas lu serait pire que se taire : la
        // page-shell nommerait un fichier au hasard.
        const vus: unknown[] = [];
        const serveur = createServer(fauxAdaptateur(), () => {}, {
            ecrivain: fauxEcrivain(),
            onEchecEcriture: (...a) => vus.push(a),
        });
        await serveur.traiter(encoder(TYPE_WRITE, 5, { rien: 'at all' }));
        expect(vus).toEqual([]);
    });
});


/** Un mutateur factice : le protocole ne connaît AUCUN système de fichiers. */
function fauxMutateur(surcharge: Partial<Mutateur> = {}): Mutateur & { vus: string[] } {
    const vus: string[] = [];
    return {
        vus,
        async renommer(de, vers, repertoire) {
            vus.push(`renommer ${de} -> ${vers} ${repertoire}`);
            return { parMove: true, octets: 0, entrees: 0 };
        },
        async remove(chemin, repertoire) {
            vus.push(`supprimer ${chemin} ${repertoire}`);
        },
        ...surcharge,
    };
}

describe('the two verbs of F3', () => {
    it('🔴 RENOMMER ALWAYS answers — with FAIT', async () => {
        // Rouge : rendre `null`. La commande resterait en vol côté pont
        // **jusqu'à son expiration**, et l'Explorateur se figerait sur une
        // panne pourtant immédiate. C'est l'invariant que `protocole.ts` énonce
        // en majuscules depuis F1, et que F3 ne relâche PAS : `TYPE_RENOMMER`
        // est une REQUÊTE, pas une annonce.
        const mutateur = fauxMutateur();
        const serveur = createServer(fauxAdaptateur(), () => {}, { mutateur });
        const reponse = await serveur.traiter(
            encoder(TYPE_RENOMMER, 11, { de: 'a.txt', vers: 'b.txt', repertoire: false }),
        );
        expect(reponse).not.toBeNull();
        expect(decoder(reponse!).type).toBe(TYPE_FAIT);
        expect(mutateur.vus).toEqual(['renommer a.txt -> b.txt false']);
    });

    it('🔴 SUPPRIMER ALWAYS answers — with FAIT', async () => {
        const mutateur = fauxMutateur();
        const serveur = createServer(fauxAdaptateur(), () => {}, { mutateur });
        const reponse = await serveur.traiter(
            encoder(TYPE_DELETE, 12, { chemin: 'd', repertoire: true }),
        );
        expect(reponse).not.toBeNull();
        expect(decoder(reponse!).type).toBe(TYPE_FAIT);
        expect(mutateur.vus).toEqual(['supprimer d true']);
    });

    it('🔴 WITHOUT a mutator, it is `protege-en-ecriture` and NOT `interne`', async () => {
        // Un lecteur monté sans mutateur et un lecteur en panne n'appellent pas
        // le même geste — le contre-exemple est l'ancien pont, qui rendait
        // `EPERM` à neuf sites distincts.
        const serveur = createServer(fauxAdaptateur(), () => {});
        const trame = decoder(
            (await serveur.traiter(
                encoder(TYPE_RENOMMER, 13, { de: 'a', vers: 'b', repertoire: false }),
            ))!,
        );
        expect(parseEchec(trame.entete).code).toBe('protege-en-ecriture');
    });

    it('🔴 a RENAME failure names BOTH paths', async () => {
        // « impossible de renommer X » ne dit pas vers quoi, et c'est
        // précisément ce que l'utilisateur doit vérifier.
        const vus: Array<[string, string]> = [];
        const mutateur = fauxMutateur({
            renommer: async () => {
                throw new FilesError('deja-present', 'already there');
            },
        });
        const serveur = createServer(fauxAdaptateur(), () => {}, {
            mutateur,
            onEchecMutation: (quoi, code) => vus.push([quoi, code]),
        });
        const trame = decoder(
            (await serveur.traiter(
                encoder(TYPE_RENOMMER, 14, { de: 'x.txt', vers: 'y.txt', repertoire: false }),
            ))!,
        );
        expect(parseEchec(trame.entete).code).toBe('deja-present');
        expect(vus).toEqual([['x.txt → y.txt', 'deja-present']]);
    });

    it('a REMOVAL failure names the path', async () => {
        const vus: Array<[string, string]> = [];
        const mutateur = fauxMutateur({
            remove: async () => {
                throw new FilesError('repertoire-non-vide', 'not empty');
            },
        });
        const serveur = createServer(fauxAdaptateur(), () => {}, {
            mutateur,
            onEchecMutation: (quoi, code) => vus.push([quoi, code]),
        });
        await serveur.traiter(encoder(TYPE_DELETE, 15, { chemin: 'd', repertoire: true }));
        expect(vus).toEqual([['d', 'repertoire-non-vide']]);
    });

    it('🔵 the COPY fallback is INSTRUMENTED, and `move()` is not', async () => {
        // L'instrumentation que la spec §3.5.1 exige. Elle part par le journal
        // parce que le navigateur est le SEUL à savoir ce qu'il a fait.
        const vus: Array<[string, string, number, number]> = [];
        const parCopie = fauxMutateur({
            renommer: async () => ({ parMove: false, octets: 4096, entrees: 3 }),
        });
        const serveur = createServer(fauxAdaptateur(), () => {}, {
            mutateur: parCopie,
            onRenommagePorCopie: (de, vers, octets, entrees) =>
                vus.push([de, vers, octets, entrees]),
        });
        await serveur.traiter(
            encoder(TYPE_RENOMMER, 16, { de: 'p', vers: 'q', repertoire: true }),
        );
        expect(vus).toEqual([['p', 'q', 4096, 3]]);

        // Et sur la branche `move`, RIEN n'est instrumenté : il n'y a rien à
        // mesurer.
        vus.length = 0;
        const parMove = createServer(fauxAdaptateur(), () => {}, {
            mutateur: fauxMutateur(),
            onRenommagePorCopie: (de, vers, octets, entrees) =>
                vus.push([de, vers, octets, entrees]),
        });
        await parMove.traiter(
            encoder(TYPE_RENOMMER, 17, { de: 'a', vers: 'b', repertoire: false }),
        );
        expect(vus).toEqual([]);
    });
});
