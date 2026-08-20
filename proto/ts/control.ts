// Messages du canal de contrôle. Doit rester aligné sur proto/src/control.rs.
//
// Canal fiable et ordonné, à faible débit : JSON versionné, lisible dans les
// journaux (contrairement au canal d'entrées binaire de proto/ts/input.ts).
// Le champ `v` est obligatoire et vérifié à l'analyse : un message absent de
// `v`, ou dont la version diffère de CONTROL_VERSION, est rejeté.

export const CONTROL_VERSION = 3;

/** Valeurs de la propriété CSS `cursor` que l'agent sait produire. */
export type CursorShape =
    | 'default' | 'text' | 'wait' | 'progress' | 'crosshair' | 'pointer'
    | 'move' | 'not-allowed' | 'help'
    | 'ns-resize' | 'ew-resize' | 'nwse-resize' | 'nesw-resize';

export interface ResizeMessage {
    v: number;
    type: 'resize';
    width: number;
    height: number;
}

export interface VisibilityMessage {
    v: number;
    type: 'visibility';
    visible: boolean;
    focused: boolean;
}

/// L'utilisateur a collé dans la fenêtre de session (sous-bloc P2).
///
/// ⚠️ **Le nom porte `Client` là où son jumeau descendant porte `Agent`**
/// (`ClipboardAgentMessage`), et les DEUX portent le même tag `'clipboard'` :
/// c'est le sens qui les distingue, pas le tag. P1 avait réservé ce nom avec
/// sa raison, deux interfaces plus bas ; P2 l'occupe.
///
/// ⚠️ **`text` est un `string`, jamais `string | null`, et l'asymétrie avec
/// `ClipboardAgentMessage` est voulue** : là-bas le `null` PORTE le refus de
/// taille, que le bandeau doit dire. Ici c'est le client qui borne AVANT
/// d'émettre — il a le bandeau sous la main —, donc il n'a jamais de refus à
/// exprimer dans le message.
export interface ClipboardClientMessage {
    v: number;
    type: 'clipboard';
    text: string;
}

export type ClientControl = ResizeMessage | VisibilityMessage | ClipboardClientMessage;

export interface ReadyMessage {
    v: number;
    type: 'ready';
    width: number;
    height: number;
    /**
     * Le micro est-il disponible pour cette session (chantier E) ?
     *
     * OPTIONNEL à dessein, et sans bump de `CONTROL_VERSION` : face à un agent
     * ancien le champ vaut `undefined`, donc falsy, donc aucun bouton n'est
     * proposé — la règle de la spec §10, obtenue gratuitement. Ne jamais le
     * rendre obligatoire : ce serait une rupture de compatibilité que le
     * numéro de version ne signalerait pas.
     */
    mic?: boolean;
}

export interface SessionEndMessage {
    v: number;
    type: 'session-end';
    reason: string;
}

export interface PointerMessage {
    v: number;
    type: 'pointer';
    visible: boolean;
    shape: CursorShape;
}

export interface RumbleMessage {
    v: number;
    type: 'rumble';
    left: number;
    right: number;
}

export interface CapabilitiesMessage {
    v: number;
    type: 'capabilities';
    gamepad: boolean;
    /// Le collage navigateur → VM est-il disponible (sous-bloc P2) ?
    ///
    /// **OPTIONNEL, et c'est le point** : un agent d'avant P2 ne porte pas ce
    /// champ, et `undefined` vaut alors `false` gratuitement — le client
    /// n'arme rien, exactement comme `ReadyMessage.mic`. Le rendre obligatoire
    /// ferait échouer `tsc` sur un message parfaitement légitime.
    clipboard?: boolean;
}

export type LinkQuality = 'bonne' | 'degradee' | 'insuffisante';
export type LinkAdaptation = 'active' | 'indisponible';

export interface LinkMessage {
    v: number;
    type: 'link';
    bitrate: number;
    width: number;
    height: number;
    quality: LinkQuality;
    adaptation: LinkAdaptation;
}

export interface AsleepMessage {
    v: number;
    type: 'asleep';
    asleep: boolean;
    reason: string;
}

export interface FullscreenMessage {
    v: number;
    type: 'fullscreen';
    active: boolean;
}

