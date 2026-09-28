import { describe, it, expect, vi } from 'vitest';
import { createDesktop, type Ton } from './shell';

/**
 * 🔴 `show` COLLECTE, IL NE FAIT PLUS RIEN. La version d'avant le
 * sous-bloc S3 posait `show: () => {}` — un NO-OP, exactement le patron que
 * le sous-bloc D10 a nommé : « une source factice qui implémente un effet de
 * bord en NO-OP rend une famille entière de défauts invisible aux tests
 * d'hôte », 456 tests verts sur un produit muet. Tant qu'il était là, AUCUN des
 * quatre tests de ton ci-dessous ne pouvait échouer — et le bandeau lui-même
 * n'était vérifié par rien.
 */
function bureauDeTest() {
    const ouvertes = new Map<string, { closed: boolean; close: () => void }>();
    const envoyes: unknown[] = [];
    const filesStates: string[] = [];
    const bandeaux: Array<{ message: string; ton: Ton }> = [];
    const etats: Array<{ texte: string; ton: Ton }> = [];
    /**
     * 🔴 LE COMPTEUR EST COLLECTÉ TEL QUEL — deux NOMBRES, un texte, un ton —
     * et non reformaté ici. Le lire d'une phrase serait rejouer le piège que F1
     * a payé neuf minutes : « Lecteur … monté » et « n'a pas pu être monté »
     * partagent une sous-chaîne, le pilote testait `includes('mont')`, et une
     * mesure entière a tourné sur un pont NON monté.
     */
    const compteurs: Array<{ dues: number; vues: number; texte: string; ton: Ton }> = [];
    const retenues: boolean[] = [];
    const bureau = createDesktop({
        ouvrirFenetre: (session) => {
            const f = { closed: false, close: () => { f.closed = true; } };
            ouvertes.set(session, f);
            return f as unknown as Window;
        },
        envoyer: (message) => { envoyes.push(message); },
        show: (message, ton) => { bandeaux.push({ message, ton }); },
        showFilesState: (texte, ton) => {
            filesStates.push(texte);
            etats.push({ texte, ton });
        },
        showRetained: (r: boolean) => {
            retenues.push(r);
        },
        showPendingWrites: (dues, vues, texte, ton) => {
            compteurs.push({ dues, vues, texte, ton });
        },
    });
    return { bureau, ouvertes, envoyes, filesStates, bandeaux, etats, compteurs, retenues };
}

describe('F5 — the HELD writes', () => {
    /**
     * 🔴 **`retenues` REMONTE, ET IL EST FILTRÉ PAR « il y a des dues ».**
     *
     * Retenir sans aucune due n'a pas de sens : le bouton proposerait de
     * reprendre ce qu'il n'y a pas à reprendre. Le pont ne l'émet pas ainsi,
     * mais s'en remettre à lui ferait dépendre l'interface d'une propriété
     * qu'aucun type ne garantit.
     */
    it('held with dues is announced', () => {
        const { bureau, retenues } = bureauDeTest();
        bureau.ecrituresDues([{ chemin: 'a.txt', octets: 1 }], true);
        expect(retenues.at(-1)).toBe(true);
    });

    it('🔴 held WITHOUT any due is NOT announced', () => {
        const { bureau, retenues } = bureauDeTest();
        bureau.ecrituresDues([], true);
        expect(retenues.at(-1)).toBe(false);
    });

    it('NON-held dues do not announce it', () => {
        const { bureau, retenues } = bureauDeTest();
        bureau.ecrituresDues([{ chemin: 'a.txt', octets: 1 }], false);
        expect(retenues.at(-1)).toBe(false);
    });

    /**
     * 🔴 **RETENIR EST UNE ALERTE, JAMAIS UN `neutre`.** Rien ne repartira sans
     * un geste de l'utilisateur, et un ton neutre laisserait croire que le pont
     * travaille encore.
     */
    it('🔴 a held resumption carries the « alerte » tone', () => {
        const { bureau, compteurs } = bureauDeTest();
        bureau.ecrituresDues([{ chemin: 'a.txt', octets: 1 }], true);
        expect(compteurs.at(-1)?.ton).toBe('alerte');
    });

    /**
     * L'état est celui du PONT, pas de l'interface : il tombe quand le pont
     * cesse de retenir, et pas avant.
     */
    it('the state drops back when the bridge stops holding', () => {
        const { bureau, retenues } = bureauDeTest();
        bureau.ecrituresDues([{ chemin: 'a.txt', octets: 1 }], true);
        expect(retenues.at(-1)).toBe(true);
        bureau.ecrituresDues([{ chemin: 'a.txt', octets: 1 }], false);
        expect(retenues.at(-1)).toBe(false);
    });
});

