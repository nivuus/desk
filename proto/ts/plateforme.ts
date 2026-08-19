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
