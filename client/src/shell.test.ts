import { describe, it, expect, vi } from 'vitest';
import { creerBureau, type Ton } from './shell';

/**
 * 🔴 `afficher` COLLECTE, IL NE FAIT PLUS RIEN. La version d'avant le
 * sous-bloc S3 posait `afficher: () => {}` — un NO-OP, exactement le patron que
 * le sous-bloc D10 a nommé : « une source factice qui implémente un effet de
 * bord en NO-OP rend une famille entière de défauts invisible aux tests
 * d'hôte », 456 tests verts sur un produit muet. Tant qu'il était là, AUCUN des
 * quatre tests de ton ci-dessous ne pouvait échouer — et le bandeau lui-même
 * n'était vérifié par rien.
 */
function bureauDeTest() {
    const ouvertes = new Map<string, { closed: boolean; close: () => void }>();
    const envoyes: unknown[] = [];
    const etatsFichiers: string[] = [];
    const bandeaux: Array<{ message: string; ton: Ton }> = [];
    const etats: Array<{ texte: string; ton: Ton }> = [];
    const bureau = creerBureau({
        ouvrirFenetre: (session) => {
            const f = { closed: false, close: () => { f.closed = true; } };
            ouvertes.set(session, f);
            return f as unknown as Window;
        },
        envoyer: (message) => { envoyes.push(message); },
        afficher: (message, ton) => { bandeaux.push({ message, ton }); },
        afficherEtatFichiers: (texte, ton) => {
            etatsFichiers.push(texte);
            etats.push({ texte, ton });
        },
    });
    return { bureau, ouvertes, envoyes, etatsFichiers, bandeaux, etats };
}

describe('page-shell', () => {
    it('ouvre une fenêtre navigateur quand le superviseur annonce une fenêtre', () => {
        const { bureau, ouvertes } = bureauDeTest();
        bureau.fenetreOuverte('w-1', 'Bloc-notes');
        expect(ouvertes.has('w-1')).toBe(true);
        expect(bureau.liste()).toEqual([{ session: 'w-1', titre: 'Bloc-notes', ouverte: true }]);
    });

    it('transmet au superviseur le viewport que la page annonce', () => {
        const { bureau, envoyes } = bureauDeTest();
        bureau.fenetreOuverte('w-1', 'Bloc-notes');
        bureau.viewportRecu('w-1', 1600, 900);
        expect(envoyes).toContainEqual({
            type: 'viewport', session: 'w-1', largeur: 1600, hauteur: 900,
        });
    });

    it("ignore un viewport pour une session qu'elle n'a pas ouverte", () => {
        // Le message vient de `postMessage` : n'importe quelle page de même
        // origine peut en émettre un. Ne relayer que ce qu'on a demandé.
        const { bureau, envoyes } = bureauDeTest();
        bureau.viewportRecu('w-inventee', 800, 600);
        expect(envoyes).toHaveLength(0);
    });

    it('ferme la fenêtre navigateur quand la fenêtre Windows disparaît', () => {
        const { bureau, ouvertes } = bureauDeTest();
        bureau.fenetreOuverte('w-1', 'Bloc-notes');
        bureau.fenetreFermee('w-1');
        expect(ouvertes.get('w-1')!.closed).toBe(true);
        expect(bureau.liste()).toEqual([]);
    });

    it('garde la fenêtre dans sa liste quand seule la page a été fermée', () => {
        // Décision de D1 : fermer une page ne ferme pas l'application
        // Windows. La shell doit donc pouvoir la rouvrir.
        const { bureau, ouvertes } = bureauDeTest();
        bureau.fenetreOuverte('w-1', 'Bloc-notes');
        ouvertes.get('w-1')!.closed = true;
        expect(bureau.liste()).toEqual([{ session: 'w-1', titre: 'Bloc-notes', ouverte: false }]);
    });

    it('rouvre une fenêtre dont la page a été fermée', () => {
        const { bureau, ouvertes } = bureauDeTest();
        bureau.fenetreOuverte('w-1', 'Bloc-notes');
        ouvertes.get('w-1')!.closed = true;
        bureau.rouvrir('w-1');
        expect(ouvertes.get('w-1')!.closed).toBe(false);
        expect(bureau.liste()).toEqual([{ session: 'w-1', titre: 'Bloc-notes', ouverte: true }]);
    });

    it('affiche un refus sans rien ouvrir', () => {
        const affiche = vi.fn();
        const bureau = creerBureau({
            ouvrirFenetre: () => { throw new Error('rien ne doit être ouvert'); },
            envoyer: () => {},
            afficher: affiche,
            afficherEtatFichiers: () => {},
        });
        bureau.refus('F9', 'plus aucune sortie virtuelle disponible');
        // Le motif du refus est REPRIS TEL QUEL, et le ton l'accompagne : le
        // sous-bloc S3 a ajouté le second argument, et une assertion à un seul
        // argument cesserait de décrire l'appel réel.
        expect(affiche).toHaveBeenCalledWith(
            expect.stringContaining('plus aucune sortie virtuelle disponible'),
            'danger',
        );
    });

    it("signale un blocage de pop-up plutôt que de l'ignorer", () => {
        // `window.open` rend `null` quand le navigateur bloque : sans ce
        // traitement, l'utilisateur verrait une fenêtre listée « ouverte »
        // qui n'existe pas.
        const affiche = vi.fn();
        const bureau = creerBureau({
            ouvrirFenetre: () => null,
            envoyer: () => {},
            afficher: affiche,
            afficherEtatFichiers: () => {},
        });
        bureau.fenetreOuverte('w-1', 'Bloc-notes');
        expect(affiche).toHaveBeenCalledWith(expect.stringContaining('pop-up'), 'danger');
        expect(bureau.liste()).toEqual([{ session: 'w-1', titre: 'Bloc-notes', ouverte: false }]);
    });
});

