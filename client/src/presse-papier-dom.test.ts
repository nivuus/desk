import { describe, expect, it, vi } from 'vitest';

import { attacherPressePapierAuDOM, type EvenementCollage } from './presse-papier-dom';
import { CONTROL_VERSION } from '../../proto/ts/control';
import { MESSAGE_ECHEC, PRESSE_PAPIER_MAX, messageDeRefus } from './presse-papier';

/// Une cible d'événements minimale, sans DOM : le module n'a besoin que de
/// `focus` et de `paste`, et l'injecter est ce qui rend ce fichier éprouvable
/// sans jsdom.
function cibleFactice() {
    const rappels = new Map<string, Set<(event: EvenementCollage) => void>>();
    return {
        addEventListener(nom: string, rappel: (event: EvenementCollage) => void) {
            if (!rappels.has(nom)) rappels.set(nom, new Set());
            rappels.get(nom)!.add(rappel);
        },
        removeEventListener(nom: string, rappel: (event: EvenementCollage) => void) {
            rappels.get(nom)?.delete(rappel);
        },
        declencher(nom: string, event?: EvenementCollage) {
            for (const rappel of rappels.get(nom) ?? []) rappel(event as EvenementCollage);
        },
        compte(nom: string) {
            return rappels.get(nom)?.size ?? 0;
        },
    };
}

/// Un `paste` factice portant `texte` en `text/plain`.
function collage(texte: string | null) {
    return {
        clipboardData: texte === null ? null : { getData: () => texte },
    };
}

