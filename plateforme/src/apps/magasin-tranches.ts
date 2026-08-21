// Le magasin des TRANCHES d'un téléversement : des octets sur DISQUE, **un
// fichier par tranche**, sous un répertoire par téléversement —
// `<racine>/<id-du-televersement>/<n>`.
//
// 🔴 LES TRANCHES NE SONT JAMAIS ASSEMBLÉES : IL N'EXISTE AUCUN FICHIER
// RECONSTITUÉ, ni au scellement ni ailleurs. Trois raisons, dans l'ordre de
// leur poids :
//
//   1. LE DISQUE DOUBLERAIT. Un installeur de 800 Mo tiendrait 1,6 Go le temps
//      de l'assemblage, et une file de dépôts simultanés ferait de ce doublement
//      la NORME plutôt que la pointe. Le service serait plein pour une copie
//      dont personne n'a besoin — l'agent lit un flux, il ne cherche pas un
//      fichier.
//   2. UN ASSEMBLÉ SERAIT UNE SECONDE SOURCE DE VÉRITÉ. Le jour où il
//      divergerait de ses tranches — écriture interrompue, tranche réécrite
//      après coup — rien ici ne saurait dire lequel des deux croire, et le
//      `sha256` du contrat n'accuserait que le dernier maillon. Le magasin n'a
//      qu'un état, et c'est ce qu'il y a sur le disque.
//   3. LE SCELLEMENT DOIT RESTER UNE DÉCISION, PAS UNE RECOPIE. Assembler en
//      ferait une opération en O(taille), qui peut échouer à mi-course et
//      laisser un demi-fichier ; sceller, c'est écrire une date.
//
// 🔴 POURQUOI LE DISQUE ET NON LA BASE : le raisonnement entier vit en tête de
// `icones.ts` et n'est pas recopié ici — un blob ne traverse pas la double
// passe sans mentir, `SERIAL` à l'envers. Les deux répertoires sont frères, et
// leurs deux variables d'environnement le sont aussi.
//
// ⚠️ **CE MAGASIN NE JUGE DE RIEN.** Il ne dit pas si un découpage est complet,
// ni si une tranche a la bonne taille. Cette règle-là est PURE et PARTAGÉE avec
// le navigateur (`proto/ts/tranches.ts`) : deux arithmétiques indépendantes
// divergeraient un jour, et le symptôme serait un scellement qui refuse sans
// qu'on sache lequel des deux bouts a tort. Ici on écrit, on liste, on relit.

import { createReadStream, createWriteStream, mkdirSync, openSync, readdirSync, rmSync, renameSync, statSync } from 'node:fs';
import { join } from 'node:path';
import { Readable } from 'node:stream';
import { pipeline } from 'node:stream/promises';
import type { Tranche } from '../../../proto/ts/tranches';

/// 🔴 L'IDENTIFIANT D'UN TÉLÉVERSEMENT DEVIENT UN NOM DE RÉPERTOIRE, ET IL
/// VIENT DU RÉSEAU. `/televersement/..%2f..%2fetc/tranche/0` doit être refusé,
/// jamais assaini : assainir en silence ferait écrire quelque part, et
/// personne ne saurait où.
///
/// La forme exigée est celle que `depot/televersement.ts::creer` produit —
/// `randomUUID()`, donc un UUID EN MINUSCULES. Le couplage est délibéré et
/// nommé : si ce format changeait un jour, la garde refuserait BRUYAMMENT tous
/// les téléversements plutôt que d'ouvrir un chemin.
///
/// ⚠️ MINUSCULES SEULEMENT, comme `empreinteValide` et pour la même raison :
/// sur un système de fichiers insensible à la casse, deux identifiants
/// distincts désigneraient le même répertoire.
export function identifiantValide(s: string): boolean {
    return /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/.test(s);
}

/// 🔴 LE NOM DE FICHIER D'UNE TRANCHE EST LE NOMBRE `n` VALIDÉ, JAMAIS UN
/// SEGMENT D'URL RECOPIÉ. Un rang est un entier SÛR positif ou nul — la base
/// zéro est celle de `proto/ts/tranches.ts`, où elle rend la position dans le
/// fichier calculable sans table.
///
/// ⚠️ `Number.isSafeInteger` N'EST PAS DE LA COQUETTERIE, et il fait DEUX
/// choses ici. Au-delà de 2^53 l'arithmétique du plan cesserait d'être exacte
/// — c'est déjà écrit dans `proto/ts/tranches.ts` — ET `String(n)` cesserait
/// d'être une suite de chiffres : `String(1e21)` vaut `'1e+21'`. Sous cette
/// garde, le nom produit est TOUJOURS `/^\d+$/`, ce qui est exactement ce que
/// `lister` reconnaît.
export function rangValide(n: number): boolean {
    return Number.isSafeInteger(n) && n >= 0;
}