describe('page-shell', () => {
    it('opens a browser window when the supervisor announces a window', () => {
        const { bureau, ouvertes } = bureauDeTest();
        bureau.fenetreOuverte('w-1', 'Bloc-notes');
        expect(ouvertes.has('w-1')).toBe(true);
        expect(bureau.list()).toEqual([{ session: 'w-1', titre: 'Bloc-notes', ouverte: true }]);
    });

    it('passes on to the supervisor the viewport the page announces', () => {
        const { bureau, envoyes } = bureauDeTest();
        bureau.fenetreOuverte('w-1', 'Bloc-notes');
        bureau.viewportRecu('w-1', 1600, 900);
        expect(envoyes).toContainEqual({
            type: 'viewport', session: 'w-1', largeur: 1600, hauteur: 900,
        });
    });

    it("ignores a viewport for a session it did not open", () => {
        // Le message vient de `postMessage` : n'importe quelle page de même
        // origine peut en émettre un. Ne relayer que ce qu'on a demandé.
        const { bureau, envoyes } = bureauDeTest();
        bureau.viewportRecu('w-inventee', 800, 600);
        expect(envoyes).toHaveLength(0);
    });

    it('closes the browser window when the Windows window disappears', () => {
        const { bureau, ouvertes } = bureauDeTest();
        bureau.fenetreOuverte('w-1', 'Bloc-notes');
        bureau.fenetreFermee('w-1');
        expect(ouvertes.get('w-1')!.closed).toBe(true);
        expect(bureau.list()).toEqual([]);
    });

    it('keeps the window in its list when only the page was closed', () => {
        // Décision de D1 : fermer une page ne ferme pas l'application
        // Windows. La shell doit donc pouvoir la rouvrir.
        const { bureau, ouvertes } = bureauDeTest();
        bureau.fenetreOuverte('w-1', 'Bloc-notes');
        ouvertes.get('w-1')!.closed = true;
        expect(bureau.list()).toEqual([{ session: 'w-1', titre: 'Bloc-notes', ouverte: false }]);
    });

    it('reopens a window whose page was closed', () => {
        const { bureau, ouvertes } = bureauDeTest();
        bureau.fenetreOuverte('w-1', 'Bloc-notes');
        ouvertes.get('w-1')!.closed = true;
        bureau.rouvrir('w-1');
        expect(ouvertes.get('w-1')!.closed).toBe(false);
        expect(bureau.list()).toEqual([{ session: 'w-1', titre: 'Bloc-notes', ouverte: true }]);
    });

    it('displays a refusal without opening anything', () => {
        const show = vi.fn();
        const bureau = createDesktop({
            ouvrirFenetre: () => { throw new Error('nothing must be opened'); },
            envoyer: () => {},
            show: show,
            showFilesState: () => {},
            showPendingWrites: () => {},
            showRetained: () => {},
        });
        bureau.refus('F9', 'no virtual output available any more');
        // Le motif du refus est REPRIS TEL QUEL, et le ton l'accompagne : le
        // sous-bloc S3 a ajouté le second argument, et une assertion à un seul
        // argument cesserait de décrire l'appel réel.
        expect(show).toHaveBeenCalledWith(
            expect.stringContaining('no virtual output available any more'),
            'danger',
        );
    });

    it("reports a pop-up block rather than ignoring it", () => {
        // `window.open` rend `null` quand le navigateur bloque : sans ce
        // traitement, l'utilisateur verrait une fenêtre listée « ouverte »
        // qui n'existe pas.
        const show = vi.fn();
        const bureau = createDesktop({
            ouvrirFenetre: () => null,
            envoyer: () => {},
            show: show,
            showFilesState: () => {},
            showPendingWrites: () => {},
            showRetained: () => {},
        });
        bureau.fenetreOuverte('w-1', 'Bloc-notes');
        expect(show).toHaveBeenCalledWith(expect.stringContaining('pop-up'), 'danger');
        expect(bureau.list()).toEqual([{ session: 'w-1', titre: 'Bloc-notes', ouverte: false }]);
    });
});