describe('attacherPressePapierAuDOM', () => {
    it('écrit le texte reçu quand la fenêtre a le focus', async () => {
        const ecrire = vi.fn().mockResolvedValue(undefined);
        const cible = cibleFactice();
        const attache = attacherPressePapierAuDOM({
            ecrire,
            focalise: () => true,
            cible,
            surMessage: vi.fn(),
            emettre: vi.fn(),
        });

        attache.recevoir({ texte: 'bonjour', octets: 7 });
        await Promise.resolve();

        expect(ecrire).toHaveBeenCalledWith('bonjour');
        attache.detacher();
    });

    /// Le dépôt différé de D3, et la seule ligne de DOM de ce module : sans
    /// focus on ne tente rien (`writeText` échouerait), et le retour du focus
    /// est ce qui sort le texte.
    it("n'écrit rien sans focus, puis écrit au retour du focus", async () => {
        const ecrire = vi.fn().mockResolvedValue(undefined);
        const cible = cibleFactice();
        let focalise = false;
        const attache = attacherPressePapierAuDOM({
            ecrire,
            focalise: () => focalise,
            cible,
            surMessage: vi.fn(),
            emettre: vi.fn(),
        });

        attache.recevoir({ texte: 'differe', octets: 7 });
        await Promise.resolve();
        expect(ecrire).not.toHaveBeenCalled();

        focalise = true;
        cible.declencher('focus');
        await Promise.resolve();

        expect(ecrire).toHaveBeenCalledWith('differe');
        attache.detacher();
    });

    /// Un refus est DIT, jamais tu — et il ne déclenche aucune écriture.
    it('dit le refus et n’écrit rien', async () => {
        const ecrire = vi.fn().mockResolvedValue(undefined);
        const surMessage = vi.fn();
        const attache = attacherPressePapierAuDOM({
            ecrire,
            focalise: () => true,
            cible: cibleFactice(),
            surMessage,
            emettre: vi.fn(),
        });

        attache.recevoir({ texte: null, octets: 100_000 });
        await Promise.resolve();

        expect(ecrire).not.toHaveBeenCalled();
        expect(surMessage).toHaveBeenCalledWith(messageDeRefus(100_000));
        attache.detacher();
    });

    /// `ECHECS_AVANT_MESSAGE` vaut 2 : le premier échec est le cas ordinaire
    /// d'une fenêtre qui perd le focus pendant l'écriture, et crier dessus
    /// ferait un bandeau permanent sur un produit qui marche.
    it('ne crie qu’au deuxième échec consécutif', async () => {
        const ecrire = vi.fn().mockRejectedValue(new Error('refusé'));
        const surMessage = vi.fn();
        const cible = cibleFactice();
        const attache = attacherPressePapierAuDOM({
            ecrire,
            focalise: () => true,
            cible,
            surMessage,
            emettre: vi.fn(),
        });

        attache.recevoir({ texte: 'un', octets: 2 });
        await Promise.resolve();
        await Promise.resolve();
        expect(surMessage).not.toHaveBeenCalled();

        attache.recevoir({ texte: 'deux', octets: 4 });
        await Promise.resolve();
        await Promise.resolve();
        expect(surMessage).toHaveBeenCalledWith(MESSAGE_ECHEC);
        attache.detacher();
    });

    /// Sans ce détachement, l'écouteur `focus` survivrait à la fin de session
    /// et écrirait le presse-papier local d'une session morte — le même défaut
    /// que les trois détachements voisins de `main.ts` existent pour éviter.
    it('détache son écouteur de focus', () => {
        const cible = cibleFactice();
        const attache = attacherPressePapierAuDOM({
            ecrire: vi.fn().mockResolvedValue(undefined),
            focalise: () => true,
            cible,
            surMessage: vi.fn(),
            emettre: vi.fn(),
        });

        expect(cible.compte('focus')).toBe(1);
        attache.detacher();
        expect(cible.compte('focus')).toBe(0);
    });

    /// 🔴 `readText()` n'est appelée NULLE PART, ni au focus ni jamais : c'est
    /// le geste de l'ancien produit (`web/index.js`), il exige une permission,
    /// et il lit une ressource privée EN DEHORS de toute intention de collage.
    /// Ce test garde cette propriété contre une régression future — le module
    /// ne reçoit aucune fonction de lecture, et son interface ne peut donc pas
    /// en acquérir une sans que ce fichier ne cesse de compiler.
    ///
    /// ✅ **CE GARDE A MORDU AU SOUS-BLOC P2, ET C'EST EXACTEMENT SON OFFICE.**
    /// L'ajout d'`emettre` l'a fait rougir, forçant à regarder la clé neuve et
    /// à trancher : `emettre` écrit sur le canal de contrôle **vers l'agent**,
    /// elle ne lit rien du presse-papier de l'utilisateur. Le seul endroit du
    /// produit où celui-ci est lu reste l'événement `paste` DE CONFIANCE, qui
    /// n'est pas une capacité reçue mais un geste de l'utilisateur — et il ne
    /// passe par aucune de ces clés. La liste est donc étendue **sciemment**,
    /// et non par accommodement.
    it('ne reçoit aucune capacité de LECTURE du presse-papier', () => {
        const options = {
            ecrire: vi.fn().mockResolvedValue(undefined),
            focalise: () => true,
            cible: cibleFactice(),
            surMessage: vi.fn(),
            emettre: vi.fn(),
        };
        expect(Object.keys(options).sort()).toEqual([
            'cible',
            'ecrire',
            'emettre',
            'focalise',
            'surMessage',
        ]);
        attacherPressePapierAuDOM(options).detacher();
    });
});

// ---------------------------------------------------------------------------
// Sous-bloc P2 — l'écouteur `paste`, le sens navigateur → VM.
// ---------------------------------------------------------------------------

