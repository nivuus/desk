import { describe, expect, it, vi } from 'vitest';
import {
    TYPE_ATTRIBUTS,
    TYPE_CREER,
    TYPE_DONNEES,
    TYPE_ECHEC,
    TYPE_ECRIRE,
    TYPE_ENTREES,
    TYPE_FAIT,
    TYPE_RENOMMER,
    TYPE_SUPPRIMER,
    TYPE_LIRE,
    TYPE_LISTER,
    TYPE_META,
    encoder,
} from '../../../proto/ts/fichiers';
import { parseDonnees, parseEchec, parseEntrees, parseMeta } from '../../../proto/ts/fichiers-entetes';
import { decoder } from '../../../proto/ts/fichiers';
import { EchecFichiers, type Adaptateur } from './adaptateur';
import { creerServeur } from './protocole';
import type { Ecrivain } from './ecriture';
import type { Mutateur } from './mutation-service';

/** Un écrivain factice : le protocole ne connaît AUCUN système de fichiers. */
function fauxEcrivain(surcharge: Partial<Ecrivain> = {}): Ecrivain & { vus: string[] } {
    const vus: string[] = [];
    return {
        vus,
        ecrire: async (chemin, position, octets, premier, dernier) => {
            vus.push(`ecrire ${chemin} @${position} +${octets.length} ${premier}/${dernier}`);
        },
        creer: async (chemin, repertoire) => {
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
        // 🔴 **LE NOM CANONIQUE TRAVERSE LE FIL.** Sans lui, le substitut
        // serait créé sous le nom que l'application a TAPÉ, et non sous celui
        // qui existe sur le poste local.
        expect(parseMeta(trame.entete).nom).toBe('Nom Stocké.txt');
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

describe('les verbes d’écriture de F2', () => {
    it('🔴 une écriture reçoit TOUJOURS un FAIT ou un ECHEC', async () => {
        // Ne rien rendre laisserait la commande en vol côté agent jusqu'à
        // `DELAI_ECRIRE` — trente secondes pendant lesquelles le fil d'écriture
        // ne pousserait plus rien, et le compteur de dues ne bougerait pas.
        const ecrivain = fauxEcrivain();
        const serveur = creerServeur(fauxAdaptateur(), () => {}, { ecrivain });
        const trame = decoder(
            (await serveur.traiter(
                encoder(
                    TYPE_ECRIRE,
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

    it('une création reçoit un FAIT', async () => {
        const ecrivain = fauxEcrivain();
        const serveur = creerServeur(fauxAdaptateur(), () => {}, { ecrivain });
        const trame = decoder(
            (await serveur.traiter(encoder(TYPE_CREER, 8, { chemin: 'dossier', repertoire: true })))!,
        );
        expect(trame.type).toBe(TYPE_FAIT);
        expect(ecrivain.vus).toEqual(['creer dossier true']);
    });

    it('🔴 LE CODE D’ÉCHEC DE L’ÉCRIVAIN TRAVERSE, il n’est pas écrasé', async () => {
        // Rendre `interne` pour tout détruirait la cause à l'émission —
        // exactement le défaut de `web/index.js:669`, qui émettait
        // `JSON.stringify(e)` et rendait `"{}"` pour toute `Error`.
        const ecrivain = fauxEcrivain({
            ecrire: async () => {
                throw new EchecFichiers('casse-ambigue', 'homonyme');
            },
        });
        const serveur = creerServeur(fauxAdaptateur(), () => {}, { ecrivain });
        const trame = decoder(
            (await serveur.traiter(
                encoder(TYPE_ECRIRE, 9, {
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

    it('🔴 sans écrivain, l’écriture est refusée en `protege-en-ecriture`', async () => {
        // PAS `interne` : « ce lecteur est en lecture seule » et « le lecteur
        // est en panne » n'appellent pas le même geste, et c'est tout l'objet
        // de `CodeEchec`.
        const serveur = creerServeur(fauxAdaptateur());
        const trame = decoder(
            (await serveur.traiter(
                encoder(TYPE_ECRIRE, 1, {
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

    it('🔴 refuse une trame dont l’en-tête et la charge se contredisent', async () => {
        // Écrire une quantité d'octets que l'émetteur ne croyait pas envoyer
        // est le genre de divergence qu'aucun contrôle en aval ne rattrape :
        // seul un condensat le dirait.
        const ecrivain = fauxEcrivain();
        const serveur = creerServeur(fauxAdaptateur(), () => {}, { ecrivain });
        const trame = decoder(
            (await serveur.traiter(
                encoder(
                    TYPE_ECRIRE,
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

describe('la dénonciation d’un échec d’écriture', () => {
    it('🔴 NOMME le fichier et la cause à la page-shell', async () => {
        // 🔴 Le navigateur est le SEUL à connaître la cause, et il n'a personne
        // à qui la dire : le code traverse bien le fil, mais il n'atteint
        // AUCUNE application Windows — le handle est refermé depuis longtemps.
        // Ce rappel est le chemin le plus court vers la seule personne que cela
        // concerne.
        const vus: Array<[string, string]> = [];
        const ecrivain = fauxEcrivain({
            ecrire: async () => {
                throw new EchecFichiers('disque-plein', 'plus de place');
            },
        });
        const serveur = creerServeur(fauxAdaptateur(), () => {}, {
            ecrivain,
            onEchecEcriture: (chemin, code) => vus.push([chemin, code]),
        });
        await serveur.traiter(
            encoder(TYPE_ECRIRE, 4, {
                chemin: 'dossier/rapport.docx',
                position: 0,
                longueur: 0,
                premier: true,
                dernier: true,
            }),
        );
        expect(vus).toEqual([['dossier/rapport.docx', 'disque-plein']]);
    });

    it('ne nomme RIEN quand l’en-tête lui-même est illisible', async () => {
        // Deviner un chemin qu'on n'a pas lu serait pire que se taire : la
        // page-shell nommerait un fichier au hasard.
        const vus: unknown[] = [];
        const serveur = creerServeur(fauxAdaptateur(), () => {}, {
            ecrivain: fauxEcrivain(),
            onEchecEcriture: (...a) => vus.push(a),
        });
        await serveur.traiter(encoder(TYPE_ECRIRE, 5, { rien: 'du tout' }));
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
        async supprimer(chemin, repertoire) {
            vus.push(`supprimer ${chemin} ${repertoire}`);
        },
        ...surcharge,
    };
}

describe('les deux verbes de F3', () => {
    it('🔴 RENOMMER répond TOUJOURS — par FAIT', async () => {
        // Rouge : rendre `null`. La commande resterait en vol côté pont
        // **jusqu'à son expiration**, et l'Explorateur se figerait sur une
        // panne pourtant immédiate. C'est l'invariant que `protocole.ts` énonce
        // en majuscules depuis F1, et que F3 ne relâche PAS : `TYPE_RENOMMER`
        // est une REQUÊTE, pas une annonce.
        const mutateur = fauxMutateur();
        const serveur = creerServeur(fauxAdaptateur(), () => {}, { mutateur });
        const reponse = await serveur.traiter(
            encoder(TYPE_RENOMMER, 11, { de: 'a.txt', vers: 'b.txt', repertoire: false }),
        );
        expect(reponse).not.toBeNull();
        expect(decoder(reponse!).type).toBe(TYPE_FAIT);
        expect(mutateur.vus).toEqual(['renommer a.txt -> b.txt false']);
    });

    it('🔴 SUPPRIMER répond TOUJOURS — par FAIT', async () => {
        const mutateur = fauxMutateur();
        const serveur = creerServeur(fauxAdaptateur(), () => {}, { mutateur });
        const reponse = await serveur.traiter(
            encoder(TYPE_SUPPRIMER, 12, { chemin: 'd', repertoire: true }),
        );
        expect(reponse).not.toBeNull();
        expect(decoder(reponse!).type).toBe(TYPE_FAIT);
        expect(mutateur.vus).toEqual(['supprimer d true']);
    });

    it('🔴 SANS mutateur, c’est `protege-en-ecriture` et NON `interne`', async () => {
        // Un lecteur monté sans mutateur et un lecteur en panne n'appellent pas
        // le même geste — le contre-exemple est l'ancien pont, qui rendait
        // `EPERM` à neuf sites distincts.
        const serveur = creerServeur(fauxAdaptateur(), () => {});
        const trame = decoder(
            (await serveur.traiter(
                encoder(TYPE_RENOMMER, 13, { de: 'a', vers: 'b', repertoire: false }),
            ))!,
        );
        expect(parseEchec(trame.entete).code).toBe('protege-en-ecriture');
    });

    it('🔴 un échec de RENOMMAGE nomme LES DEUX chemins', async () => {
        // « impossible de renommer X » ne dit pas vers quoi, et c'est
        // précisément ce que l'utilisateur doit vérifier.
        const vus: Array<[string, string]> = [];
        const mutateur = fauxMutateur({
            renommer: async () => {
                throw new EchecFichiers('deja-present', 'déjà là');
            },
        });
        const serveur = creerServeur(fauxAdaptateur(), () => {}, {
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

    it('un échec de SUPPRESSION nomme le chemin', async () => {
        const vus: Array<[string, string]> = [];
        const mutateur = fauxMutateur({
            supprimer: async () => {
                throw new EchecFichiers('repertoire-non-vide', 'pas vide');
            },
        });
        const serveur = creerServeur(fauxAdaptateur(), () => {}, {
            mutateur,
            onEchecMutation: (quoi, code) => vus.push([quoi, code]),
        });
        await serveur.traiter(encoder(TYPE_SUPPRIMER, 15, { chemin: 'd', repertoire: true }));
        expect(vus).toEqual([['d', 'repertoire-non-vide']]);
    });

    it('🔵 le repli de COPIE est INSTRUMENTÉ, et `move()` ne l’est pas', async () => {
        // L'instrumentation que la spec §3.5.1 exige. Elle part par le journal
        // parce que le navigateur est le SEUL à savoir ce qu'il a fait.
        const vus: Array<[string, string, number, number]> = [];
        const parCopie = fauxMutateur({
            renommer: async () => ({ parMove: false, octets: 4096, entrees: 3 }),
        });
        const serveur = creerServeur(fauxAdaptateur(), () => {}, {
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
        const parMove = creerServeur(fauxAdaptateur(), () => {}, {
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
