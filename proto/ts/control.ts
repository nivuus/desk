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

export type ClientControl = ResizeMessage | VisibilityMessage;

export interface ReadyMessage {
    v: number;
    type: 'ready';
    width: number;
    height: number;
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

export type AgentControl =
    | ReadyMessage | SessionEndMessage
    | PointerMessage | RumbleMessage | CapabilitiesMessage | LinkMessage
    | AsleepMessage;

const TYPES_AGENT = [
    'ready', 'session-end', 'pointer', 'rumble', 'capabilities', 'link', 'asleep',
] as const;

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

export function parseAgentControl(raw: string): AgentControl {
    const parsed = JSON.parse(raw) as Partial<AgentControl>;
    if (parsed.v !== CONTROL_VERSION) {
        throw new Error(`version de contrôle non supportée : ${parsed.v}`);
    }
    if (!TYPES_AGENT.includes(parsed.type as (typeof TYPES_AGENT)[number])) {
        throw new Error(`type de contrôle inconnu : ${parsed.type}`);
    }
    return parsed as AgentControl;
}
