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
    /**
     * 🔴 LE COMPTEUR EST COLLECTÉ TEL QUEL — deux NOMBRES, un texte, un ton —
     * et non reformaté ici. Le lire d'une phrase serait rejouer le piège que F1
     * a payé neuf minutes : « Lecteur … monté » et « n'a pas pu être monté »
     * partagent une sous-chaîne, le pilote testait `includes('mont')`, et une
     * mesure entière a tourné sur un pont NON monté.
     */
    const compteurs: Array<{ dues: number; vues: number; texte: string; ton: Ton }> = [];
    const retenues: boolean[] = [];
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
        afficherRetenues: (r: boolean) => {
            retenues.push(r);
        },
        afficherEcrituresDues: (dues, vues, texte, ton) => {
            compteurs.push({ dues, vues, texte, ton });
        },
    });
    return { bureau, ouvertes, envoyes, etatsFichiers, bandeaux, etats, compteurs, retenues };
}

describe('F5 — les écritures RETENUES', () => {
    /**
     * 🔴 **`retenues` REMONTE, ET IL EST FILTRÉ PAR « il y a des dues ».**
     *
     * Retenir sans aucune due n'a pas de sens : le bouton proposerait de
     * reprendre ce qu'il n'y a pas à reprendre. Le pont ne l'émet pas ainsi,
     * mais s'en remettre à lui ferait dépendre l'interface d'une propriété
     * qu'aucun type ne garantit.
     */
    it('retenues avec des dues est annoncé', () => {
        const { bureau, retenues } = bureauDeTest();
        bureau.ecrituresDues([{ chemin: 'a.txt', octets: 1 }], true);
        expect(retenues.at(-1)).toBe(true);
    });

    it('🔴 retenues SANS aucune due n’est PAS annoncé', () => {
        const { bureau, retenues } = bureauDeTest();
        bureau.ecrituresDues([], true);
        expect(retenues.at(-1)).toBe(false);
    });

    it('des dues NON retenues ne l’annoncent pas', () => {
        const { bureau, retenues } = bureauDeTest();
        bureau.ecrituresDues([{ chemin: 'a.txt', octets: 1 }], false);
        expect(retenues.at(-1)).toBe(false);
    });

    /**
     * 🔴 **RETENIR EST UNE ALERTE, JAMAIS UN `neutre`.** Rien ne repartira sans
     * un geste de l'utilisateur, et un ton neutre laisserait croire que le pont
     * travaille encore.
     */
    it('🔴 une reprise retenue porte le ton « alerte »', () => {
        const { bureau, compteurs } = bureauDeTest();
        bureau.ecrituresDues([{ chemin: 'a.txt', octets: 1 }], true);
        expect(compteurs.at(-1)?.ton).toBe('alerte');
    });

    /**
     * L'état est celui du PONT, pas de l'interface : il tombe quand le pont
     * cesse de retenir, et pas avant.
     */
    it('l’état retombe quand le pont cesse de retenir', () => {
        const { bureau, retenues } = bureauDeTest();
        bureau.ecrituresDues([{ chemin: 'a.txt', octets: 1 }], true);
        expect(retenues.at(-1)).toBe(true);
        bureau.ecrituresDues([{ chemin: 'a.txt', octets: 1 }], false);
        expect(retenues.at(-1)).toBe(false);
    });
});

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
            afficherEcrituresDues: () => {},
            afficherRetenues: () => {},
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
            afficherEcrituresDues: () => {},
            afficherRetenues: () => {},
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
            afficherEcrituresDues: () => {},
            afficherRetenues: () => {},
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

describe('le compteur d’écritures dues', () => {
    it('🔴 rend DEUX nombres, dont un CUMULATIF qui ne redescend jamais', () => {
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

    it('l’annonce ÉCRASE l’état, elle ne s’y ajoute pas', () => {
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

    it('🔴 NOMME les fichiers, parce que `beforeunload` ne le peut pas', () => {
        // ⛔ Le message personnalisé de `beforeunload` est IGNORÉ par tous les
        // navigateurs modernes. Les nommer DANS LA PAGE est ce qui reste.
        const { bureau, compteurs } = bureauDeTest();
        bureau.ecrituresDues([{ chemin: 'dossier/rapport final.docx', octets: 12 }], false);
        expect(compteurs.at(-1)?.texte).toContain('dossier/rapport final.docx');
    });

    it('🔴 un échec NOMME le fichier ET la cause', () => {
        // « Une écriture a échoué » ne dit pas à l'utilisateur quel document
        // rouvrir, ni s'il doit libérer de la place ou rendre une permission.
        const { bureau, compteurs } = bureauDeTest();
        bureau.ecrituresDues([{ chemin: 'note.txt', octets: 3 }], false);
        bureau.ecritureEchouee('note.txt', 'disque-plein');
        expect(compteurs.at(-1)?.texte).toContain('note.txt');
        expect(compteurs.at(-1)?.texte).toContain('disque-plein');
        expect(compteurs.at(-1)?.ton).toBe('danger');
    });

    it('🔴 zéro due efface le texte ET pose le ton neutre', () => {
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

    it('une écriture qui finit par arriver efface son échec', () => {
        const { bureau, compteurs } = bureauDeTest();
        bureau.ecrituresDues([{ chemin: 'a.txt', octets: 1 }], false);
        bureau.ecritureEchouee('a.txt', 'interne');
        bureau.ecrituresDues([], false);
        expect(compteurs.at(-1)?.ton).toBe('neutre');
        expect(compteurs.at(-1)?.texte).toBe('');
    });

    it('🔴 ne prévient PAS quand il n’y a rien à perdre', () => {
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

describe('les mutations en échec (F3)', () => {
    it('🔴 sont NOMMÉES, et un renommage porte SES DEUX chemins', () => {
        // « impossible de renommer X » ne dit pas vers quoi — et c'est
        // précisément ce que l'utilisateur doit vérifier : la destination
        // existe peut-être déjà.
        const v = vues();
        v.bureau.mutationEchouee('brouillon.txt → note.txt', 'deja-present');
        expect(v.dernieresDues()?.texte).toContain('brouillon.txt → note.txt');
        expect(v.dernieresDues()?.texte).toContain('deja-present');
    });

    it('🔴 sont un DANGER même sans aucune écriture due', () => {
        // C'est ce qui les distingue d'une écriture en échec : les deux côtés
        // ont divergé, et rien ne les réconciliera tout seul.
        const v = vues();
        v.bureau.mutationEchouee('a → b', 'introuvable');
        expect(v.dernieresDues()?.ton).toBe('danger');
        expect(v.dernieresDues()?.dues).toBe(0);
    });

    it('🔴 NE DISPARAISSENT PAS quand les écritures dues redescendent à zéro', () => {
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

    it('sont effacées au REMONTAGE du lecteur, et là seulement', () => {
        const v = vues();
        v.bureau.mutationEchouee('a → b', 'introuvable');
        v.bureau.lecteurDemonte();
        expect(v.dernieresDues()?.texte).toBe('');
        expect(v.dernieresDues()?.ton).toBe('neutre');
    });

    it('s’accumulent, à l’inverse des écritures dues qui écrasent', () => {
        const v = vues();
        v.bureau.mutationEchouee('a → b', 'x');
        v.bureau.mutationEchouee('c', 'y');
        const texte = v.dernieresDues()?.texte ?? '';
        expect(texte).toContain('a → b');
        expect(texte).toContain('c');
        expect(texte).toContain('2 renommages ou suppressions');
    });
});