/// Le résultat d'un dépôt de tranche.
///
/// 🔴 LA FRONTIÈRE EST CELLE DE `proto/ts/tranches.ts` : ce qui est un DÉFAUT
/// DE PROGRAMME lève, ce qui est une DONNÉE DE FIL devient un résultat. Un
/// identifiant ou un rang mal formés lèvent — la route les a déjà refusés avec
/// `identifiantValide`/`rangValide`, et un appel qui arrive ici avec une
/// mauvaise valeur est un câblage fautif, pas un déposant maladroit. Le
/// dépassement de plafond, lui, est le comportement NORMAL d'un pair qui envoie
/// trop : il doit se traduire en refus HTTP sans qu'un `catch` ait à deviner le
/// code d'après un message d'erreur.
export type ResultatEcriture =
    | { ok: true; octets: number }
    | { ok: false; motif: 'plafond-depasse'; plafond: number };

export interface MagasinTranches {
    racine: string;
    /// Dépose une tranche EN FLUX, sous un plafond dur d'octets.
    ecrire(id: string, n: number, flux: AsyncIterable<Uint8Array>, plafondOctets: number): Promise<ResultatEcriture>;
    /// Les tranches réellement présentes sur le disque, avec leurs tailles.
    lister(id: string): Tranche[];
    /// Un flux qui concatène les rangs demandés, dans l'ordre donné.
    concatener(id: string, rangs: readonly number[]): Readable;
    /// Retire tout un téléversement.
    supprimer(id: string): void;
}

