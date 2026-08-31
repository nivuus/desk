// LE CÂBLAGE DU BUREAU DANS LE HUB : élection, socket de la session de
// contrôle, canal entre onglets, DOM.
//
// 🔴 CE FICHIER EST LE SUCCESSEUR DE `shell-page.ts`, ET IL NE REPREND PAS SON
// DÉFAUT : ses dépendances sont INJECTÉES, et les trois règles qu'il emploie
// (`porteur.ts`, `fenetres-dom.ts`, `shell.ts`) sont testées ailleurs.
//
// ⚠️ AUCUNE RÈGLE ICI. Une condition qui déciderait quelque chose du produit
// doit descendre dans `porteur.ts` ou `shell.ts`.
//
// 🔴 CETTE DÉCLARATION A ÉTÉ PRISE EN DÉFAUT DEUX FOIS, ET LES DEUX SONT
// NOMMÉES PLUTÔT QUE TUES :
//   ① revue round 1 — « quelle liste cet onglet peint-il » ÉTAIT une règle,
//      posée ici sous la forme d'un `bureau.liste()` appelé sans condition par
//      la minuterie. Descendue dans `porteur.ts::fenetresAPeindre`.
//   ② revue FINALE (31 août 2026, Minor ④) — « qui ouvre la fenêtre au clic
//      Rouvrir » en était une autre, sous la forme d'un ternaire
//      `porteur ? bureau.rouvrir : window.open`. Descendue dans
//      `porteur.ts::ouvertureParLeBureau`. **La séquence de promotion**
//      (redemander un jeton, installer le pont, ouvrir le socket) est
//      descendue dans `porteur.ts::promouvoir` par la même revue.
//
// ⚠️ CE QUI RESTE ICI, ET QUE LA DÉCLARATION NE DOIT PAS PROMETTRE D'AVOIR
// SORTI : le choix des TEXTES affichés et le branchement des écouteurs DOM.
// Changer un texte ne change aucune décision du produit — c'est le critère
// reproductible de ce dépôt, et il est appliqué ici plutôt que supposé.

// ⚠️ AUCUN import d'`adresseSignaling` ICI : l'URL arrive par `deps`, calculée
// par `hub/page.ts`. L'importer sans l'employer serait un `TS6133`, c'est-à-dire
// un ÉCHEC de `tsc --noEmit`, pas un avertissement.
import { composer } from '../prefixe';
import { creerBureau, type FenetreConnue, type Ton } from '../shell';
import { dessinerFenetres } from './fenetres-dom';
import { installerLePont } from './fichiers-dom';
import {
    NOM_VERROU,
    batirDemande,
    batirEtat,
    elire,
    estDemandeEtat,
    estPlacePrise,
    fenetresAPeindre,
    lireEtat,
    lireTrame,
    ouvertureParLeBureau,
    promouvoir,
    type Election,
} from './porteur';

declare global {
    interface Navigator {
        locks?: { request(nom: string, options: { mode: 'exclusive' }, pendant: () => Promise<void>): Promise<void> };
    }
}

/// Ce qu'un onglet qui ne tient pas le bureau dit de LUI-MÊME.
///
/// 🔴 **UN SUIVEUR ÉTAIT MUET SUR SON PROPRE ÉTAT** (Minor ⑥ de la revue
/// finale) : le seul texte qui l'expliquait était écrit dans `#etat-fichiers`,
/// **à l'intérieur d'un `<details>` replié**, et `#statut` restait vide.
/// ⚠️ **CE N'EST PAS UN MESSAGE D'ERREUR** — la décision « aucune erreur au
/// second onglet » (spec §2) n'interdit pas d'INFORMER, et cette ligne est ce
/// qui rend compréhensible le fait qu'un « Lancer » cliqué ici fasse paraître
/// la fenêtre dans l'autre onglet.
const TEXTE_SUIVEUR = 'Bureau tenu par un autre onglet.';

