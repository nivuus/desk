// LE RENOMMAGE ET LA SUPPRESSION, côté poste local. **PUR** : ni DOM, ni
// WebRTC, ni trame binaire ; la racine lui est INJECTÉE, comme à
// `adaptateur.ts` et à `ecriture.ts`.
//
// ════════════════════════════════════════════════════════════════════════════
// 🔵 LE CHIFFRE DE COÛT DE LA SPEC §3.5.1 EST FAUX POUR CE MONTAGE
// ════════════════════════════════════════════════════════════════════════════
//
// La spec écrit : « le repli […] fait transiter TOUT LE CONTENU DU FICHIER DEUX
// FOIS sur le canal. Renommer un fichier de 1 Gio sur le chemin de repli coûte
// donc 2 Gio de canal ».
//
// **Cela n'est vrai que si le PONT orchestre la copie**, par une suite de
// `Lire` et d'`Ecrire`. F3 ne l'orchestre pas : le renommage est **UN SEUL
// MESSAGE** (`Renommer { de, vers }`), et la copie de repli se fait entre deux
// poignées qui vivent toutes deux dans le navigateur, sur le disque du poste
// local. **Coût du repli sur le canal : ZÉRO octet, dans les deux branches.**
//
// CE QUE LE REPLI COÛTE QUAND MÊME, et qu'il ne faut pas effacer avec le
// chiffre ci-dessus :
//
//   - **il n'est pas atomique** — une coupure au milieu laisse deux copies,
//     dont l'une porte le nom cible et est partielle. La spec le dit ; c'est
//     toujours vrai, et **ce n'est pas réparable ici** ;
//   - il **double transitoirement l'occupation disque** du poste local ;
//   - il est en **O(taille)** en temps et, pour un répertoire, en **O(nombre
//     d'entrées)** appels FSA — sur un répertoire profond, cela peut être long,
//     et **RIEN ICI NE LE BORNE** ;
//   - ✅ **F4 L'A MESURÉE (21 août 2026), ET LE COÛT EST NUL À CES RANGS.**
//     Le repli a couru pour la PREMIÈRE fois — F3 l'avait livré sans qu'aucune
//     de ses lignes ne coure —, forcé par une injection qui retire `move`.
//     64 Kio : 173 / 193 ms ; 1 Mio : 125 / 126 ms, deux exécutions. Le bras
//     TÉMOIN, sans neutraliser `move`, rend 159 / 162 et 126 / 126 ms :
//     **indistinguable**. C'est bien un temps LOCAL — la trace du produit le
//     dit, « zéro octet sur le canal » — et la marge à `DELAI_MUTATION` (15 s)
//     est de deux ordres de grandeur.
//     ⚠️ **La moitié RÉPERTOIRE reste hors de portée du produit** : ProjFS
//     refuse le renommage d'un répertoire avant de consulter le fournisseur.
//
// ════════════════════════════════════════════════════════════════════════════
// 🔴 `move()` ÉCRASE, ET C'EST POURQUOI LA VÉRIFICATION PRÉCÈDE
// ════════════════════════════════════════════════════════════════════════════
//
// `FileSystemHandle.move()` **n'appartient pas à la norme** du File System
// Access API : c'est une extension Chromium. La spec §3.5.1 le dit, et l'ancien
// pont s'en sert (`web/index.js:628`, `:644`).
//
// 🔴 **ELLE ÉCRASE SILENCIEUSEMENT UNE DESTINATION EXISTANTE, ET C'EST MESURÉ**
// — la sonde S2, deux exécutions identiques sur Chrome 151 :
// un fichier contenant `AVANT` est écrasé par
// `agresseur.move(racine, 'S2-Victime.txt')`, `{ issue: "ok", contenu: "APRES" }`,
// **sans erreur**. *Aucun document du dépôt ne le disait avant celui-ci.*
// La résolution de la destination vient donc AVANT, dans les deux branches —
// sans quoi renommer `brouillon.txt` en `note.txt` détruirait `note.txt` sans un
// mot. Journal : `journaux-pont-fichiers-f3/s2-move-casse.txt`.
//
// 🔴 **ET ELLE N'EXISTE PAS SUR UN RÉPERTOIRE** — même sonde,
// `move_repertoire: { present: false }`. Le plan de F3 tenait le fait que
// l'ancien pont ne l'appelait que sur des fichiers (`web/index.js:628`) pour un
// « **indice, pas preuve** » ; **la mesure tranche**.
//
// ⚠️ **CONSÉQUENCE, ET ELLE RENVERSE LE VOCABULAIRE DU PLAN** : pour un
// RÉPERTOIRE, la copie n'est pas un « repli » — **c'est LE chemin, le seul.**
// Le renommage d'un répertoire contenant un sous-répertoire, que le défaut de
// `web/index.js:631` rendait TOUJOURS impossible, ne marche que par là.
//
// ⚠️ **`move()` EST DÉTECTÉE À L'APPEL, jamais capturée au chargement du
// module.** Une détection faite une fois pour toutes serait fausse le jour où
// l'on injecterait un autre système de fichiers — et c'est exactement ce que
// font les tests de ce module, qui emploient DEUX faux : l'un qui l'expose,
// l'autre non.
//
// ════════════════════════════════════════════════════════════════════════════
// 🔴 LE CAS PARTICULIER QUI DÉTRUIT : `a.txt` → `A.txt`
// ════════════════════════════════════════════════════════════════════════════
//
// Ni la spec ni l'ancien pont ne le traitent. Sur un poste local INSENSIBLE à
// la casse, la destination « existe déjà » — **et c'est la source elle-même**.
// Une implémentation naïve refuse (`deja-present`) ou, pire, écrase.
//
// **Règle de F3** : si l'unique homonyme de la destination EST la source, c'est
// un **renommage de casse pure**, il est licite, et le repli passe par un **nom
// intermédiaire** — deux mouvements, jamais un écrasement.
//
// ⚠️ **CE CHEMIN N'EST PAS EXERÇABLE PAR L'INSTRUMENT DE RECETTE, et la sonde
// S2 le mesure** : OPFS est **SENSIBLE à la casse**
// (`opfs_sensible_a_la_casse: true`), donc `S2-Pure.txt` → `S2-PURE.TXT` y
// réussit DIRECTEMENT, sans aucune collision à résoudre. Le nom intermédiaire
// n'est éprouvé que sur l'hôte, par le faux INSENSIBLE de `mutation.test.ts`.
// **Ne pas lire un « ok » de la sonde comme une validation de ce chemin.**
//
// ════════════════════════════════════════════════════════════════════════════
// ⚠️ LA SUPPRESSION N'EST PAS RÉCURSIVE — DIVERGENCE AVEC LA SPEC §3.5
// ════════════════════════════════════════════════════════════════════════════
//
// Elle écrit `dir.removeEntry(nom, { recursive })`. **F3 appelle
// `removeEntry(nom)` SANS `recursive`.**
//
// Raison : `recursive: true` transforme UN geste dans la VM en **destruction
// récursive** sur le disque du poste local, sur la foi d'un miroir qu'aucune
// preuve ne dit à jour. Windows, lui, ne supprime jamais un répertoire non vide
// en un geste : l'Explorateur et `rd /s` effacent les enfants un à un, et
// **chaque enfant produit sa propre notification**. Le miroir non récursif suit
// donc Windows pas à pas.
//
// 🔵 **Bénéfice second, et il n'est pas décoratif** : si le navigateur répond
// que le répertoire n'est pas vide, cela veut dire que **le miroir a dérivé** —
// et `repertoire-non-vide` devient une cause RÉELLE et DIAGNOSTIQUE au lieu
// d'un code jamais produit.
//
// ⚠️ **Ce que cela suppose, et qui N'EST PAS MESURÉ** : que ProjFS émette bien
// une notification de suppression PAR ENFANT, y compris pour des enfants jamais
// énumérés ni hydratés. C'est la question ③ de la sonde S1. Si la réponse est
// non, la suppression d'un répertoire non vide laissera les enfants sur le
// poste local — **dégrade, ne bloque pas**.

