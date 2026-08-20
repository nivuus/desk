/**
 * LES TROIS BOUTONS DE THÈME — le sélecteur de thème DU PRODUIT.
 *
 * ✅ CE MODULE EST DU PRODUIT DEPUIS LE SOUS-BLOC S3, et il a des tests.
 * Il était né en S1 comme code d'INSTRUMENT — il ne servait alors que les
 * pages du jugement humain du §8 — et son en-tête déclarait « ce n'est pas le
 * sélecteur de thème du produit : celui-là appartient à S3, qui décidera s'il
 * le réemploie tel quel ou le réécrit ». S3 a décidé : il le RÉEMPLOIE. Il n'y
 * a pas de seconde copie — c'est la règle que le §4.1 de la spec applique aux
 * valeurs, et il n'y a pas de raison de l'accorder aux modules.
 *
 * ── OÙ IL VIT, ET OÙ IL NE VIT PAS ────────────────────────────────────────
 * Sur la PAGE-SHELL, que la spec §5.2 nomme, et sur l'ÉCRAN DE CONNEXION, que
 * la spec ne nomme pas parce qu'il n'existait pas quand elle a été écrite
 * (son §2.5 le dit) — EXTENSION RAISONNÉE, déclarée : `shell-page.ts` y
 * redirige tout visiteur sans jeton, si bien que c'est aujourd'hui la première
 * surface, et parfois la seule, qu'un utilisateur non authentifié voie. Un
 * défaut sombre qu'on ne peut pas changer avant de s'être connecté est ce que
 * la spec §11 range sous « réversible par un utilisateur en un clic dès S3 ».
 *
 * ⛔ JAMAIS DANS LA FENÊTRE DE SESSION. La spec §5.2 l'interdit nommément :
 * une barre d'outils sur un jeu en plein écran est une régression. « La
 * fenêtre de session SUIT, elle ne choisit pas. »
 *
 * ── LES DÉPENDANCES SONT INJECTÉES ────────────────────────────────────────
 * ⚠️ NI `window`, NI `document`, NI `localStorage` NE SONT LUS DANS LE CHEMIN
 * TESTÉ. `client/` n'a NI jsdom NI happy-dom (mesuré : `client/package.json`
 * ne porte que `typescript`, `vite`, `vitest`, et aucun `@vitest-environment`
 * n'existe dans `client/src/`). Un module de produit qui lirait un global en
 * tête serait impossible à charger sous Node, donc impossible à tester. C'est
 * le patron de `theme.ts` et de `fullscreen.ts`, et `installerSelecteurDeTheme
 * AuDOM` est la couture qui les relie aux vrais objets — comme
 * `armerPleinEcranAuDOM`.
 *
 * ⚠️ `Coffre` et `Racine` sont IMPORTÉS de `theme.ts`, jamais redéclarés ici.
 * Deux copies d'un contrat divergent en silence.
 */
import { CLE_THEME, appliquer, choisir, surStockageModifie, themeStocke } from './theme';
import type { Coffre, Racine, Theme } from './theme';

/** Ce qu'un bouton de thème doit savoir faire — rien de plus. */
export interface BoutonDeTheme {
    dataset: { theme?: string };
    textContent: string | null;
    setAttribute(nom: string, valeur: string): void;
    addEventListener(type: 'click', ecouteur: () => void): void;
}

/** L'hôte qui reçoit les trois boutons. */
export interface HoteDeSelecteur {
    append(bouton: BoutonDeTheme): void;
}

/** La source des événements `storage` — `window` en production. */
export interface SourceDeStockage {
    addEventListener(
        type: 'storage',
        ecouteur: (evenement: { key: string | null; newValue: string | null }) => void,
    ): void;
}

export interface OptionsSelecteurDeTheme {
    hote: HoteDeSelecteur;
    racine: Racine;
    coffre: Coffre;
    source: SourceDeStockage;
    /** Fabrique un bouton VIERGE ; l'appelant y met ce qu'il veut de visuel. */
    creerBouton(): BoutonDeTheme;
    /** Rappelé après CHAQUE changement de thème, quelle qu'en soit l'origine. */
    apres: () => void;
}

/**
 * Pose les trois boutons dans `hote`, applique l'état stocké, et branche
 * l'écoute de `storage`.
 *
 * ⚠️ `hote` N'EST PAS VIDÉ : deux appels sur le même élément y poseraient six
 * boutons. C'est l'appelant qui décide.
 */
