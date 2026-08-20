/**
 * Miroir TypeScript de `proto/src/plateforme.rs` — le canal plateforme <->
 * agent (`/agent`).
 *
 * ⚠️ TOUTE MODIFICATION SE RÉPERCUTE DES DEUX CÔTÉS, et
 * `plateforme-vectors.json` est là pour que l'oubli se voie : il porte les
 * chaînes exactes, et les deux langages les vérifient.
 *
 * ⚠️ CE PARSEUR VALIDE À LA FRONTIÈRE PUIS CASTE — il ne vérifie PAS chaque
 * champ. C'est le précédent exact de `parseAgentControl` (`control.ts`), et il
 * est écrit ici pour qu'un lecteur ne croie pas à une validation plus forte
 * qu'elle n'est : `v` et `type` sont contrôlés, le reste est tenu pour conforme
 * parce que l'émetteur est le service lui-même, jamais un tiers.
 */

export const PLATEFORME_VERSION = 3;

/** Pourquoi la plateforme refuse. `enrolement` est INDISTINCT par
 * construction : distinguer « VM inconnue » de « secret faux » serait un
 * oracle d'énumération. */
export type MotifCanal = 'version' | 'forme' | 'enrolement' | 'sequence';

/**
 * Une application telle que l'agent la découvre sur le disque de la VM.
 *
 * ⚠️ `arguments` est BRUT et SENSIBLE À LA CASSE, contrairement à `cible` et
 * `repertoire` qui sont normalisés. Deux chemins Windows qui ne diffèrent que
 * par la casse désignent le même fichier ; deux lignes de commande qui ne
 * diffèrent que par la casse d'un argument sont deux invocations distinctes.
 */
export interface Application {
    /** Empreinte du triplet `(cible, arguments, repertoire)` — l'identité. */
    cle: string;
    /** Le nom du `.lnk`, sans son extension. */
    nom: string;
    /** Le chemin du `.lnk` LUI-MÊME, et c'est lui qu'on lance. */
    chemin: string;
    cible: string;
    /** BRUTS (voir ci-dessus). Vide = `''`, jamais absent. */
    arguments: string;
    repertoire: string;
    /**
     * L'empreinte SHA-256 du PNG de l'icône, en hexadécimal minuscule — ou
     * `null` quand l'extraction a échoué.
     *
     * ⚠️ UNE APPLICATION SANS ICÔNE VAUT MIEUX QU'UNE APPLICATION ABSENTE.
     * `null` n'est pas une erreur, et le champ reste PRÉSENT sur le fil.
     */
    icone: string | null;
    /** Toujours présent. Vaut `'non-mesuree'` quand `icone` est `null`. */
    source_max: SourceMax;
}

/**
 * D'où vient l'image : la plus grande entrée réellement PRÉSENTE dans le
 * répertoire d'icônes de la source.
 *
 * 🔴 CE N'EST PAS LA TAILLE RENDUE. Mesuré le 20 août 2026 sur deux témoins
 * fabriqués (`agent/testdata/g2-temoin-{48,256}.ico`) : un `.ico` ne contenant
 * QU'UNE entrée 48×48, interrogé à 256, rend 256×256 32bpp — par
 * `IShellItemImageFactory` comme par `PrivateExtractIconsW`, sans
 * `SIIGBF_SCALEUP` et MÊME avec `SIIGBF_BIGGERSIZEOK`. Un critère qui
 * comparerait la taille rendue à 256 NE PEUT PAS ÉCHOUER.
 *
 * 🔵 `'non-mesuree'` s'écrit avec un TIRET, jamais un tiret bas : c'est le
 * `rename_all = "kebab-case"` du Rust sur une variante à DEUX MOTS, donc la
 * seule du module dont la convention soit observable.
 */
export type SourceMax = { pixels: number } | 'non-mesuree';

/**
 * Le CHAMP `string` de `Application` à valider, un par un — voir
 * `estApplication`.
 *
 * 🔴 `icone` ET `source_max` N'Y SONT PAS, ET LES Y METTRE SERAIT UN DÉFAUT
 * SILENCIEUX. Cette liste est parcourue par `estChaine` : y ajouter `icone`
 * ferait REFUSER TOUT CATALOGUE dont une seule application n'a pas d'icône —
 * `null` n'est pas une chaîne —, avec le motif `forme`, c'est-à-dire un
 * catalogue entier perdu sans qu'aucune trace ne dise pourquoi. Les deux
 * champs neufs ont donc leurs propres gardes.
 */
