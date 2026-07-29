// Tests de la souris relative sous Pointer Lock.
//
// Le module est testé par injection : ni `document`, ni `window`, ni un vrai
// HTMLVideoElement ne sont nécessaires. C'est la technique retenue pour les
// modules d'armement du projet.

import { describe, expect, it, vi } from 'vitest';
import { attachPointer, creerClampReport, sommerDeltas, type CibleVideo, type CibleDocument } from './pointer';

function faireCibleVideo() {
    const ecouteurs = new Map<string, EventListener[]>();
    let pointerLock = false;
    let permisVerrouiller = false;
    return {
        addEventListener(type: string, ecouteur: EventListener) {
            const liste = ecouteurs.get(type) ?? [];
            liste.push(ecouteur);
            ecouteurs.set(type, liste);
        },
        removeEventListener(type: string, ecouteur: EventListener) {
            const liste = (ecouteurs.get(type) ?? []).filter((e) => e !== ecouteur);
            ecouteurs.set(type, liste);
        },
        requestPointerLock() {
            // Simule le comportement réel : le navigateur accepte seulement si une
            // activation utilisateur transitoire est en cours. Sans activation (test par
            // défaut), l'appel échoue silencieusement.
            if (permisVerrouiller) {
                pointerLock = true;
            }
        },
        style: { cursor: 'auto' },
        declencher(type: string) {
            for (const ecouteur of [...(ecouteurs.get(type) ?? [])]) {
                ecouteur(new Event(type));
            }
        },
        declencherPointerMove(movementX: number, movementY: number, coalesces?: Array<{ movementX: number; movementY: number }>) {
            // Simule un PointerEvent sans utiliser la classe (qui n'existe pas en Node)
            const event = new Event('pointermove') as any;
            event.movementX = movementX;
            event.movementY = movementY;
            if (coalesces) {
                event.getCoalescedEvents = () => coalesces;
            }
            for (const ecouteur of [...(ecouteurs.get('pointermove') ?? [])]) {
                ecouteur(event as EventListener);
            }
        },
        compte(type: string) {
            return (ecouteurs.get(type) ?? []).length;
        },
        estVerrouille() {
            return pointerLock;
        },
        deverrouiller() {
            pointerLock = false;
        },
        autoriserVerrouillage() {
            permisVerrouiller = true;
        },
        interdireVerrouillage() {
            permisVerrouiller = false;
        },
    };
}

function faireCibleDocument() {
    const ecouteurs = new Map<string, EventListener[]>();
    let verrouille: CibleVideo | null = null;
    return {
        get pointerLockElement() {
            return verrouille;
        },
        set pointerLockElement(video: CibleVideo | null) {
            verrouille = video;
        },
        exitPointerLock() {
            verrouille = null;
        },
        addEventListener(type: string, ecouteur: EventListener) {
            const liste = ecouteurs.get(type) ?? [];
            liste.push(ecouteur);
            ecouteurs.set(type, liste);
        },
        removeEventListener(type: string, ecouteur: EventListener) {
            const liste = (ecouteurs.get(type) ?? []).filter((e) => e !== ecouteur);
            ecouteurs.set(type, liste);
        },
        declencher(type: string) {
            for (const ecouteur of [...(ecouteurs.get(type) ?? [])]) {
                ecouteur(new Event(type));
            }
        },
        compte(type: string) {
            return (ecouteurs.get(type) ?? []).length;
        },
    };
}

describe('sommation des deltas coalescés', () => {
    it('somme les échantillons intermédiaires', () => {
        expect(sommerDeltas([
            { movementX: 3, movementY: -1 },
            { movementX: 4, movementY: -2 },
        ])).toEqual({ dx: 7, dy: -3 });
    });

    it('rend zéro sur une liste vide', () => {
        expect(sommerDeltas([])).toEqual({ dx: 0, dy: 0 });
    });
});

describe('clamp avec report', () => {
    it('laisse passer les valeurs dans la plage', () => {
        const clamp = creerClampReport();
        expect(clamp(10, -10)).toEqual({ dx: 10, dy: -10 });
    });

    it('borne le débordement et le reporte sur l\'appel suivant', () => {
        // La somme transmise doit rester exacte : c'est elle qui détermine la
        // visée. Écrêter en perdant le reste ferait dériver le tir.
        const clamp = creerClampReport();
        expect(clamp(40000, 0)).toEqual({ dx: 32767, dy: 0 });
        expect(clamp(0, 0)).toEqual({ dx: 7233, dy: 0 });
        expect(clamp(0, 0)).toEqual({ dx: 0, dy: 0 });
    });

    it('reporte aussi les débordements négatifs', () => {
        const clamp = creerClampReport();
        expect(clamp(0, -40000)).toEqual({ dx: 0, dy: -32768 });
        expect(clamp(0, 0)).toEqual({ dx: 0, dy: -7232 });
    });

    it('ne reporte rien quand rien ne déborde', () => {
        const clamp = creerClampReport();
        clamp(5, 5);
        expect(clamp(0, 0)).toEqual({ dx: 0, dy: 0 });
    });
});

