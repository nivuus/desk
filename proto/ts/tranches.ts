/**
 * La règle de DÉCOUPAGE EN TRANCHES d'un téléversement — PURE, et partagée
 * entre les deux bouts qui en dépendent.
 *
 * 🔴 POURQUOI ELLE VIT DANS `proto/ts/` ET NON DANS LE CLIENT : deux
 * arithmétiques indépendantes — l'une qui découpe et dépose (le navigateur),
 * l'autre qui vérifie que le découpage est complet avant de sceller (la
 * plateforme) — divergeraient un jour, et le symptôme serait un SCELLEMENT QUI
 * REFUSE SANS QU'ON SACHE LEQUEL DES DEUX BOUTS A TORT. C'est exactement le
 * patron que ce dépôt a payé sur `TYPES_AGENT` et sur la variante
 * `battement-recu` restée verte sur cinquante tests : une forme reproduite à la
 * main de chaque côté casse le pont sans casser un seul test. Ici il n'y a
 * qu'une arithmétique, et les deux bouts l'importent.
 *
 * ⚠️ AUCUN `node:`, AUCUN DOM, AUCUNE DÉPENDANCE — ce module doit charger dans
 * un navigateur comme dans le service. C'est la condition pour qu'il soit
 * réellement partagé plutôt que recopié.
 *
 * ⚠️ `taille`, `tailleTranche` et `octets` sont des `number` : au-delà de 2^53
 * l'arithmétique cesserait d'être exacte, et les deux bouts divergeraient en
 * silence. Un téléversement de 9 pétaoctets n'existe pas ; la borne est nommée,
 * pas gardée — même arbitrage, et pour la même raison, que celui de
 * `fichiers-entetes.ts`.
 */

/**
 * Une tranche : son RANG et le nombre d'octets qu'elle porte.
 *
 * ⚠️ `n` EST UN RANG À BASE ZÉRO, et ce choix est portant : il rend la position
 * de la tranche dans le fichier calculable sans table — c'est exactement
 * `n * tailleTranche`. Une base 1 obligerait chaque appelant à retrancher un,
 * et le jour où l'un des deux oublierait, tout le fichier serait décalé d'une
 * tranche sans qu'aucune taille ne bouge.
 */
export interface Tranche {
    n: number;
    octets: number;
}

/**
 * Le verdict d'un découpage reçu.
 *
 * 🔴 `incoherentes` N'EST PAS `manquantes`, ET LES CONFONDRE SERAIT UNE BOUCLE
 * SANS FIN. Une tranche ABSENTE est un trou : la redemander la comble. Une
 * tranche PRÉSENTE À LA MAUVAISE TAILLE est une ERREUR DE PROTOCOLE — les deux
 * bouts ne s'accordent plus sur le découpage —, et la redemander ne la
 * réparerait JAMAIS : le déposant renverrait la même chose, indéfiniment. Le
 * premier cas se rattrape, le second doit faire échouer le téléversement et le
 * dire.
 */
export type Verdict =
    | { etat: 'complet' }
    | { etat: 'manquantes'; n: number[] }
    | { etat: 'incoherentes'; n: number[] };

/**
 * Vérifie le CONTRAT du téléversement — sa taille et son pas de découpage.
 *
 * 🔴 CE QUI EST GARDÉ ICI LÈVE ; CE QUI VIENT DU FIL NE LÈVE JAMAIS. La
 * frontière est délibérée, et c'est la seule de ce module :
 *
 * - `taille` et `tailleTranche` sont le CONTRAT, arrêté à la déclaration du
 *   téléversement et détenu par l'appelant. Un contrat absurde — un pas nul,
 *   une taille négative — est un défaut de PROGRAMME, pas une donnée reçue :
 *   le rendre sous forme de verdict le déguiserait en anomalie de transfert, et
 *   le déposant passerait sa vie à recompléter des tranches qui n'existent pas.
 *   Il lève, et il nomme la valeur fautive.
 * - `presentes`, en revanche, est ce que le fil a apporté. Rien n'y lève :
 *   tout y devient un verdict, parce qu'un pair malveillant ou déréglé ne doit
 *   pas pouvoir faire tomber le vérificateur en lui envoyant n'importe quoi.
 *
 * ⚠️ Un pas de découpage NUL ne se contente pas d'être absurde : il rendrait
 * `Math.ceil(taille / 0)` égal à `Infinity`, et la boucle du plan ne
 * s'arrêterait pas. La garde est donc aussi ce qui empêche ce module de figer
 * son appelant.
 */
