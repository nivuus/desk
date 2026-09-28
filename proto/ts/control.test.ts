import { describe, expect, it } from 'vitest';
import type { CapabilitiesMessage, MicStateMessage, ReadyMessage } from './control';
import {
    CONTROL_VERSION,
    TYPES_AGENT,
    encodeClipboard,
    encodeResize,
    encodeVisibility,
    parseAgentControl,
} from './control';

describe('control protocol', () => {
    it('encodes a resize', () => {
        expect(JSON.parse(encodeResize(1280, 720))).toEqual({
            v: CONTROL_VERSION,
            type: 'resize',
            width: 1280,
            height: 720,
        });
    });

    it('rounds and bounds the dimensions', () => {
        expect(JSON.parse(encodeResize(0, 719.6))).toEqual({
            v: CONTROL_VERSION,
            type: 'resize',
            width: 1,
            height: 720,
        });
    });

    // Chantier E : `mic` est OPTIONNEL, et son absence vaut « pas de micro ».
    it('parses a ready message WITHOUT mic: the field stays undefined', () => {
        const msg = parseAgentControl(
            `{"v":${CONTROL_VERSION},"type":"ready","width":800,"height":600}`,
        ) as ReadyMessage;
        expect(msg.mic).toBeUndefined();
        // …et `undefined` est falsy : c'est ce qui fait qu'un client récent
        // face à un agent ancien n'affiche AUCUN bouton, sans une ligne de
        // code pour le décider.
        expect(Boolean(msg.mic)).toBe(false);
    });

    it('parses a ready message WITH mic and keeps it', () => {
        const msg = parseAgentControl(
            `{"v":${CONTROL_VERSION},"type":"ready","width":800,"height":600,"mic":true}`,
        ) as ReadyMessage;
        expect(msg.mic).toBe(true);
    });

    it('parses a ready message', () => {
        const msg = parseAgentControl(`{"v":${CONTROL_VERSION},"type":"ready","width":800,"height":600}`);
        expect(msg).toEqual({ v: CONTROL_VERSION, type: 'ready', width: 800, height: 600 });
    });

    it('parses a session end', () => {
        const msg = parseAgentControl(`{"v":${CONTROL_VERSION},"type":"session-end","reason":"closed"}`);
        expect(msg.type).toBe('session-end');
    });

    it('rejects an unknown version', () => {
        expect(() => parseAgentControl('{"v":9,"type":"ready","width":1,"height":1}')).toThrow(
            /control version/,
        );
    });

    it('rejects an absent version', () => {
        expect(() => parseAgentControl('{"type":"ready","width":1,"height":1}')).toThrow(
            /control version/,
        );
    });

    it('rejects an unknown type', () => {
        expect(() => parseAgentControl(`{"v":${CONTROL_VERSION},"type":"autre"}`)).toThrow(/control type/);
    });

    it('parses a pointer message', () => {
        const message = parseAgentControl(
            `{"type":"pointer","v":${CONTROL_VERSION},"visible":false,"shape":"ns-resize"}`,
        );
        expect(message).toEqual({ type: 'pointer', v: CONTROL_VERSION, visible: false, shape: 'ns-resize' });
    });

    it('parses a rumble message', () => {
        const message = parseAgentControl(`{"type":"rumble","v":${CONTROL_VERSION},"left":255,"right":0}`);
        expect(message).toEqual({ type: 'rumble', v: CONTROL_VERSION, left: 255, right: 0 });
    });

    it('parses a capabilities message', () => {
        const message = parseAgentControl(`{"type":"capabilities","v":${CONTROL_VERSION},"gamepad":false}`);
        expect(message).toEqual({ type: 'capabilities', v: CONTROL_VERSION, gamepad: false });
    });

    it('rejects control version 1, now obsolete', () => {
        expect(() => parseAgentControl('{"type":"ready","v":1,"width":1,"height":1}')).toThrow();
    });

    it("parses the state of the link", () => {
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

    it('encodes a visibility at the current version', () => {
        const json = JSON.parse(encodeVisibility(false, true));
        expect(json).toEqual({ v: CONTROL_VERSION, type: 'visibility', visible: false, focused: true });
    });

    it('accepts an asleep message coming from the agent', () => {
        const raw = JSON.stringify({ v: CONTROL_VERSION, type: 'asleep', asleep: true, reason: 'evincee' });
        expect(parseAgentControl(raw)).toEqual({
            v: CONTROL_VERSION, type: 'asleep', asleep: true, reason: 'evincee',
        });
    });

    it('parses a fullscreen message', () => {
        const message = parseAgentControl(
            JSON.stringify({ v: CONTROL_VERSION, type: 'fullscreen', active: true }),
        );
        expect(message).toEqual({ v: CONTROL_VERSION, type: 'fullscreen', active: true });
    });

    it('parses a clipboard message carrying text', () => {
        const message = parseAgentControl(
            JSON.stringify({ v: CONTROL_VERSION, type: 'clipboard', text: 'bonjour', bytes: 7 }),
        );
        expect(message).toEqual({ v: CONTROL_VERSION, type: 'clipboard', text: 'bonjour', bytes: 7 });
    });

    it('parses a clipboard refusal, text at null', () => {
        const message = parseAgentControl(
            JSON.stringify({ v: CONTROL_VERSION, type: 'clipboard', text: null, bytes: 102400 }),
        );
        expect(message).toEqual({ v: CONTROL_VERSION, type: 'clipboard', text: null, bytes: 102400 });
    });

    // 🔴 La vérification de `v` précède celle du type : ce test la fige pour
    // la variante neuve. Sans elle, un agent d'une version future ferait
    // écrire n'importe quoi dans le presse-papier local.
    it('rejects a clipboard at version 2', () => {
        const raw = JSON.stringify({ v: 2, type: 'clipboard', text: 'bonjour', bytes: 7 });
        expect(() => parseAgentControl(raw)).toThrow(/unsupported control version/);
    });

    // 🔴 Le témoin d'EXÉCUTION de la dérivation de `TYPES_AGENT`. Les DIX
    // valeurs sont écrites À LA MAIN ici, précisément pour que le test soit
    // indépendant de la table qu'il juge : une clé de trop dans `ALL_AGENT`
    // le fait tomber, et une clé manquante fait d'abord tomber `tsc`.
    //
    // ⚠️ **Neuf jusqu'au sous-bloc A1, DIX depuis** — et le garde `tsc` a été
    // VU échouer avant que la clé ne soit posée :
    //   « Property 'accent' is missing in type { ready: true; … } but required
    //     in type Record<… | "accent", true> »
    // C'est le seul garde de compilation côté TypeScript, et RA1-4 exigeait
    // qu'il soit vu, pas supposé.
    it('TYPES_AGENT contains exactly the types of the union', () => {
        expect(TYPES_AGENT.slice().sort()).toEqual(
            [
                'ready', 'session-end', 'pointer', 'rumble', 'capabilities',
                'link', 'asleep', 'fullscreen', 'clipboard', 'accent',
                'mic-state',
            ].sort(),
        );
    });

    // Sous-bloc A1 : la variante d'accent traverse `parseAgentControl`.
    // ROUGE si l'interface, l'union ou `ALL_AGENT` manquaient — les trois
    // sont éprouvés d'un coup ici, à l'EXÉCUTION.
    it('parses an accent', () => {
        const raw = JSON.stringify({ v: CONTROL_VERSION, type: 'accent', couleur: '#7aa2f7' });
        expect(parseAgentControl(raw)).toEqual({
            v: CONTROL_VERSION,
            type: 'accent',
            couleur: '#7aa2f7',
        });
    });

    // 🔴 La vérification de `v` précède celle du type, comme pour le
    // presse-papier : sans elle, un agent d'une version future ferait poser
    // n'importe quoi sur `--accent-fenetre`.
    it('rejects an accent at version 2', () => {
        const raw = JSON.stringify({ v: 2, type: 'accent', couleur: '#7aa2f7' });
        expect(() => parseAgentControl(raw)).toThrow(/unsupported control version/);
    });

    // ── Bloc E3 : la variante `mic-state` ───────────────────────────────────
    //
    // ⚠️ **Divergence V1, LÉGUÉE et non fermée :** il n'existe AUCUN fichier de
    // vecteurs partagé pour `AgentControl`. Ces assertions épinglent la forme
    // de fil **côté TypeScript** ; `proto/src/control/tests.rs` épingle **la
    // sienne**. Les deux s'accordent parce que deux mains ont écrit la même
    // chaîne, et **rien ne le vérifie** : un renommage de clé appliqué d'un
    // seul côté resterait vert des deux côtés.

    it('parses a refused microphone state', () => {
        const msg = parseAgentControl(
            `{"v":${CONTROL_VERSION},"type":"mic-state","granted":false}`,
        ) as MicStateMessage;
        expect(msg.type).toBe('mic-state');
        expect(msg.granted).toBe(false);
    });

    it('parses a granted microphone state', () => {
        const msg = parseAgentControl(
            `{"v":${CONTROL_VERSION},"type":"mic-state","granted":true}`,
        ) as MicStateMessage;
        expect(msg.granted).toBe(true);
    });

    it("mic-state is in TYPES_AGENT, hence in the derivation of the union", () => {
        // Le témoin d'EXÉCUTION de `ALL_AGENT` : `tsc` garde déjà la liste,
        // mais un test ne peut pas constater une erreur de compilation.
        expect(TYPES_AGENT).toContain('mic-state');
    });

    it('rejects a microphone state at the wrong version', () => {
        expect(() => parseAgentControl('{"v":2,"type":"mic-state","granted":true}')).toThrow(
            /unsupported control version/,
        );
    });
});

// ---------------------------------------------------------------------------
// Sous-bloc P2 du chantier presse-papier — le sens navigateur → VM.
// ---------------------------------------------------------------------------

describe('the browser → VM paste', () => {
    // ROUGE si l'encodeur est absent, ou s'il n'émet pas la version courante.
    // La forme exacte est celle que `proto/src/control.rs` désérialise, avec
    // `deny_unknown_fields` : un champ de plus serait refusé côté agent.
    it('encodeClipboard returns the shape serde accepts', () => {
        expect(encodeClipboard('bonjour')).toBe(
            JSON.stringify({ v: CONTROL_VERSION, type: 'clipboard', text: 'bonjour' }),
        );
    });

    // ROUGE si l'encodeur perdait les caractères non-ASCII ou les sauts de
    // ligne — c'est `JSON.stringify` qui les porte, et ce test le fige.
    it('encodeClipboard carries line breaks and accents', () => {
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
    it('CapabilitiesMessage.clipboard is optional', () => {
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
