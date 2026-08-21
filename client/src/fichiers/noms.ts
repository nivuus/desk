// LE CANONICALISEUR DE NOM — le remède de casse EN LECTURE, et la seule
// réponse à ce que F1 lègue en n°1. **PUR** : ni DOM, ni WebRTC, ni trame ;
// le répertoire parent lui est INJECTÉ, comme à `adaptateur.ts`.
//
// ════════════════════════════════════════════════════════════════════════════
// 🔴 CE QUE CE MODULE CORRIGE, ET CE QU'IL NE PEUT PAS CORRIGER
// ════════════════════════════════════════════════════════════════════════════
//
// Le legs n°1 de F1 dit « la casse rend le mauvais fichier ». **Les deux
// moitiés du phénomène n'ont pas la même gravité, et la moitié que F1 a
// MESURÉE est probablement la bénigne.** Ceci est une RELECTURE de ses pièces,
// pas une mesure neuve, et c'est dit :
//
//   - **La moitié VM.** `Casse.txt` était HYDRATÉ (F1 relève
//     `racine hydratee … octets=42 entrees=1`). NTFS, insensible à la casse,
//     résout `casse.txt` sur le fichier local SANS JAMAIS ATTEINDRE LE PONT.
//     L'application obtient le bon contenu du bon fichier, et rien n'est écrit
//     sous un mauvais nom : aucun substitut n'est créé. **C'est le
//     comportement NORMAL de Windows, pas un défaut** — et ce module n'y peut
//     rien : quand NTFS répond, nous ne sommes pas consultés.
//   - **La moitié NAVIGATEUR.** Sur un poste local INSENSIBLE à la casse,
//     `getFileHandle('CASSE.TXT')` ouvre `Casse.txt`. **C'est là que le
//     mauvais fichier est rendu**, et c'est là qu'une écriture écraserait.
//     **Cette moitié-là n'a JAMAIS été observée** : l'instrument de recette
//     est OPFS, et si OPFS est sensible à la casse elle ne peut pas s'y
//     produire.
//   - **L'incohérence que F1 relève** — `casse.txt` passe, `GROS.BIN` échoue,
//     dans la MÊME exécution — se lit alors sans mystère : le premier est
//     résolu par NTFS sans nous, le second atteint le pont et bute sur un OPFS
//     sensible à la casse.
//
// ⚠️ CE QUI TRANCHERAIT : une exécution où le fichier demandé avec une autre
// casse n'a JAMAIS été hydraté, ET où le « poste local » est insensible à la
// casse. **Aucune des deux conditions n'est disponible sur ce montage**, et le
// document de résultats de F3 le redit.
//
// CE QUI EST DÉMONTRÉ ICI, EN REVANCHE : le comportement sur l'hôte, avec DEUX
// faux — l'un sensible, l'autre INSENSIBLE à la casse —, et la disparition de
// l'incohérence `GROS.BIN` sur l'instrument.
//
// ════════════════════════════════════════════════════════════════════════════
// 🔵 LA NORMALISATION UNICODE, QUE F2 DÉCLARE NON TRAITÉE ET LÈGUE ICI
// ════════════════════════════════════════════════════════════════════════════
//
// `client/src/fichiers/ecriture.ts` l'écrit en toutes lettres : « ELLE NE VOIT
// PAS LA NORMALISATION UNICODE. macOS stocke ses noms en NFD, Windows en NFC :
// `été.txt` peut y exister sous deux suites d'unités de code différentes, que
// `===` distingue et que l'utilisateur ne distingue pas. La garde créerait
// alors un DOUBLON au lieu d'écraser — moins grave que la perte, mais faux.
// NON TRAITÉ, déclaré ; c'est le canonicaliseur de F3. »
//
// **Il est traité** : le pliage applique `normalize('NFC')` AVANT le repli de
// casse. Deux noms qui ne diffèrent que par leur forme de normalisation sont
// donc des homonymes, exactement comme deux noms qui ne diffèrent que par la
// casse — et pour la même raison : **l'utilisateur ne les distingue pas**.
//
// ⚠️ L'ORDRE COMPTE, ET IL N'EST PAS ARBITRAIRE. `toLowerCase()` puis
// `normalize()` n'est pas la même fonction que `normalize()` puis
// `toLowerCase()` : le repli de casse d'Unicode peut produire des séquences
// qui se re-normalisent. On normalise D'ABORD.
//
// ════════════════════════════════════════════════════════════════════════════
// ⚠️ AUCUN CACHE. LE PARENT EST ÉNUMÉRÉ À CHAQUE RÉSOLUTION.
// ════════════════════════════════════════════════════════════════════════════
//
// C'est cher — `adaptateur.ts` déclare déjà le coût d'un `getFile()` par entrée
// au listage —, et c'est DÉLIBÉRÉ : un cache que rien n'invalide est
// exactement le défaut de l'ancien pont (`src/file.js`, cache SANS TTL), et le
// seul moyen de le vider — `Rafraichir` — est un livrable de **F5**.
//
// **F3 échange donc de la latence contre une correction, et c'est F4 qui dira
// ce que l'échange coûte.**
//
// ⛔ **F4 NE L'A PAS DIT, ET IL FAUT L'ÉCRIRE PLUTÔT QUE DE LAISSER CROIRE LE
// CONTRAIRE** (21 août 2026). Aucun geste de sa campagne n'exerce la
// canonicalisation de casse : ses gabarits n'ont ni homonyme de casse ni chemin
// à corriger. **Le coût de cet échange reste DÛ.** L'optimisation évidente — court-circuiter
// l'énumération quand `poignee.name` rend déjà le nom stocké — N'EST PAS
// ÉCRITE : elle repose sur un fait que ce montage ne peut pas établir (il
// faudrait un vrai `showDirectoryPicker()`, que F1 a mesuré inatteignable sur
// cet hôte). **Léguée, pas implémentée à moitié.**

