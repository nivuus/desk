// LA COPIE D'ARBRE ET SON RETRAIT — la branche du renommage qui n'a pas de
// `move()`. **PUR** : ni DOM, ni WebRTC, ni trame ; les poignées sont
// injectées, comme partout dans `fichiers/`.
//
// ════════════════════════════════════════════════════════════════════════════
// 🔴 POUR UN RÉPERTOIRE, CE N'EST PAS UN « REPLI » — C'EST LE SEUL CHEMIN
// ════════════════════════════════════════════════════════════════════════════
//
// La sonde S2 de F3 le mesure, deux exécutions identiques sur Chrome 151 :
// **`move()` N'EXISTE PAS sur un répertoire** (`move_repertoire:
// { present: false }`). Le plan de F3 tenait le fait que l'ancien pont ne
// l'appelait que sur des fichiers (`web/index.js:628`) pour un « indice, pas
// preuve » ; la mesure tranche. Journal :
// `docs/superpowers/plans/journaux-pont-fichiers-f3/s2-move-casse.txt`.
//
// ════════════════════════════════════════════════════════════════════════════
// POURQUOI CETTE EXTRACTION, ET POURQUOI ELLE ARRIVE AVANT LE FRANCHISSEMENT
// ════════════════════════════════════════════════════════════════════════════
//
// `mutation.ts` était à **497** lignes pour une porte à 500 — marge **3**. Le
// dépôt a franchi ce plafond quatre sous-blocs de suite sans le voir passer, et
// à chaque fois c'est le relevé d'un chantier voisin qui l'a nommé en premier.
// **Ici l'extraction précède l'addition**, ce qui est le geste que D9 a inventé
// et que D10 a joué trois fois.
//
// La ligne de partage est une responsabilité : `mutation.ts` décide **QUOI**
// faire — résoudre, refuser, choisir la branche —, ce module fait **COMMENT**
// on recopie et on retire un arbre.

import { classer, type PoigneeBase } from './adaptateur';
import type { RacineMutable, TraceRenommage } from './mutation';

/** Ouvre un répertoire existant, ou classe l'échec. */
export async function ouvrirRepertoire(
    parent: RacineMutable,
    nom: string,
): Promise<RacineMutable> {
    try {
        return await parent.getDirectoryHandle(nom);
    } catch (e) {
        throw classer(e, 'introuvable');
    }
}

export async function copierFichier(
    parentSource: RacineMutable,
    nomSource: string,
    parentDest: RacineMutable,
    nomDest: string,
    trace: TraceRenommage,
): Promise<void> {
    try {
        const fichier = await (await parentSource.getFileHandle(nomSource)).getFile();
        const cible = await parentDest.getFileHandle(nomDest, { create: true });
        // ⚠️ **SANS `keepExistingData`** — la destination est neuve ou vide de
        // droit, et le défaut de l'ancien pont était précisément de garder la
        // queue d'octets d'un fichier réécrit plus court (spec §12).
        const flux = await cible.createWritable();
        const octets = new Uint8Array(await fichier.slice(0, fichier.size).arrayBuffer());
        await flux.write({ type: 'write', position: 0, data: octets });
        // 🔵 LA COMMITTAISON EST ICI, ET NULLE PART AILLEURS.
        await flux.close();
        trace.octets += octets.length;
        trace.entrees += 1;
    } catch (e) {
        throw classer(e, 'introuvable');
    }
}

/**
 * Recrée l'arbre, feuille à feuille.
 *
 * 🔴 **LE DÉFAUT DE L'ANCIEN PONT QU'ON REFUSE DE REJOUER** :
 * `web/index.js:631` écrit `const newDir = await newDir.getDirectoryHandle(...)`
 * **à l'intérieur du bloc où `newDir` est le paramètre** — une zone morte
 * temporelle, donc un `ReferenceError`. **Le renommage d'un répertoire
 * contenant un sous-répertoire y échoue donc TOUJOURS.**
 *
 * ⛔ **CE CHEMIN N'EST JAMAIS EMPRUNTÉ, ET C'EST LA RECETTE DE F3 QUI L'A
 * ÉTABLI.** Une rédaction antérieure disait « c'est le critère (1) de F3,
 * écrit pour exercer exactement ce cas » : **la mesure la réfute**. ProjFS
 * REFUSE le renommage d'un répertoire **avant de consulter le fournisseur** —
 * `Cette demande n'est pas prise en charge`, et **aucune** notification
 * `code=32` au journal, deux exécutions de recette plus la sonde S1. Le
 * critère (1) b n'est donc pas livrable, et **aucune ligne de cette fonction
 * n'a jamais couru en conditions réelles**.
 *
 * ⚠️ Elle reste écrite et testée sur l'hôte : le défaut de l'ancien pont est
 * réel, et le jour où un chemin l'atteindra — un copier/supprimer piloté
 * depuis la VM — c'est ici qu'il faudra regarder.
 */