/// Ouvre — ou crée — la racine des téléversements, et JOURNALISE le chemin.
///
/// ⚠️ LA LIGNE DE JOURNAL N'EST PAS DÉCORATIVE, et c'est le même argument que
/// pour le magasin d'icônes : `PLATEFORME_TELEVERSEMENTS` est facultative, donc
/// un opérateur peut se tromper de répertoire sans que rien ne casse. Ici, la
/// conséquence est même MOINS réparable que pour les icônes — un téléversement
/// perdu ne se reconstruit pas tout seul, il faut que l'utilisateur redépose.
/// Le chemin retenu doit se lire.
export function ouvrirMagasinTranches(
    racine: string,
    journaliser: (chemin: string) => void,
): MagasinTranches {
    mkdirSync(racine, { recursive: true });
    journaliser(racine);

    const repertoireDe = (id: string): string => {
        if (!identifiantValide(id)) {
            throw new Error(`identifiant de téléversement invalide : ${JSON.stringify(id)}`);
        }
        return join(racine, id);
    };

    const cheminDe = (id: string, n: number): string => {
        const rep = repertoireDe(id);
        if (!rangValide(n)) {
            throw new Error(`rang de tranche invalide : ${JSON.stringify(n)}`);
        }
        return join(rep, String(n));
    };

    return {
        racine,

        /// 🔴 EN FLUX, JAMAIS EN ACCUMULANT EN MÉMOIRE. Une tranche est de
        /// l'ordre de plusieurs mégaoctets et N dépôts peuvent courir de front :
        /// un `Buffer.concat` ferait du service une bombe mémoire pilotée par
        /// ses clients.
        ///
        /// 🔴 LE PLAFOND EST DUR, ET IL COUPE LA SOURCE. Au franchissement on
        /// LÈVE dans le milieu du `pipeline`, ce qui détruit le flux entrant :
        /// on cesse de LIRE le corps de la requête plutôt que de le drainer
        /// pour le jeter. Un plafond qui laisserait couler 800 Mo avant de
        /// refuser n'en serait pas un.
        ///
        /// ⚠️ LE PLAFOND BORNE LE DISQUE, IL NE JUGE PAS LE DÉCOUPAGE. Une
        /// tranche plus COURTE que prévue passe ici sans un mot : c'est
        /// `proto/ts/tranches.ts::verdict` qui la déclarera `incoherentes` au
        /// scellement, et ce module ne connaît pas le contrat.
        ///
        /// 🔴 L'ÉCRITURE EST ATOMIQUE — fichier temporaire, puis `rename` —, et
        /// l'enjeu est PLUS LOURD ICI QUE POUR LES ICÔNES. Un dépôt interrompu
        /// laisserait sinon une tranche TRONQUÉE sous son nom définitif ; la
        /// reprise la verrait présente, `verdict` la dirait `incoherentes`, et
        /// une incohérence ne se répare PAS en redemandant — elle fait échouer
        /// le téléversement entier. Une coupure réseau empoisonnerait donc un
        /// dépôt de 800 Mo sans qu'aucune trace ne le dise.
        async ecrire(
            id: string,
            n: number,
            flux: AsyncIterable<Uint8Array>,
            plafondOctets: number,
        ): Promise<ResultatEcriture> {
            const cible = cheminDe(id, n);
            mkdirSync(join(racine, id), { recursive: true });

            // Le suffixe aléatoire évite que deux dépôts concurrents du même
            // rang n'écrivent le même temporaire — même parade qu'`icones.ts`.
            const provisoire = `${cible}.${process.pid}.${Math.random().toString(36).slice(2)}.part`;
            // ⚠️ OUVERT SYNCHRONEMENT, ET C'EST LE CORRECTIF DE LA COURSE
            // décrite dans le `catch` ci-dessous : à partir d'ici le fichier
            // EXISTE, donc le `rmSync` du chemin d'erreur ne peut plus le
            // manquer. `createWriteStream` reçoit le descripteur et non le
            // chemin ; il le fermera lui-même (`autoClose`).
            const fd = openSync(provisoire, 'w');
            let octets = 0;
            let depasse = false;
            try {
                await pipeline(
                    flux,
                    async function* borner(source: AsyncIterable<Uint8Array>) {
                        for await (const morceau of source) {
                            octets += morceau.byteLength;
                            if (octets > plafondOctets) {
                                depasse = true;
                                throw new Error(
                                    `tranche ${n} au-delà du plafond de ${plafondOctets} octets`,
                                );
                            }
                            yield morceau;
                        }
                    },
                    createWriteStream('', { fd, autoClose: true }),
                );
            } catch (erreur) {
                // 🔴 LE FICHIER PARTIEL EST SUPPRIMÉ, quelle que soit la cause :
                // dépassement, coupure, disque plein. Un `.part` abandonné
                // n'est jamais compté comme une tranche (voir `lister`), mais
                // il occuperait le disque jusqu'à la purge.
                //
                // 🔴 ET CE `rmSync` A ÉTÉ INEFFICACE UNE FOIS SUR DIX — MESURÉ,
                // PAS SUPPOSÉ : une sonde directe sur `ecrire`, hors HTTP, a
                // relevé **42 répertoires non vides sur 400 dépassements**,
                // chacun portant un `.part`. La cause était une COURSE, et non
                // un chemin d'erreur oublié : `createWriteStream(chemin)` ouvre
                // le fichier de façon ASYNCHRONE. Sur un dépassement, notre
                // générateur lève AVANT que l'`open(2)` n'ait abouti ;
                // `rmSync` courait alors sur un fichier qui n'existait pas
                // encore — `{ force: true }` avalant le `ENOENT` en silence —
                // et l'ouverture le créait juste après.
                //
                // ✅ LE REMÈDE N'EST PAS UN RÉESSAI MAIS UNE SUPPRESSION DE LA
                // COURSE : le descripteur est ouvert par `openSync` AVANT le
                // `pipeline`, si bien que l'inode existe déjà quand le
                // `pipeline` démarre. Il n'y a donc plus d'instant où le
                // fichier soit à la fois « en cours de création » et
                // supprimable. Un réessai temporisé aurait réduit la fenêtre
                // sans la fermer, et aurait rendu le défaut intermittent au
                // lieu de le supprimer.
                //
                // ⚠️ Ce n'était PAS un trou de protocole — `lister` ignore les
                // noms non numériques, donc aucune fausse tranche n'a jamais
                // été comptée et le scellement n'en voyait rien. C'était une
                // FUITE DE DISQUE, sur un service qui accepte 4 Gio.
                rmSync(provisoire, { force: true });
                if (depasse) return { ok: false, motif: 'plafond-depasse', plafond: plafondOctets };
                throw erreur;
            }
            renameSync(provisoire, cible);
            return { ok: true, octets };
        },

        /// 🔴 LA REPRISE EST UN LISTAGE DE RÉPERTOIRE, JAMAIS UNE COMPTABILITÉ.
        /// Une table `tranches_presentes` divergerait du disque le jour où un
        /// fichier serait perdu — et c'est PRÉCISÉMENT le jour où l'on a besoin
        /// de le savoir. C'est ce qui rend le magasin auto-reconstructible :
        /// tout ce qui manque est redemandé, et le déposant recomplète.
        ///
        /// ⚠️ SEULS LES NOMS PUREMENT NUMÉRIQUES SONT DES TRANCHES. Un `.part`
        /// laissé par un dépôt mort n'en est pas une, et le compter ferait
        /// paraître complète une tranche qui n'a jamais fini de s'écrire.
        ///
        /// ⚠️ LA LISTE EST TRIÉE, pour la raison de `proto/ts/tranches.ts` : un
        /// relevé qui dépendrait de l'ordre de `readdir` ne serait comparable
        /// ni d'une exécution à l'autre, ni d'un système de fichiers à l'autre.
        ///
        /// ⚠️ UN RÉPERTOIRE ABSENT REND UNE LISTE VIDE, jamais une erreur : un
        /// téléversement déclaré dont aucune tranche n'est encore arrivée est
        /// l'état NORMAL du premier dépôt, et `verdict` en dira `manquantes`.
        lister(id: string): Tranche[] {
            const rep = repertoireDe(id);
            let noms: string[];
            try {
                noms = readdirSync(rep);
            } catch {
                return [];
            }
            const tranches: Tranche[] = [];
            for (const nom of noms) {
                if (!/^\d+$/.test(nom)) continue;
                const n = Number(nom);
                if (!rangValide(n)) continue;
                try {
                    const etat = statSync(join(rep, nom));
                    // Une tranche disparue entre le `readdir` et le `stat` est
                    // simplement absente : le listage est un instantané, et
                    // `verdict` la dira `manquantes` — ce qui se répare.
                    if (etat.isFile()) tranches.push({ n, octets: etat.size });
                } catch {
                    continue;
                }
            }
            return tranches.sort((a, b) => a.n - b.n);
        },

        /// Concatène les rangs DEMANDÉS, dans l'ordre donné.
        ///
        /// 🔴 LES RANGS SONT UN PARAMÈTRE, PAS UN LISTAGE, ET C'EST CE QUI
        /// DONNE SON SENS À LA GARDE CI-DESSOUS. Si ce flux listait lui-même le
        /// répertoire, une tranche disparue depuis le verdict serait simplement
        /// absente de la liste : le flux se terminerait PROPREMENT, plus court,
        /// et l'agent calculerait une empreinte fausse sans que personne ne
        /// sache pourquoi. En servant le plan que l'appelant a fait vérifier, un
        /// fichier manquant devient une ERREUR de flux.
        ///
        /// 🔴 UNE TRANCHE SUPPRIMÉE SOUS LES PIEDS DU FLUX DONNE UNE ERREUR,
        /// JAMAIS UN FLUX TRONQUÉ SILENCIEUX. `createReadStream` sur un fichier
        /// absent émet `error` ; l'itération le relance, le générateur meurt, et
        /// `Readable.from` détruit le lisible avec cette erreur — le
        /// consommateur reçoit `error`, pas `end`.
        ///
        /// ⚠️ TOUS LES CHEMINS SONT VALIDÉS D'AVANCE, avant que le flux n'existe :
        /// un rang fautif lève à l'appel, où l'appelant peut encore répondre,
        /// plutôt qu'au milieu d'une réponse déjà commencée.
        concatener(id: string, rangs: readonly number[]): Readable {
            const chemins = rangs.map((n) => cheminDe(id, n));
            return Readable.from(
                (async function* () {
                    for (const chemin of chemins) {
                        for await (const morceau of createReadStream(chemin)) {
                            yield morceau as Uint8Array;
                        }
                    }
                })(),
                // ⚠️ `objectMode: false` EST EXIGÉ : `Readable.from` est en mode
                // objet par défaut, et un consommateur qui attend des octets
                // recevrait un flux dont la contre-pression se compte en
                // morceaux plutôt qu'en octets.
                { objectMode: false },
            );
        },

        /// ⚠️ `force` : supprimer un téléversement qui n'a jamais reçu de
        /// tranche est un succès, pas une erreur — c'est l'état d'une purge qui
        /// passe après un dépôt abandonné avant sa première trame.
        supprimer(id: string): void {
            rmSync(repertoireDe(id), { recursive: true, force: true });
        },
    };
}