/// Le nom du verrou d'élection, PRÉFIXÉ par la VM.
///
/// 🔴 SANS LE PRÉFIXE, deux VMs différentes ouvertes dans deux onglets
/// s'excluraient l'une l'autre : le défaut que P3 a corrigé sur le nom de
/// session, réintroduit par la porte de derrière. `composer` rend le nom nu
/// quand aucun préfixe n'est connu — exactement le comportement d'avant P3.
export function nomDuVerrou(prefixe: string): string {
    return composer(prefixe, NOM_VERROU);
}

export interface CanalDiffusion {
    postMessage(message: unknown): void;
}

/// Diffuse l'état aux autres onglets **seulement s'il a changé**, et rend la
/// nouvelle empreinte.
///
/// ⚠️ LE PORTEUR REDESSINE À 1 Hz (la fermeture d'une fenêtre par
/// l'utilisateur ne prévient personne : on relit l'état plutôt que d'attendre
/// un événement qui n'existe pas). Diffuser à chaque tour réveillerait tous
/// les onglets une fois par seconde pour rien.
export function diffuserSiChange(
    canal: CanalDiffusion,
    fenetres: FenetreConnue[],
    empreintePrecedente: string,
): string {
    const empreinte = JSON.stringify(fenetres);
    if (empreinte === empreintePrecedente) return empreintePrecedente;
    canal.postMessage(batirEtat(fenetres));
    return empreinte;
}

export interface DepsBureauPage {
    signalingUrl: string;
    /// 🔴 **UN FOURNISSEUR, JAMAIS UNE CHAÎNE — ET CE CHAMP PORTAIT UNE CHAÎNE
    /// JUSQU'À LA REVUE FINALE DU 31 AOÛT 2026** (critique ①). Voir
    /// `porteur.ts::DepsPromotion` pour le défaut mesuré : un suiveur promu
    /// des heures plus tard présentait un jeton de **dix minutes**, expiré,
    /// et la promotion ne pouvait pas fonctionner en usage réel.
    ///
    /// ⚠️ **UN TEST FIGE L'ABSENCE DE JETON SCALAIRE DANS CETTE INTERFACE**
    /// (`porteur-dom.test.ts`) : c'est la JONCTION qui était fausse, pas la
    /// règle de fraîcheur, et un test d'`assurerAccesFrais` ne l'aurait
    /// jamais vue.
    jetonFrais(): Promise<string | undefined>;
    /// Le préfixe de VM (`prefixe.ts::lirePrefixe`), ou `''` s'il n'y en a pas.
    ///
    /// 🔴 **IL DOIT ÊTRE LU APRÈS QUE `GET /vm` A RÉPONDU** — critique ② de la
    /// revue finale : le hub ne posait aucun préfixe, `lirePrefixe()` rendait
    /// `''`, et le hub écoutait `bureau` pendant que l'agent annonçait sur
    /// `<prefixe>:bureau`. C'est `hub/page.ts::demarrer` qui garantit cet
    /// ordre ; ce module ne fait que recevoir la valeur.
    prefixe: string;
    /// `?faute-fichiers=1` — variable de BANC, jamais une configuration
    /// livrée. Lue UNE fois par la page et passée en argument, jamais relue
    /// ici : c'est la convention de `PLEIN_ECRAN` et de `PART_SONDAGE` côté
    /// agent — le mécanisme lit un drapeau qu'on lui donne.
    fautesArmees: boolean;
    elements: {
        statut: HTMLDivElement;
        liste: HTMLUListElement;
        modele: HTMLTemplateElement;
        sectionFenetres: HTMLElement;
        sectionFichiers: HTMLDetailsElement;
        boutonDossier: HTMLButtonElement;
        etatFichiers: HTMLDivElement;
        ecrituresDues: HTMLDivElement;
        actionsFichiers: HTMLParagraphElement;
        boutonRafraichir: HTMLButtonElement;
        boutonReprendre: HTMLButtonElement;
    };
}