import { EchecFichiers, classer, type PoigneeBase, type PoigneeFichier } from './adaptateur';
import type { FluxInscriptible, RacineInscriptible } from './ecriture';
import { canoniser, canoniserOuLever } from './noms';
import { copierFichier, copierRepertoire, ouvrirRepertoire, retirerArbre } from './copie';

/** Ce qu'on sait faire d'une poignée de fichier qu'on veut déplacer. */
export interface PoigneeFichierMutable extends PoigneeFichier {
    createWritable(options?: { keepExistingData?: boolean }): Promise<FluxInscriptible>;
    /** **NON STANDARD** — extension Chromium. Absente ⇒ le repli local. */
    move?(parent: RacineMutable, nom: string): Promise<void>;
}

/**
 * Une racine sur laquelle on peut muter.
 *
 * 🔵 **`move?` EST OPTIONNELLE DANS LE TYPE, et c'est ce qui permet d'écrire
 * DEUX faux — l'un qui l'expose, l'autre non — et de voir les deux branches
 * vertes sur l'hôte.** Un type qui l'imposerait rendrait le repli
 * **inatteignable par un test**.
 */
export interface RacineMutable extends RacineInscriptible {
    getDirectoryHandle(nom: string, options?: { create?: boolean }): Promise<RacineMutable>;
    getFileHandle(nom: string, options?: { create?: boolean }): Promise<PoigneeFichierMutable>;
    /** ⚠️ **SANS `recursive`** — voir l'en-tête. */
    removeEntry(nom: string): Promise<void>;
    /** **NON STANDARD**. */
    move?(parent: RacineMutable, nom: string): Promise<void>;
}

