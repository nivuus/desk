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
    /// réglée.
    ///
    /// 🔴 **SON TYPE ÉTAIT `Promise<never>` JUSQU'À LA REVUE FINALE DU 31 AOÛT
    /// 2026, ET C'ÉTAIT LE DÉFAUT** (Important ③) : une promesse `never` ne
    /// peut se régler que par un REJET, si bien que le porteur démis par
    /// `estPlacePrise` n'avait AUCUN moyen de rendre son verrou — sa partition
    /// n'aurait alors plus jamais eu de porteur. Elle est `Promise<void>` et
    /// se résout **exactement une fois**, quand `Election::relacher` est
    /// appelée ; sur le chemin nominal, personne ne l'appelle et la propriété
    /// « ne se résout jamais » est préservée mot pour mot.
    ///
    /// `undefined` quand `navigator.locks` n'existe pas.
    verrou?: (nom: string, pendant: () => Promise<void>) => void;
    /// Cet onglet tient la session : il ouvre le socket.
    devenirPorteur(): void;
    /// Cet onglet suit : il n'ouvre AUCUN socket et affiche l'état diffusé.
    devenirSuiveur(): void;
}

/// Ce que `elire` rend : le moyen de RENDRE le verrou détenu.
export interface Election {
    /// Relâche le verrou, s'il est détenu. **Idempotente**, et sans effet
    /// tant que le verrou n'a pas été obtenu — il n'y a alors rien à rendre.
    ///
    /// ⚠️ **CET ONGLET NE SE REMET PAS DANS LA FILE**, et c'est délibéré : se
    /// redemander le verrou aussitôt le reprendrait dans l'instant (personne
    /// d'autre n'attend dans CETTE partition), la plateforme le refuserait de
    /// nouveau, et la boucle refus → relâche → reprise tournerait sans fin.
    /// **Limite déclarée** : après une démission, cet onglet-ci reste suiveur
    /// jusqu'à son rechargement. Ce qui est gagné est que le verrou est LIBRE,
    /// donc qu'un autre onglet — ou un rechargement — peut prendre la place,
    /// ce qui était impossible avant.
    relacher(): void;
}

/// Élit cet onglet, ou l'installe en suiveur en attendant son tour.
///
/// ⚠️ **LE REPLI SANS `navigator.locks` EST OPTIMISTE, ET C'EST DÉLIBÉRÉ** :
/// se déclarer suiveur ferait qu'AUCUN onglet n'ouvrirait jamais la session,
/// et le produit serait mort sur ce navigateur-là. On tente, la plateforme
/// tranche, et `estPlacePrise` rattrape le perdant en silence.
export function elire(nomVerrou: string, deps: DepsElection): Election {
    if (deps.verrou === undefined) {
        deps.devenirPorteur();
        // Aucun verrou n'est détenu sur ce chemin : il n'y a rien à rendre.
        return { relacher: () => {} };
    }
    let rendre: (() => void) | undefined;
    deps.devenirSuiveur();
    deps.verrou(nomVerrou, () => {
        deps.devenirPorteur();
        // 🔴 UNE PROMESSE QUE PERSONNE NE RÉSOUT SUR LE CHEMIN NOMINAL : c'est
        // l'idiome des Web Locks pour tenir un verrou jusqu'à la mort du
        // contexte. Le navigateur le libère à la fermeture de l'onglet, sans
        // qu'aucun code n'ait à l'orchestrer — y compris sur un plantage, où
        // aucun `beforeunload` ne courrait. Le seul appelant de `resoudre` est
        // `relacher`, ci-dessous.
        return new Promise<void>((resoudre) => {
            rendre = resoudre;
        });
    });
    return {
        relacher: () => {
            rendre?.();
            rendre = undefined;
        },
    };
}

/* ── LA PROMOTION — CE QUE FAIT UN ONGLET QUI VIENT DE PRENDRE LA PLACE ──── */

