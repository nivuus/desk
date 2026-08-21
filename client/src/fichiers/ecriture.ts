// L'écrivain : il pose les octets du pont dans le répertoire du poste local.
// **PUR** — ni DOM, ni WebRTC, ni trame binaire ; la racine lui est INJECTÉE,
// comme à `adaptateur.ts`, et c'est ce qui le rend testable sous le Node de
// Vitest, qui n'a aucune File System Access API.
//
// 🔴 CE QUE F2 FAIT DU DÉFAUT DE CASSE, DEVENU UNE PERTE DE DONNÉES
//
// F1 a mesuré, TROIS exécutions sur trois : avec `Casse.txt` sur le poste
// local, `casse.txt` ET `CASSE.TXT` rendent le CONTENU de `Casse.txt`, sans
// erreur. En LECTURE, c'est un mauvais fichier rendu. EN ÉCRITURE, C'EST UN
// FICHIER ÉCRASÉ.
//
// ⚠️ ET LE MÉCANISME EST PIRE DU CÔTÉ NAVIGATEUR QUE DU CÔTÉ VM :
//
//   - Côté VM, l'écart de casse est ABSORBÉ par NTFS sur un fichier DÉJÀ
//     hydraté : le pont reçoit alors la casse RÉELLE de l'entrée locale, et la
//     poussée est juste. Ce chemin-là n'est pas dangereux.
//   - Côté navigateur, `getFileHandle(nom, { create: true })` s'exécute sur le
//     système de fichiers du POSTE LOCAL, insensible à la casse sur Windows et
//     sur macOS par défaut. Une poussée vers `CASSE.TXT` y ouvre donc
//     `Casse.txt` et L'ÉCRASE. C'est LÀ que la perte se produit.
//   - Le cas qui l'atteint : un fichier JAMAIS HYDRATÉ, créé dans la VM avec
//     une casse différente d'une entrée locale existante. NTFS n'a rien à
//     résoudre, la notification porte la casse de la VM, et la poussée écrase
//     l'homonyme local.
//
// LA RÈGLE, et elle est PURE : avant toute écriture ou création, l'écrivain
// énumère le répertoire parent et cherche le nom demandé EXACTEMENT.
//
//   - nom exact trouvé                  → on écrit dedans ;
//   - aucun nom, aucun homonyme         → on crée ;
//   - un homonyme à la casse près SEUL  → `casse-ambigue`, ON N'ÉCRIT RIEN.
//
// ⚠️ COÛT, ÉCRIT : une énumération du répertoire parent PAR ÉCRITURE. Sur un
// répertoire à mille entrées, c'est mille noms parcourus — moins cher que le
// listage, qui ouvre chaque fichier (`adaptateur.ts`), mais non nul. Mesurable
// en F4, pas ici. L'alternative — la table de correspondance alimentée par
// l'énumération, que F1 lègue à F3 — la supprimerait ; F2 ne la construit PAS,
// parce qu'un cache que rien n'invalide est le défaut de l'ancien pont
// (`src/file.js`, cache SANS TTL) et que `Rafraichir` est un livrable de F5.
//
// ⚠️ CE QUE F2 NE FAIT PAS : il ne corrige PAS la casse en LECTURE. `casse.txt`
// continuera de rendre le contenu de `Casse.txt`. La garde ne protège que le
// sens ÉCRITURE, le seul où l'erreur DÉTRUIT quelque chose.
//
// ⚠️ ET ELLE NE VOIT PAS LA NORMALISATION UNICODE. macOS stocke ses noms en
// NFD, Windows en NFC : `été.txt` peut y exister sous deux suites d'unités de
// code différentes, que `===` distingue et que l'utilisateur ne distingue pas.
// La garde créerait alors un DOUBLON au lieu d'écraser — moins grave que la
// perte, mais faux. NON TRAITÉ, déclaré ; c'est le canonicaliseur de F3.

import {
    EchecFichiers,
    classer,
    type PoigneeBase,
    type PoigneeFichier,
    type PoigneeRepertoire,
} from './adaptateur';

/* ── LES POIGNÉES INSCRIPTIBLES ───────────────────────────────────────────
   Un SOUS-ENSEMBLE STRUCTUREL de plus, décrit par ce dont on se sert. La vraie
   `FileSystemDirectoryHandle` les satisfait sans conversion — `canal.ts` le
   vérifie à la compilation, exactement comme pour la lecture en F1. */

