// Le frein : combien d'échecs récents une clé a-t-elle accumulés, et faut-il
// refuser le suivant sans le payer.
//
// Ce module est PUR — une `Map`, aucune horloge lue, aucun socket, aucune base
// —, sur le patron exact de `signaling/propriete.ts`, et pour la même raison :
// c'est ce qui le rend testable sans ouvrir la moindre connexion. `maintenant`
// est un PARAMÈTRE partout, comme dans `agents/fraicheur.ts`,
// `identite/jeton.ts` et `depot/session.ts`.
//
// 🔴 IL REFUSE, IL NE RETARDE PAS. Un frein par temporisation garde le socket
// ouvert pendant l'attente : c'est un SECOND déni de service offert à
// l'attaquant, celui-là même qu'on prétendait fermer. Ce module ne rend qu'un
// verdict ; l'appelant répond 429 immédiatement, avec `Retry-After`.
//
// 🔴 POURQUOI L'ÉTAT EST EN MÉMOIRE ET NON EN BASE, et la première raison est
// décisive :
//   1. un frein en base fait ÉCRIRE l'attaquant. Chaque tentative deviendrait
//      un `INSERT`/`UPDATE` : le frein serait un amplificateur de charge,
//      exactement ce qu'il existe pour empêcher ;
//   2. le gestionnaire `message` de `ws` est SYNCHRONE (`signaling/relais.ts`)
//      et une promesse rejetée y abat tout le process Node — le frein du canal
//      `/agent` doit donc pouvoir répondre SANS `await` ;
//   3. un WebSocket vit dans un processus et un seul (spec §3.1), et la
//      scalabilité horizontale est hors périmètre v1 (spec §9) : il n'y a
//      aucune seconde instance avec qui partager cet état.
//
// ⚠️ LE COÛT, nommé et NON corrigé, sur le patron de `ProprieteDeSession` :
// ce frein NE SURVIT PAS à un redémarrage du service. Un attaquant qui
// parviendrait à le faire redémarrer remettrait les compteurs à zéro — mais
// s'il le peut, il a déjà mieux à faire. Ce qui reste vrai sans réserve : LE
// FREIN D'UNE INSTANCE NE PROTÈGE QUE CETTE INSTANCE. Deux instances
// multiplieraient chaque budget par deux, sans que rien ne le dise — c'est
// pourquoi le déploiement n'en déclare qu'une seule.
//
// ⚠️ L'ARBITRAGE QUI SE RETOURNE CONTRE L'UTILISATEUR LÉGITIME, et il faut
// l'écrire : un attaquant peut brûler le budget d'un compte qu'il vise et en
// refuser l'accès à son propriétaire pendant la fenêtre. C'est l'arbitrage
// classique du verrouillage de compte. Il est accepté parce que (a) la fenêtre
// est courte, (b) le déblocage par courriel exigerait un SMTP, que la spec §9
// range hors périmètre v1, et (c) l'alternative — ne freiner que par adresse —
// laisse passer la force brute CIBLÉE, qui est la menace nommée.
//
// ⚠️ LA FENÊTRE EST ANCRÉE AU PREMIER ÉCHEC D'UNE SÉRIE, ET N'EST PAS
// GLISSANTE — divergence assumée avec le mot « glissante » du plan, et voici
// pourquoi. Une fenêtre réellement glissante retient un horodatage PAR ÉCHEC,
// donc `O(max)` par clé : à `ECHECS_MAX_ADRESSE = 50` et `ENTREES_MAX =
// 10 000`, c'est un demi-million d'horodatages, soit un ordre de grandeur
// au-dessus du coût que D2 calcule pour la borne (« quelques centaines de
// kilo-octets »). Ici chaque clé coûte deux nombres, et la borne de D2 est
// tenue au sens où elle a été calculée.
//   LE PRIX DE CE CHOIX, et il est réel : à la frontière de deux fenêtres, un
//   attaquant peut placer `max` échecs juste avant et `max` juste après, soit
//   `2 × max` en rafale. C'est un facteur DEUX, pas un ordre de grandeur, et
//   il ne rend aucun secret devinable — il rend la rafale deux fois plus
//   longue.
//   ET IL A UNE CONTREPARTIE QUI SERT L'ARBITRAGE CI-DESSUS : la fenêtre
//   n'étant PAS repoussée par les échecs suivants, un attaquant qui martèle un
//   compte ne prolonge pas indéfiniment le verrouillage de son propriétaire.
//   Le compte revient à `FENETRE_MS` du PREMIER échec, quoi qu'il arrive.

/// Le budget d'une clé. Il est passé à chaque appel plutôt que retenu par le
/// frein : c'est ce qui laisse deux clés de budgets DIFFÉRENTS (un compte et
/// une adresse) vivre dans la même table, sans qu'une seconde table ne
/// diverge le jour où l'une des deux sera durcie.
export interface Budget {
    readonly max: number;
    readonly fenetreMs: number;
}

