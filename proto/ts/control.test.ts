import { describe, expect, it } from 'vitest';
import { CONTROL_VERSION, encodeResize, parseAgentControl } from './control';

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
        const msg = parseAgentControl('{"v":2,"type":"ready","width":800,"height":600}');
        expect(msg).toEqual({ v: 2, type: 'ready', width: 800, height: 600 });
    });

    it('analyse une fin de session', () => {
        const msg = parseAgentControl('{"v":2,"type":"session-end","reason":"fermée"}');
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
        expect(() => parseAgentControl('{"v":2,"type":"autre"}')).toThrow(/type de contrôle/);
    });

    it('analyse un message de pointeur', () => {
        const message = parseAgentControl('{"type":"pointer","v":2,"visible":false,"shape":"ns-resize"}');
        expect(message).toEqual({ type: 'pointer', v: 2, visible: false, shape: 'ns-resize' });
    });

    it('analyse un message de vibration', () => {
        const message = parseAgentControl('{"type":"rumble","v":2,"left":255,"right":0}');
        expect(message).toEqual({ type: 'rumble', v: 2, left: 255, right: 0 });
    });

    it('analyse un message de capacités', () => {
        const message = parseAgentControl('{"type":"capabilities","v":2,"gamepad":false}');
        expect(message).toEqual({ type: 'capabilities', v: 2, gamepad: false });
    });

    it('rejette la version de contrôle 1, devenue obsolète', () => {
        expect(() => parseAgentControl('{"type":"ready","v":1,"width":1,"height":1}')).toThrow();
    });
});