describe('file drive state', () => {
    it('mounting the drive displays the folder name', () => {
        const { bureau, filesStates } = bureauDeTest();
        bureau.lecteurMonte('Mes documents');
        expect(filesStates.at(-1)).toContain('Mes documents');
    });

    it('🔴 unmounting the drive ERASES the state', () => {
        // 🔴 C'est le défaut relevé en D5 : le bandeau `#status` gardait son
        // `textContent` après `expirer()`, si bien que lire le texte prouvait
        // qu'un message était ARRIVÉ, jamais qu'il était AFFICHÉ. Une recette
        // entière a lu un bandeau périmé en croyant lire l'état courant.
        //
        // La chaîne vide n'est donc pas un détail de présentation : c'est
        // l'assertion elle-même.
        const { bureau, filesStates } = bureauDeTest();
        bureau.lecteurMonte('Mes documents');
        bureau.lecteurDemonte();
        expect(filesStates.at(-1)).toBe('');
    });

    it('a mount failure is told apart from an unmount', () => {
        // « rien n'est partagé » et « le partage a raté, voici pourquoi »
        // n'appellent pas le même geste de l'utilisateur : le second lui dit
        // quoi corriger, le premier lui dit seulement de recommencer.
        const { bureau, filesStates } = bureauDeTest();
        bureau.lecteurEchoue('signaling unreachable');
        expect(filesStates.at(-1)).toContain('signaling unreachable');
        expect(filesStates.at(-1)).not.toBe('');
    });

    it('a remount replaces the previous name instead of adding to it', () => {
        const { bureau, filesStates } = bureauDeTest();
        bureau.lecteurMonte('Premier');
        bureau.lecteurMonte('Second');
        expect(filesStates.at(-1)).toContain('Second');
        expect(filesStates.at(-1)).not.toContain('Premier');
    });
});