/**
 * Les octets qu'un flux du navigateur accepte.
 *
 * 🔴 **`Uint8Array<ArrayBuffer>` ET NON `Uint8Array` NU, ET C'EST LE CONTRÔLE
 * STRUCTUREL DE `canal.ts` QUI L'A EXIGÉ.** `Uint8Array` seul vaut
 * `Uint8Array<ArrayBufferLike>`, donc **`SharedArrayBuffer` compris** — et la
 * vraie `FileSystemWritableFileStream.write` n'accepte qu'un `BufferSource`,
 * c'est-à-dire un `ArrayBufferView<ArrayBuffer>`. La vraie poignée ne
 * satisfaisait donc **PAS** `RacineInscriptible`, et personne ne l'avait vu :
 * le « contrôle de compatibilité structurelle » de F2 était un `as` vers un
 * sous-type, qui asserte au lieu de vérifier.
 *
 * ⚠️ **Ce n'est PAS une incompatibilité d'exécution** — un `Uint8Array` adossé
 * à un `ArrayBuffer` ordinaire est un `BufferSource` parfaitement valide. C'est
 * le TYPE qui mentait, en promettant d'accepter des vues sur mémoire partagée
 * que ce module ne produit ni ne reçoit jamais. Le resserrer, c'est le rendre
 * vrai.
 */
export type OctetsInscriptibles = Uint8Array<ArrayBuffer>;

/** Le flux d'écriture rendu par `createWritable()`. */
export interface FluxInscriptible {
    write(donnees: {
        type: 'write';
        position: number;
        data: OctetsInscriptibles;
    }): Promise<void>;
    /**
     * 🔵 LA COMMITTAISON EST ICI, ET NULLE PART AILLEURS. `createWritable()`
     * écrit dans un fichier d'échange et ne commet qu'au `close()` : une
     * poussée interrompue en plein vol laisse donc le fichier local INCHANGÉ.
     *
     * ⚠️ C'est excellent — pas de fichier à moitié écrit chez l'utilisateur —
     * et cela a un revers : une interruption ne rend RIEN, pas même le début.
     * INFÉRENCE de la spécification de la File System Access API, NON MESURÉE
     * ici ; le critère ⑤ de la recette est écrit pour l'éprouver.
     */
    close(): Promise<void>;
}

export interface PoigneeFichierInscriptible extends PoigneeFichier {
    createWritable(options?: { keepExistingData?: boolean }): Promise<FluxInscriptible>;
}

export interface RacineInscriptible extends PoigneeRepertoire {
    getDirectoryHandle(
        nom: string,
        options?: { create?: boolean },
    ): Promise<RacineInscriptible>;
    getFileHandle(
        nom: string,
        options?: { create?: boolean },
    ): Promise<PoigneeFichierInscriptible>;
    values(): AsyncIterable<PoigneeBase>;
}

export interface Ecrivain {
    ecrire(
        chemin: string,
        position: number,
        octets: Uint8Array,
        premier: boolean,
        dernier: boolean,
    ): Promise<void>;
    creer(chemin: string, repertoire: boolean): Promise<void>;
    /** Ferme tout flux resté ouvert. Appelé à la fermeture du canal. */
    abandonner(): void;
}

/** `"a/b/c"` → `["a","b","c"]`, `""` → `[]`. */
function composants(chemin: string): string[] {
    return chemin.split('/').filter((c) => c.length > 0);
}

