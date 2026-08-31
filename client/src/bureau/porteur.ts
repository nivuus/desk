// L'ÉLECTION DE L'ONGLET QUI TIENT LA SESSION DE CONTRÔLE — la règle, pure
// et testée. Le câblage (socket, `BroadcastChannel`, DOM) vit dans
// `porteur-dom.ts`.
//
// 🔴 POURQUOI UNE ÉLECTION EXISTE. Le rôle `client` est EXCLUSIF par session
// (`plateforme/src/signaling/appariement.ts::declarer` — « un client est déjà
// connecté à la session … »). Tant que le bureau vivait dans une fenêtre
// NOMMÉE (`window.open(url, 'nivuus-bureau')`), il ne pouvait pas y en avoir
// deux. Depuis que le hub — atteint par l'URL racine, donc ouvrable en autant
// d'onglets qu'on veut — porte cette session, la question se pose vraiment.
//
// 🔵 LA PRIMITIVE EST **Web Locks**, PRISE TELLE QUELLE PLUTÔT QUE
// RECONSTRUITE. Un onglet qui obtient le verrou le tient jusqu'à sa mort, et
// le navigateur le libère lui-même : c'est exactement « le premier garde la
// session, et sa fermeture promeut un autre ». Une élection écrite à la main
// sur `BroadcastChannel` devrait DÉTECTER LA MORT D'UN PAIR, ce qu'aucun
// événement ne signale — c'est le trou que `setInterval(redessiner, 1000)`
// bouche déjà ailleurs, faute de mieux.

import type { FenetreConnue } from '../shell';

/// Le nom du verrou. ⚠️ **IL SE COMPOSE AVEC LE PRÉFIXE DE VM** avant usage
/// (`prefixe.ts::composer`), exactement comme le nom de session : sans lui,
/// deux VMs différentes ouvertes dans deux onglets s'excluraient l'une
/// l'autre — le défaut que P3 a corrigé sur le nom de session, réintroduit
/// par la porte de derrière.
export const NOM_VERROU = 'nivuus-bureau';

export type Role = 'porteur' | 'suiveur';

export interface DepsElection {
    /// Demande le verrou exclusif. `pendant` est appelée QUAND il est obtenu,
    /// et le verrou est tenu tant que la promesse qu'elle rend n'est pas
    /// résolue — d'où son type `Promise<never>` : on ne le rend jamais.
    ///
    /// `undefined` quand `navigator.locks` n'existe pas.
    verrou?: (nom: string, pendant: () => Promise<never>) => void;
    /// Cet onglet tient la session : il ouvre le socket.
    devenirPorteur(): void;
    /// Cet onglet suit : il n'ouvre AUCUN socket et affiche l'état diffusé.
    devenirSuiveur(): void;
}

/// Élit cet onglet, ou l'installe en suiveur en attendant son tour.
///
/// ⚠️ **LE REPLI SANS `navigator.locks` EST OPTIMISTE, ET C'EST DÉLIBÉRÉ** :
/// se déclarer suiveur ferait qu'AUCUN onglet n'ouvrirait jamais la session,
/// et le produit serait mort sur ce navigateur-là. On tente, la plateforme
/// tranche, et `estPlacePrise` rattrape le perdant en silence.
export function elire(nomVerrou: string, deps: DepsElection): void {
    if (deps.verrou === undefined) {
        deps.devenirPorteur();
        return;
    }
    deps.devenirSuiveur();
    deps.verrou(nomVerrou, () => {
        deps.devenirPorteur();
        // 🔴 UNE PROMESSE QUI NE SE RÉSOUT JAMAIS : c'est l'idiome des Web
        // Locks pour tenir un verrou jusqu'à la mort du contexte. Le
        // navigateur le libère à la fermeture de l'onglet, sans qu'aucun code
        // n'ait à l'orchestrer — y compris sur un plantage, où aucun
        // `beforeunload` ne courrait.
        return new Promise<never>(() => {});
    });
}

/// Ce refus est-il « la place est déjà prise » ?
///
/// 🔴 **SUR LE MOTIF TYPÉ, JAMAIS SUR LA PHRASE.** `reason` est du français
/// destiné à un humain, et il se reformule ; un client qui le comparerait
/// casserait en silence le jour où quelqu'un l'améliore. C'est le piège de
/// F1, payé neuf minutes sur deux messages qui partageaient une sous-chaîne.
///
/// ⚠️ **TOUT AUTRE REFUS REND `false`, ET C'EST LE POINT** : le frein de
/// volume (`trop-de-requetes`, avec son `retryApresS`) doit rester VISIBLE.
/// L'avaler ferait de cette élection la panne muette qu'elle prétend éviter.
export function estPlacePrise(message: unknown): boolean {
    if (typeof message !== 'object' || message === null) return false;
    return (message as { motif?: unknown }).motif === 'role-occupe';
}

/// L'état que le porteur diffuse aux autres onglets.
///
/// 🔴 **IL NE PORTE QUE DE L'ÉTAT, JAMAIS UN ORDRE.** `window.open` exige une
/// activation utilisateur **dans l'onglet qui a le geste** : relayer un clic
/// vers le porteur le ferait ouvrir hors activation, donc bloqué. Ce serait
/// déplacer le mur d'un cran — ce que `hub/bureau.ts` a explicitement refusé
/// de faire. Chaque onglet ouvre ses propres fenêtres depuis ses propres
/// clics.
export interface EtatDiffuse {
    type: 'etat-bureau';
    fenetres: FenetreConnue[];
}

export function batirEtat(fenetres: FenetreConnue[]): EtatDiffuse {
    return { type: 'etat-bureau', fenetres };
}

/// Lit un message reçu sur le canal, ou rend `undefined` si ce n'en est pas
/// un des nôtres.
///
/// ⚠️ **UN `BroadcastChannel` EST PARTAGÉ PAR ORIGINE** : tout ce qui y passe
/// ne vient pas forcément de nous, et une entrée mal formée est ÉCARTÉE plutôt
/// que laissée passer — une liste à moitié valide vaut mieux qu'un `undefined`
/// sur le champ `titre` au moment de peindre.
export function lireEtat(donnees: unknown): FenetreConnue[] | undefined {
    if (typeof donnees !== 'object' || donnees === null) return undefined;
    const message = donnees as { type?: unknown; fenetres?: unknown };
    if (message.type !== 'etat-bureau') return undefined;
    if (!Array.isArray(message.fenetres)) return undefined;
    return message.fenetres.filter(
        (f: unknown): f is FenetreConnue =>
            typeof f === 'object' &&
            f !== null &&
            typeof (f as FenetreConnue).session === 'string' &&
            typeof (f as FenetreConnue).titre === 'string' &&
            typeof (f as FenetreConnue).ouverte === 'boolean',
    );
}
