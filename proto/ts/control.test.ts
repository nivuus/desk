import { describe, expect, it } from 'vitest';
import { CONTROL_VERSION, encodeResize, encodeVisibility, parseAgentControl } from './control';

describe('protocole de contrôle', () => {
    it('encode un redimensionnement', () => {
        expect(JSON.parse(encodeResize(1280, 720))).toEqual({
            v: CONTROL_VERSION,
            type: 'resize',
            width: 1280,
            height: 720,
        });
    });

    it('arrondit et borne les dimensions', () => {
        expect(JSON.parse(encodeResize(0, 719.6))).toEqual({
            v: CONTROL_VERSION,
            type: 'resize',
            width: 1,
            height: 720,
        });
    });

    it('analyse un message ready', () => {
        const msg = parseAgentControl(`{"v":${CONTROL_VERSION},"type":"ready","width":800,"height":600}`);
        expect(msg).toEqual({ v: CONTROL_VERSION, type: 'ready', width: 800, height: 600 });
    });

    it('analyse une fin de session', () => {
        const msg = parseAgentControl(`{"v":${CONTROL_VERSION},"type":"session-end","reason":"fermée"}`);
        expect(msg.type).toBe('session-end');
    });

    it('rejette une version inconnue', () => {
        expect(() => parseAgentControl('{"v":9,"type":"ready","width":1,"height":1}')).toThrow(
            /version de contrôle/,
        );
    });

    it('rejette une version absente', () => {
        expect(() => parseAgentControl('{"type":"ready","width":1,"height":1}')).toThrow(
            /version de contrôle/,
        );
    });

    it('rejette un type inconnu', () => {
        expect(() => parseAgentControl(`{"v":${CONTROL_VERSION},"type":"autre"}`)).toThrow(/type de contrôle/);
    });

    it('analyse un message de pointeur', () => {
        const message = parseAgentControl(
            `{"type":"pointer","v":${CONTROL_VERSION},"visible":false,"shape":"ns-resize"}`,
        );
        expect(message).toEqual({ type: 'pointer', v: CONTROL_VERSION, visible: false, shape: 'ns-resize' });
    });

    it('analyse un message de vibration', () => {
        const message = parseAgentControl(`{"type":"rumble","v":${CONTROL_VERSION},"left":255,"right":0}`);
        expect(message).toEqual({ type: 'rumble', v: CONTROL_VERSION, left: 255, right: 0 });
    });

    it('analyse un message de capacités', () => {
        const message = parseAgentControl(`{"type":"capabilities","v":${CONTROL_VERSION},"gamepad":false}`);
        expect(message).toEqual({ type: 'capabilities', v: CONTROL_VERSION, gamepad: false });
    });

    it('rejette la version de contrôle 1, devenue obsolète', () => {
        expect(() => parseAgentControl('{"type":"ready","v":1,"width":1,"height":1}')).toThrow();
    });

    it("analyse l'état du lien", () => {
        const message = parseAgentControl(
            JSON.stringify({
                type: 'link',
                v: CONTROL_VERSION,
                bitrate: 4_000_000,
                width: 1280,
                height: 720,
                quality: 'degradee',
                adaptation: 'active',
            }),
        );
        expect(message).toEqual({
            type: 'link',
            v: CONTROL_VERSION,
            bitrate: 4_000_000,
            width: 1280,
            height: 720,
            quality: 'degradee',
            adaptation: 'active',
        });
    });

    it('encode une visibilité à la version courante', () => {
        const json = JSON.parse(encodeVisibility(false, true));
        expect(json).toEqual({ v: CONTROL_VERSION, type: 'visibility', visible: false, focused: true });
    });

    it('accepte un message asleep venant de l’agent', () => {
        const raw = JSON.stringify({ v: CONTROL_VERSION, type: 'asleep', asleep: true, reason: 'evincee' });
        expect(parseAgentControl(raw)).toEqual({
            v: CONTROL_VERSION, type: 'asleep', asleep: true, reason: 'evincee',
        });
    });
});
