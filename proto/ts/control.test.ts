import { describe, expect, it } from 'vitest';
import type { CapabilitiesMessage, ReadyMessage } from './control';
import {
    CONTROL_VERSION,
    TYPES_AGENT,
    encodeClipboard,
    encodeResize,
    encodeVisibility,
    parseAgentControl,
} from './control';

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

    // Chantier E : `mic` est OPTIONNEL, et son absence vaut « pas de micro ».
    it('analyse un message ready SANS mic : le champ reste undefined', () => {
        const msg = parseAgentControl(
            `{"v":${CONTROL_VERSION},"type":"ready","width":800,"height":600}`,
        ) as ReadyMessage;
        expect(msg.mic).toBeUndefined();
        // …et `undefined` est falsy : c'est ce qui fait qu'un client récent
        // face à un agent ancien n'affiche AUCUN bouton, sans une ligne de
        // code pour le décider.
        expect(Boolean(msg.mic)).toBe(false);
    });

    it('analyse un message ready AVEC mic et le conserve', () => {
        const msg = parseAgentControl(
            `{"v":${CONTROL_VERSION},"type":"ready","width":800,"height":600,"mic":true}`,
        ) as ReadyMessage;
        expect(msg.mic).toBe(true);
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

    it('analyse un message de plein écran', () => {
        const message = parseAgentControl(
            JSON.stringify({ v: CONTROL_VERSION, type: 'fullscreen', active: true }),
        );
        expect(message).toEqual({ v: CONTROL_VERSION, type: 'fullscreen', active: true });
    });

    it('analyse un message de presse-papier portant du texte', () => {
        const message = parseAgentControl(
            JSON.stringify({ v: CONTROL_VERSION, type: 'clipboard', text: 'bonjour', bytes: 7 }),
        );
        expect(message).toEqual({ v: CONTROL_VERSION, type: 'clipboard', text: 'bonjour', bytes: 7 });
    });

    it('analyse un refus de presse-papier, text à null', () => {
        const message = parseAgentControl(
            JSON.stringify({ v: CONTROL_VERSION, type: 'clipboard', text: null, bytes: 102400 }),
        );
        expect(message).toEqual({ v: CONTROL_VERSION, type: 'clipboard', text: null, bytes: 102400 });
    });

    // 🔴 La vérification de `v` précède celle du type : ce test la fige pour
    // la variante neuve. Sans elle, un agent d'une version future ferait
    // écrire n'importe quoi dans le presse-papier local.
    it('rejette un presse-papier en version 2', () => {
        const raw = JSON.stringify({ v: 2, type: 'clipboard', text: 'bonjour', bytes: 7 });
        expect(() => parseAgentControl(raw)).toThrow(/version de contrôle non supportée/);
    });

    // 🔴 Le témoin d'EXÉCUTION de la dérivation de `TYPES_AGENT`. Les neuf
    // valeurs sont écrites À LA MAIN ici, précisément pour que le test soit
    // indépendant de la table qu'il juge : une clé de trop dans `TOUS_AGENT`
    // le fait tomber, et une clé manquante fait d'abord tomber `tsc`.
    it('TYPES_AGENT contient exactement les type de l’union', () => {
        expect(TYPES_AGENT.slice().sort()).toEqual(
            [
                'ready', 'session-end', 'pointer', 'rumble', 'capabilities',
                'link', 'asleep', 'fullscreen', 'clipboard',
            ].sort(),
        );
    });
});

// ---------------------------------------------------------------------------
// Sous-bloc P2 du chantier presse-papier — le sens navigateur → VM.
// ---------------------------------------------------------------------------

describe('le collage navigateur → VM', () => {
    // ROUGE si l'encodeur est absent, ou s'il n'émet pas la version courante.
    // La forme exacte est celle que `proto/src/control.rs` désérialise, avec
    // `deny_unknown_fields` : un champ de plus serait refusé côté agent.
    it('encodeClipboard rend la forme que serde accepte', () => {
        expect(encodeClipboard('bonjour')).toBe(
            JSON.stringify({ v: CONTROL_VERSION, type: 'clipboard', text: 'bonjour' }),
        );
    });

    // ROUGE si l'encodeur perdait les caractères non-ASCII ou les sauts de
    // ligne — c'est `JSON.stringify` qui les porte, et ce test le fige.
    it('encodeClipboard porte les sauts de ligne et l accentuation', () => {
        const decode = JSON.parse(encodeClipboard('une\r\ndeux\néàü')) as { text: string };
        expect(decode.text).toBe('une\r\ndeux\néàü');
    });

    // 🔴 `CapabilitiesMessage.clipboard` doit être OPTIONNEL.
    //
    // ROUGE si on le rend obligatoire : un agent d'avant P2 ne le porte pas, et
    // `undefined` doit valoir `false` GRATUITEMENT — exactement comme
    // `ReadyMessage.mic`. Le témoin est un `satisfies` : un objet sans le champ
    // doit rester assignable, ce que `tsc` refuserait si le champ était requis.
    //
    // ⚠️ **Ce test-ci se juge à `npm run typecheck`, PAS à `vitest`** — vitest
    // s'appuie sur esbuild, qui transpile sans vérifier les types. L'annotation
    // explicite ci-dessous est le garde : si `clipboard` devenait obligatoire,
    // `tsc` refuserait cette affectation. Les deux `expect` ne font qu'ancrer la
    // conséquence d'exécution ; c'est la ligne de type qui porte la propriété.
    it('CapabilitiesMessage.clipboard est optionnel', () => {
        const ancien: CapabilitiesMessage = {
            v: CONTROL_VERSION,
            type: 'capabilities',
            gamepad: true,
        };
        expect(ancien.clipboard).toBeUndefined();
        // Et le lire en booléen rend `false` sans qu'on ait rien à écrire.
        expect(Boolean(ancien.clipboard)).toBe(false);
    });
});