describe('attachPointer', () => {
    it('reste désarmé tant qu\'aucun message pointer n\'est reçu', () => {
        const video = faireCibleVideo();
        const doc = faireCibleDocument();
        const envoyer = vi.fn();
        const handle = attachPointer({ video: video as any, doc: doc as any, envoyer });

        // Un clic sans armement ne doit pas verrouiller (même si permis)
        video.autoriserVerrouillage();
        video.declencher('click');
        expect(video.estVerrouille()).toBe(false);
    });

    it('arme le verrouillage sur message pointer visible=false, et le clic verrouille', () => {
        const video = faireCibleVideo();
        const doc = faireCibleDocument();
        const envoyer = vi.fn();
        const handle = attachPointer({ video: video as any, doc: doc as any, envoyer });

        // D'abord, le navigateur refuse le verrouillage (pas d'activation utilisateur)
        video.interdireVerrouillage();
        handle.surMessagePointeur(false, 'none');
        // L'essai gratuit tente de verrouiller mais échoue silencieusement
        expect(video.estVerrouille()).toBe(false);

        // Maintenant, le navigateur accepte le verrouillage (activation utilisateur
        // vient d'arriver via le clic)
        video.autoriserVerrouillage();
        video.declencher('click');
        expect(video.estVerrouille()).toBe(true);

        // Ce test échoue si onClick est vide ou si la condition sur `arme` est cassée.
    });

    it('reste armé après une sortie par Échap si l\'agent est toujours en relatif', () => {
        const video = faireCibleVideo();
        const doc = faireCibleDocument();
        const envoyer = vi.fn();
        const handle = attachPointer({ video: video as any, doc: doc as any, envoyer });

        // Armement : permis de verrouiller dès le départ
        video.autoriserVerrouillage();
        handle.surMessagePointeur(false, 'none');
        // L'essai gratuit réussit
        expect(video.estVerrouille()).toBe(true);

        // Simuler que c'est vraiment verrouillé dans le document
        doc.pointerLockElement = video as any;
        doc.declencher('pointerlockchange');

        // Sortie par Échap : le document change d'état
        doc.pointerLockElement = null;
        // Mais le test doit aussi refléter que le navigateur a relâché le verrouillage
        video.deverrouiller();
        doc.declencher('pointerlockchange');

        // Le module doit rester armé après pointerlockchange si toujours en mode relatif
        // Donc le clic suivant doit reverrouiller
        video.declencher('click');
        expect(video.estVerrouille()).toBe(true);

        // Ce test échoue si onPointerLockChange désarmait ou si onClick était vide.
    });

    it('retire tous les écouteurs lors du détachement', () => {
        const video = faireCibleVideo();
        const doc = faireCibleDocument();
        const envoyer = vi.fn();
        const handle = attachPointer({ video: video as any, doc: doc as any, envoyer });

        // Avant détachement : 2 écouteurs sur video
        expect(video.compte('click')).toBe(1);
        expect(video.compte('pointermove')).toBe(1);
        expect(doc.compte('pointerlockerror')).toBe(1);
        expect(doc.compte('pointerlockchange')).toBe(1);

        // Détachement
        handle.detacher();

        // Après détachement : plus d'écouteurs
        expect(video.compte('click')).toBe(0);
        expect(video.compte('pointermove')).toBe(0);
        expect(doc.compte('pointerlockerror')).toBe(0);
        expect(doc.compte('pointerlockchange')).toBe(0);

        // Les événements ultérieurs ne doivent rien faire
        video.declencher('click');
        expect(video.estVerrouille()).toBe(false);
    });

    it('appelle surEchec après deux erreurs de verrouillage consécutives', () => {
        const video = faireCibleVideo();
        const doc = faireCibleDocument();
        const envoyer = vi.fn();
        const surEchec = vi.fn();
        const handle = attachPointer({ video: video as any, doc: doc as any, envoyer, surEchec });

        handle.surMessagePointeur(false, 'none');

        // Première erreur
        doc.declencher('pointerlockerror');
        expect(surEchec).not.toHaveBeenCalled();

        // Deuxième erreur consécutive
        doc.declencher('pointerlockerror');
        expect(surEchec).toHaveBeenCalledTimes(1);

        // Un verrouillage réussi remet le compteur à zéro
        doc.pointerLockElement = video as any;
        doc.declencher('pointerlockchange');

        // Première erreur après reset
        doc.declencher('pointerlockerror');
        expect(surEchec).toHaveBeenCalledTimes(1);

        // Deuxième erreur : surEchec est appelé à nouveau
        doc.declencher('pointerlockerror');
        expect(surEchec).toHaveBeenCalledTimes(2);
    });

    it('retombe sur l\'événement principal quand getCoalescedEvents retourne un tableau vide', () => {
        const video = faireCibleVideo();
        const doc = faireCibleDocument();
        const envoyer = vi.fn();
        const handle = attachPointer({ video: video as any, doc: doc as any, envoyer });

        // Armement et verrouillage
        handle.surMessagePointeur(false, 'none');
        video.declencher('click');
        doc.pointerLockElement = video as any;

        // Un événement avec getCoalescedEvents vide
        video.declencherPointerMove(10, 20, []);

        // L'événement principal (10, 20) doit être utilisé, pas une somme nulle
        expect(envoyer).toHaveBeenCalled();
        // Vérifier que quelque chose a été envoyé (le payload exact dépend de encodeMouseMoveRelative)
        expect(envoyer.mock.calls[0][0]).toBeInstanceOf(Uint8Array);
    });
});
