// Le magasin d'icônes de la plateforme : des octets sur DISQUE, adressés par
// leur contenu, un fichier par empreinte.
//
// 🔴 POURQUOI LE DISQUE ET NON LA BASE. Trois raisons, dans l'ordre de leur
// poids :
//
//   1. UN BLOB NE TRAVERSE PAS LA DOUBLE PASSE SANS MENTIR. PostgreSQL n'a pas
//      de type `BLOB` (il a `bytea`) ; SQLite, lui, accepte N'IMPORTE QUEL nom
//      de type par affinité — le lint de `base/sous-ensemble.test.ts` le
//      documente pour `SERIAL`, mesure à l'appui. Écrire `BYTEA` passerait donc
//      les DEUX passes en signifiant deux choses différentes : c'est le piège
//      `SERIAL` À L'ENVERS, et aucun des deux gardes du dépôt ne l'attrape.
//   2. La doctrine est déjà écrite par la spécification, qui tranche pour la
//      reprise de téléversement en faveur d'« un LISTAGE DE RÉPERTOIRE, jamais
//      une table de comptabilité qui pourrait diverger du disque ».
//   3. Le volume : 4 576 398 octets mesurés pour ce seul catalogue.
//
// 🔴 ET C'EST L'AUTO-RECONSTRUCTION QUI REND LE DISQUE ACCEPTABLE, pas une
// commodité. Un magasin perdu — conteneur sans volume, répertoire mal nommé —
// se remplit tout seul : l'inventaire des manquantes interroge le DISQUE, donc
// tout ce qui manque est redemandé à la réconciliation suivante. C'est le
// critère ⑦ de recette, et il doit être ÉPROUVÉ plutôt que supposé.

import { createHash } from 'node:crypto';
import { mkdirSync, readFileSync, renameSync, rmSync, writeFileSync } from 'node:fs';
import { existsSync } from 'node:fs';
import { readdir, rm, stat } from 'node:fs/promises';
import { join } from 'node:path';

/// 🔴 TOUS LES 50 NOMS EXAMINÉS, LE TOUR REND LA MAIN AU BOUCLE D'ÉVÈNEMENTS
/// (round de correction 2) — `setImmediate` plutôt qu'un `Promise.resolve()`
/// : ce dernier ne planifie qu'une MICROTÂCHE, qui ne cède la main à AUCUNE
/// E/S ni horloge en attente ; `setImmediate` planifie une VRAIE tâche, après
/// la phase "poll" — ce qui laisse une requête HTTP ou un message WebSocket
/// déjà prêts s'exécuter avant l'entrée suivante. MESURÉ (banc de 20 000
/// icônes, 25 août 2026, AVANT ce remède) : 222 ms d'un seul tenant,
/// ZÉRO battement de 10 ms servi pendant (≈22 attendus) — le port était
/// ouvert, et rien ne répondait. ⚠️ NON CALIBRÉ : 50 est raisonné (assez petit
/// pour qu'aucune E/S en attente ne patiente plus de quelques passages
/// d'entrées, assez grand pour ne pas noyer le tour sous des tâches de
/// planification), jamais mesuré finement. La mesure AVANT ce remède est
/// celle de la REVUE (round de correction 2), pas la mienne : reprise ici
/// pour ne pas la perdre, avec sa provenance dite.
const PAS_DE_REPRISE = 50;

async function rendreLaMain(): Promise<void> {
    await new Promise<void>((resolve) => setImmediate(resolve));
}

/// ⚠️ **NON CALIBRÉE.** Aucune constante de ce dépôt ne l'est.
///
/// 🔴 LE PLANCHER DE RÉFÉRENCE (voir `evincer`, plus bas) EST CE QUI DISTINGUE
/// UNE ÉVICTION D'UNE CORRUPTION : évincer une icône encore nommée par une
/// application ferait disparaître son image sans que rien ne le dise.
///
/// ⚠️ CE QUE CETTE RÈGLE NE FAIT PAS : elle ne borne PAS le disque. Un
/// catalogue qui grossit sans cesse grossit sans cesse. Le plafond de taille a
/// été ÉCARTÉ par décision, parce qu'il peut évincer un objet encore référencé
/// — c'est-à-dire échanger une croissance visible contre une panne silencieuse.
export const AGE_EVICTION_ICONE_MS = 180 * 24 * 60 * 60_000;

/// 🔴 EXACTEMENT 64 CARACTÈRES HEXADÉCIMAUX MINUSCULES, ET RIEN D'AUTRE.
///
/// Sans cette garde, `:sha256` est UN COMPOSANT DE CHEMIN FOURNI PAR LE
/// RÉSEAU, et `..` y est significatif. Le dépôt écrit déjà « ne jamais
/// interpoler une valeur d'environnement brute dans un composant de chemin » ;
/// ici c'est PIRE — elle vient d'un pair.
///
/// ⚠️ MINUSCULES SEULEMENT, et ce n'est pas de la coquetterie : sur un système
/// de fichiers insensible à la casse, `AB…` et `ab…` désigneraient le MÊME
/// fichier sous deux empreintes différentes, et l'adressage par contenu
/// cesserait d'être une bijection.
export function empreinteValide(s: string): boolean {
    return /^[0-9a-f]{64}$/.test(s);
}

export interface Magasin {
    possede(empreinte: string): boolean;
    manquantes(annoncees: readonly string[]): string[];
    ecrire(empreinte: string, octets: Buffer): void;
    lire(empreinte: string): Buffer | undefined;
    /// Évince PAR ÂGE, avec un PLANCHER DE RÉFÉRENCE — voir `AGE_EVICTION_ICONE_MS`.
    /// `maintenant` est un PARAMÈTRE, jamais lu de l'horloge : même règle que
    /// partout ailleurs dans ce dépôt (`depot/application.ts`, etc.), et c'est
    /// ce qui rend `icones.test.ts` capable de rejouer un âge exact.
    evincer(options: { maintenant: number; referencees: ReadonlySet<string> }): Promise<void>;
    repertoire: string;
}

