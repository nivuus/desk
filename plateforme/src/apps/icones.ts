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
import { join } from 'node:path';

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
    };
}