describe("l'écouteur de collage", () => {
    function monter(surMessage = vi.fn()) {
        const cible = cibleFactice();
        const emettre = vi.fn();
        const attache = attacherPressePapierAuDOM({
            ecrire: vi.fn().mockResolvedValue(undefined),
            focalise: () => true,
            cible,
            surMessage,
            emettre,
        });
        return { cible, emettre, surMessage, attache };
    }

    // 🔴 ROUGE si l'écouteur est absent : rien ne remonterait jamais à l'agent.
    // La forme émise est celle que `proto/src/control.rs` désérialise, avec
    // `deny_unknown_fields` — un encodeur maison serait refusé par serde.
    it('émet le texte collé sur le canal de contrôle', () => {
        const { cible, emettre } = monter();
        cible.declencher('paste', collage('bonjour'));
        expect(emettre).toHaveBeenCalledOnce();
        expect(JSON.parse(emettre.mock.calls[0][0] as string)).toEqual({
            v: CONTROL_VERSION,
            type: 'clipboard',
            text: 'bonjour',
        });
    });

    // 🔴 ROUGE si l'on émettait une chaîne vide : elle VIDERAIT le
    // presse-papier de la VM sans que l'utilisateur l'ait demandé.
    it("un collage vide n'émet rien", () => {
        const { cible, emettre } = monter();
        cible.declencher('paste', collage(''));
        expect(emettre).not.toHaveBeenCalled();
    });

    // Un `paste` sans `clipboardData` (une image, un format inconnu) est le
    // même cas : rien à émettre, et rien à dire.
    it("un collage sans clipboardData n'émet rien", () => {
        const { cible, emettre } = monter();
        cible.declencher('paste', collage(null));
        expect(emettre).not.toHaveBeenCalled();
    });

    // 🔴 **LA BORNE CÔTÉ CLIENT EST OBLIGATOIRE.** Sans elle, l'agent la ferait
    // respecter — mais le canal aurait DÉJÀ porté la charge, et le bandeau ne
    // paraîtrait jamais : l'agent refuse en journalisant, sans rien renvoyer.
    it('au-delà de la borne : rien n émis, et le refus est DIT', () => {
        const surMessage = vi.fn();
        const { cible, emettre } = monter(surMessage);
        const trop = 'a'.repeat(PRESSE_PAPIER_MAX + 1);
        cible.declencher('paste', collage(trop));
        expect(emettre).not.toHaveBeenCalled();
        expect(surMessage).toHaveBeenCalledWith(messageDeRefus(PRESSE_PAPIER_MAX + 1));
    });

    // Le cas limite exact passe : rouge si la comparaison est un `>=`.
    it('exactement la borne passe', () => {
        const { cible, emettre } = monter();
        cible.declencher('paste', collage('a'.repeat(PRESSE_PAPIER_MAX)));
        expect(emettre).toHaveBeenCalledOnce();
    });

    // 🔴 **LA BORNE COMPTE DES OCTETS D'UTF-8, PAS DES UNITÉS UTF-16.**
    // ROUGE si l'implémentation est `texte.length` : ce texte compte
    // `PRESSE_PAPIER_MAX / 2` unités UTF-16 — donc passerait — pour
    // `PRESSE_PAPIER_MAX * 2` octets, soit le DOUBLE de ce que l'agent accepte.
    // Le client émettrait alors une charge que l'agent refuserait en silence.
    it('la borne compte des octets UTF-8, pas des unités UTF-16', () => {
        const { cible, emettre, surMessage } = monter();
        const emojis = '😀'.repeat(PRESSE_PAPIER_MAX / 4);
        expect(emojis.length).toBe(PRESSE_PAPIER_MAX / 2);
        cible.declencher('paste', collage(emojis + '😀'));
        expect(emettre).not.toHaveBeenCalled();
        expect(surMessage).toHaveBeenCalled();
    });

    // 🔴 **LE GARDE N°3 CÂBLÉ** : un texte qu'on vient de recevoir de l'agent
    // n'est pas réémis vers lui. Sans cet appel, chaque collage d'un contenu
    // venu de la VM produirait un aller-retour complet.
    it("ne réémet pas un texte qu'on vient de recevoir", () => {
        const { cible, emettre, attache } = monter();
        attache.recevoir({ texte: 'venu-de-la-vm', octets: 13 });
        cible.declencher('paste', collage('venu-de-la-vm'));
        expect(emettre).not.toHaveBeenCalled();
    });

    // Le jumeau du précédent : sans lui, un `aEmettre` qui rendrait toujours
    // `undefined` passerait le test ci-dessus et le collage serait mort.
    it('réémet bien un texte DIFFÉRENT après une réception', () => {
        const { cible, emettre, attache } = monter();
        attache.recevoir({ texte: 'venu-de-la-vm', octets: 13 });
        cible.declencher('paste', collage('autre chose'));
        expect(emettre).toHaveBeenCalledOnce();
    });

    // ROUGE si `detacher` oubliait le `paste` : l'écouteur survivrait à la fin
    // de session et émettrait pour une session morte.
    it('detacher retire AUSSI l écouteur de collage', () => {
        const { cible, attache } = monter();
        expect(cible.compte('paste')).toBe(1);
        attache.detacher();
        expect(cible.compte('paste')).toBe(0);
        expect(cible.compte('focus')).toBe(0);
    });

});