const CHAMPS_APPLICATION: ReadonlyArray<keyof Application> = [
    'cle', 'nom', 'chemin', 'cible', 'arguments', 'repertoire',
];

/**
 * Ce qu'un ordre de lancement a réellement fait.
 *
 * 🔴 `raccourci` CONTRE `cible` EST CE QUI REND LE CRITÈRE DE RECETTE
 * DÉCIDABLE : lancer par la cible reconstruite au lieu du `.lnk` passerait un
 * critère qui ne dirait que « quelque chose s'est lancé ».
 *
 * ⚠️ CE N'EST PAS UN `MotifCanal` : deux valeurs de `MotifCanal` FERMENT le
 * socket, et un lancement raté ne doit fermer aucun canal.
 */
export type IssueLancement = 'raccourci' | 'cible' | 'inconnue' | 'echec';

const ISSUES: ReadonlyArray<IssueLancement> = ['raccourci', 'cible', 'inconnue', 'echec'];

export interface EnrolerMessage { v: number; type: 'enroler'; vm: string; secret: string }
export interface BattementMessage { v: number; type: 'battement' }
/**
 * Le catalogue de la VM, en DIFF.
 *
 * 🔴 `complet` A UNE SÉMANTIQUE NOMMÉE : à `true`, la plateforme marque
 * disparue TOUTE ligne de cette VM absente d'`applications` et ignore
 * `disparues` ; à `false`, elle applique le delta. L'agent émet `true` à
 * chaque (ré)enrôlement, ce qui rend la perte d'un message montant sans
 * conséquence — ce canal est un `push` sans garantie de livraison, et sans ce
 * renvoi complet une perte laisserait la plateforme divergente SANS TERME.
 */
export interface CatalogueMessage {
    v: number;
    type: 'catalogue';
    complet: boolean;
    applications: Application[];
    /** Des CLÉS, jamais des objets. */
    disparues: string[];
}
export interface LanceeMessage {
    v: number;
    type: 'lancee';
    demande: string;
    issue: IssueLancement;
}

export type VersLaPlateforme =
    | EnrolerMessage
    | BattementMessage
    | CatalogueMessage
    | LanceeMessage;

export interface EnroleMessage {
    v: number;
    type: 'enrole';
    prefixe: string;
    jeton: string;
    /** En MILLISECONDES, comme tout horodatage de ce service. */
    expire_a: number;
}
export interface BattementRecuMessage {
    v: number;
    type: 'battement-recu';
    jeton: string;
    expire_a: number;
}
/**
 * Le refus, et LA SEULE ENVELOPPE HORS VERSIONNEMENT de ce protocole.
 *
 * 🔴 `motif` EST UNE `string`, PAS UN `MotifCanal`, ET C'EST DÉLIBÉRÉ
 * (correction du 20 août 2026, miroir de `proto/src/plateforme.rs`). Un
 * lecteur doit pouvoir lire un refus émis par une version qu'il ne connaît
 * pas — sans quoi un agent périmé ne peut jamais apprendre POURQUOI il est
 * refusé et boucle sans terme, ce que la recette G1 a mesuré : 0 ligne de
 * refus, 10 reprises. Un motif ajouté par une version future doit donc rester
 * lisible et journalisable tel quel.
 *
 * ⚠️ EN ÉCRITURE, RIEN N'EST LIBRE : `encodeRefus` prend un `MotifCanal`. La
 * tolérance est une tolérance de LECTURE.
 *
 * 🔴 SA FORME EST GELÉE — `type`, `v`, `motif`, et rien d'autre, jamais. Un
 * champ ajouté ici serait rejeté par les lecteurs Rust antérieurs
 * (`deny_unknown_fields`) et annulerait à lui seul toute la tolérance.
 */