/// 🔴 **CETTE SÉQUENCE VIVAIT DANS `porteur-dom.ts`, ET C'EST LÀ QUE LA
/// CRITIQUE ① DE LA REVUE FINALE S'EST LOGÉE.** Le socket était ouvert avec
/// `deps.jeton`, **une chaîne figée au chargement de la page**. Or un suiveur
/// n'est promu qu'à la mort du porteur, potentiellement des heures plus tard,
/// et `DUREE_JETON_ACCES_MS` vaut **dix minutes**
/// (`plateforme/src/identite/jeton.ts`) : il présentait donc un jeton expiré,
/// `garde.verifier` rendait `motif: 'expire'`, `estPlacePrise` rendait `false`,
/// et `canalDeControlePerdu()` écrasait le refus par « Rechargez la page ».
/// **La promotion — la seule chose qui justifie toute cette élection — ne
/// pouvait pas fonctionner en usage réel.**
///
/// 🔵 **LA SPEC §3 ANNONÇAIT DÉJÀ `ouvrirSocket` COMME UNE DÉPENDANCE
/// INJECTÉE ; l'implémentation ne l'avait pas suivie**, et c'est ce qui
/// laissait cette jonction hors de portée de tout test.
export interface DepsPromotion {
    /// Redemande un jeton **frais** (`jeton.ts::assurerAccesFrais`). Un
    /// FOURNISSEUR, jamais une valeur : c'est toute la correction.
    jetonFrais(): Promise<string | undefined>;
    /// Installe le pont fichiers pour cet onglet, désormais porteur.
    ///
    /// ⚠️ **IL NE REÇOIT PAS LE JETON, ET C'EST VOULU** : « Choisir mon
    /// dossier » est un geste qui peut arriver n'importe quand après la
    /// promotion, donc un jeton passé ICI serait périmé au moment du clic —
    /// le défaut qu'on vient de corriger, réintroduit d'un cran plus bas. Le
    /// pont redemande le sien au clic (`bureau/fichiers-dom.ts`).
    installerPont(): void;
    /// Ouvre le socket de la session de contrôle, avec le jeton FRAIS.
    ouvrirSocket(jeton: string): void;
    /// Aucun jeton obtenable : le dire de façon ACTIONNABLE, plutôt qu'ouvrir
    /// un socket voué au refus.
    sansJeton(): void;
}

/// L'ordre est le point : le pont d'abord, le socket ensuite, et **le jeton
/// redemandé avant les deux**.
export async function promouvoir(deps: DepsPromotion): Promise<void> {
    const jeton = await deps.jetonFrais();
    if (jeton === undefined) {
        deps.sansJeton();
        return;
    }
    deps.installerPont();
    deps.ouvrirSocket(jeton);
}