describe("l'état reçu AVANT l'attache", () => {
    // ── L'ÉTAT REÇU AVANT L'ATTACHE (moitié CLIENT du legs n°3 de P1) ────
    //
    // 🔴 `client/src/main.ts` N'A AUCUN TEST et ne peut pas en avoir : module
    // d'entrée, effets de bord au premier niveau, non importable. La RÈGLE vit
    // donc ici, et ces quatre tests SONT sa seule couverture ; les deux lignes
    // de câblage de `main.ts`, elles, n'en ont aucune, et leur seul contrôle de
    // bout en bout est le critère ① de la recette.

    it('un `initial` fourni est écrit au montage si la fenêtre a le focus', async () => {
        const ecrire = vi.fn().mockResolvedValue(undefined);
        // ROUGE avant le paramètre `initial` : rien n'est écrit au montage.
        attacherPressePapierAuDOM({
            ecrire,
            focalise: () => true,
            cible: cibleFactice(),
            surMessage: vi.fn(),
            emettre: vi.fn(),
            initial: { texte: 'copie-avant-attache', octets: 19 },
        });
        await Promise.resolve();
        expect(ecrire).toHaveBeenCalledWith('copie-avant-attache');
    });

    it("un `initial` fourni SANS focus n'est pas écrit au montage, et l'est au retour du focus", async () => {
        const ecrire = vi.fn().mockResolvedValue(undefined);
        const cible = cibleFactice();
        let focalise = false;
        // ROUGE = appeler `ecrire` directement au montage au lieu de passer par
        // `ecrireSiPossible` : le DÉPÔT DIFFÉRÉ de D3 doit rester le seul
        // chemin d'écriture, y compris ici.
        attacherPressePapierAuDOM({
            ecrire,
            focalise: () => focalise,
            cible,
            surMessage: vi.fn(),
            emettre: vi.fn(),
            initial: { texte: 'differe', octets: 7 },
        });
        await Promise.resolve();
        expect(ecrire).not.toHaveBeenCalled();

        focalise = true;
        cible.declencher('focus');
        await Promise.resolve();
        expect(ecrire).toHaveBeenCalledWith('differe');
    });

    it('un `initial` portant un REFUS dit le bandeau au montage', async () => {
        const surMessage = vi.fn();
        // ROUGE = ne rejouer que les textes : la fenêtre attendrait un contenu
        // qui n'arrivera jamais, sans rien pour lui dire pourquoi.
        attacherPressePapierAuDOM({
            ecrire: vi.fn().mockResolvedValue(undefined),
            focalise: () => true,
            cible: cibleFactice(),
            surMessage,
            emettre: vi.fn(),
            initial: { texte: null, octets: 123456 },
        });
        await Promise.resolve();
        expect(surMessage).toHaveBeenCalledWith(messageDeRefus(123456));
    });

    it("sans `initial`, le montage n'écrit rien et ne dit rien", async () => {
        const ecrire = vi.fn().mockResolvedValue(undefined);
        const surMessage = vi.fn();
        // ROUGE = rejouer un `Recu` vide quand le paramètre est absent : le
        // client écrirait une chaîne vide dans son presse-papier local à chaque
        // attache. C'est le paramètre FACULTATIF qui garantit que le
        // comportement d'avant P3 est préservé mot pour mot.
        attacherPressePapierAuDOM({
            ecrire,
            focalise: () => true,
            cible: cibleFactice(),
            surMessage,
            emettre: vi.fn(),
        });
        await Promise.resolve();
        expect(ecrire).not.toHaveBeenCalled();
        expect(surMessage).not.toHaveBeenCalled();
    });
});