/// Ouvre — ou crée — le magasin, et JOURNALISE le chemin retenu.
///
/// ⚠️ LA LIGNE DE JOURNAL N'EST PAS DÉCORATIVE : `PLATEFORME_ICONES` est
/// facultative, donc un opérateur peut se tromper de répertoire sans que rien
/// ne casse — le magasin se reconstruirait ailleurs, en silence, en
/// retéléversant tout. Le chemin retenu doit se lire.
export function ouvrirMagasin(repertoire: string, journaliser: (chemin: string) => void): Magasin {
    mkdirSync(repertoire, { recursive: true });
    journaliser(repertoire);

    const chemin = (empreinte: string): string => {
        if (!empreinteValide(empreinte)) {
            throw new Error(`empreinte d'icône invalide : ${JSON.stringify(empreinte)}`);
        }
        return join(repertoire, empreinte);
    };

    return {
        repertoire,

        /// 🔴 UNE EXISTENCE DE FICHIER, JAMAIS UNE TABLE. Une table de
        /// comptabilité divergerait du magasin le jour où un fichier serait
        /// perdu — et c'est PRÉCISÉMENT le jour où l'on a besoin de le savoir.
        possede(empreinte: string): boolean {
            return empreinteValide(empreinte) && existsSync(join(repertoire, empreinte));
        },

        /// Le complément, DANS L'ORDRE D'ANNONCE et sans doublon.
        ///
        /// ⚠️ L'ordre d'annonce est celui du catalogue, donc celui dans lequel
        /// l'utilisateur verra les icônes arriver. Trier le perdrait pour rien.
        ///
        /// ⚠️ UNE EMPREINTE MAL FORMÉE N'EST PAS « MANQUANTE » : elle est
        /// ignorée. La redemander ferait boucler l'agent sur une valeur que la
        /// route refuserait de toute façon.
        manquantes(annoncees: readonly string[]): string[] {
            const vues = new Set<string>();
            const manque: string[] = [];
            for (const e of annoncees) {
                if (!empreinteValide(e) || vues.has(e)) continue;
                vues.add(e);
                if (!existsSync(join(repertoire, e))) manque.push(e);
            }
            return manque;
        },

        /// 🔴 RECALCULE L'EMPREINTE, ET REFUSE SI ELLE DIFFÈRE.
        ///
        /// C'est la troisième des trois vérifications de la spécification —
        /// « aucun saut ne fait confiance au précédent ». **Sans elle,
        /// l'adressage par contenu n'en serait PAS un** : un agent fautif
        /// empoisonnerait le magasin d'un fichier qui ne correspond pas à son
        /// nom, et le `Cache-Control: immutable` de la route rendrait
        /// l'empoisonnement PERMANENT dans les caches.
        ///
        /// 🔴 L'ÉCRITURE EST ATOMIQUE : fichier temporaire puis `rename`. Un
        /// `PUT` interrompu laisserait sinon un fichier TRONQUÉ **sous un nom
        /// qui promet son contenu**, et le maillon suivant le servirait sans
        /// jamais le relire.
        ecrire(empreinte: string, octets: Buffer): void {
            const cible = chemin(empreinte);
            const reel = createHash('sha256').update(octets).digest('hex');
            if (reel !== empreinte) {
                throw new Error(
                    `empreinte annoncée ${empreinte} mais contenu en ${reel} : refusé`,
                );
            }
            // Le suffixe aléatoire évite que deux `PUT` concurrents de la même
            // empreinte n'écrivent le même temporaire.
            const provisoire = `${cible}.${process.pid}.${Math.random().toString(36).slice(2)}.part`;
            try {
                writeFileSync(provisoire, octets);
                renameSync(provisoire, cible);
            } catch (erreur) {
                rmSync(provisoire, { force: true });
                throw erreur;
            }
        },

        lire(empreinte: string): Buffer | undefined {
            if (!empreinteValide(empreinte)) return undefined;
            try {
                return readFileSync(join(repertoire, empreinte));
            } catch {
                return undefined;
            }
        },

        /// 🔴 LE PLANCHER D'ABORD : une empreinte RÉFÉRENCÉE n'est jamais
        /// examinée pour son âge, quelle que soit sa vétusté. C'est la seule
        /// chose qui distingue une éviction d'une corruption — voir le
        /// commentaire de `AGE_EVICTION_ICONE_MS`.
        ///
        /// ⚠️ UN NOM QUI N'EST PAS UNE EMPREINTE VALIDE N'EST JAMAIS TOUCHÉ :
        /// un fichier étranger déposé à la main dans le magasin (le cas
        /// couvert par `icones.test.ts::'un fichier étranger…'`) n'est pas de
        /// la responsabilité de cette éviction.
        async evincer({ maintenant, referencees }: { maintenant: number; referencees: ReadonlySet<string> }): Promise<void> {
            let noms: string[];
            try {
                noms = await readdir(repertoire);
            } catch {
                return;
            }
            let i = 0;
            for (const nom of noms) {
                if (i > 0 && i % PAS_DE_REPRISE === 0) await rendreLaMain();
                i += 1;
                if (!empreinteValide(nom) || referencees.has(nom)) continue;
                let mtimeMs: number;
                try {
                    mtimeMs = (await stat(join(repertoire, nom))).mtimeMs;
                } catch {
                    continue;
                }
                if (maintenant - mtimeMs >= AGE_EVICTION_ICONE_MS) {
                    await rm(join(repertoire, nom), { force: true });
                }
            }
        },
    };
}
