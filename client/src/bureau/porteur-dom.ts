// THE DESKTOP'S WIRING IN THE HUB: election, control session socket,
// cross-tab channel, DOM.
//
// 🔴 THIS FILE IS THE SUCCESSOR OF `shell-page.ts`, AND IT DOES NOT CARRY OVER ITS
// DEFECT: its dependencies are INJECTED, and the three rules it uses
// (`porteur.ts`, `fenetres-dom.ts`, `shell.ts`) are tested elsewhere.
//
// ⚠️ NO RULE HERE. A condition that would decide something about the product
// must move down into `porteur.ts` or `shell.ts`.
//
// 🔴 THIS DECLARATION WAS CAUGHT OUT TWICE, AND BOTH ARE
// NAMED RATHER THAN KEPT QUIET:
//   ① review round 1 — "which list does this tab paint" WAS a rule,
//      set here in the form of a `bureau.liste()` called unconditionally by
//      the timer. Moved down into `porteur.ts::fenetresAPeindre`.
//   ② FINAL review (August 31st, 2026, Minor ④) — "who opens the window on a
//      Reopen click" was another, in the form of a ternary
//      `porteur ? bureau.rouvrir : window.open`. Moved down into
//      `porteur.ts::ouvertureParLeBureau`. **The promotion sequence**
//      (request a token again, install the bridge, open the socket) was
//      moved down into `porteur.ts::promouvoir` by the same review.
//
// ⚠️ WHAT STAYS HERE, AND WHICH THE DECLARATION MUST NOT CLAIM TO HAVE
// MOVED OUT: the choice of displayed TEXTS and the wiring of DOM listeners.
// Changing a text changes no product decision — it is this repository's
// reproducible criterion, and it is applied here rather than assumed.

// ⚠️ NO import of `adresseSignaling` HERE: the URL arrives through `deps`, computed
// by `hub/page.ts`. Importing it without using it would be a `TS6133`, that is,
// a FAILURE of `tsc --noEmit`, not a warning.
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

/// What a tab that does not hold the desktop says about ITSELF.
///
/// 🔴 **A FOLLOWER WAS SILENT ABOUT ITS OWN STATE** (Minor ⑥ of the final
/// review): the only text explaining it was written in `#etat-fichiers`,
/// **inside a collapsed `<details>`**, and `#statut` stayed empty.
/// ⚠️ **IT IS NOT AN ERROR MESSAGE** — the decision "no error on the
/// second tab" (spec §2) does not forbid INFORMING, and this line is what
/// makes understandable the fact that a "Launch" clicked here makes
/// the window appear in the other tab.
const TEXTE_SUIVEUR = 'Bureau tenu par un autre onglet.';

/// The election lock's name, PREFIXED by the VM.
///
/// 🔴 WITHOUT THE PREFIX, two different VMs opened in two tabs
/// would exclude each other: the defect P3 fixed on the session
/// name, reintroduced through the back door. `composer` returns the bare name
/// when no prefix is known — exactly the behaviour from before P3.
export function nomDuVerrou(prefixe: string): string {
    return composer(prefixe, NOM_VERROU);
}

export interface CanalDiffusion {
    postMessage(message: unknown): void;
}

/// Broadcasts the state to the other tabs **only if it has changed**, and returns the
/// new fingerprint.
///
/// ⚠️ THE CARRIER REDRAWS AT 1 Hz (the user closing a window
/// warns nobody: we reread the state rather than wait for
/// an event that does not exist). Broadcasting at every round would wake all
/// tabs once per second for nothing.
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
    /// 🔴 **A PROVIDER, NEVER A STRING — AND THIS FIELD CARRIED A STRING
    /// UNTIL THE FINAL REVIEW OF AUGUST 31st, 2026** (critique ①). See
    /// `porteur.ts::DepsPromotion` for the measured defect: a follower promoted
    /// hours later presented a **ten-minute** token, expired,
    /// and promotion could not work in real use.
    ///
    /// ⚠️ **A TEST FREEZES THE ABSENCE OF A SCALAR TOKEN IN THIS INTERFACE**
    /// (`porteur-dom.test.ts`): it is the JUNCTION that was wrong, not the
    /// freshness rule, and a test of `assurerAccesFrais` would never have
    /// seen it.
    jetonFrais(): Promise<string | undefined>;
    /// The VM prefix (`prefixe.ts::lirePrefixe`), or `''` if there is none.
    ///
    /// 🔴 **IT MUST BE READ AFTER `GET /vm` HAS ANSWERED** — critique ② of the
    /// final review: the hub set no prefix, `lirePrefixe()` returned
    /// `''`, and the hub listened on `bureau` while the agent announced on
    /// `<prefixe>:bureau`. It is `hub/page.ts::demarrer` that guarantees this
    /// order; this module only receives the value.
    prefixe: string;
    /// `?faute-fichiers=1` — BENCH variable, never a shipped
    /// configuration. Read ONCE by the page and passed as an argument, never reread
    /// here: it is the convention of `PLEIN_ECRAN` and `PART_SONDAGE` on the
    /// agent side — the mechanism reads a flag it is given.
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
    /// The LAST state received on the channel, FOLLOWER side. `undefined` as long
    /// as no broadcast has arrived yet — that is what
    /// `fenetresAPeindre` distinguishes from an empty list broadcast for real.
    let dernierEtatRecu: FenetreConnue[] | undefined;
    // ⚠️ DECLARED BEFORE `creerBureau`, whose `envoyer` callback reads it: a
    // closure capturing a `let` declared further down compiles, but reads
    // badly — and the temporal dead zone is an error class avoided
    // by layout rather than by vigilance.
    let socket: WebSocket | undefined;
    /// Returned by `elire`. `undefined` as long as the election has not been set —
    /// the fallback without Web Locks calls `devenirPorteur` SYNCHRONOUSLY, hence
    /// before this assignment.
    let election: Election | undefined;

    const poserTon = (element: HTMLElement, ton: Ton): void => {
        element.classList.remove('message--succes', 'message--alerte', 'message--danger');
        if (ton !== 'neutre') element.classList.add(`message--${ton}`);
    };

    /// 🔴 `/index.html`, NOT `/`: the root serves the HUB since batch 14, and
    /// `/?session=…` would open the hub with a parameter it ignores, never
    /// a session.
    ///
    /// ⚠️ **A SINGLE PLACE BUILDS THIS URL**, used by the carrier (through
    /// `bureau.ouvrirFenetre`) AND by the follower: the final review found it
    /// written twice, in two places that would have had to be kept in agreement.
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