export function creerEcrivain(racine: RacineInscriptible): Ecrivain {
    /**
     * Les flux ouverts, UN PAR CHEMIN.
     *
     * ⚠️ Ouvrir un flux par MORCEAU rendrait `keepExistingData` obligatoire —
     * donc le défaut de l'ancien pont, qui laissait sa queue d'octets à un
     * fichier réécrit plus court (spec §12).
     */
    const flux = new Map<string, FluxInscriptible>();

    /** Descend les `jusqua` premiers composants, en les CRÉANT au besoin. */
    async function descendre(parts: string[], jusqua: number): Promise<RacineInscriptible> {
        let ici = racine;
        for (let i = 0; i < jusqua; i += 1) {
            try {
                ici = await ici.getDirectoryHandle(parts[i], { create: true });
            } catch (e) {
                throw classer(e, 'chemin-introuvable');
            }
        }
        return ici;
    }

    /**
     * 🔴 LA GARDE DE CASSE. Rend le nom à employer, ou LÈVE.
     *
     * Elle rend le nom EXACT quand il existe, le nom demandé quand rien n'y
     * ressemble, et lève `casse-ambigue` quand un homonyme ne diffère que par
     * la casse. Dans ce dernier cas, ON N'ÉCRIT RIEN.
     */
    async function nomSur(parent: RacineInscriptible, nom: string): Promise<string> {
        const homonymes: string[] = [];
        try {
            for await (const enfant of parent.values()) {
                if (enfant.name === nom) return nom;
                if (enfant.name.toLowerCase() === nom.toLowerCase()) homonymes.push(enfant.name);
            }
        } catch (e) {
            throw classer(e, 'chemin-introuvable');
        }
        if (homonymes.length > 0) {
            // ⚠️ Le message NOMME les deux, parce que c'est tout ce que
            // l'utilisateur pourra faire : renommer l'un des deux. Le CODE, lui,
            // traverse le fil ; le message reste dans la console du navigateur
            // et dans la page-shell.
            throw new EchecFichiers(
                'casse-ambigue',
                `« ${nom} » ne diffère de « ${homonymes.join(' », « ')} » que par la casse : ` +
                    `écrire écraserait le mauvais fichier, rien n'a été écrit`,
            );
        }
        return nom;
    }

    async function ouvrir(chemin: string): Promise<FluxInscriptible> {
        const parts = composants(chemin);
        if (parts.length === 0) {
            throw new EchecFichiers('introuvable', 'la racine n’est pas un fichier');
        }
        const parent = await descendre(parts, parts.length - 1);
        const nom = await nomSur(parent, parts[parts.length - 1]);
        try {
            const poignee = await parent.getFileHandle(nom, { create: true });
            // 🔴 SANS `keepExistingData`, ET C'EST LE DÉFAUT DE L'ANCIEN PONT
            // QU'ON REFUSE DE REJOUER : il employait `keepExistingData: true`
            // sans `truncate`, si bien qu'UN FICHIER RÉÉCRIT PLUS COURT
            // CONSERVAIT SA QUEUE D'OCTETS (spec §12). Le fichier local aurait
            // alors un contenu que la VM n'a jamais eu.
            return await poignee.createWritable();
        } catch (e) {
            throw classer(e, 'introuvable');
        }
    }

    return {
        async ecrire(chemin, position, octets, premier, dernier) {
            if (premier) {
                // ⚠️ Un `premier` sur un chemin DÉJÀ ouvert ne peut venir que
                // d'un rejeu dont le flux précédent n'a jamais été fermé — une
                // poussée interrompue, puis relancée. On ferme l'ancien plutôt
                // que d'en laisser DEUX ouverts sur le même fichier : le
                // second `close()` gagnerait, et le premier laisserait son
                // fichier d'échange derrière lui.
                const ancien = flux.get(chemin);
                if (ancien !== undefined) {
                    flux.delete(chemin);
                    await ancien.close().catch(() => {});
                }
                flux.set(chemin, await ouvrir(chemin));
            }
            const ouvert = flux.get(chemin);
            if (ouvert === undefined) {
                // ⚠️ Un morceau qui n'est PAS le premier sur un chemin sans flux
                // : le pont et le navigateur ont divergé. Ouvrir ici écrirait
                // un fichier tronqué à ce morceau-ci, ce qui est PIRE que de
                // refuser — la troncature serait silencieuse.
                throw new EchecFichiers(
                    'interne',
                    `morceau non initial sur « ${chemin} » sans flux ouvert`,
                );
            }
            try {
                // ⚠️ **LE RESSERREMENT DE TYPE SE FAIT ICI, ET UNE SEULE FOIS.**
                // `proto/ts/fichiers` rend un `Uint8Array` NU — donc
                // `Uint8Array<ArrayBufferLike>`, `SharedArrayBuffer` compris —
                // parce que c'est ce que le décodeur de trame produit. Rien, à
                // l'exécution, ne peut lui donner une vue sur mémoire
                // partagée : la trame vient d'un `ArrayBuffer` de
                // `RTCDataChannel`. **La copie est donc gratuite en pratique et
                // honnête en type** : elle dit ce que le module reçoit
                // réellement, plutôt que de l'asserter.
                //
                // 🔵 C'est le contrôle structurel de `canal.ts` qui a exigé ce
                // resserrement — voir [`OctetsInscriptibles`].
                const donnees: OctetsInscriptibles = new Uint8Array(octets);
                await ouvert.write({ type: 'write', position, data: donnees });
            } catch (e) {
                // Le flux est perdu : le retirer, sinon le morceau suivant
                // écrirait dans un flux mort et l'échec changerait de cause.
                flux.delete(chemin);
                throw classer(e, 'introuvable');
            }
            if (dernier) {
                flux.delete(chemin);
                try {
                    // 🔵 LA COMMITTAISON.
                    await ouvert.close();
                } catch (e) {
                    throw classer(e, 'introuvable');
                }
            }
        },

        async creer(chemin, repertoire) {
            const parts = composants(chemin);
            if (parts.length === 0) {
                throw new EchecFichiers('deja-present', 'la racine existe déjà');
            }
            const parent = await descendre(parts, parts.length - 1);
            const nom = await nomSur(parent, parts[parts.length - 1]);
            try {
                if (repertoire) {
                    await parent.getDirectoryHandle(nom, { create: true });
                } else {
                    // ⚠️ CRÉER, ET RIEN DE PLUS : aucun `createWritable()` ici.
                    // En ouvrir un TRONQUERAIT un fichier local existant, alors
                    // qu'une création est sans effet sur ce qui est déjà là.
                    await parent.getFileHandle(nom, { create: true });
                }
            } catch (e) {
                throw classer(e, 'introuvable');
            }
        },

        abandonner() {
            // ⚠️ `close()` ET NON `abort()` : un flux abandonné sans être fermé
            // laisse son fichier d'échange derrière lui. Rien n'est attendu —
            // cette fonction est appelée depuis la fermeture du canal, qui est
            // synchrone.
            for (const [, ouvert] of flux) void ouvert.close().catch(() => {});
            flux.clear();
        },
    };
}