function verifierContrat(taille: number, tailleTranche: number): void {
    if (!Number.isInteger(taille) || taille < 0) {
        throw new Error(
            `tranches : taille invalide (${taille}) — un entier positif ou nul est attendu`,
        );
    }
    if (!Number.isInteger(tailleTranche) || tailleTranche <= 0) {
        throw new Error(
            `tranches : tailleTranche invalide (${tailleTranche}) — un entier strictement positif est attendu`,
        );
    }
}

/**
 * Le découpage ATTENDU d'un fichier de `taille` octets par pas de
 * `tailleTranche`.
 *
 * Les rangs sont contigus de `0` à `n - 1`, et la somme des `octets` vaut
 * EXACTEMENT `taille` — c'est l'invariant que les tests épinglent, et c'est le
 * seul qui distingue un plan juste d'un plan tronqué.
 *
 * 🔴 `Math.ceil` ET NON `Math.floor`, ET LA DIFFÉRENCE EST LA QUEUE DU FICHIER.
 * Avec `floor`, un fichier de 10 octets découpé par 4 rendrait DEUX tranches de
 * 4 — huit octets — et `verdict` déclarerait alors `complet` un fichier
 * TRONQUÉ de deux octets, sans qu'aucune trace ne le dise. C'est la mutation
 * qui juge ce module : elle ne casse aucun cas multiple exact, et elle abîme
 * silencieusement tous les autres.
 *
 * ⚠️ `taille === 0` REND ZÉRO TRANCHE, jamais une tranche vide. Un fichier vide
 * est un fichier légitime : il n'a rien à déposer, et son verdict est `complet`
 * sur une liste vide. Fabriquer une tranche de zéro octet obligerait le
 * déposant à envoyer une trame sans contenu pour sceller un fichier sans
 * contenu, et `ceil(0 / pas)` vaut déjà 0 — la propriété est celle de
 * l'arithmétique, pas d'un cas particulier ajouté à la main.
 *
 * ⚠️ UNE TAILLE MULTIPLE EXACTE DU PAS NE PRODUIT PAS DE TRANCHE FINALE VIDE,
 * pour la même raison : `ceil(8 / 4)` vaut 2, pas 3. La dernière tranche vaut
 * `taille - n * tailleTranche`, qui n'est nul que si aucune tranche n'existe.
 */
export function plan(taille: number, tailleTranche: number): Tranche[] {
    verifierContrat(taille, tailleTranche);

    const tranches: Tranche[] = [];
    const combien = Math.ceil(taille / tailleTranche);
    for (let n = 0; n < combien; n += 1) {
        // La dernière tranche est la seule qui puisse être plus courte que le
        // pas : `min` la borne sans qu'il faille traiter son cas à part.
        const octets = Math.min(tailleTranche, taille - n * tailleTranche);
        tranches.push({ n, octets });
    }
    return tranches;
}