export interface RefusMessage { v: number; type: 'refus'; motif: string }
/**
 * Lancer une application de la VM.
 *
 * ⚠️ L'ORDRE NE PORTE PAS LE CHEMIN DU RACCOURCI, il porte la clé, et l'agent
 * la résout dans SON PROPRE catalogue — celui qu'il vient de lire sur le
 * disque. La copie de la plateforme peut être vieille d'une réconciliation ;
 * celle de l'agent ne l'est jamais. `demande` apparie l'ordre à sa `lancee`.
 */
export interface LancerMessage { v: number; type: 'lancer'; demande: string; cle: string }

/**
 * Les empreintes que la plateforme n'a PAS, parmi celles que le dernier
 * `catalogue` a annoncées.
 *
 * 🔴 ELLE N'EST PAS ÉMISE QUAND L'ENSEMBLE EST VIDE : une liste vide coûterait
 * un message par réconciliation sur un disque au repos, ce que le diff de G1
 * existe précisément pour éviter. La règle vit chez l'appelant
 * (`plateforme/src/agents/canal.ts`), qui seul connaît l'ensemble.
 *
 * ⚠️ LES OCTETS NE L'EMPRUNTENT JAMAIS : ce message ne porte qu'un inventaire.
 * Les images passent par `PUT /icone/:sha256`.
 */
export interface IconesManquantesMessage {
    v: number;
    type: 'icones-manquantes';
    empreintes: string[];
}

export type DepuisLaPlateforme =
    | EnroleMessage
    | BattementRecuMessage
    | RefusMessage
    | LancerMessage
    | IconesManquantesMessage;

/**
 * Les seuls `type` que ce parseur accepte — le sens PLATEFORME -> AGENT.
 *
 * 🔴 `enroler` et `battement` en sont ABSENTS À DESSEIN : les accepter ferait
 * qu'un pair traiterait son propre message comme une réponse, confusion de
 * sens qu'aucun contrôle de version ne verrait.
 *
 * 🔴 LA LISTE EST DÉRIVÉE DE L'UNION, ET C'EST UN REMÈDE STRUCTUREL, PAS UN
 * TEST DE PLUS. Écrite à la main, elle est le jumeau exact de `TYPES_AGENT`
 * (`control.ts`), que rien ne confronte à son union et dont l'oubli ne casse
 * « ni compilation ni test ». Ici, `Record<DepuisLaPlateforme['type'], true>`
 * fait REFUSER PAR `tsc` toute variante ajoutée à l'union sans sa clé — la
 * rouge est le typecheck lui-même, et elle a été jouée.
 *
 * ⚠️ L'exhaustivité seule est vérifiée par le type ; l'ABSENCE des types du
 * sens inverse ne l'est pas — un `enroler: true` de trop serait une erreur
 * `tsc` (clé hors de l'union), donc les deux sens sont bien couverts.
 */
const TOUS_DEPUIS: Record<DepuisLaPlateforme['type'], true> = {
    enrole: true,
    'battement-recu': true,
    refus: true,
    lancer: true,
    'icones-manquantes': true,
};
const TYPES_DEPUIS = Object.keys(TOUS_DEPUIS) as DepuisLaPlateforme['type'][];

/** Exposée pour que le test puisse comparer la liste dérivée à son union. */
export function typesDepuis(): readonly DepuisLaPlateforme['type'][] {
    return TYPES_DEPUIS;
}

/**
 * ⚠️ L'ORDRE DES CHAMPS EST `type` PUIS `v`, ET IL EST DÉLIBÉRÉ : serde émet le
 * tag interne EN PREMIER (`plateforme.rs`), et `JSON.stringify` respecte
 * l'ordre d'insertion. Écrire `v` d'abord — ce que fait `control.ts` — produit
 * une chaîne DIFFÉRENTE de celle du Rust. Rien ne casserait pour autant (les
 * deux bouts parsent du JSON, ils ne comparent pas des chaînes), mais
 * `plateforme-vectors.json` fige la chaîne EXACTE et la vérifie des deux
 * côtés : la parité octet pour octet est ce qui rend ce vecteur décidable.
 * Cette divergence a été trouvée par le test, pas par la relecture.
 */
export function encodeEnroler(vm: string, secret: string): string {
    const message: EnrolerMessage = { type: 'enroler', v: PLATEFORME_VERSION, vm, secret };
    return JSON.stringify(message);
}

export function encodeBattement(): string {
    const message: BattementMessage = { type: 'battement', v: PLATEFORME_VERSION };
    return JSON.stringify(message);
}

