// Messages du canal de contrôle. Doit rester aligné sur proto/src/control.rs.
//
// Canal fiable et ordonné, à faible débit : JSON versionné, lisible dans les
// journaux (contrairement au canal d'entrées binaire de proto/ts/input.ts).
// Le champ `v` est obligatoire et vérifié à l'analyse : un message absent de
// `v`, ou dont la version diffère de CONTROL_VERSION, est rejeté.

export const CONTROL_VERSION = 1;

export interface ResizeMessage {
    v: number;
    type: 'resize';
    width: number;
    height: number;
}

export type ClientControl = ResizeMessage;

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

export type AgentControl = ReadyMessage | SessionEndMessage;

export function encodeResize(width: number, height: number): string {
    const message: ResizeMessage = {
        v: CONTROL_VERSION,
        type: 'resize',
        width: Math.max(1, Math.round(width)),
        height: Math.max(1, Math.round(height)),
    };
    return JSON.stringify(message);
}

export function parseAgentControl(raw: string): AgentControl {
    const parsed = JSON.parse(raw) as Partial<AgentControl>;
    if (parsed.v !== CONTROL_VERSION) {
        throw new Error(`version de contrôle non supportée : ${parsed.v}`);
    }
    if (parsed.type !== 'ready' && parsed.type !== 'session-end') {
        throw new Error(`type de contrôle inconnu : ${parsed.type}`);
    }
    return parsed as AgentControl;
}
