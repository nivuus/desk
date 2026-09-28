// RENAMING AND DELETION, on the local machine side. **PURE**: neither DOM, nor
// WebRTC, nor binary frame; the root is INJECTED into it, as in
// `adaptateur.ts` and `ecriture.ts`.
//
// ════════════════════════════════════════════════════════════════════════════
// 🔵 THE COST FIGURE OF SPEC §3.5.1 IS WRONG FOR THIS SETUP
// ════════════════════════════════════════════════════════════════════════════
//
// The spec writes: "the fallback […] makes THE WHOLE CONTENT OF THE FILE TRAVEL TWICE
// over the channel. Renaming a 1 GiB file on the fallback path therefore
// costs 2 GiB of channel".
//
// **That is only true if the BRIDGE orchestrates the copy**, through a sequence of
// `Lire` and `Ecrire`. F3 does not orchestrate it: renaming is **ONE SINGLE
// MESSAGE** (`Renommer { de, vers }`), and the fallback copy happens between two
// handles that both live in the browser, on the local machine's
// disk. **Cost of the fallback on the channel: ZERO bytes, in both branches.**
//
// WHAT THE FALLBACK COSTS ANYWAY, and which must not be erased by the
// figure above:
//
//   - **it is not atomic** — a cut in the middle leaves two copies,
//     one of which carries the target name and is partial. The spec says so; it is
//     still true, and **it is not repairable here**;
//   - it **transiently doubles the disk usage** of the local machine;
//   - it is **O(size)** in time and, for a directory, **O(number
//     of entries)** FSA calls — on a deep directory, that can be long,
//     and **NOTHING HERE BOUNDS IT**;
//   - ✅ **F4 MEASURED IT (August 21st, 2026), AND THE COST IS NIL AT THESE SIZES.**
//     The fallback ran for the FIRST time — F3 had delivered it without any
//     of its lines running —, forced by an injection that removes `move`.
//     64 KiB: 173 / 193 ms; 1 MiB: 125 / 126 ms, two runs. The CONTROL
//     arm, without neutralising `move`, gives 159 / 162 and 126 / 126 ms:
//     **indistinguishable**. It is indeed a LOCAL time — the product's trace
//     says so, "zero bytes on the channel" — and the margin to `DELAI_MUTATION` (15 s)
//     is two orders of magnitude.
//     ⚠️ **The DIRECTORY half stays out of the product's reach**: ProjFS
//     refuses the renaming of a directory before consulting the provider.
//
// ════════════════════════════════════════════════════════════════════════════
// 🔴 `move()` OVERWRITES, AND THAT IS WHY THE CHECK COMES FIRST
// ════════════════════════════════════════════════════════════════════════════
//
// `FileSystemHandle.move()` **does not belong to the standard** of the File System
// Access API: it is a Chromium extension. Spec §3.5.1 says so, and the old
// bridge uses it (`web/index.js:628`, `:644`).
//
// 🔴 **IT SILENTLY OVERWRITES AN EXISTING DESTINATION, AND IT IS MEASURED**
// — probe S2, two identical runs on Chrome 151:
// a file holding its original content is overwritten by
// `agresseur.move(racine, 'S2-Victime.txt')`, which reports `issue: "ok"` with the mover's content,
// **without error**. *No document of the repository said so before this one.*
// Resolving the destination therefore comes FIRST, in both branches —
// otherwise renaming `draft.txt` to `note.txt` would destroy `note.txt` without a
// word. Log: `journaux-pont-fichiers-f3/s2-move-casse.txt`.
//
// 🔴 **AND IT DOES NOT EXIST ON A DIRECTORY** — same probe,
// `move_repertoire: { present: false }`. F3's plan held the fact that
// the old bridge only called it on files (`web/index.js:628`) as a
// "**hint, not proof**"; **the measurement settles it**.
//
// ⚠️ **CONSEQUENCE, AND IT REVERSES THE PLAN'S VOCABULARY**: for a
// DIRECTORY, the copy is not a "fallback" — **it is THE path, the only one.**
// Renaming a directory containing a subdirectory, which the defect of
// `web/index.js:631` made ALWAYS impossible, only works that way.
//
// ⚠️ **`move()` IS DETECTED AT CALL TIME, never captured when the module
// loads.** A detection made once and for all would be wrong the day
// another file system got injected — and that is exactly what
// this module's tests do, using TWO fakes: one exposing it,
// the other not.
//
// ════════════════════════════════════════════════════════════════════════════
// 🔴 THE SPECIAL CASE THAT DESTROYS: `a.txt` → `A.txt`
// ════════════════════════════════════════════════════════════════════════════
//
// Neither the spec nor the old bridge handles it. On a local machine INSENSITIVE to
// case, the destination "already exists" — **and it is the source itself**.
// A naive implementation refuses (`deja-present`) or, worse, overwrites.
//
// **F3's rule**: if the destination's only namesake IS the source, it is
// a **pure case rename**, it is legal, and the fallback goes through an **intermediate
// name** — two moves, never an overwrite.
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