import { EchecFichiers, classer, type PoigneeRepertoire } from './adaptateur';
import { CODES_ECHEC, type CodeEchec } from '../../../proto/ts/fichiers';

/** Ce que la résolution d'un composant de chemin peut rendre. */
export type Resolution =
    /** Le nom **CANONIQUE**, c'est-à-dire celui qui est STOCKÉ. */
    | { sorte: 'trouve'; nom: string }
    | { sorte: 'absent' }
    /** Plusieurs entrées se replient sur le même nom : on ne rend RIEN. */
    | { sorte: 'ambigu'; noms: string[] };

/**
 * Le pliage sous lequel deux noms sont « le même » pour un utilisateur.
 *
 * ⚠️ NFC D'ABORD, repli de casse ENSUITE — voir l'en-tête.
 */
export function plier(nom: string): string {
    return nom.normalize('NFC').toLowerCase();
}

/**
 * Résout `demande` dans `parent`, et rend le nom **STOCKÉ**.
 *
 * Les quatre issues, dans l'ordre où elles sont décidées :
 *
 *   1. un nom **exact** trouvé → c'est lui, et c'est le nom canonique ;
 *   2. aucun exact, **exactement UN** homonyme → c'est lui, et le nom
 *      canonique est **le nom STOCKÉ**, pas le nom demandé ;
 *   3. aucun exact, **PLUSIEURS** homonymes → `ambigu`, on ne rend rien ;
 *   4. rien du tout → `absent`.
 *
 * 🔴 LA RÈGLE 1 PRIME, ET CE N'EST PAS UN DÉTAIL. Sur un poste local SENSIBLE à
 * la casse portant `note.txt` ET `Note.txt`, demander `note.txt` est
 * parfaitement désigné : sans la primauté de l'exact, il deviendrait `ambigu`
 * et on refuserait une lecture légitime.
 */
export async function canoniser(
    parent: PoigneeRepertoire,
    demande: string,
): Promise<Resolution> {
    const cible = plier(demande);
    const homonymes: string[] = [];
    try {
        for await (const enfant of parent.values()) {
            // Règle 1 : l'exact court-circuite tout, y compris l'ambiguïté.
            if (enfant.name === demande) return { sorte: 'trouve', nom: demande };
            if (plier(enfant.name) === cible) homonymes.push(enfant.name);
        }
    } catch (e) {
        throw classer(e, 'chemin-introuvable');
    }
    if (homonymes.length === 1) return { sorte: 'trouve', nom: homonymes[0] };
    if (homonymes.length > 1) return { sorte: 'ambigu', noms: homonymes };
    return { sorte: 'absent' };
}

/**
 * [`canoniser`], mais qui LÈVE au lieu de rendre `absent` ou `ambigu`.
 *
 * `siAbsent` distingue les deux façons d'être introuvable, exactement comme
 * `adaptateur.classer` : un composant INTERMÉDIAIRE manquant rend
 * `chemin-introuvable`, le composant FINAL rend `introuvable`. ProjFS les
 * distingue aussi (`ERROR_PATH_NOT_FOUND` contre `ERROR_FILE_NOT_FOUND`), et
 * l'Explorateur n'en dit pas la même chose.
 */
export async function canoniserOuLever(
    parent: PoigneeRepertoire,
    demande: string,
    siAbsent: CodeEchec,
): Promise<string> {
    const r = await canoniser(parent, demande);
    switch (r.sorte) {
        case 'trouve':
            return r.nom;
        case 'absent':
            throw new EchecFichiers(siAbsent, `« ${demande} » n’existe pas`);
        case 'ambigu':
            // ⚠️ Le message NOMME les homonymes, parce que c'est tout ce que
            // l'utilisateur pourra faire : en renommer un. Le CODE traverse le
            // fil ; le message reste dans la console et dans la page-shell.
            throw new EchecFichiers(
                'casse-ambigue',
                `« ${demande} » ne se distingue pas de « ${r.noms.join(' », « ')} » : ` +
                    `rendre l’un d’eux choisirait le mauvais fichier, rien n’a été fait`,
            );
    }
}