/** Ce qu'un renommage a coûté LOCALEMENT — l'instrumentation que la spec exige. */
export interface TraceRenommage {
    /** `true` si `move()` a servi, `false` si le repli a copié. */
    parMove: boolean;
    /** Octets recopiés. **Zéro sur la branche `move`.** */
    octets: number;
    /** Entrées recréées. **Zéro sur la branche `move`**, 1 pour un fichier. */
    entrees: number;
}

/** `"a/b/c"` → `["a","b","c"]`, `""` → `[]`. */
function composants(chemin: string): string[] {
    return chemin.split('/').filter((c) => c.length > 0);
}

/**
 * Descend les `jusqua` premiers composants **en les canonicalisant**, sans en
 * créer aucun.
 */
async function descendre(
    racine: RacineMutable,
    parts: string[],
    jusqua: number,
): Promise<RacineMutable> {
    let ici = racine;
    for (let i = 0; i < jusqua; i += 1) {
        const nom = await canoniserOuLever(ici, parts[i], 'chemin-introuvable');
        try {
            ici = await ici.getDirectoryHandle(nom);
        } catch (e) {
            throw classer(e, 'chemin-introuvable');
        }
    }
    return ici;
}

/** Descend en CRÉANT les répertoires manquants — pour la destination. */
async function descendreEnCreant(
    racine: RacineMutable,
    parts: string[],
    jusqua: number,
): Promise<RacineMutable> {
    let ici = racine;
    for (let i = 0; i < jusqua; i += 1) {
        // ⚠️ On canonicalise D'ABORD : sans cela, `archives/` et `Archives/`
        // deviendraient deux répertoires sur un poste SENSIBLE à la casse, et
        // le même sur un poste insensible — deux comportements pour un chemin.
        const r = await canoniser(ici, parts[i]);
        const nom = r.sorte === 'trouve' ? r.nom : parts[i];
        if (r.sorte === 'ambigu') {
            throw new EchecFichiers(
                'casse-ambigue',
                `« ${parts[i]} » ne se distingue pas de « ${r.noms.join(' », « ')} »`,
            );
        }
        try {
            ici = await ici.getDirectoryHandle(nom, { create: true });
        } catch (e) {
            throw classer(e, 'chemin-introuvable');
        }
    }
    return ici;
}