describe('shell page — the banner TONE', () => {
    // La règle est ICI, dans `shell.ts`, et non dans le câblage : `shell-page.ts`
    // ne fait que relayer. Une condition qui apparaîtrait là-bas serait au
    // mauvais endroit.

    it('gives the DANGER tone to a refusal', () => {
        const { bureau, bandeaux } = bureauDeTest();
        bureau.refus('Bloc-notes', 'no output available any more');
        expect(bandeaux).toHaveLength(1);
        expect(bandeaux[0].ton).toBe('danger');
    });

    it('gives the DANGER tone to a blocked pop-up', () => {
        // L'utilisateur doit AGIR — autoriser les pop-ups. Un bandeau neutre
        // dirait que la fenêtre est en route ; elle n'existera jamais.
        const bandeaux: Array<{ message: string; ton: Ton }> = [];
        const sansPopup = createDesktop({
            ouvrirFenetre: () => null,
            envoyer: () => {},
            show: (message, ton) => { bandeaux.push({ message, ton }); },
            showFilesState: () => {},
            showPendingWrites: () => {},
            showRetained: () => {},
        });
        sansPopup.fenetreOuverte('w-1', 'Bloc-notes');
        expect(bandeaux).toHaveLength(1);
        expect(bandeaux[0].ton).toBe('danger');
        expect(bandeaux[0].message).toContain('blocked the pop-up');
    });

    it('tells a SUCCESSFUL mount from a FAILURE by the tone, not only by the text', () => {
        // `shell.ts` exige déjà que les deux textes soient distincts ; le ton
        // ne doit pas défaire cette distinction en les rendant identiques à
        // l'œil.
        const { bureau, etats } = bureauDeTest();
        bureau.lecteurMonte('Documents');
        bureau.lecteurEchoue('permission refused');
        expect(etats.map((e) => e.ton)).toEqual(['succes', 'danger']);
    });

    it('on an unmount, the string stays EMPTY', () => {
        const { bureau, etats } = bureauDeTest();
        bureau.lecteurDemonte();
        expect(etats).toHaveLength(1);
        expect(etats[0].texte).toBe('');
    });

    it("on an unmount, the banner takes NO tone — separate assertion", () => {
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

describe('the due writes counter', () => {
    it('🔴 returns TWO numbers, one of them CUMULATIVE that never goes down', () => {
        // 🔴 Le remettre à zéro rendrait un verdict négatif INDISCERNABLE d'une
        // mesure non prise : `dues = 0` est aussi ce que rend une machine où
        // rien n'a encore eu lieu. *Un verdict négatif exige que la chose
        // mesurée soit ABSENTE, pas seulement nulle.*
        const { bureau, compteurs } = bureauDeTest();
        bureau.ecrituresDues([{ chemin: 'a.txt', octets: 3 }], false);
        bureau.ecrituresDues([], false);
        expect(compteurs.map((c) => [c.dues, c.vues])).toEqual([
            [1, 1],
            [0, 1],
        ]);
    });

    it('the announcement OVERWRITES the state, it does not add to it', () => {
        // Le pont envoie l'ÉTAT complet de son journal à chaque changement :
        // cumuler ferait qu'un chemin acquitté resterait affiché POUR TOUJOURS.
        const { bureau, compteurs } = bureauDeTest();
        bureau.ecrituresDues(
            [
                { chemin: 'a.txt', octets: 1 },
                { chemin: 'b.txt', octets: 2 },
            ],
            false,
        );
        bureau.ecrituresDues([{ chemin: 'b.txt', octets: 2 }], false);
        expect(compteurs.at(-1)?.dues).toBe(1);
        expect(compteurs.at(-1)?.texte).toContain('b.txt');
        expect(compteurs.at(-1)?.texte).not.toContain('a.txt');
    });

    it('🔴 NAMES the files, because `beforeunload` cannot', () => {
        // ⛔ Le message personnalisé de `beforeunload` est IGNORÉ par tous les
        // navigateurs modernes. Les nommer DANS LA PAGE est ce qui reste.
        const { bureau, compteurs } = bureauDeTest();
        bureau.ecrituresDues([{ chemin: 'dossier/rapport final.docx', octets: 12 }], false);
        expect(compteurs.at(-1)?.texte).toContain('dossier/rapport final.docx');
    });

    it('🔴 a failure NAMES the file AND the cause', () => {
        // « Une écriture a échoué » ne dit pas à l'utilisateur quel document
        // rouvrir, ni s'il doit libérer de la place ou rendre une permission.
        const { bureau, compteurs } = bureauDeTest();
        bureau.ecrituresDues([{ chemin: 'note.txt', octets: 3 }], false);
        bureau.ecritureEchouee('note.txt', 'disque-plein');
        expect(compteurs.at(-1)?.texte).toContain('note.txt');
        expect(compteurs.at(-1)?.texte).toContain('disque-plein');
        expect(compteurs.at(-1)?.ton).toBe('danger');
    });

    it('🔴 zero due erases the text AND sets the neutral tone', () => {
        // 🔴 C'EST LE DÉFAUT DE D5, que `lecteurDemonte` documente déjà contre
        // lui-même : un bandeau qui garde son texte fait lire un état PÉRIMÉ
        // comme l'état courant. Et un ton coloré sans texte serait une alarme
        // sans énoncé — les deux propriétés sont éprouvées SÉPARÉMENT.
        const { bureau, compteurs } = bureauDeTest();
        bureau.ecrituresDues([{ chemin: 'a.txt', octets: 1 }], false);
        bureau.ecrituresDues([], false);
        expect(compteurs.at(-1)?.texte).toBe('');
        expect(compteurs.at(-1)?.ton).toBe('neutre');
    });

    it('a write that finally arrives erases its failure', () => {
        const { bureau, compteurs } = bureauDeTest();
        bureau.ecrituresDues([{ chemin: 'a.txt', octets: 1 }], false);
        bureau.ecritureEchouee('a.txt', 'interne');
        bureau.ecrituresDues([], false);
        expect(compteurs.at(-1)?.ton).toBe('neutre');
        expect(compteurs.at(-1)?.texte).toBe('');
    });

    it('🔴 does NOT warn when there is nothing to lose', () => {
        // Prévenir TOUJOURS apprendrait à l'utilisateur à ignorer
        // l'avertissement, ce qui le rendrait inutile exactement le jour où il
        // compte.
        const { bureau } = bureauDeTest();
        expect(bureau.doitPrevenir()).toBe(false);
        bureau.ecrituresDues([{ chemin: 'a.txt', octets: 1 }], false);
        expect(bureau.doitPrevenir()).toBe(true);
        bureau.ecrituresDues([], false);
        expect(bureau.doitPrevenir()).toBe(false);
    });
});

/** Le harnais des tests de mutation : `bureauDeTest` plus un accès au dernier compteur. */
function vues() {
    const h = bureauDeTest();
    return {
        bureau: h.bureau,
        dernieresDues: () => h.compteurs[h.compteurs.length - 1],
    };
}

describe('the failed mutations (F3)', () => {
    it('🔴 are NAMED, and a rename carries ITS TWO paths', () => {
        // « impossible de renommer X » ne dit pas vers quoi — et c'est
        // précisément ce que l'utilisateur doit vérifier : la destination
        // existe peut-être déjà.
        const v = vues();
        v.bureau.mutationEchouee('brouillon.txt → note.txt', 'deja-present');
        expect(v.dernieresDues()?.texte).toContain('brouillon.txt → note.txt');
        expect(v.dernieresDues()?.texte).toContain('deja-present');
    });

    it('🔴 are a DANGER even without any due write', () => {
        // C'est ce qui les distingue d'une écriture en échec : les deux côtés
        // ont divergé, et rien ne les réconciliera tout seul.
        const v = vues();
        v.bureau.mutationEchouee('a → b', 'introuvable');
        expect(v.dernieresDues()?.ton).toBe('danger');
        expect(v.dernieresDues()?.dues).toBe(0);
    });

    it('🔴 DO NOT DISAPPEAR when the due writes drop back to zero', () => {
        // Rouge : les effacer dans `ecrituresDues`, comme les échecs
        // d'écriture. Une divergence définitive s'effacerait alors toute seule,
        // et l'utilisateur ne saurait jamais qu'un fichier n'a pas été renommé
        // sur son poste.
        const v = vues();
        v.bureau.ecrituresDues([{ chemin: 'x.txt', octets: 1 }], false);
        v.bureau.mutationEchouee('a → b', 'introuvable');
        v.bureau.ecrituresDues([], false);
        expect(v.dernieresDues()?.texte).toContain('a → b');
        expect(v.dernieresDues()?.ton).toBe('danger');
    });

    it('are erased when the drive is REMOUNTED, and only then', () => {
        const v = vues();
        v.bureau.mutationEchouee('a → b', 'introuvable');
        v.bureau.lecteurDemonte();
        expect(v.dernieresDues()?.texte).toBe('');
        expect(v.dernieresDues()?.ton).toBe('neutre');
    });

    it('accumulate, unlike the due writes which overwrite', () => {
        const v = vues();
        v.bureau.mutationEchouee('a → b', 'x');
        v.bureau.mutationEchouee('c', 'y');
        const texte = v.dernieresDues()?.texte ?? '';
        expect(texte).toContain('a → b');
        expect(texte).toContain('c');
        expect(texte).toContain('2 renames or removals');
    });
});

// 🔴 CORRECTIF DU LEGS DES FREINS MANQUANTS (round de correction 1,
// critique ④), 25 août 2026 — AVANT CE LOT, `type:'error'` ne correspondait à
// AUCUNE branche de l'aiguillage de `shell-page.ts`, et le socket
// n'installait ni `close` ni `error` : un refus arrivé sur `/signal` (le
// budget « toute requête » que ce même lot ouvre, `signaling/relais.ts`)
// laissait la page affichée « bureau connecté », morte en silence.
describe('shell page — the refusal and the loss of the control channel', () => {
    it('a channel refusal displays the REASON, as DANGER', () => {
        const { bureau, bandeaux } = bureauDeTest();
        bureau.canalDeControleRefuse('too many requests', 'trop-de-requetes', undefined);
        expect(bandeaux).toHaveLength(1);
        expect(bandeaux[0].ton).toBe('danger');
        expect(bandeaux[0].message).toContain('too many requests');
    });

    it('the VOLUME refusal carries the suggested delay, when supplied', () => {
        // `retryApresS` n'accompagne QUE `trop-de-requetes` (`relais.ts`) —
        // c'est la moitié la moins chère du remède au verrouillage que
        // `agent/src/superviseur/boucle/surveillance_pont.rs` documente.
        const { bureau, bandeaux } = bureauDeTest();
        bureau.canalDeControleRefuse('too many requests', 'trop-de-requetes', 42);
        expect(bandeaux[0].message).toContain('42');
    });

    it("a handshake refusal WITHOUT a delay promises no wait", () => {
        const { bureau, bandeaux } = bureauDeTest();
        bureau.canalDeControleRefuse('invalid token', 'jeton-invalide', undefined);
        expect(bandeaux[0].message).not.toMatch(/attempt/);
    });

    it('without a `reason`, the `motif` serves as fallback', () => {
        const { bureau, bandeaux } = bureauDeTest();
        bureau.canalDeControleRefuse(undefined, 'trop-de-requetes', undefined);
        expect(bandeaux[0].message).toContain('trop-de-requetes');
    });

    it('losing the channel is announced, as DANGER — never silently', () => {
        const { bureau, bandeaux } = bureauDeTest();
        bureau.canalDeControlePerdu();
        expect(bandeaux).toHaveLength(1);
        expect(bandeaux[0].ton).toBe('danger');
        expect(bandeaux[0].message).toMatch(/lost|reload/i);
    });
});