export function encodeCatalogue(
    complet: boolean,
    applications: Application[],
    disparues: string[],
): string {
    const message: CatalogueMessage = {
        type: 'catalogue',
        v: PLATEFORME_VERSION,
        complet,
        applications,
        disparues,
    };
    return JSON.stringify(message);
}

export function encodeLancee(demande: string, issue: IssueLancement): string {
    const message: LanceeMessage = { type: 'lancee', v: PLATEFORME_VERSION, demande, issue };
    return JSON.stringify(message);
}

/**
 * Les seuls `type` que le parseur de la PLATEFORME accepte — le sens
 * AGENT -> PLATEFORME.
 *
 * 🔴 Symétrique de `TYPES_DEPUIS`, et DÉRIVÉE DE L'UNION pour la même raison
 * exactement : les types de réponse en sont ABSENTS À DESSEIN. Les accepter
 * ferait que la plateforme traiterait sa propre réponse comme une demande.
 */
const TOUS_VERS: Record<VersLaPlateforme['type'], true> = {
    enroler: true,
    battement: true,
    catalogue: true,
    lancee: true,
};
const TYPES_VERS = Object.keys(TOUS_VERS) as VersLaPlateforme['type'][];

/** Exposée pour que le test puisse comparer la liste dérivée à son union. */
export function typesVers(): readonly VersLaPlateforme['type'][] {
    return TYPES_VERS;
}

/**
 * Ce que rend `parseVersLaPlateforme`.
 *
 * 🔴 UN VERDICT, JAMAIS UNE EXCEPTION, et c'est ce qui le distingue de son
 * jumeau `parseDepuisLaPlateforme`. La plateforme doit RÉPONDRE un motif typé
 * au pair — `{"type":"refus","motif":…}` — avant de décider quoi faire du
 * socket ; une exception l'obligerait à deviner le motif depuis un message
 * d'erreur, ou à répondre le même motif pour toutes les causes.
 */
export type LectureVersLaPlateforme =
    | { ok: true; message: VersLaPlateforme }
    | { ok: false; motif: MotifCanal };

function estObjetJson(valeur: unknown): valeur is Record<string, unknown> {
    return typeof valeur === 'object' && valeur !== null && !Array.isArray(valeur);
}

function chaineNonVide(valeur: unknown): valeur is string {
    return typeof valeur === 'string' && valeur.length > 0;
}

/** ⚠️ `arguments` est LÉGITIMEMENT VIDE : la garde est `string`, pas `chaineNonVide`. */
function estChaine(valeur: unknown): valeur is string {
    return typeof valeur === 'string';
}

/**
 * ⚠️ `null` EST UNE VALEUR ATTENDUE, PAS UNE ABSENCE. La garde exige que la
 * clé soit PRÉSENTE — `'icone' in valeur` — puis que sa valeur soit `null` ou
 * une chaîne. Se contenter de `=== null || typeof === 'string'` accepterait
 * un objet SANS le champ, `valeur.icone` valant alors `undefined`… qui n'est
 * ni `null` ni une chaîne, donc le cas serait refusé par accident. Écrire la
 * présence explicitement rend la propriété lisible plutôt qu'heureuse.
 */
function estIcone(valeur: Record<string, unknown>): boolean {
    if (!('icone' in valeur)) return false;
    return valeur.icone === null || estChaine(valeur.icone);
}

/**
 * 🔴 UN OBJET QUELCONQUE NE PASSE PAS. `{"pixels":"gros"}` est refusé, et
 * `{"pixels":256,"bonus":1}` aussi : la forme est exactement l'une des deux
 * que le Rust sait émettre, et rien d'autre.
 */
function estSourceMax(valeur: unknown): valeur is SourceMax {
    if (valeur === 'non-mesuree') return true;
    if (!estObjetJson(valeur)) return false;
    const cles = Object.keys(valeur);
    if (cles.length !== 1 || cles[0] !== 'pixels') return false;
    return typeof valeur.pixels === 'number' && Number.isInteger(valeur.pixels);
}

function estApplication(valeur: unknown): valeur is Application {
    if (!estObjetJson(valeur)) return false;
    if (!CHAMPS_APPLICATION.every((champ) => estChaine(valeur[champ]))) return false;
    return estIcone(valeur) && estSourceMax(valeur.source_max);
}

