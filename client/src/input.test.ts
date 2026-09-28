// Tests de l'exception clavier du sous-bloc P2 — et d'elle seule.
//
// ⚠️ **Ce fichier ne couvre PAS le pointeur, la molette ni le menu
// contextuel** : ces chemins touchent `document.pointerLockElement`,
// `getBoundingClientRect` et `setPointerCapture`, et la suite du client tourne
// en environnement `node`, sans DOM. Ils restaient sans couverture avant P2 et
// le restent — déclaré, pas dissimulé.
//
// Le clavier, lui, est couvert parce que P2 a **injecté sa cible**
// (`CibleClavier`) au lieu de la prendre du global : c'est ce qui rend la
// LIAISON entre `raccourcis.ts` et l'écouteur éprouvable, et c'est la liaison
// qui porte le risque R5 de la spec.

import { beforeEach, describe, expect, it, vi } from 'vitest';

import { attachInput, type CibleClavier } from './input';

/// Cible clavier factice : retient les rappels et les rejoue à la demande.
class ClavierFactice implements CibleClavier {
    private rappels = new Map<string, (event: KeyboardEvent) => void>();

    addEventListener(nom: 'keydown' | 'keyup', rappel: (event: KeyboardEvent) => void): void {
        this.rappels.set(nom, rappel);
    }
    removeEventListener(nom: 'keydown' | 'keyup', rappel: (event: KeyboardEvent) => void): void {
        if (this.rappels.get(nom) === rappel) this.rappels.delete(nom);
    }
    /// Rejoue un événement, et rend l'espion de `preventDefault`.
    frapper(nom: 'keydown' | 'keyup', touche: Partial<KeyboardEvent> & { code: string }) {
        const preventDefault = vi.fn();
        const event = {
            ctrlKey: false,
            shiftKey: false,
            altKey: false,
            metaKey: false,
            ...touche,
            preventDefault,
        } as unknown as KeyboardEvent;
        this.rappels.get(nom)?.(event);
        return preventDefault;
    }
    get attaches(): number {
        return this.rappels.size;
    }
}

let clavier: ClavierFactice;
let envois: Uint8Array[];
let arme: boolean;
let detacher: () => void;

function monter(): void {
    clavier = new ClavierFactice();
    envois = [];
    arme = true;
    const channel = {
        readyState: 'open',
        send: (p: Uint8Array) => envois.push(p),
    } as unknown as RTCDataChannel;
    // Aucun de ces membres n'est touché par les chemins clavier : le seul
    // usage du `video` est le pointeur, que ce fichier n'exerce pas.
    const video = { addEventListener: vi.fn(), removeEventListener: vi.fn() } as unknown as
        HTMLVideoElement;
    detacher = attachInput({ video, channel, clavier, collageArme: () => arme });
}

beforeEach(monter);

describe("the narrow paste exception", () => {
    // 🔴 **LA ROUGE CENTRALE DE P2.** Jusqu'au commit `4cf2206`, `onKeyDown`
    // appelait `preventDefault()` SANS CONDITION : aucun événement `paste` ne
    // pouvait naître dans la fenêtre de session. Le §0 du plan établit par
    // mesure (deux exécutions) que retirer ce `preventDefault` sur le seul
    // `KeyV` suffit à faire naître un `paste` de confiance sur un `<video>`
    // focalisé — et ce test est ce qui garde la propriété désormais.
    it('Ctrl+V: neither preventDefault, nor byte sent', () => {
        const pd = clavier.frapper('keydown', { ctrlKey: true, code: 'KeyV' });
        expect(pd).not.toHaveBeenCalled();
        expect(envois).toHaveLength(0);
    });

    // 🔴 **C'EST LE RISQUE R5.** Une condition élargie rendrait le navigateur
    // au clavier, et `Ctrl+W` fermerait la fenêtre de session.
    it.each(['KeyW', 'KeyT', 'KeyN'])('Ctrl+%s: preventDefault AND byte sent', (code) => {
        const pd = clavier.frapper('keydown', { ctrlKey: true, code });
        expect(pd).toHaveBeenCalledOnce();
        expect(envois).toHaveLength(1);
    });

    // Le modificateur lui-même part normalement : le retenir ferait perdre à
    // la VM un `Ctrl` que l'utilisateur tient peut-être pour autre chose. La
    // sonde établit que son `preventDefault` n'empêche PAS le `paste`.
    it('ControlLeft alone: preventDefault AND byte sent', () => {
        const pd = clavier.frapper('keydown', { ctrlKey: true, code: 'ControlLeft' });
        expect(pd).toHaveBeenCalledOnce();
        expect(envois).toHaveLength(1);
    });

    // 🔴 Le relâchement est retenu AVEC son `preventDefault`. Laisser partir le
    // seul `V`↑ ferait voir à la VM un relâchement sans enfoncement — ce qui
    // peut débloquer une répétition clavier —, et il arriverait APRÈS les
    // quatre touches que l'agent injecte.
    it('keyup of V under Ctrl: preventDefault, but no byte', () => {
        const pd = clavier.frapper('keyup', { ctrlKey: true, code: 'KeyV' });
        expect(pd).toHaveBeenCalledOnce();
        expect(envois).toHaveLength(0);
    });

    it('Shift+Insert is handled like Ctrl+V', () => {
        const pd = clavier.frapper('keydown', { shiftKey: true, code: 'Insert' });
        expect(pd).not.toHaveBeenCalled();
        expect(envois).toHaveLength(0);
    });
});

describe('the gate on Capabilities.clipboard', () => {
    // 🔴 **SANS CE GATE, `PRESSE_PAPIER=0` DONNERAIT LE PIRE DES DEUX MONDES** :
    // le client retiendrait le `Ctrl+V` alors que personne ne l'injecterait
    // côté VM. La touche serait perdue, et l'utilisateur verrait un raccourci
    // mort.
    it("disarmed, Ctrl+V gets back its pre-P2 behaviour", () => {
        arme = false;
        const pd = clavier.frapper('keydown', { ctrlKey: true, code: 'KeyV' });
        expect(pd).toHaveBeenCalledOnce();
        expect(envois).toHaveLength(1);
    });

    // 🔴 **UNE FERMETURE, PAS UN BOOLÉEN CAPTURÉ À L'ATTACHE.** `Capabilities`
    // peut arriver après `attachInput` ; un booléen figé vaudrait `false` à
    // jamais, et le collage serait mort sans qu'aucune trace ne le dise.
    //
    // ROUGE si `collageArme` était lu une seule fois, au montage.
    it("the arming is re-read on EACH keystroke, not captured on mount", () => {
        arme = false;
        expect(clavier.frapper('keydown', { ctrlKey: true, code: 'KeyV' })).toHaveBeenCalledOnce();
        arme = true;
        expect(clavier.frapper('keydown', { ctrlKey: true, code: 'KeyV' })).not.toHaveBeenCalled();
    });
});

describe('detaching', () => {
    // ROUGE si `detacher` retirait les écouteurs de `window` alors qu'ils ont
    // été posés sur la cible injectée : ils survivraient à la fin de session.
    it('removes both listeners from the injected target', () => {
        expect(clavier.attaches).toBe(2);
        detacher();
        expect(clavier.attaches).toBe(0);
    });
});
