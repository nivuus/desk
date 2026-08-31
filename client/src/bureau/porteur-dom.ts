// LE CÂBLAGE DU BUREAU DANS LE HUB : élection, socket de la session de
// contrôle, canal entre onglets, DOM.
//
// 🔴 CE FICHIER EST LE SUCCESSEUR DE `shell-page.ts`, ET IL NE REPREND PAS SON
// DÉFAUT : ses dépendances sont INJECTÉES, et les trois règles qu'il emploie
// (`porteur.ts`, `fenetres-dom.ts`, `shell.ts`) sont testées ailleurs.
//
// ⚠️ AUCUNE RÈGLE ICI. Une condition qui déciderait quelque chose du produit
// doit descendre dans `porteur.ts` ou `shell.ts`.

// ⚠️ AUCUN import d'`adresseSignaling` ICI : l'URL arrive par `deps`, calculée
// par `hub/page.ts`. L'importer sans l'employer serait un `TS6133`, c'est-à-dire
// un ÉCHEC de `tsc --noEmit`, pas un avertissement.
import { composer } from '../prefixe';
import { creerBureau, type FenetreConnue, type Ton } from '../shell';
import { dessinerFenetres } from './fenetres-dom';
import { installerLePont } from './fichiers-dom';
import { NOM_VERROU, batirEtat, elire, estPlacePrise, lireEtat } from './porteur';

declare global {
    interface Navigator {
        locks?: { request(nom: string, options: { mode: 'exclusive' }, pendant: () => Promise<never>): Promise<void> };
    }
}

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
    jeton: string;
    /// Le préfixe de VM (`prefixe.ts::lirePrefixe`), ou `''` s'il n'y en a pas.
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
    // ⚠️ DÉCLARÉ AVANT `creerBureau`, dont le rappel `envoyer` le lit : une
    // fermeture qui capture un `let` déclaré plus bas compile, mais se lit
    // mal — et la zone morte temporelle est une classe d'erreur qu'on évite
    // par la disposition plutôt que par la vigilance.
    let socket: WebSocket | undefined;

    const poserTon = (element: HTMLElement, ton: Ton): void => {
        element.classList.remove('message--succes', 'message--alerte', 'message--danger');
        if (ton !== 'neutre') element.classList.add(`message--${ton}`);
    };

    const bureau = creerBureau({
        ouvrirFenetre(session) {
            // 🔴 `/index.html`, PAS `/` : la racine sert le HUB depuis le lot
            // 14, et `/?session=…` ouvrirait le hub avec un paramètre qu'il
            // ignore, jamais une session.
            return window.open(`/index.html?session=${encodeURIComponent(session)}`, `guac-${session}`);
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

    const redessiner = (): void => {
        const fenetres = bureau.liste();
        dessinerFenetres(fenetres, {
            liste: el.liste,
            modele: el.modele,
            section: el.sectionFenetres,
            rouvrir: (session) => { bureau.rouvrir(session); redessiner(); },
        });
        if (porteur) empreinte = diffuserSiChange(canal, fenetres, empreinte);
    };

    // ── LE SUIVEUR : il n'ouvre AUCUN socket, et peint ce qu'on lui dit ────
    canal.addEventListener('message', (evenement) => {
        if (porteur) return;
        const fenetres = lireEtat(evenement.data);
        if (fenetres === undefined) return;
        dessinerFenetres(fenetres, {
            liste: el.liste,
            modele: el.modele,
            section: el.sectionFenetres,
            // ⚠️ UN SUIVEUR OUVRE SA PROPRE FENÊTRE, DEPUIS SON PROPRE CLIC :
            // `window.open` exige l'activation de CET onglet-ci. Il ne
            // prévient pas le porteur, dont la liste continuera d'afficher
            // « fermée » jusqu'à sa prochaine relecture. **Limite déclarée** :
            // le remède serait un ordre sur le canal, que la conception exclut
            // (spec §4), ou une méthode neuve sur `shell.ts`, que la spec
            // laisse INCHANGÉ (spec §6). Recliquer « Rouvrir » ramène la même
            // fenêtre au premier plan : aucun dommage.
            rouvrir: (session) => {
                window.open(`/index.html?session=${encodeURIComponent(session)}`, `guac-${session}`);
            },
        });
    });

    const ouvrirLaSession = (): void => {
        porteur = true;
        socket = new WebSocket(deps.signalingUrl);
        socket.addEventListener('open', () => {
            socket!.send(JSON.stringify({ role: 'client', session: sessionDeControle, jeton: deps.jeton }));
            el.statut.textContent = 'bureau connecté';
            poserTon(el.statut, 'neutre');
        });
        socket.addEventListener('message', (evenement) => {
            const message = JSON.parse(evenement.data);
            if (message.type === 'fenetre-ouverte') bureau.fenetreOuverte(message.session, message.titre);
            else if (message.type === 'fenetre-fermee') bureau.fenetreFermee(message.session);
            else if (message.type === 'refus') bureau.refus(message.titre, message.motif);
            else if (message.type === 'error') {
                // 🔴 « LA PLACE EST PRISE » N'EST PAS UNE ERREUR À MONTRER.
                // Ce chemin n'est atteint que dans le REPLI (navigateur sans
                // Web Locks) : cet onglet a tenté, un autre tenait déjà. Il
                // redevient suiveur, EN SILENCE — un second onglet n'est pas
                // une faute de l'utilisateur. Tout autre refus, dont le frein
                // de volume, reste affiché.
                if (estPlacePrise(message)) { porteur = false; socket?.close(); return; }
                bureau.canalDeControleRefuse(message.reason, message.motif, message.retryApresS);
            }
            redessiner();
        });
        socket.addEventListener('close', () => {
            // ⚠️ SILENCIEUX SI NOUS AVONS CÉDÉ LA PLACE : `canalDeControlePerdu`
            // dirait « Rechargez la page », ce qui serait faux ici.
            if (porteur) bureau.canalDeControlePerdu();
        });
    };

    elire(nomDuVerrou(deps.prefixe), {
        verrou:
            typeof navigator !== 'undefined' && 'locks' in navigator
                ? (nom, pendant) => void navigator.locks!.request(nom, { mode: 'exclusive' }, pendant)
                : undefined,
        devenirPorteur: ouvrirLaSession,
        devenirSuiveur: () => { porteur = false; },
    });

    installerLePont({
        bureau,
        signalingUrl: deps.signalingUrl,
        fautesArmees: deps.fautesArmees,
        boutonDossier: el.boutonDossier,
        boutonRafraichir: el.boutonRafraichir,
        boutonReprendre: el.boutonReprendre,
        section: el.sectionFichiers,
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

    // La fermeture d'une page par l'utilisateur ne prévient personne : on
    // relit l'état périodiquement plutôt que d'attendre un événement qui
    // n'existe pas.
    setInterval(redessiner, 1000);
}