export function installerSelecteurDeTheme(options: OptionsSelecteurDeTheme): void {
    const { hote, racine, coffre, source, creerBouton, apres } = options;
    const etats: Theme[] = ['systeme', 'clair', 'sombre'];

    // 🔴 LES BOUTONS SONT RETENUS ICI, ET NON RELUS PAR `querySelectorAll`.
    // La version d'instrument interrogeait le DOM de l'hôte ; outre qu'aucun
    // double ne peut le faire sans jsdom, elle marquait aussi les boutons d'une
    // installation ANTÉRIEURE sur le même hôte.
    const boutons: BoutonDeTheme[] = [];
    const marquer = () => {
        const courant = themeStocke(coffre);
        for (const bouton of boutons) {
            bouton.setAttribute('aria-pressed', String(bouton.dataset.theme === courant));
        }
    };

    // L'amorce a déjà posé l'attribut avant la première peinture ; ce rappel
    // couvre le cas où le stockage a changé entre l'amorce et l'exécution de ce
    // module.
    appliquer(racine, themeStocke(coffre));

    // La bascule venue d'une AUTRE fenêtre — c'est la moitié que `choisir()` ne
    // peut pas couvrir, `storage` ne se déclenchant jamais chez l'écrivain.
    //
    // ✅ IL RAPPELLE `marquer()`, ET C'EST UNE CORRECTION DU SOUS-BLOC S3.
    // Jusque-là il ne le faisait pas, et ce module DÉCLARAIT le défaut :
    // « un `aria-pressed` posé ici reste celui du thème d'avant tant que
    // l'utilisateur ne clique pas dans CETTE fenêtre. Le défaut est réel et il
    // est PRÉEXISTANT ». Il l'était en effet — la tâche d'extraction de S1
    // avait raison de ne pas le corriger, une extraction qui corrige rendant
    // fausse sa propre preuve que rien n'a bougé.
    //
    // 🔴 CE QUI A CHANGÉ N'EST PAS LE DÉFAUT, C'EST CE QUE CE MODULE EST. Sur
    // un INSTRUMENT, un `aria-pressed` périmé est une gêne. Sur le PRODUIT,
    // c'est une interface qui MENT sur son propre état — et le cas d'une
    // fenêtre voisine qui change le thème n'est pas un cas limite : c'est le
    // cas NOMINAL du multi-fenêtres, qui est la raison d'être même du
    // mécanisme `storage` (spec §4.2). La page-shell ouvre N fenêtres de
    // session sur la même origine ; toutes reçoivent cet événement.
    //
    // ⚠️ `marquer()` LIT le coffre, il ne l'écrit pas. Écrire ici renverrait un
    // `storage` aux fenêtres voisines, qui le renverraient à leur tour, sans
    // terme — `selecteur-theme.test.ts` garde cette propriété par un test qui
    // PEUT tomber, la fermeture détenant bien le coffre.
    source.addEventListener('storage', (evenement) => {
        if (evenement.key !== CLE_THEME) return;
        surStockageModifie(racine, evenement.key, evenement.newValue);
        marquer();
        apres();
    });

    for (const etat of etats) {
        const bouton = creerBouton();
        bouton.dataset.theme = etat;
        bouton.textContent = etat;
        bouton.addEventListener('click', () => {
            choisir(coffre, racine, etat);
            marquer();
            apres();
        });
        boutons.push(bouton);
        hote.append(bouton);
    }
    marquer();
}

/**
 * LA COUTURE VERS LE VRAI DOM — la seule fonction de ce module qui touche
 * `document`, `window` et `localStorage`, et donc la seule qui ne soit pas
 * testable sans navigateur. Elle ne porte AUCUNE règle : elle ne fait que
 * fournir les objets réels à la fonction ci-dessus. Même partage que
 * `armerPleinEcran` / `armerPleinEcranAuDOM` dans `client/src/fullscreen.ts`.
 *
 * ⚠️ LES DEUX CLASSES SONT ÉCRITES EN LITTÉRAL, un `classList.add` par classe.
 * Le contrôle §7.9 ne voit que les littéraux ; une classe composée
 * (`` `bouton--${variante}` ``) lui serait invisible, et la convention §6.4 du
 * plan S3 est ce qui rend ce contrôle utile.
 *
 * ⚠️ LES BOUTONS N'ONT PAS DE LIBELLÉ ACCESSIBLE AUTRE QUE LEUR TEXTE
 * (`systeme`, `clair`, `sombre`) et leur `aria-pressed`. Aucune primitive ne
 * porte de rôle ARIA — les primitives sont du CSS, la sémantique reste au
 * balisage —, et un groupe `role="group"` avec son libellé appartiendrait au
 * balisage de chaque page. Déclaré plutôt que supposé fait.
 */
export function installerSelecteurDeThemeAuDOM(
    hote: HTMLElement,
    apres: () => void = () => {},
): void {
    installerSelecteurDeTheme({
        // ⚠️ L'HÔTE EST ADAPTÉ PLUTÔT QUE `HTMLElement` N'ÉLARGISSE LE
        // CONTRAT. `HTMLElement.append` accepte `(...nodes: (string|Node)[])`,
        // que `HoteDeSelecteur.append(bouton: BoutonDeTheme)` ne satisfait
        // pas — et c'est TypeScript qui l'a dit, pas une supposition. Élargir
        // `BoutonDeTheme` jusqu'à `Node` pour faire taire l'erreur aurait rendu
        // le double de test impossible à écrire sans jsdom, c'est-à-dire aurait
        // rendu ce module intestable pour satisfaire le compilateur.
        hote: { append: (bouton) => hote.append(bouton as unknown as HTMLElement) },
        racine: document.documentElement,
        coffre: localStorage,
        source: window,
        creerBouton: () => {
            const bouton = document.createElement('button');
            bouton.type = 'button';
            bouton.classList.add('bouton');
            bouton.classList.add('bouton--secondaire');
            return bouton;
        },
        apres,
    });
}