// ════════════════════════════════════════════════════════════════════════════
// L'INJECTION DE FAUTE — l'instrument du critère (4) de F3
// ════════════════════════════════════════════════════════════════════════════
//
// TROIS des douze causes du §5 ne sont atteignables par AUCUN geste réel sur
// ce montage : `acces-refuse` (OPFS n'a aucun modèle de permission, F1 §3),
// `disque-plein` (`QuotaExceededError` n'y est pas provocable) et le délai
// dépassé (il faudrait un navigateur qui ne réponde jamais).
//
// ⚠️ **Cette phrase annonçait « quatre » et n'en nommait que trois.** Corrigé
// sur le compte, et la recette de F3 a trouvé les DEUX qui manquaient — elles
// ne relèvent pas de l'injection, mais d'un fait de plateforme :
// `repertoire-non-vide` et `deja-present` disent que le miroir a DÉRIVÉ, et
// **ProjFS montre à la VM le contenu que seul le poste local connaît**. Windows
// résout donc la dérive AVANT nous, et ces deux codes restent hors d'atteinte
// par un geste réel. Ce sont bien CINQ causes sur douze, pour deux raisons
// différentes qu'il ne faut pas confondre.
//
// ⚠️ **UNE INJECTION PROUVE QUE LA TABLE N'EST PAS DÉCORATIVE ; ELLE NE PROUVE
// PAS QUE LA CAUSE EST ATTEIGNABLE EN EXPLOITATION.** Les deux colonnes sont
// distinguées au §0.5 du plan, et le document de résultats les gardera
// distinctes.

/** Le préfixe d'un composant de chemin qui demande une faute. */
export const PREFIXE_FAUTE = '.faute-';

/**
 * Le suffixe qui demande un **silence** — le navigateur ne répond JAMAIS.
 *
 * ⚠️ Ce n'est pas un `CodeEchec` : il n'y a rien à mettre sur le fil, et c'est
 * précisément le point. Le pont doit constater l'expiration lui-même,
 * c'est-à-dire exercer `DelaiDepasse`, la seule cause qu'aucune réponse ne peut
 * produire.
 */
export const FAUTE_SILENCE = 'silence';

/**
 * Lève — ou se tait à jamais — si le PREMIER composant du chemin demande une
 * faute **et** que l'injection est ARMÉE.
 *
 * 🔴 **DÉSARMÉE PAR DÉFAUT, ET LE DRAPEAU EST UN ARGUMENT.** Le lire depuis ce
 * module (`location.search`, une variable de module) le rendrait intestable, et
 * surtout : un utilisateur qui créerait un dossier nommé `.faute-disque-plein`
 * casserait son propre pont. Le drapeau est lu **une fois** dans
 * `shell-page.ts` et passé en argument, comme `PLEIN_ECRAN` l'est côté agent.
 *
 * ⚠️ **LE PREMIER COMPOSANT, ET LUI SEUL.** Un balayage de tous les composants
 * ferait qu'un chemin traversant un dossier ainsi nommé — même en profondeur —
 * échouerait, ce qui rendrait l'injection difficile à cibler et impossible à
 * désarmer par le geste.
 */
export async function injecterFaute(
    parts: readonly string[],
    armee: boolean,
): Promise<void> {
    if (!armee || parts.length === 0) return;
    const premier = parts[0];
    if (!premier.startsWith(PREFIXE_FAUTE)) return;
    const demande = premier.slice(PREFIXE_FAUTE.length);
    if (demande === FAUTE_SILENCE) {
        // ⚠️ UNE PROMESSE QUI NE SE RÉSOUT JAMAIS. C'est le seul moyen
        // d'exercer `DelaiDepasse` : une réponse, quelle qu'elle soit,
        // empêcherait le pont d'expirer.
        return new Promise<void>(() => {});
    }
    if ((CODES_ECHEC as readonly string[]).includes(demande)) {
        throw new EchecFichiers(
            demande as CodeEchec,
            `faute injectée par « ${premier} » : banc, jamais une configuration livrée`,
        );
    }
    // ⚠️ Un `.faute-` dont le suffixe n'est PAS un code connu est DIT, jamais
    // avalé : sans cela, une coquille de recette produirait un « introuvable »
    // ordinaire, et l'opérateur croirait avoir exercé un code qu'il n'a pas
    // exercé.
    throw new EchecFichiers(
        'interne',
        `« ${premier} » demande une faute inconnue « ${demande} » : ` +
            `les codes connus sont ${CODES_ECHEC.join(', ')}, plus « ${FAUTE_SILENCE} »`,
    );
}