function estIssue(valeur: unknown): valeur is IssueLancement {
    return ISSUES.includes(valeur as IssueLancement);
}

/**
 * Lit un message venant de l'agent.
 *
 * ⚠️ CELUI-CI VALIDE CHAQUE CHAMP, LÀ OÙ SON JUMEAU CASTE, et l'asymétrie est
 * délibérée : l'émetteur d'un `DepuisLaPlateforme` est le service lui-même,
 * l'émetteur d'un `VersLaPlateforme` est un pair du réseau. C'est le SEUL
 * endroit de ce fichier où les octets viennent d'un tiers qui n'a aucune
 * raison d'être bien élevé — un `vm` absent traverserait sinon jusqu'à la
 * requête SQL, et le refus qui en sortirait dirait `enrolement`, c'est-à-dire
 * « secret faux », pour un message qui n'a jamais porté de VM.
 *
 * 🔴 LA VERSION EST CONTRÔLÉE AVANT LE TYPE. Elle est l'enveloppe : un message
 * d'une version future peut donner à un `type` connu un sens que nous
 * ignorons, et le refuser pour `forme` désignerait la mauvaise cause au pair
 * qui lit le motif pour décider s'il doit se mettre à jour ou se corriger.
 */
export function parseVersLaPlateforme(raw: string): LectureVersLaPlateforme {
    let parsed: unknown;
    try {
        parsed = JSON.parse(raw);
    } catch {
        return { ok: false, motif: 'forme' };
    }
    // `null` est le cas dangereux : `null.type` LÈVE, quand un nombre ou une
    // chaîne rendraient `undefined` par auto-boxing. Même garde que le relais.
    if (!estObjetJson(parsed)) return { ok: false, motif: 'forme' };

    // 🔴 STRICTE, ET SANS DÉFAUT : un `parsed.v ?? PLATEFORME_VERSION`
    // accepterait un message SANS champ `v`, et un `v: null` avec lui.
    if (parsed.v !== PLATEFORME_VERSION) return { ok: false, motif: 'version' };

    if (!TYPES_VERS.includes(parsed.type as (typeof TYPES_VERS)[number])) {
        return { ok: false, motif: 'forme' };
    }

    if (parsed.type === 'enroler') {
        if (!chaineNonVide(parsed.vm) || !chaineNonVide(parsed.secret)) {
            return { ok: false, motif: 'forme' };
        }
        return {
            ok: true,
            message: { type: 'enroler', v: PLATEFORME_VERSION, vm: parsed.vm, secret: parsed.secret },
        };
    }

    if (parsed.type === 'catalogue') {
        // ⚠️ CHAQUE CHAMP EST VALIDÉ, et pas seulement le type : c'est le seul
        // parseur du fichier dont les octets viennent d'un tiers. Un
        // `applications` absent traverserait sinon jusqu'à la requête SQL.
        if (typeof parsed.complet !== 'boolean') return { ok: false, motif: 'forme' };
        if (!Array.isArray(parsed.applications)) return { ok: false, motif: 'forme' };
        if (!Array.isArray(parsed.disparues) || !parsed.disparues.every(estChaine)) {
            return { ok: false, motif: 'forme' };
        }
        if (!parsed.applications.every(estApplication)) return { ok: false, motif: 'forme' };
        return {
            ok: true,
            message: {
                type: 'catalogue',
                v: PLATEFORME_VERSION,
                complet: parsed.complet,
                applications: parsed.applications,
                disparues: parsed.disparues,
            },
        };
    }

    if (parsed.type === 'lancee') {
        if (!chaineNonVide(parsed.demande)) return { ok: false, motif: 'forme' };
        if (!estIssue(parsed.issue)) return { ok: false, motif: 'forme' };
        return {
            ok: true,
            message: {
                type: 'lancee',
                v: PLATEFORME_VERSION,
                demande: parsed.demande,
                issue: parsed.issue,
            },
        };
    }

    return { ok: true, message: { type: 'battement', v: PLATEFORME_VERSION } };
}