/// Le presse-papier de la VM a changé.
///
/// L'interface s'appelle `ClipboardAgentMessage` et non `ClipboardMessage`,
/// alors qu'elle est seule aujourd'hui : le sous-bloc P2 ajoutera un
/// `ClipboardClientMessage` portant le MÊME tag `'clipboard'` dans l'autre
/// sens. Aucune collision réelle — `parseAgentControl` n'analyse que
/// `AgentControl`, et un message client ne passe jamais par là — mais les
/// deux interfaces ne peuvent pas porter le même nom. Nommer celle-ci
/// maintenant évite à P2 de renommer du code livré.
///
/// ⚠️ **`text` est `string | null`, jamais optionnel.** L'agent l'émet
/// toujours ; un `?` ferait passer un message tronqué en route pour un refus.
/// `null` EST le refus, et `bytes` en porte alors la taille.
export interface ClipboardAgentMessage {
    v: number;
    type: 'clipboard';
    text: string | null;
    bytes: number;
}

export type AgentControl =
    | ReadyMessage | SessionEndMessage
    | PointerMessage | RumbleMessage | CapabilitiesMessage | LinkMessage
    | AsleepMessage | FullscreenMessage | ClipboardAgentMessage;

/// 🔴 Écrit comme un enregistrement EXHAUSTIF typé par l'union, jamais comme
/// un littéral : ajouter une variante à `AgentControl` sans ajouter sa clé
/// ici fait échouer `npm run typecheck`, parce qu'un `Record<K, true>` dont
/// une clé manque est une erreur `tsc`.
///
/// **Avant ce remède, l'oubli ne cassait NI la compilation NI aucun test.**
/// `parseAgentControl` levait, `client/src/webrtc.ts` interceptait, et le
/// message était simplement perdu contre un `console.warn` — un mode de
/// défaillance entièrement silencieux, sur le fichier qui définit le
/// protocole. C'est le même remède que celui du chantier de gestion d'apps,
/// appliqué ici à sa source.
const TOUS_AGENT: Record<AgentControl['type'], true> = {
    ready: true,
    'session-end': true,
    pointer: true,
    rumble: true,
    capabilities: true,
    link: true,
    asleep: true,
    fullscreen: true,
    clipboard: true,
};

/// Exporté pour que la dérivation ait un témoin d'EXÉCUTION, et pas seulement
/// un témoin de compilation. Élargissement de surface assumé et déclaré :
/// sans lui, `TOUS_AGENT` n'est gardé que par `tsc`, et un test ne peut pas
/// constater que la liste et l'union coïncident.
export const TYPES_AGENT = Object.keys(TOUS_AGENT) as AgentControl['type'][];

export function encodeResize(width: number, height: number): string {
    const message: ResizeMessage = {
        v: CONTROL_VERSION,
        type: 'resize',
        width: Math.max(1, Math.round(width)),
        height: Math.max(1, Math.round(height)),
    };
    return JSON.stringify(message);
}

export function encodeVisibility(visible: boolean, focused: boolean): string {
    const message: VisibilityMessage = {
        v: CONTROL_VERSION,
        type: 'visibility',
        visible,
        focused,
    };
    return JSON.stringify(message);
}

/// Encode un collage venu du navigateur.
///
/// 🔴 **Il n'existe délibérément AUCUN `TYPES_CLIENT` pour garder cette union
/// exhaustive, et ce n'est pas un oubli.** `TOUS_AGENT` existe parce que
/// `parseAgentControl` PARSE `AgentControl` côté TypeScript : une variante
/// oubliée y était perdue contre un `console.warn`, en silence. Dans ce
/// sens-ci il n'y a rien à parser — le client ENCODE, et c'est `serde` qui
/// désérialise côté Rust, avec un `match` exhaustif que le compilateur garde
/// (il l'a d'ailleurs exigé au moment où la variante est née). Un témoin
/// d'exhaustivité écrit ici serait **incapable d'échouer**, c'est-à-dire le
/// mode de défaillance que ce dépôt paie depuis D7.
export function encodeClipboard(text: string): string {
    const message: ClipboardClientMessage = {
        v: CONTROL_VERSION,
        type: 'clipboard',
        text,
    };
    return JSON.stringify(message);
}

/// ⚠️ **Ce parseur ne valide QUE `v` et `type`**, puis CASTE. Il ne regarde
/// aucun autre champ, et un test qui prétendrait vérifier qu'il « accepte un
/// `capabilities` sans `clipboard` » serait donc décoratif : il ne pourrait
/// pas échouer. C'est le typage (`clipboard?: boolean`) qui porte cette
/// propriété, et un `satisfies` qui la mesure.
export function parseAgentControl(raw: string): AgentControl {
    const parsed = JSON.parse(raw) as Partial<AgentControl>;
    if (parsed.v !== CONTROL_VERSION) {
        throw new Error(`version de contrôle non supportée : ${parsed.v}`);
    }
    if (!TYPES_AGENT.includes(parsed.type as AgentControl['type'])) {
        throw new Error(`type de contrôle inconnu : ${parsed.type}`);
    }
    return parsed as AgentControl;
}
