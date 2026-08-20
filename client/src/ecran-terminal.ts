// L'ÉCRAN PLEIN CADRE DES ÉTATS TERMINAUX — sous-projet ⑥, sous-bloc S4,
// tâche 9. C'est la famille 1 du §5.2 de la spec : « un état terminal cesse
// d'occuper six lignes dans un bandeau de 12 px de marge ».
//
// ═══════════════════════════════════════════════════════════════════════════
// 🔴 CE MODULE NE DÉPLACE AUCUNE FRONTIÈRE, IL REND VISIBLE CELLE QUE LE CODE
// PORTE DÉJÀ. « Terminal » n'est pas un mot inventé ici : `status.ts` distingue
// depuis le chantier E un message TERMINAL d'un message PERSISTANT et d'un
// message ordinaire, et EXACTEMENT DEUX appels du produit passent
// `terminal: true` — la fin de session et l'échec de connexion, tous deux dans
// `main.ts`. Les onze autres appels de ce fichier restent au bandeau, y compris
// les messages persistants du lien dégradé et du sommeil.
//
// 🔴 L'ÉCRAN SE BRANCHE DANS `creerStatut`, PAS DANS UN APPELANT DE PLUS.
// `status.ts` existe précisément pour qu'« aucun appelant ne puisse oublier la
// garde » : lui ajouter une cible OPTIONNELLE garde ce point d'écriture unique,
// là où un `ecran.montrer(...)` écrit à côté de `statut.afficher(...)` dans
// `main.ts` serait deux écritures que rien n'oblige à rester d'accord.
//
// ⚠️ PUR, DÉPENDANCES INJECTÉES, TESTABLE SANS DOM — la convention de
// `status.ts`, `audio.ts` et `fullscreen.ts`. Le seul point qui touche le DOM
// est `creerEcranTerminalAuDOM`, l'adaptateur d'une ligne appelé par `main.ts`,
// exactement comme `armerPleinEcranAuDOM` de `fullscreen.ts`.
//
// ⚠️ CET ÉCRAN NE PORTE AUCUNE ACTION — ni « réessayer », ni « fermer ». Une
// action est un comportement de PRODUIT, qui appartient au sous-projet ②, et
// l'inventer ici la ferait naître sans recette. QUE CE SOIT LA BONNE FORME EST
// UN JUGEMENT HUMAIN (spec §8) : aucune commande ne le dira, et aucun œil n'est
// passé sur ⑥ d'un bout à l'autre.
//
// ⚠️ DUPLICATION DÉCLARÉE, NON RÉSORBÉE. `client/src/shell.ts` déclare déjà un
// type `Ton` et `client/src/connexion.ts` une table `CLASSE_DE_TON`. S4 ne les
// unifie pas : cela toucherait deux surfaces closes, sans critère capable
// d'attraper une régression et sans œil pour la voir. C'est le legs n°8 de la
// liste que ⑥ laisse ouverte.
// ═══════════════════════════════════════════════════════════════════════════

/// Le ton d'un état terminal. Une fin NORMALE (l'utilisateur a fermé
/// l'application distante) n'est pas une erreur ; un échec de connexion en est
/// une. Les deux sites terminaux de `main.ts` ne changent QUE pour porter ce
/// mot.
export type TonTerminal = 'neutre' | 'danger';

/// Ce dont ce module a besoin de l'écran, et rien de plus.
export interface CibleEcranTerminal {
    /// Le conteneur plein cadre. `hidden` tant qu'aucun état terminal n'est
    /// survenu — et la feuille doit poser `.ecran[hidden] { display: none }`
    /// EXPLICITEMENT, un sélecteur de classe l'emportant en spécificité sur le
    /// `[hidden]` de la feuille de l'agent utilisateur. Sans cette règle,
    /// l'écran serait visible dès le chargement, sur toutes les sessions ;
    /// `client/src/style.test.ts` en fait une commande.
    racine: { hidden: boolean };
    /// Le titre, écrit depuis le TON et jamais depuis l'appelant.
    titre: { textContent: string };
    /// La raison, écrite depuis le message, et qui porte la classe de ton.
    raison: { textContent: string; className: string };
}

export interface EcranTerminal {
    /// Lève l'écran, y écrit le message, et pose le ton. Un second appel
    /// REMPLACE le premier — c'est la dernière information définitive qui
    /// gagne, la même règle que `status.ts` applique au bandeau.
    montrer(message: string, ton: TonTerminal): void;
}

/// ⚠️ LE TITRE EST DÉRIVÉ DU TON, ET NON RECOPIÉ DANS LE HTML. Un titre
/// statique serait FAUX de l'un des deux cas : « Session terminée » ment sur un
/// échec où aucune session n'a jamais commencé. Le déduire du ton coûte cette
/// table et le rend juste des deux côtés.
/// ⚠️ QUE CES DEUX LIBELLÉS SOIENT LES BONS MOTS EST UN JUGEMENT HUMAIN, et
/// c'est un de plus que les sept que le plan de S4 prévoyait — il est déclaré
/// plutôt que passé sous silence.
const TITRE: Record<TonTerminal, string> = {
    neutre: 'Session terminée',
    danger: 'Échec de la session',
};

/// La classe entière est RÉÉCRITE à chaque appel, jamais ajoutée : c'est ce qui
/// fait qu'un second état terminal neutre efface le `--danger` du premier.
const CLASSE: Record<TonTerminal, string> = {
    neutre: 'message',
    danger: 'message message--danger',
};

export function creerEcranTerminal(cible: CibleEcranTerminal): EcranTerminal {
    return {
        montrer(message, ton) {
            cible.titre.textContent = TITRE[ton];
            cible.raison.textContent = message;
            cible.raison.className = CLASSE[ton];
            cible.racine.hidden = false;
        },
    };
}

/// L'adaptateur DOM, appelé par `main.ts` — la convention
/// `armerPleinEcranAuDOM` de `fullscreen.ts`. Il ne porte AUCUNE règle : tout
/// ce qui se teste vit dans `creerEcranTerminal` ci-dessus.
export function creerEcranTerminalAuDOM(): EcranTerminal {
    return creerEcranTerminal({
        racine: document.querySelector<HTMLDivElement>('#fin')!,
        titre: document.querySelector<HTMLHeadingElement>('#fin .ecran__titre')!,
        raison: document.querySelector<HTMLParagraphElement>('#fin-raison')!,
    });
}