export function installerLeBureau(deps: DepsBureauPage): void {
    const el = deps.elements;
    const sessionDeControle = composer(deps.prefixe, 'bureau');
    const canal = new BroadcastChannel(nomDuVerrou(deps.prefixe));
    let empreinte = '';
    let porteur = false;
    /// Le DERNIER état reçu sur le canal, côté SUIVEUR. `undefined` tant
    /// qu'aucune diffusion n'est encore arrivée — c'est ce que
    /// `fenetresAPeindre` distingue d'une liste vide diffusée pour de vrai.
    let dernierEtatRecu: FenetreConnue[] | undefined;
    // ⚠️ DÉCLARÉ AVANT `creerBureau`, dont le rappel `envoyer` le lit : une
    // fermeture qui capture un `let` déclaré plus bas compile, mais se lit
    // mal — et la zone morte temporelle est une classe d'erreur qu'on évite
    // par la disposition plutôt que par la vigilance.
    let socket: WebSocket | undefined;
    /// Rendue par `elire`. `undefined` tant que l'élection n'a pas été posée —
    /// le repli sans Web Locks appelle `devenirPorteur` SYNCHRONEMENT, donc
    /// avant cette affectation.
    let election: Election | undefined;

    const poserTon = (element: HTMLElement, ton: Ton): void => {
        element.classList.remove('message--succes', 'message--alerte', 'message--danger');
        if (ton !== 'neutre') element.classList.add(`message--${ton}`);
    };

    /// 🔴 `/index.html`, PAS `/` : la racine sert le HUB depuis le lot 14, et
    /// `/?session=…` ouvrirait le hub avec un paramètre qu'il ignore, jamais
    /// une session.
    ///
    /// ⚠️ **UN SEUL ENDROIT CONSTRUIT CETTE URL**, employé par le porteur (via
    /// `bureau.ouvrirFenetre`) ET par le suiveur : la revue finale l'a trouvée
    /// écrite deux fois, à deux endroits qu'il aurait fallu tenir d'accord.
    const ouvrirUneFenetre = (session: string): Window | null =>
        window.open(`/index.html?session=${encodeURIComponent(session)}`, `guac-${session}`);

    const bureau = creerBureau({
        ouvrirFenetre(session) {
            return ouvrirUneFenetre(session);
        },
        envoyer(message) {
            if (socket?.readyState === WebSocket.OPEN) socket.send(JSON.stringify(message));
        },
        afficher(message, ton) { el.statut.textContent = message; poserTon(el.statut, ton); },
        afficherEtatFichiers(texte, ton) { el.etatFichiers.textContent = texte; poserTon(el.etatFichiers, ton); },
        afficherEcrituresDues(dues, vues, texte, ton) {
            // 🔴 LES NOMBRES DANS DES ATTRIBUTS `data-*`, LE TEXTE DANS LA
            // PAGE. Les pilotes de recette lisent les attributs, JAMAIS le
            // texte — piège de F1.
            el.ecrituresDues.dataset.dues = String(dues);
            el.ecrituresDues.dataset.vues = String(vues);
            el.ecrituresDues.textContent = texte;
            poserTon(el.ecrituresDues, ton);
        },
        afficherRetenues(retenues) {
            el.boutonReprendre.hidden = !retenues;
            el.actionsFichiers.dataset.retenues = String(retenues);
            // Un pont qui retient ses écritures a quelque chose à dire MAINTENANT.
            if (retenues) el.sectionFichiers.open = true;
        },
    });

    // ── LE SEUL CHEMIN DE PEINTURE, POUR LES DEUX RÔLES ─────────────────────
    // 🔴 LA MINUTERIE ET LA RÉCEPTION D'UNE DIFFUSION APPELAIENT CHACUNE LEUR
    // PROPRE `dessinerFenetres(...)`, EN DOUBLE — c'est cette duplication qui
    // a produit la critique ① : la minuterie peignait depuis `bureau.liste()`
    // sans se soucier du rôle, effaçant chez un suiveur, moins d'une seconde
    // après, ce que la réception venait de montrer. Il n'y a plus qu'un seul
    // chemin : `redessiner()`, appelé par les DEUX déclencheurs, qui demande
    // à `fenetresAPeindre` (pure, testée) ce qu'il faut peindre.
    const redessiner = (): void => {
        const role = porteur ? 'porteur' : 'suiveur';
        const listePropre = bureau.liste();
        const fenetres = fenetresAPeindre(role, listePropre, dernierEtatRecu);
        dessinerFenetres(fenetres, {
            liste: el.liste,
            modele: el.modele,
            section: el.sectionFenetres,
            rouvrir: (session) => {
                if (ouvertureParLeBureau(role)) {
                    bureau.rouvrir(session);
                    redessiner();
                    return;
                }
                // ⚠️ UN SUIVEUR OUVRE SA PROPRE FENÊTRE, DEPUIS SON PROPRE
                // CLIC : `window.open` exige l'activation de CET onglet-ci. Il
                // ne prévient pas le porteur, et RIEN NE LE RATTRAPE ENSUITE :
                // `shell.ts::liste` calcule `ouverte` depuis le HANDLE que le
                // porteur détient LUI-MÊME (`e.fenetre !== null &&
                // !e.fenetre.closed`), et ce handle-là reste fermé pour
                // toujours — le suiveur vient de créer un AUTRE objet
                // `Window`, que le porteur ne voit jamais. La liste du porteur
                // dira donc « fermée » EN PERMANENCE, jusqu'à ce que le
                // PORTEUR LUI-MÊME clique « Rouvrir ». **Limite déclarée** :
                // le remède serait un ordre sur le canal, que la conception
                // exclut (spec §4), ou une méthode neuve sur `shell.ts`, que
                // la spec laisse INCHANGÉ (spec §6). Recliquer « Rouvrir »,
                // côté suiveur, ramène la même fenêtre au premier plan chez
                // LUI : aucun dommage pour lui ; seule la vue du porteur reste
                // fausse.
                //
                // 🔴 **ET UNE SECONDE CONSÉQUENCE, AJOUTÉE PAR LA REVUE FINALE
                // (Important ⑤) — UN COMMENTAIRE INCOMPLET SUR UNE LIMITE
                // DÉCLARÉE VAUT UNE PREUVE FAUSSE : LA FENÊTRE AINSI ROUVERTE
                // N'ANNONCE JAMAIS SON VIEWPORT.** `viewport-dom.ts` poste par
                // `window.opener`, donc vers CE suiveur ; `bureau.viewportRecu`
                // y retourne immédiatement (sa table `connues` est vide, aucun
                // `fenetreOuverte` ne l'ayant jamais alimentée) et `envoyer`
                // est un no-op faute de socket. **Conséquence, celle du lot
                // 33 : le recadrage et la taille de la fenêtre restent ceux de
                // la session précédente, et rien ne le trace.** Limite
                // déclarée, non corrigée : la corriger supposerait de relayer
                // un message vers le porteur, ce que la spec §4 exclut.
                ouvrirUneFenetre(session);
            },
        });
        // La diffusion, elle, reste réservée au porteur, et porte SA PROPRE
        // liste — jamais `fenetres`, qui chez un suiveur est le dernier état
        // REÇU : le rediffuser bouclerait l'écho au lieu de porter du neuf.
        if (porteur) empreinte = diffuserSiChange(canal, listePropre, empreinte);
    };

    // ── LE SUIVEUR : il n'ouvre AUCUN socket ; il mémorise ce qu'on lui
    // diffuse et le fait peindre par LE MÊME `redessiner()` que la minuterie
    // — un seul chemin, jamais deux qui pourraient diverger.
    canal.addEventListener('message', (evenement) => {
        // 🔴 **UN ONGLET QUI REJOINT APRÈS STABILISATION NE RECEVAIT JAMAIS
        // RIEN** (Important ① de la revue finale) : `diffuserSiChange` ne
        // poste que sur CHANGEMENT, et le porteur ignorait tout message du
        // canal. Le porteur répond désormais à une demande d'état en
        // REMETTANT SON EMPREINTE À `''`, ce qui fait repartir la diffusion au
        // `redessiner` suivant — y compris pour une liste vide, dont
        // l'empreinte `'[]'` diffère de `''`. Un suiveur ignore la demande
        // d'un autre suiveur : il n'a rien à diffuser.
        if (estDemandeEtat(evenement.data)) {
            if (!porteur) return;
            empreinte = '';
            redessiner();
            return;
        }
        if (porteur) return;
        const fenetres = lireEtat(evenement.data);
        if (fenetres === undefined) return;
        dernierEtatRecu = fenetres;
        redessiner();
    });

    // 🔴 **LE PONT FICHIERS SUIT L'ÉLECTION, ET C'EST UNE CONSÉQUENCE DE CETTE
    // TÂCHE, PAS UN OUBLI** (Important ③, revue round 1) : la session du pont
    // (`fichiers/canal.ts::sessionDuPont`) est FIXE PAR VM et porte, elle
    // aussi, le rôle `client` — EXCLUSIF. Le raisonnement de `porteur.ts`
    // pour la session de contrôle (« tant que le bureau vivait dans une
    // fenêtre NOMMÉE, il ne pouvait pas y en avoir deux ») vaut MOT POUR MOT
    // ici. Un suiveur qui l'installerait quand même ouvrirait un second
    // socket que la plateforme refuserait — pas une erreur à montrer : un
    // ÉTAT à DIRE, l'onglet suiveur n'étant pas fautif de ne pas gérer les
    // fichiers. **Les deux fonctions ci-dessous sont PARTAGÉES** entre
    // `devenirSuiveur` et le rattrapage du repli optimiste démis
    // (`estPlacePrise`, plus bas) : les deux chemins mènent au même « cet
    // onglet ne gère pas les fichiers ».
    const desactiverLePont = (): void => {
        el.boutonDossier.disabled = true;
        el.etatFichiers.textContent = 'Les fichiers sont gérés par l’onglet qui tient le bureau.';
        poserTon(el.etatFichiers, 'neutre');
    };
    const activerLePont = (): void => {
        el.boutonDossier.disabled = false;
        el.etatFichiers.textContent = '';
        poserTon(el.etatFichiers, 'neutre');
    };

    const ouvrirLaSession = (): void => {
        porteur = true;
        // CET onglet vient d'être promu : annuler l'état que `devenirSuiveur`
        // avait posé — y compris le texte de `#statut`, qui dirait sinon
        // « Bureau tenu par un autre onglet » alors que c'est CELUI-CI qui le
        // tient désormais.
        el.statut.textContent = 'connexion du bureau…';
        poserTon(el.statut, 'neutre');
        activerLePont();
        // 🔴 **LA SÉQUENCE EST DANS `porteur.ts::promouvoir`, PURE ET TESTÉE**
        // (critique ① de la revue finale). Elle redemande un jeton FRAIS
        // avant d'ouvrir le socket : un suiveur n'est promu qu'à la mort du
        // porteur, potentiellement des heures après le chargement, et un jeton
        // figé au chargement vit **dix minutes**.
        void promouvoir({
            jetonFrais: () => deps.jetonFrais(),
            // Le pont est installé ICI plutôt qu'au montage du module — ainsi
            // un onglet promu PLUS TARD (le cas ordinaire : suiveur au
            // chargement, porteur seulement quand le précédent ferme)
            // l'obtient lui aussi, sans code supplémentaire.
            installerPont: () =>
                installerLePont({
                    bureau,
                    signalingUrl: deps.signalingUrl,
                    // ⚠️ LE FOURNISSEUR, PAS LE JETON QU'ON VIENT D'OBTENIR :
                    // « Choisir mon dossier » est un geste qui peut arriver
                    // n'importe quand après la promotion.
                    jetonFrais: () => deps.jetonFrais(),
                    fautesArmees: deps.fautesArmees,
                    boutonDossier: el.boutonDossier,
                    boutonRafraichir: el.boutonRafraichir,
                    boutonReprendre: el.boutonReprendre,
                    section: el.sectionFichiers,
                }),
            ouvrirSocket: (jeton) => ouvrirLeSocket(jeton),
            sansJeton: () => {
                // ⚠️ ACTIONNABLE, et non « une erreur est survenue » : le
                // rechargement relance `assurerAccesFrais`, donc Pomerium.
                el.statut.textContent =
                    'Votre session a expiré. Rechargez la page pour vous reconnecter.';
                poserTon(el.statut, 'danger');
                desactiverLePont();
            },
        });
    };

    const ouvrirLeSocket = (jeton: string): void => {
        socket = new WebSocket(deps.signalingUrl);
        socket.addEventListener('open', () => {
            socket!.send(JSON.stringify({ role: 'client', session: sessionDeControle, jeton }));
            el.statut.textContent = 'bureau connecté';
            poserTon(el.statut, 'neutre');
        });
        socket.addEventListener('message', (evenement) => {
            // 🔴 `JSON.parse` NU ICI JUSQU'À LA REVUE FINALE (Minor ③) : une
            // trame non-JSON levait dans un gestionnaire d'événement. La garde
            // vit dans `porteur.ts::lireTrame`, jumelle de celle que
            // `plateforme/src/signaling/relais.ts` a dû ajouter de son côté.
            const message = lireTrame(evenement.data);
            if (message === undefined) return;
            if (message.type === 'fenetre-ouverte') bureau.fenetreOuverte(message.session as string, message.titre as string);
            else if (message.type === 'fenetre-fermee') bureau.fenetreFermee(message.session as string);
            else if (message.type === 'refus') bureau.refus(message.titre as string, message.motif as string);
            else if (message.type === 'error') {
                if (estPlacePrise(message)) {
                    // 🔴 « LA PLACE EST PRISE » N'EST PAS UNE ERREUR À
                    // MONTRER. Cet onglet a tenté de devenir porteur, un
                    // autre tenait déjà la session côté plateforme. Il
                    // redevient suiveur, EN SILENCE — un second onglet n'est
                    // pas une faute de l'utilisateur. Tout autre refus, dont
                    // le frein de volume, reste affiché.
                    //
                    // ⚠️ **CE CHEMIN N'EST PAS RÉSERVÉ AU REPLI SANS WEB
                    // LOCKS** — une affirmation trop large (Minor round 1) :
                    // les Web Locks sont cloisonnés PAR PARTITION DE
                    // STOCKAGE (une fenêtre de navigation privée, ou un
                    // second navigateur, tient SA PROPRE partition), donc un
                    // onglet peut très bien détenir SON verrou et viser
                    // pourtant la MÊME session côté plateforme. Ce chemin est
                    // donc atteint aussi HORS repli, chaque fois que deux
                    // partitions distinctes visent la même VM.
                    porteur = false;
                    socket?.close();
                    // 🔴 **LE VERROU EST RENDU, ET IL NE L'ÉTAIT PAS**
                    // (Important ③ de la revue finale) : la promesse tenue par
                    // `elire` était un `Promise<never>` que rien ne résolvait,
                    // si bien qu'un porteur démis gardait le verrou POUR
                    // TOUJOURS et que sa partition n'avait **plus jamais** de
                    // porteur. Il repart en suiveur, verrou libéré.
                    // ⚠️ Il ne se remet PAS dans la file : voir
                    // `porteur.ts::Election::relacher` pour la raison (une
                    // boucle refus → relâche → reprise) et pour la limite que
                    // cela laisse.
                    election?.relacher();
                    // Minor round 1 : le bandeau tenait encore « bureau
                    // connecté », posé de façon optimiste à l'ouverture du
                    // socket, AVANT de savoir si la plateforme refuserait.
                    // Le dire tel quel après la démotion : cet onglet N'EST
                    // PLUS celui qui tient le bureau.
                    el.statut.textContent = TEXTE_SUIVEUR;
                    poserTon(el.statut, 'neutre');
                    // 🔴 CE CHEMIN (repli SANS Web Locks) a installé le pont
                    // de façon OPTIMISTE, EN MÊME TEMPS que `porteur = true`
                    // ci-dessus, avant de savoir si la plateforme refuserait
                    // -- exactement comme le bandeau. Le rattraper de la
                    // même façon : ce n'est plus cet onglet qui gère les
                    // fichiers.
                    desactiverLePont();
                    redessiner();
                    return;
                }
                bureau.canalDeControleRefuse(
                    message.reason as string | undefined,
                    message.motif as string | undefined,
                    message.retryApresS as number | undefined,
                );
            }
            redessiner();
        });
        socket.addEventListener('close', () => {
            // ⚠️ SILENCIEUX SI NOUS AVONS CÉDÉ LA PLACE : `canalDeControlePerdu`
            // dirait « Rechargez la page », ce qui serait faux ici.
            if (porteur) bureau.canalDeControlePerdu();
        });
    };

    election = elire(nomDuVerrou(deps.prefixe), {
        verrou:
            typeof navigator !== 'undefined' && 'locks' in navigator
                ? (nom, pendant) => void navigator.locks!.request(nom, { mode: 'exclusive' }, pendant)
                : undefined,
        devenirPorteur: ouvrirLaSession,
        devenirSuiveur: () => {
            porteur = false;
            // ⚠️ **DIRE SON ÉTAT, PAS SEULEMENT LE TAIRE** (Minor ⑥) : sans
            // cette ligne, `#statut` restait VIDE et la seule explication du
            // rôle de cet onglet vivait dans `#etat-fichiers`, à l'intérieur
            // d'un `<details>` REPLIÉ. Un onglet promu plus tard écrase ce
            // texte dès `ouvrirLaSession` (« connexion du bureau… »).
            el.statut.textContent = TEXTE_SUIVEUR;
            poserTon(el.statut, 'neutre');
            desactiverLePont();
        },
    });

    window.addEventListener('beforeunload', (evenement) => {
        if (!bureau.doitPrevenir()) return;
        evenement.preventDefault();
    });

    // Les pages de session annoncent leur viewport par `postMessage` sur leur
    // ouvreuse — c'est-à-dire ici.
    window.addEventListener('message', (evenement) => {
        if (evenement.origin !== window.location.origin) return;
        const message = evenement.data;
        if (message?.type === 'viewport') {
            bureau.viewportRecu(message.session, message.largeur, message.hauteur);
        }
    });

    // 🔴 **LA DEMANDE D'ÉTAT, POSÉE AU MONTAGE** (Important ① de la revue
    // finale). Sans elle, un onglet qui rejoint APRÈS stabilisation — trois
    // fenêtres, rien qui bouge — reste sur une liste vide POUR TOUJOURS,
    // `diffuserSiChange` ne postant que sur changement.
    //
    // ⚠️ **POSTÉE SANS CONDITION DE RÔLE, ET C'EST CORRECT** : un
    // `BroadcastChannel` ne délivre PAS à son propre émetteur, et il n'existe
    // qu'un porteur par partition — un onglet qui vient d'être élu porteur ne
    // peut donc pas se réveiller lui-même, et sa demande ne trouve personne à
    // qui la poser. Attendre de connaître le rôle exigerait d'attendre que
    // Web Locks tranche, c'est-à-dire de retarder la seule chose qui rende un
    // suiveur utile.
    canal.postMessage(batirDemande());

    // La fermeture d'une page par l'utilisateur ne prévient personne : on
    // relit l'état périodiquement plutôt que d'attendre un événement qui
    // n'existe pas.
    setInterval(redessiner, 1000);
}