describe('état du lecteur de fichiers', () => {
    it('monter le lecteur affiche le nom du dossier', () => {
        const { bureau, etatsFichiers } = bureauDeTest();
        bureau.lecteurMonte('Mes documents');
        expect(etatsFichiers.at(-1)).toContain('Mes documents');
    });

    it('🔴 démonter le lecteur EFFACE l’état', () => {
        // 🔴 C'est le défaut relevé en D5 : le bandeau `#status` gardait son
        // `textContent` après `expirer()`, si bien que lire le texte prouvait
        // qu'un message était ARRIVÉ, jamais qu'il était AFFICHÉ. Une recette
        // entière a lu un bandeau périmé en croyant lire l'état courant.
        //
        // La chaîne vide n'est donc pas un détail de présentation : c'est
        // l'assertion elle-même.
        const { bureau, etatsFichiers } = bureauDeTest();
        bureau.lecteurMonte('Mes documents');
        bureau.lecteurDemonte();
        expect(etatsFichiers.at(-1)).toBe('');
    });

    it('un échec de montage se distingue d’un démontage', () => {
        // « rien n'est partagé » et « le partage a raté, voici pourquoi »
        // n'appellent pas le même geste de l'utilisateur : le second lui dit
        // quoi corriger, le premier lui dit seulement de recommencer.
        const { bureau, etatsFichiers } = bureauDeTest();
        bureau.lecteurEchoue('signaling injoignable');
        expect(etatsFichiers.at(-1)).toContain('signaling injoignable');
        expect(etatsFichiers.at(-1)).not.toBe('');
    });

    it('un remontage remplace le nom précédent au lieu de s’y ajouter', () => {
        const { bureau, etatsFichiers } = bureauDeTest();
        bureau.lecteurMonte('Premier');
        bureau.lecteurMonte('Second');
        expect(etatsFichiers.at(-1)).toContain('Second');
        expect(etatsFichiers.at(-1)).not.toContain('Premier');
    });
});

describe('page-shell — le TON du bandeau', () => {
    // La règle est ICI, dans `shell.ts`, et non dans le câblage : `shell-page.ts`
    // ne fait que relayer. Une condition qui apparaîtrait là-bas serait au
    // mauvais endroit.

    it('donne le ton DANGER à un refus', () => {
        const { bureau, bandeaux } = bureauDeTest();
        bureau.refus('Bloc-notes', 'plus aucune sortie disponible');
        expect(bandeaux).toHaveLength(1);
        expect(bandeaux[0].ton).toBe('danger');
    });

    it('donne le ton DANGER à une pop-up bloquée', () => {
        // L'utilisateur doit AGIR — autoriser les pop-ups. Un bandeau neutre
        // dirait que la fenêtre est en route ; elle n'existera jamais.
        const bandeaux: Array<{ message: string; ton: Ton }> = [];
        const sansPopup = creerBureau({
            ouvrirFenetre: () => null,
            envoyer: () => {},
            afficher: (message, ton) => { bandeaux.push({ message, ton }); },
            afficherEtatFichiers: () => {},
        });
        sansPopup.fenetreOuverte('w-1', 'Bloc-notes');
        expect(bandeaux).toHaveLength(1);
        expect(bandeaux[0].ton).toBe('danger');
        expect(bandeaux[0].message).toContain('bloqué la pop-up');
    });

    it('distingue le montage RÉUSSI de l’ÉCHEC par le ton, pas seulement par le texte', () => {
        // `shell.ts` exige déjà que les deux textes soient distincts ; le ton
        // ne doit pas défaire cette distinction en les rendant identiques à
        // l'œil.
        const { bureau, etats } = bureauDeTest();
        bureau.lecteurMonte('Documents');
        bureau.lecteurEchoue('permission refusée');
        expect(etats.map((e) => e.ton)).toEqual(['succes', 'danger']);
    });

    it('sur un démontage, la chaîne reste VIDE', () => {
        const { bureau, etats } = bureauDeTest();
        bureau.lecteurDemonte();
        expect(etats).toHaveLength(1);
        expect(etats[0].texte).toBe('');
    });

    it("sur un démontage, le bandeau ne prend AUCUN ton — assertion séparée", () => {
        // 🔴 SÉPARÉE DE LA PRÉCÉDENTE À DESSEIN : la vacuité du texte et la
        // neutralité du ton sont deux propriétés, et `expect` interrompt au
        // premier échec. Les fondre ferait qu'un ton `danger` sur un bandeau
        // vide — une couleur sans message — passerait inaperçu dès que
        // l'assertion de texte tomberait la première.
        const { bureau, etats } = bureauDeTest();
        bureau.lecteurDemonte();
        expect(etats[0].ton).toBe('neutre');
    });
});