export interface Verdict {
    readonly freine: boolean;
    /// Secondes à attendre, arrondies VERS LE HAUT. Vaut 0 quand rien n'est
    /// freiné. ⚠️ Jamais 0 quand quelque chose l'est : un `Retry-After: 0`
    /// inviterait le demandeur à revenir immédiatement.
    readonly retryApresS: number;
}

/// ⚠️ LES QUATRE CONSTANTES SONT NON CALIBRÉES, et elles rejoignent la liste
/// que ce dépôt tient depuis le chantier C : `BPP_MIN`, `FACTEUR_FOCUS`,
/// `PART_DORMANTE_BPS`, `HYSTERESIS`, `TAILLE_MAX_SORTIE`,
/// `SEUIL_INJOIGNABLE_MS`, `DUREE_JETON_ACCES_MS`. Aucun jugement d'usage
/// n'a été porté sur aucune d'elles.

/// La mémoire d'un échec.
export const FENETRE_MS = 15 * 60_000;

/// Les essais contre UN compte. Petit : c'est le seul frein qui ferme la force
/// brute CIBLÉE.
export const ECHECS_MAX_COMPTE = 5;

/// Les essais depuis UNE adresse, tous comptes confondus. Dix fois plus grand :
/// derrière un NAT, plusieurs utilisateurs légitimes partagent une adresse, et
/// c'est le seul frein qui ferme le BALAYAGE de comptes.
export const ECHECS_MAX_ADRESSE = 50;

/// 🔴 LA MOITIÉ QUI COMPTE. Un frein qui retiendrait une entrée par clé vue
/// serait un vecteur d'ÉPUISEMENT MÉMOIRE : un attaquant essaie un million de
/// courriels distincts, chacun une seule fois, et la table grandit sans
/// qu'aucun budget ne soit jamais dépassé. Le frein devient l'attaque.
///
/// ⚠️ CE QUE L'ÉVICTION COÛTE, et qui n'est pas rattrapable ici : sous
/// saturation, un attaquant peut faire évincer l'entrée d'un compte qu'il vise
/// pour lui rendre son budget. C'est le prix de la borne, et la borne vaut
/// mieux que la mémoire.
export const ENTREES_MAX = 10_000;

/// Les deux budgets du service, construits UNE fois.
///
/// ⚠️ ILS SONT ICI, ET NON CHEZ LEURS APPELANTS, POUR QUE `/auth/*` ET
/// `/agent` NE PUISSENT PAS DIVERGER. D4 le dit : « pas de seconde table —
/// deux freins distincts divergeraient le jour où l'un serait durci ». La même
/// raison vaut pour les budgets et pour la forme des clés ci-dessous.
export const BUDGET_COMPTE: Budget = { max: ECHECS_MAX_COMPTE, fenetreMs: FENETRE_MS };
export const BUDGET_ADRESSE: Budget = { max: ECHECS_MAX_ADRESSE, fenetreMs: FENETRE_MS };

/// 🔴 LES CLÉS SONT PRÉFIXÉES, ET LES PRÉFIXES SONT DISJOINTS. Sans eux, une
/// VM nommée `203.0.113.7` partagerait le budget de l'adresse `203.0.113.7`, et
/// un attaquant pourrait épuiser l'un pour fermer l'autre. Les trois
/// constructeurs vivent ici pour que personne n'en écrive un quatrième.

/// ⚠️ LE COURRIEL EST NORMALISÉ EN MINUSCULES ET DÉTOURÉ. Sans quoi
/// `ADA@exemple.test` serait une seconde clé, et le budget d'un compte se
/// multiplierait par le nombre de casses qu'un attaquant sait écrire.
export function cleCompte(email: string): string {
    return `compte:${email.trim().toLowerCase()}`;
}

export function cleAdresse(adresse: string): string {
    return `adr:${adresse}`;
}

export function cleVm(vmId: string): string {
    return `agent:${vmId}`;
}

interface Entree {
    compte: number;
    /// L'instant du PREMIER échec de la série — voir l'en-tête : c'est lui qui
    /// ancre la fenêtre, et il n'est pas repoussé par les échecs suivants.
    premierA: number;
    /// Retenue avec l'entrée, et non relue du budget de l'appelant : la purge
    /// et l'éviction doivent pouvoir dater une entrée sans savoir de quelle
    /// clé elle vient.
    fenetreMs: number;
}

export class Frein {
    private readonly entrees = new Map<string, Entree>();
    private readonly entreesMax: number;
    private evincees = 0;

    /// `entreesMax` est un PARAMÈTRE, et pas seulement pour le test : une
    /// instance qui servirait un déploiement plus grand n'a pas à recompiler.
    constructor(entreesMax: number = ENTREES_MAX) {
        this.entreesMax = entreesMax;
    }