export async function copierRepertoire(
    parentSource: RacineMutable,
    nomSource: string,
    parentDest: RacineMutable,
    nomDest: string,
    trace: TraceRenommage,
): Promise<void> {
    const source = await ouvrirRepertoire(parentSource, nomSource);
    let cible: RacineMutable;
    try {
        cible = await parentDest.getDirectoryHandle(nomDest, { create: true });
    } catch (e) {
        throw classer(e, 'introuvable');
    }
    trace.entrees += 1;
    // ⚠️ L'énumération est MATÉRIALISÉE avant de muter : itérer un répertoire
    // qu'on modifie pendant l'itération n'a pas de sémantique définie.
    const enfants: PoigneeBase[] = [];
    try {
        for await (const enfant of source.values()) enfants.push(enfant);
    } catch (e) {
        throw classer(e, 'introuvable');
    }
    for (const enfant of enfants) {
        if (enfant.kind === 'directory') {
            await copierRepertoire(source, enfant.name, cible, enfant.name, trace);
        } else {
            await copierFichier(source, enfant.name, cible, enfant.name, trace);
        }
    }
}

/**
 * Retire un répertoire et tout ce qu'il porte, **feuille à feuille**.
 *
 * 🔴 **CE N'EST PAS `recursive: true`, ET LA DIFFÉRENCE EST TOUT LE POINT.**
 * `removeEntry(nom)` sans `recursive` refuse un répertoire non vide (le faux de
 * test le refuse comme le navigateur réel), et le repli de renommage doit
 * pourtant retirer l'arbre source qu'il vient de recopier. Deux voies :
 *
 *   - `recursive: true` — **REFUSÉE** : elle détruirait sur la foi d'un miroir
 *     qu'aucune preuve ne dit à jour, et c'est l'argument entier de l'en-tête ;
 *   - descendre nous-mêmes et retirer **ce qu'on vient de copier**, entrée par
 *     entrée, du bas vers le haut. **C'est ce qui est fait.**
 *
 * 🔵 **La seconde est PLUS SÛRE que la première, pas seulement plus verbeuse** :
 * on ne retire QUE ce que [`copierRepertoire`] a énuméré et recopié quelques
 * lignes plus tôt. Une entrée apparue entre-temps sur le poste local **fait
 * échouer le retrait** au lieu d'être emportée en silence — et `deplacer`
 * remonte alors l'échec, source intacte.
 *
 * ⚠️ **[`supprimer`], elle, N'APPELLE JAMAIS CETTE FONCTION.** Une suppression
 * demandée par la VM ne retire qu'UNE entrée : Windows envoie une notification
 * PAR ENFANT, et le miroir le suit pas à pas. Les deux chemins sont voisins et
 * ne doivent pas être unifiés.
 */
export async function retirerArbre(parent: RacineMutable, nom: string): Promise<void> {
    const dossier = await parent.getDirectoryHandle(nom);
    const enfants: PoigneeBase[] = [];
    for await (const enfant of dossier.values()) enfants.push(enfant);
    for (const enfant of enfants) {
        if (enfant.kind === 'directory') {
            await retirerArbre(dossier, enfant.name);
        } else {
            await dossier.removeEntry(enfant.name);
        }
    }
    // Le répertoire est vide MAINTENANT : `removeEntry` sans `recursive`
    // l'accepte. S'il ne l'est pas, c'est qu'une entrée est apparue entre
    // l'énumération et ici — et le refus est le bon comportement.
    await parent.removeEntry(nom);
}
