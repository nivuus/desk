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
        const msg = parseAgentControl('{"v":1,"type":"ready","width":800,"height":600}');
        expect(msg).toEqual({ v: 1, type: 'ready', width: 800, height: 600 });
    });

    it('analyse une fin de session', () => {
        const msg = parseAgentControl('{"v":1,"type":"session-end","reason":"fermée"}');
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
        expect(() => parseAgentControl('{"v":1,"type":"autre"}')).toThrow(/type de contrôle/);
    });
});