    /// Consulte SANS RIEN ENREGISTRER. Appelée AVANT tout travail coûteux —
    /// avant `scrypt`, avant le moindre accès à la base.
    ///
    /// 🔴 `consulter` ET `echec` SONT DEUX FONCTIONS, JAMAIS UNE. Une fonction
    /// unique qui consulterait ET compterait ferait payer un échec à une
    /// requête légitime arrivée pendant la fenêtre, et rendrait le frein
    /// AUTO-ENTRETENU : un attaquant maintiendrait un compte bloqué
    /// indéfiniment sans jamais tenter un mot de passe.
    consulter(cles: readonly (readonly [string, Budget])[], maintenant: number): Verdict {
        let restantMs = 0;
        for (const [cle, budget] of cles) {
            const entree = this.entrees.get(cle);
            if (entree === undefined) continue;
            // `>=`, jamais `>` : au budget exactement, on refuse. Un `>`
            // accorderait un essai de plus que ce que la constante annonce.
            if (entree.compte >= budget.max) {
                const restant = entree.premierA + entree.fenetreMs - maintenant;
                // 🔴 UN RESTE NON POSITIF *EST* L'EXPIRATION, et c'est pourquoi
                // il n'y a PAS de second test d'expiration ici. `restantMs`
                // part de 0 : une entree dont la fenetre est close rend un
                // reste <= 0, qui ne peut donc jamais l'elever.
                //
                // ⚠️ CE COMMENTAIRE A ETE ECRIT APRES UNE MUTATION QUI N'A RIEN
                // PERTURBE. Un `if (this.expiree(entree, maintenant)) continue;`
                // vivait deux lignes plus haut, et le RETIRER laissait les onze
                // tests VERTS : il etait EXACTEMENT redondant avec la
                // comparaison ci-dessous, `maintenant - premierA >= fenetreMs`
                // etant la meme proposition que `premierA + fenetreMs -
                // maintenant <= 0`. Une ligne qu'aucun test ne peut faire
                // tomber n'est pas une ceinture : c'est du code mort qui donne
                // l'APPARENCE d'une garde, et qui aurait fait croire l'an
                // prochain que l'expiration est decidee la. Elle est decidee
                // ICI, et la mutation `T1-c` porte desormais sur cette ligne.
                if (restant > restantMs) restantMs = restant;
            }
        }
        if (restantMs <= 0) return { freine: false, retryApresS: 0 };
        // Arrondi VERS LE HAUT : voir `Verdict.retryApresS`.
        return { freine: true, retryApresS: Math.ceil(restantMs / 1000) };
    }

    /// Enregistre un échec sur chacune des clés.
    echec(cles: readonly (readonly [string, Budget])[], maintenant: number): void {
        for (const [cle, budget] of cles) {
            const entree = this.entrees.get(cle);
            if (entree !== undefined && !this.expiree(entree, maintenant)) {
                entree.compte += 1;
                continue;
            }
            if (entree === undefined) this.faireDeLaPlace(maintenant);
            // Une entrée expirée est RÉARMÉE sur place, jamais accumulée : la
            // fenêtre repart du présent, et la table ne grandit pas.
            this.entrees.set(cle, { compte: 1, premierA: maintenant, fenetreMs: budget.fenetreMs });
        }
    }

    /// Efface une clé — appelée sur un SUCCÈS.
    ///
    /// ⚠️ L'APPELANT NE LUI PASSE QUE LA CLÉ DE COMPTE. La passer aussi sur la
    /// clé d'adresse BLANCHIRAIT un attaquant qui possède un compte valide : il
    /// lui suffirait de s'y connecter entre deux rafales pour rendre son
    /// budget d'adresse à zéro. Ce module n'impose rien — il efface ce qu'on
    /// lui nomme —, et c'est la route qui tient la règle.
    succes(cle: string): void {
        this.entrees.delete(cle);
    }

    /// Pour la trace, et pour le test du plafond.
    taille(): number {
        return this.entrees.size;
    }

    /// 🔴 UNE ÉVICTION N'EST JAMAIS SILENCIEUSE POUR L'EXPLOITANT : ce compteur
    /// monte, la trace le lit, et une saturation cesse d'être invisible.
    evictions(): number {
        return this.evincees;
    }

    private expiree(entree: Entree, maintenant: number): boolean {
        return maintenant - entree.premierA >= entree.fenetreMs;
    }

    /// Purge d'abord, évince ensuite — et jamais l'inverse : évincer une entrée
    /// VIVANTE alors que la table est pleine d'entrées mortes rendrait son
    /// budget à un attaquant pour rien.
    private faireDeLaPlace(maintenant: number): void {
        if (this.entrees.size < this.entreesMax) return;

        for (const [cle, entree] of this.entrees) {
            if (this.expiree(entree, maintenant)) this.entrees.delete(cle);
        }
        if (this.entrees.size < this.entreesMax) return;

        // Encore pleine : on évince celle dont la fenêtre se referme le plus
        // tôt. C'est celle dont la disparition coûte le moins de mémoire du
        // passé — et c'est un choix, pas une évidence : voir `ENTREES_MAX`.
        let plusTot: string | undefined;
        let plusTotA = Number.POSITIVE_INFINITY;
        for (const [cle, entree] of this.entrees) {
            const finA = entree.premierA + entree.fenetreMs;
            if (finA < plusTotA) {
                plusTotA = finA;
                plusTot = cle;
            }
        }
        if (plusTot !== undefined) {
            this.entrees.delete(plusTot);
            this.evincees += 1;
        }
    }
}
