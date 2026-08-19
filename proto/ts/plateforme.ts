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

export const PLATEFORME_VERSION = 1;

/** Pourquoi la plateforme refuse. `enrolement` est INDISTINCT par
 * construction : distinguer « VM inconnue » de « secret faux » serait un
 * oracle d'énumération. */
export type MotifCanal = 'version' | 'forme' | 'enrolement' | 'sequence';

export interface EnrolerMessage { v: number; type: 'enroler'; vm: string; secret: string }
export interface BattementMessage { v: number; type: 'battement' }

export type VersLaPlateforme = EnrolerMessage | BattementMessage;

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
export interface RefusMessage { v: number; type: 'refus'; motif: MotifCanal }

export type DepuisLaPlateforme = EnroleMessage | BattementRecuMessage | RefusMessage;

/**
 * Les seuls `type` que ce parseur accepte — le sens PLATEFORME -> AGENT.
 *
 * 🔴 `enroler` et `battement` en sont ABSENTS À DESSEIN : les accepter ferait
 * qu'un pair traiterait son propre message comme une réponse, confusion de
 * sens qu'aucun contrôle de version ne verrait.
 */
const TYPES_DEPUIS = ['enrole', 'battement-recu', 'refus'] as const;

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

/**
 * Les seuls `type` que le parseur de la PLATEFORME accepte — le sens
 * AGENT -> PLATEFORME.
 *
 * 🔴 Symétrique de `TYPES_DEPUIS`, et pour la même raison exactement : les
 * trois types de réponse en sont ABSENTS À DESSEIN. Les accepter ferait que la
 * plateforme traiterait sa propre réponse comme une demande.
 */
const TYPES_VERS = ['enroler', 'battement'] as const;

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

/**
 * Lit un message venant de la plateforme.
 *
 * 🔴 LA COMPARAISON DE VERSION EST STRICTE (`!==`), ET LE CHAMP N'A AUCUN
 * DÉFAUT. Un `parsed.v ?? PLATEFORME_VERSION` accepterait un message SANS
 * champ `v`, et un message `v: null` avec lui — c'est exactement le trou que
 * `verifie_version` refuse côté Rust, et que son commentaire nomme.
 */
export function parseDepuisLaPlateforme(raw: string): DepuisLaPlateforme {
    const parsed = JSON.parse(raw) as Partial<DepuisLaPlateforme>;
    if (parsed.v !== PLATEFORME_VERSION) {
        throw new Error(`version de plateforme non supportée : ${parsed.v}`);
    }
    if (!TYPES_DEPUIS.includes(parsed.type as (typeof TYPES_DEPUIS)[number])) {
        throw new Error(`type de message de plateforme inconnu : ${parsed.type}`);
    }
    return parsed as DepuisLaPlateforme;
}