/// Qui ouvre la fenêtre quand on clique « Rouvrir » ?
///
/// 🔴 **C'EST UNE RÈGLE, ET ELLE ÉTAIT DANS LE CÂBLAGE** (Minor ④ de la revue
/// finale) : `porteur-dom.ts` déclare « AUCUNE RÈGLE ICI » et portait pourtant
/// ce ternaire. Le porteur passe par `bureau.rouvrir`, qui MÉMORISE le handle
/// `Window` et permet à `shell.ts::liste` de dire « ouverte » ; un suiveur
/// n'a pas de `bureau` alimenté et ouvre directement — sa fenêtre est réelle,
/// mais le porteur ne la voit pas (legs déclaré du chantier).
export function ouvertureParLeBureau(role: Role): boolean {
    return role === 'porteur';
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
/// déplacer le mur d'un cran — ce que `porteur-dom.ts` refuse explicitement
/// de faire (voir son suiveur, qui ouvre depuis SON PROPRE clic). Chaque
/// onglet ouvre ses propres fenêtres depuis ses propres clics.
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

/// La demande qu'un onglet qui vient d'arriver pose sur le canal.
///
/// 🔴 **SANS ELLE, UN ONGLET QUI REJOINT APRÈS STABILISATION NE REÇOIT JAMAIS
/// RIEN** (Important ① de la revue finale). `diffuserSiChange` ne poste que sur
/// CHANGEMENT d'empreinte, et le porteur ignorait tout message du canal : en
/// régime — trois fenêtres, rien qui bouge —, un second onglet montrait une
/// liste **vide, pour toujours**, ce qui contredit la spec §4 (« Les onglets
/// non porteurs l'affichent à l'identique »).
///
/// 🔵 **CE N'EST PAS UN ORDRE, ET LE §4 TIENT TOUJOURS.** Le canal ne
/// transporte que de l'état ; une demande d'état est une demande de
/// DIFFUSION, jamais un ordre d'ouvrir quoi que ce soit — aucune activation
/// utilisateur n'est en jeu.
export interface DemandeEtat {
    type: 'demande-etat';
}

export function batirDemande(): DemandeEtat {
    return { type: 'demande-etat' };
}

/// Ce message est-il une demande d'état ?
///
/// ⚠️ **SUR LE TYPE, JAMAIS SUR LA PRÉSENCE** : un `BroadcastChannel` est
/// partagé par origine, et tout ce qui y passe ne vient pas de nous — même
/// raison que `lireEtat` ci-dessous.
export function estDemandeEtat(donnees: unknown): boolean {
    if (typeof donnees !== 'object' || donnees === null) return false;
    return (donnees as { type?: unknown }).type === 'demande-etat';
}

/// Lit une trame BRUTE du socket de signaling, ou rend `undefined` si ce n'en
/// est pas une exploitable.
///
/// 🔴 **AJOUTÉE PAR LA REVUE FINALE (Minor ③) : `JSON.parse(evenement.data)`
/// ÉTAIT NU DANS L'ÉCOUTEUR DU SOCKET DE CONTRÔLE.** Une trame non-JSON y
/// levait, et l'exception remontait dans un gestionnaire d'événement.
/// `plateforme/src/signaling/relais.ts` a lui-même dû ajouter le garde
/// symétrique, et `client/src/webrtc.ts::parseSignalingMessage` porte le même
/// depuis longtemps : `JSON.parse` réussit sur `"null"`, `"42"`, `'"x"'` et
/// `"[1,2]"`, et `null.type` lève une `TypeError`. On valide donc **objet non
/// nul et non tableau** avant toute lecture de propriété.
export function lireTrame(brut: unknown): Record<string, unknown> | undefined {
    if (typeof brut !== 'string') return undefined;
    let analyse: unknown;
    try {
        analyse = JSON.parse(brut);
    } catch {
        return undefined;
    }
    if (typeof analyse !== 'object' || analyse === null || Array.isArray(analyse)) return undefined;
    return analyse as Record<string, unknown>;
}

/// Ce qu'un onglet doit PEINDRE, étant donné son rôle, sa PROPRE liste (celle
/// que `shell.ts::creerBureau().liste()` rend), et le DERNIER état reçu sur
/// le canal — jamais `bureau.liste()` seule.
///
/// 🔴 **CETTE RÈGLE VIVAIT DANS `porteur-dom.ts`, DONT L'EN-TÊTE DÉCLARE
/// N'EN PORTER AUCUNE — ET C'EST LÀ QUE LE DÉFAUT S'EST LOGÉ** (revue round
/// 1, critique ①). Sur un SUIVEUR, `bureau.liste()` est structurellement
/// VIDE : aucun message `fenetre-ouverte` n'atteint son `bureau`, qui
/// n'ouvre aucun socket (`porteur-dom.ts::ouvrirLaSession` ne court QUE chez
/// le porteur), et son `rouvrir` appelle `window.open` DIRECTEMENT sans
/// passer par `bureau.rouvrir`. Une minuterie qui repeindrait depuis
/// `bureau.liste()` chez un suiveur EFFACERAIT donc, moins d'une seconde
/// après chaque diffusion reçue, la liste qu'elle venait de montrer — pas
/// une absence d'information, une information FAUSSE : la panne muette que
/// ce chantier prétend éviter.
///
/// 🔴 **LIMITE DÉCLARÉE, PAS CORRIGÉE (Important ② de la revue finale du
/// 31 août 2026) : LA PROMOTION EFFACE LA LISTE QUE LE SUIVEUR AFFICHAIT.**
/// Un onglet promu bascule sur la branche `porteur`, dont `listePropre` est
/// **structurellement vide** — son `bureau` n'a jamais reçu le moindre
/// `fenetre-ouverte`, faute de socket avant la promotion. Il peint donc `[]`
/// au premier tour, et l'état qu'il montrait disparaît.
/// **L'effet net, dit en clair** : *fermer l'onglet porteur prive
/// définitivement les autres onglets de la liste des fenêtres* — le porteur
/// promu ne la retrouvera que si l'agent réannonce (fenêtres en
/// `AttendLeViewport`, lot 17), jamais pour une fenêtre déjà `Vivante`.
/// ⚠️ **CE N'EST PAS UN OUBLI : LA CORRECTION EST HORS DE PORTÉE DE CE LOT.**
/// Elle supposerait une méthode neuve sur `client/src/shell.ts` — semer le
/// `bureau` avec l'état reçu —, or la spec §6 gèle ce fichier nommément
/// (« INCHANGÉ : la règle du bureau est déjà pure et testée »). C'est une
/// décision de conception qui appartient au propriétaire du dépôt, pas à une
/// vague de correction.
export function fenetresAPeindre(
    role: Role,
    listePropre: FenetreConnue[],
    dernierEtatRecu: FenetreConnue[] | undefined,
): FenetreConnue[] {
    // Le porteur EST la source de vérité : sa propre liste, toujours — un
    // état reçu avant sa propre promotion serait périmé.
    if (role === 'porteur') return listePropre;
    // Le suiveur n'a QUE ce qu'on lui a diffusé. Rien reçu encore n'est pas
    // un mensonge : c'est l'état initial exact, avant toute diffusion.
    return dernierEtatRecu ?? [];
}