/**
 * Renomme `de` en `vers`, tous deux relatifs à la racine.
 *
 * ⚠️ **`repertoire` est TRANSPORTÉ depuis le rappel ProjFS**, jamais
 * redécouvert : le navigateur le redemanderait au prix d'un aller-retour, et se
 * tromperait sur une entrée que le renommage vient de faire disparaître.
 */
export async function renommer(
    racine: RacineMutable,
    de: string,
    vers: string,
    repertoire: boolean,
): Promise<TraceRenommage> {
    const partsDe = composants(de);
    const partsVers = composants(vers);
    if (partsDe.length === 0 || partsVers.length === 0) {
        throw new EchecFichiers('non-supporte', 'la racine ne se renomme pas');
    }
    const parentSource = await descendre(racine, partsDe, partsDe.length - 1);
    const nomSource = await canoniserOuLever(
        parentSource,
        partsDe[partsDe.length - 1],
        'introuvable',
    );
    const parentDest = await descendreEnCreant(racine, partsVers, partsVers.length - 1);
    const nomDemande = partsVers[partsVers.length - 1];

    // ── LA RÉSOLUTION DE LA DESTINATION, ET ELLE PRÉCÈDE TOUT ────────────────
    // 🔴 `move()` ÉCRASE : sans ce bloc, renommer `brouillon.txt` en `note.txt`
    // détruirait `note.txt` sans un mot.
    const dest = await canoniser(parentDest, nomDemande);
    const memeParent = parentSource === parentDest;
    let cassePure = false;
    if (dest.sorte === 'ambigu') {
        throw new EchecFichiers(
            'casse-ambigue',
            `« ${nomDemande} » ne se distingue pas de « ${dest.noms.join(' », « ')} »`,
        );
    }
    if (dest.sorte === 'trouve') {
        // 🔴 **LE RENOMMAGE DE CASSE PURE.** Si l'unique homonyme de la
        // destination EST la source, ce n'est pas une collision : c'est
        // `a.txt` → `A.txt`, et il est licite.
        if (memeParent && dest.nom === nomSource) {
            cassePure = true;
        } else {
            throw new EchecFichiers(
                'deja-present',
                `« ${vers} » existe déjà sous le nom « ${dest.nom} »`,
            );
        }
    }

    if (cassePure) {
        // Deux mouvements, JAMAIS un écrasement : sur un poste insensible à la
        // casse, se déplacer sur soi-même est ou bien refusé, ou bien — pire —
        // une troncature.
        const intermediaire = nomIntermediaire(nomSource);
        await deplacer(parentSource, nomSource, parentSource, intermediaire, repertoire);
        await deplacer(parentSource, intermediaire, parentDest, nomDemande, repertoire);
        return { parMove: true, octets: 0, entrees: 0 };
    }
    return deplacer(parentSource, nomSource, parentDest, nomDemande, repertoire);
}

/**
 * Un nom intermédiaire qui ne peut collisionner avec rien.
 *
 * ⚠️ **Il porte un composant aléatoire**, et non un suffixe fixe : deux
 * renommages de casse pure concurrents dans le même répertoire se
 * marcheraient dessus, et le second détruirait le fichier du premier.
 */
function nomIntermediaire(source: string): string {
    const jeton = Math.random().toString(36).slice(2, 10);
    return `${source}.pont-${jeton}.tmp`;
}