/**
 * Confronte les tranches REÇUES au découpage ATTENDU, et rend l'un des trois
 * verdicts.
 *
 * 🔴 `incoherentes` PRIME SUR `manquantes`, ET L'ORDRE EST LA MOITIÉ DE LA
 * RÈGLE. Un dépôt qui porterait à la fois un trou et une tranche mal taillée
 * doit être signalé INCOHÉRENT : annoncer d'abord le trou ferait recompléter la
 * tranche absente, puis re-vérifier, puis retomber sur la même incohérence — la
 * boucle exacte que la distinction existe pour empêcher. On nomme d'abord ce
 * qui ne se répare pas.
 *
 * ⚠️ LES LISTES SONT TRIÉES ET SANS DOUBLON, toujours. Un verdict qui dépendrait
 * de l'ordre d'arrivée des tranches ne serait ni comparable d'une exécution à
 * l'autre, ni lisible dans un journal — et deux relevés du même défaut
 * paraîtraient différents.
 *
 * ⚠️ `presentes` EST DE LA DONNÉE DE FIL, ET SA FORME EST CELLE DE L'APPELANT.
 * Ce module ne parse pas : il suppose que les entrées ont déjà passé la garde
 * de forme, comme `parse*` de `fichiers-entetes.ts` la fait passer avant que
 * la règle ne s'applique. Une entrée dont le `n` ne serait pas un rang valide
 * n'est pour autant PAS silencieusement écartée — elle tombe dans
 * `incoherentes`, et la liste rend le `n` REÇU tel quel, pour qu'un journal
 * montre ce qui a réellement été envoyé plutôt qu'une valeur nettoyée.
 */
export function verdict(
    taille: number,
    tailleTranche: number,
    presentes: Tranche[],
): Verdict {
    verifierContrat(taille, tailleTranche);

    const attendu = new Map<number, number>();
    for (const t of plan(taille, tailleTranche)) attendu.set(t.n, t.octets);

    const incoherentes = new Set<number>();
    const vues = new Set<number>();

    for (const recue of presentes) {
        const { n, octets } = recue;

        // 🔴 UN DOUBLON EST UNE INCOHÉRENCE, MÊME SI LES DEUX OCCURRENCES
        // S'ACCORDENT SUR LA TAILLE. Deux dépôts qui revendiquent le même rang
        // veulent dire que l'un a écrasé l'autre, et RIEN ICI NE PEUT SAVOIR
        // LEQUEL A GAGNÉ : les octets réellement écrits peuvent venir de la
        // seconde trame comme de la première, et deux trames de même longueur
        // ne portent pas forcément le même contenu. Traiter le cas comme
        // anodin scellerait un fichier dont une tranche est indéterminée. On
        // le refuse, et on le nomme.
        if (vues.has(n)) {
            incoherentes.add(n);
            continue;
        }
        vues.add(n);

        // Un rang hors du plan — négatif, au-delà de la dernière tranche, ou
        // simplement pas un rang — n'a pas de taille attendue à laquelle le
        // comparer : il est incohérent par lui-même.
        const prevu = attendu.get(n);
        if (prevu === undefined) {
            incoherentes.add(n);
            continue;
        }

        // La comparaison est stricte DANS LES DEUX SENS : une tranche plus
        // COURTE que prévue est un transfert amputé, une tranche plus LONGUE
        // déborderait sur sa voisine. Ni l'une ni l'autre ne se répare en la
        // redemandant.
        if (!Number.isInteger(octets) || octets !== prevu) {
            incoherentes.add(n);
        }
    }

    if (incoherentes.size > 0) {
        return { etat: 'incoherentes', n: trier(incoherentes) };
    }

    const manquantes = new Set<number>();
    for (const n of attendu.keys()) {
        if (!vues.has(n)) manquantes.add(n);
    }
    if (manquantes.size > 0) {
        return { etat: 'manquantes', n: trier(manquantes) };
    }

    return { etat: 'complet' };
}

/**
 * Trie une liste de rangs par valeur croissante.
 *
 * ⚠️ LE COMPARATEUR EST EXPLICITE, et ce n'est pas de la coquetterie : le tri
 * par défaut de JavaScript compare des CHAÎNES, si bien que `[2, 10]` en
 * ressortirait `[10, 2]`. Un plan de plus de dix tranches suffit à rencontrer
 * le cas.
 */
function trier(rangs: Set<number>): number[] {
    return [...rangs].sort((a, b) => a - b);
}