/**
 * Les trois réponses de la plateforme, encodées ICI et nulle part ailleurs.
 *
 * 🔴 L'ORDRE DES CHAMPS EST `type` PUIS `v`, comme pour le sens inverse et
 * pour la même raison : serde émet le tag interne en premier, et
 * `plateforme-vectors.json` fige la chaîne EXACTE que les DEUX langages
 * doivent produire. Ces trois fonctions existent précisément pour que la
 * plateforme n'ait pas à recopier la forme du message dans son propre code —
 * une copie divergerait en silence, et rien ne comparerait plus rien.
 */
export function encodeEnrole(prefixe: string, jeton: string, expireA: number): string {
    const message: EnroleMessage = {
        type: 'enrole',
        v: PLATEFORME_VERSION,
        prefixe,
        jeton,
        expire_a: expireA,
    };
    return JSON.stringify(message);
}

export function encodeBattementRecu(jeton: string, expireA: number): string {
    const message: BattementRecuMessage = {
        type: 'battement-recu',
        v: PLATEFORME_VERSION,
        jeton,
        expire_a: expireA,
    };
    return JSON.stringify(message);
}

export function encodeRefus(motif: MotifCanal): string {
    const message: RefusMessage = { type: 'refus', v: PLATEFORME_VERSION, motif };
    return JSON.stringify(message);
}

export function encodeLancer(demande: string, cle: string): string {
    const message: LancerMessage = { type: 'lancer', v: PLATEFORME_VERSION, demande, cle };
    return JSON.stringify(message);
}

/**
 * ⚠️ L'APPELANT DOIT VÉRIFIER QUE `empreintes` N'EST PAS VIDE avant d'appeler :
 * cet encodeur ne le fait pas pour lui, parce qu'il ne saurait pas quoi rendre
 * à la place.
 */
export function encodeIconesManquantes(empreintes: string[]): string {
    const message: IconesManquantesMessage = {
        type: 'icones-manquantes',
        v: PLATEFORME_VERSION,
        empreintes,
    };
    return JSON.stringify(message);
}

/**
 * Lit un message venant de la plateforme.
 *
 * 🔴 LA COMPARAISON DE VERSION EST STRICTE (`!==`), ET LE CHAMP N'A AUCUN
 * DÉFAUT. Un `parsed.v ?? PLATEFORME_VERSION` accepterait un message SANS
 * champ `v`, et un message `v: null` avec lui — c'est exactement le trou que
 * `verifie_version` refuse côté Rust, et que son commentaire nomme.
 *
 * 🔴 LE `refus` EN EST EXEMPTÉ, ET LE TYPE EST DONC CONTRÔLÉ AVANT LA VERSION
 * ICI — à l'inverse exact de `parseVersLaPlateforme`, qui contrôle la version
 * d'abord. L'asymétrie est le remède du 20 août 2026 : un refus dit « je ne te
 * servirai pas », ce qui se comprend sans négociation de version, et c'est le
 * SEUL message qu'un pair périmé doive pouvoir lire. Le champ `v` y reste
 * obligatoire et reste un nombre — seule sa VALEUR est tolérée —, sans quoi on
 * rouvrirait le trou du `v` absent que le paragraphe ci-dessus ferme.
 *
 * ⚠️ CETTE FONCTION N'EST APPELÉE PAR AUCUN CODE DE LA PLATEFORME, qui ÉMET
 * les `DepuisLaPlateforme` sans jamais en lire. Elle est le miroir exécutable
 * du lecteur Rust, et c'est `plateforme-vectors.json` (clé `refus_lisibles`)
 * qui force les deux à s'accorder.
 */
export function parseDepuisLaPlateforme(raw: string): DepuisLaPlateforme {
    const parsed = JSON.parse(raw) as Partial<DepuisLaPlateforme>;
    if (!TYPES_DEPUIS.includes(parsed.type as (typeof TYPES_DEPUIS)[number])) {
        throw new Error(`type de message de plateforme inconnu : ${parsed.type}`);
    }
    if (typeof parsed.v !== 'number') {
        throw new Error(`version de plateforme absente ou non numérique : ${parsed.v}`);
    }
    if (parsed.type !== 'refus' && parsed.v !== PLATEFORME_VERSION) {
        throw new Error(`version de plateforme non supportée : ${parsed.v}`);
    }
    return parsed as DepuisLaPlateforme;
}