/** `move()` si elle existe, la copie locale sinon. */
async function deplacer(
    parentSource: RacineMutable,
    nomSource: string,
    parentDest: RacineMutable,
    nomDest: string,
    repertoire: boolean,
): Promise<TraceRenommage> {
    // ⚠️ **DÉTECTÉE À L'APPEL**, sur la poignée réellement obtenue.
    const poignee: PoigneeBase & { move?: unknown } = repertoire
        ? await ouvrirRepertoire(parentSource, nomSource)
        : await ouvrirFichier(parentSource, nomSource);
    if (typeof poignee.move === 'function') {
        try {
            await (poignee as { move(p: RacineMutable, n: string): Promise<void> }).move(
                parentDest,
                nomDest,
            );
            return { parMove: true, octets: 0, entrees: 0 };
        } catch (e) {
            throw classer(e, 'introuvable');
        }
    }
    // ── LE REPLI, ENTIÈREMENT DANS LE NAVIGATEUR ─────────────────────────────
    const trace = { parMove: false, octets: 0, entrees: 0 };
    if (repertoire) {
        await copierRepertoire(parentSource, nomSource, parentDest, nomDest, trace);
    } else {
        await copierFichier(parentSource, nomSource, parentDest, nomDest, trace);
    }
    // 🔴 **LA SOURCE N'EST RETIRÉE QU'APRÈS**, et une copie interrompue la
    // laisse donc INTACTE. L'inverse perdrait le fichier sur une coupure.
    try {
        if (repertoire) {
            await retirerArbre(parentSource, nomSource);
        } else {
            await parentSource.removeEntry(nomSource);
        }
    } catch (e) {
        throw classer(e, 'introuvable');
    }
    return trace;
}

async function ouvrirFichier(
    parent: RacineMutable,
    nom: string,
): Promise<PoigneeFichierMutable> {
    try {
        return await parent.getFileHandle(nom);
    } catch (e) {
        throw classer(e, 'introuvable');
    }
}

/**
 * Supprime `chemin`, relatif à la racine.
 *
 * ⚠️ **`removeEntry(nom)` SANS `recursive`** — voir l'en-tête.
 */
export async function supprimer(
    racine: RacineMutable,
    chemin: string,
    _repertoire: boolean,
): Promise<void> {
    const parts = composants(chemin);
    if (parts.length === 0) {
        throw new EchecFichiers('non-supporte', 'la racine ne se supprime pas');
    }
    const parent = await descendre(racine, parts, parts.length - 1);
    const nom = await canoniserOuLever(parent, parts[parts.length - 1], 'introuvable');
    try {
        await parent.removeEntry(nom);
    } catch (e) {
        // 🔴 **`InvalidModificationError` VEUT DIRE DEUX CHOSES SELON LE VERBE,
        // ET `adaptateur.classer` NE PEUT PAS LES DÉPARTAGER.**
        //
        // Sur une CRÉATION, elle veut dire « une entrée du même nom existe » —
        // et `classer` la traduit en `deja-present`, ce que F2 a écrit. Sur un
        // `removeEntry` SANS `recursive`, elle veut dire **« le répertoire
        // n'est pas vide »**, ce qui est un diagnostic tout différent : le
        // miroir a dérivé.
        //
        // La classification est donc faite ICI, où le verbe est connu.
        // L'élargir dans `classer` ferait qu'une création rendrait
        // `repertoire-non-vide`, ou l'inverse.
        // ✅ **CE NOM D'EXCEPTION EST MESURÉ, pas supposé** : la sonde S2 rend
        // `remove_non_vide: "REFUSE:InvalidModificationError"` sur un
        // `removeEntry` SANS `recursive` d'un répertoire non vide, deux
        // exécutions identiques. `repertoire-non-vide` est donc bien
        // atteignable — ce n'est pas un code écrit pour la table.
        if (e instanceof DOMException && e.name === 'InvalidModificationError') {
            throw new EchecFichiers(
                'repertoire-non-vide',
                `« ${chemin} » n’est pas vide sur le poste local : le miroir a dérivé, ` +
                    `rien n’a été supprimé`,
            );
        }
        throw classer(e, 'introuvable');
    }
}
