// Le microphone du navigateur vers l'agent (chantier E) : la bascule, et ses
// quatre états.
//
// **La permission se demande AU CLIC, jamais à l'ouverture de session**
// (spec §9). Le transceiver montant, lui, est déclaré dès l'offre initiale et
// SANS PISTE (`webrtc.ts`) : c'est ce qui permet d'allumer le micro par un
// simple `replaceTrack`, sans seconde offre, alors que `connectSession` n'a
// aucun chemin pour renégocier.
//
// **L'extinction est RÉELLE.** `replaceTrack(null)` ET `track.stop()`.
// `enabled = false` seul laisserait le périphérique ouvert et l'indicateur de
// Chrome allumé : la spec §9 qualifie ce mensonge visuel d'inacceptable « sur
// cette fonction précisément », et c'est la seule fonction du produit qui
// capte l'utilisateur chez lui.
//
// Les dépendances sont INJECTÉES plutôt que lues dans les objets globaux
// (`navigator.mediaDevices`), comme `audio.ts`, `resize.ts` et
// `visibilite.ts` : c'est ce qui rend le module testable sans DOM.

/// Les quatre états, et ce que chacun demande au bouton (spec §10).
///
/// La spec §9 en annonce TROIS — « fermé, actif, refusé par le navigateur ».
/// Le quatrième vient de son propre tableau §10, qui distingue « permission
/// refusée par l'utilisateur » (message pour la rétablir) de « aucun
/// périphérique d'entrée côté navigateur » (bouton désactivé, avec
/// l'explication). Les confondre enverrait régler une permission qui n'est pas
/// en cause.
export type EtatMicro = 'ferme' | 'actif' | 'refuse' | 'indisponible';

/// Les contraintes de capture (spec §7).
///
/// **L'annulation d'écho est celle du NAVIGATEUR**, décision de la spec §4 :
/// c'est le seul endroit qui connaisse à la fois le flux capté et le flux
/// restitué. ⚠️ Elle est structurellement INCOMPLÈTE en multi-fenêtres
/// (décision 7 du plan de ce chantier) : chaque page n'annule que ce qu'ELLE
/// restitue, et n'a aucune connaissance du son des fenêtres voisines, qui
/// sortent pourtant du même haut-parleur. Ce n'est pas réparable ici.
///
/// ⚠️ **PRIVÉE, et son test RECOPIE le littéral au lieu de l'importer.** Une
/// première rédaction l'exportait et le test assertait
/// `toHaveBeenCalledWith(CONTRAINTES)` : les deux côtés de l'égalité lisaient
/// alors le MÊME objet, et remplacer les trois contraintes par `audio: true`
/// laissait le test VERT — mutation jouée, constatée verte, corrigée. C'est le
/// « contrôle incapable d'échouer » que ce dépôt paie depuis D6 : une valeur de
/// référence ne se partage pas avec ce qui la vérifie.
const CONTRAINTES: MediaStreamConstraints = {
    audio: { echoCancellation: true, noiseSuppression: true, autoGainControl: true },
};

/// Ce que le message d'état « refusé » doit porter : COMMENT rétablir la
/// permission (spec §10), pas seulement le fait du refus. Sans cela,
/// l'utilisateur qui a cliqué « bloquer » une fois n'a plus aucun moyen visible
/// de revenir en arrière — Chrome ne repropose jamais la boîte de dialogue.
const DETAIL_REFUS =
    "micro refusé — autorisez-le dans les réglages du site (icône à gauche de la barre d'adresse), puis recliquez";

/// Idem pour l'absence de périphérique.
const DETAIL_SANS_PERIPHERIQUE = "aucun microphone détecté sur cet ordinateur";

/// Les `name` d'erreur que `getUserMedia` emploie pour un refus de PERMISSION,
/// et eux seuls. Tout le reste — `NotFoundError`, `NotReadableError`,
/// `OverconstrainedError`, `AbortError`, une panne quelconque — décrit un
/// périphérique absent ou inutilisable, pas un choix de l'utilisateur.
///
/// ⚠️ Le défaut par défaut est « indisponible », pas « refusé », et c'est
/// délibéré : afficher « autorisez le micro » à quelqu'un dont le micro est
/// débranché l'envoie chercher un réglage qui ne changera rien.
const REFUS_DE_PERMISSION = new Set(['NotAllowedError', 'SecurityError', 'PermissionDeniedError']);

export interface OptionsMicro {
    /// L'émetteur de la piste montante, rendu par `connectSession`.
    sender: Pick<RTCRtpSender, 'replaceTrack'>;
    /// `navigator.mediaDevices.getUserMedia` en production.
    demanderFlux: (contraintes: MediaStreamConstraints) => Promise<MediaStream>;
    /// Appelé à CHAQUE changement d'état. `detail` porte le message destiné à
    /// l'utilisateur pour les deux états d'échec, et rien pour les deux autres.
    surEtat?: (etat: EtatMicro, detail?: string) => void;
}

export interface Micro {
    /// Allume si éteint, éteint si allumé. **Ne rejette JAMAIS** : le micro ne
    /// tue jamais une session qui fonctionne (spec §10). Rend l'état atteint.
    basculer(): Promise<EtatMicro>;
    etat(): EtatMicro;
    /// Fin de session : éteint réellement, et rend la bascule inerte.
    detacher(): void;
}

export function attacherMicro(options: OptionsMicro): Micro {
    let etat: EtatMicro = 'ferme';
    let piste: MediaStreamTrack | undefined;
    /// Une demande de flux est en vol : la boîte de dialogue de permission est
    /// ouverte, et l'utilisateur peut y rester longtemps.
    let enVol = false;
    let detache = false;

    const annoncer = (nouvel: EtatMicro, detail?: string): EtatMicro => {
        etat = nouvel;
        options.surEtat?.(nouvel, detail);
        return etat;
    };

    /// L'extinction, et la seule. **`stop()` AVANT `replaceTrack(null)`** : le
    /// `stop()` est ce qui rend le périphérique et éteint l'indicateur, et il
    /// doit survivre à un `replaceTrack` qui rejetterait — ce qui arrive sur
    /// une `RTCPeerConnection` déjà fermée (`InvalidStateError`).
    const eteindre = (): void => {
        piste?.stop();
        piste = undefined;
    };

    const classer = (erreur: unknown): EtatMicro => {
        const nom = erreur instanceof Error ? erreur.name : '';
        if (REFUS_DE_PERMISSION.has(nom)) return annoncer('refuse', DETAIL_REFUS);
        // Le message du navigateur est joint quand il n'est pas un simple
        // `NotFoundError` : sur une panne, c'est la seule information qu'on ait.
        const detail =
            nom === 'NotFoundError' || nom === 'DevicesNotFoundError'
                ? DETAIL_SANS_PERIPHERIQUE
                : `micro indisponible : ${erreur instanceof Error ? erreur.message : String(erreur)}`;
        return annoncer('indisponible', detail);
    };

    const allumer = async (): Promise<EtatMicro> => {
        enVol = true;
        try {
            const flux = await options.demanderFlux(CONTRAINTES);
            const obtenue = flux.getAudioTracks()[0];

            // ⚠️ La session a pu se terminer PENDANT que la boîte de dialogue
            // était ouverte, et l'utilisateur autoriser après. Sans cette
            // garde, la piste arrive dans le vide : plus personne ne la
            // détient, `detacher()` est déjà passé, et le micro reste ouvert
            // jusqu'à la fermeture de l'onglet — exactement la fuite que
            // l'extinction réelle existe pour empêcher.
            if (detache) {
                obtenue?.stop();
                return etat;
            }

            // `getUserMedia({audio})` rend toujours une piste en pratique ; s'il
            // n'en rend aucune, c'est « indisponible » et non « actif » — sans
            // quoi le bouton s'allumerait sur un flux vide, et la spec §10 dit
            // qu'un échec silencieux est le pire cas.
            if (!obtenue) return annoncer('indisponible', DETAIL_SANS_PERIPHERIQUE);

            piste = obtenue;
            await options.sender.replaceTrack(obtenue);
            return annoncer('actif');
        } catch (erreur) {
            // La piste a pu être obtenue avant que `replaceTrack` ne rejette :
            // l'arrêter est le seul moyen de ne pas laisser le micro ouvert
            // sur un chemin d'erreur.
            eteindre();
            return classer(erreur);
        } finally {
            enVol = false;
        }
    };

    return {
        etat: () => etat,

        async basculer(): Promise<EtatMicro> {
            if (detache) return etat;
            // Deux clics rapides : le second tombe pendant que la boîte de
            // dialogue du premier est encore ouverte. Sans cette garde, il
            // demanderait un SECOND flux, donc une seconde piste — et la
            // première fuirait, jamais arrêtée.
            if (enVol) return etat;

            if (piste) {
                eteindre();
                // `replaceTrack(null)` après le `stop()`, et toléré s'il
                // rejette : sur une connexion déjà fermée il lève
                // `InvalidStateError`, ce qui ne doit pas empêcher l'état de
                // revenir à « fermé » ni faire rejeter `basculer`.
                await options.sender.replaceTrack(null).catch(() => {});
                return annoncer('ferme');
            }

            // Un refus ou une indisponibilité ne sont PAS terminaux : l'état
            // « refusé » dit à l'utilisateur d'aller rétablir la permission,
            // ce conseil serait inapplicable si le clic suivant ne retentait
            // pas. Un micro rebranché est le cas symétrique.
            return allumer();
        },

        detacher(): void {
            detache = true;
            eteindre();
            annoncer('ferme');
        },
    };
}

// ── Le bouton ───────────────────────────────────────────────────────────────
//
// Séparé de `attacherMicro` pour la même raison que `attachFullscreen` l'est de
// `armerPleinEcran` : la bascule est une machine à états qui ne connaît aucun
// DOM, le bouton est ce qui la montre. Le second dépend du premier, jamais
// l'inverse.

/// Ce dont ce module a besoin du bouton, et rien de plus.
export interface BoutonMicro {
    hidden: boolean;
    disabled: boolean;
    title: string;
    dataset: { etat?: string };
    addEventListener(type: string, ecouteur: EventListener): void;
    removeEventListener(type: string, ecouteur: EventListener): void;
}

export interface OptionsBoutonMicro extends OptionsMicro {
    bouton: BoutonMicro;
    /// Le message destiné à l'utilisateur, sur les deux états d'échec
    /// seulement. `main.ts` le pousse dans le bandeau de statut.
    surMessage?: (texte: string) => void;
}

export interface ControleBoutonMicro {
    /// À appeler sur le message `ready` de l'agent, avec `message.mic` TEL
    /// QUEL — `undefined` compris.
    annoncerDisponibilite(mic: boolean | undefined): void;
    /// La bascule en cours, pour que le test puisse l'attendre. En production
    /// personne ne l'attend : un clic est asynchrone par nature.
    enCours(): Promise<unknown>;
    detacher(): void;
}

/// Ce que le bouton affiche en survol, par état.
const TITRES: Record<EtatMicro, string> = {
    ferme: 'Microphone — cliquez pour parler',
    actif: 'Microphone actif — cliquez pour couper',
    refuse: 'Microphone refusé — voir le message',
    indisponible: 'Aucun microphone disponible',
};

export function attacherBoutonMicro(options: OptionsBoutonMicro): ControleBoutonMicro {
    const { bouton, surMessage } = options;
    let enVol: Promise<unknown> = Promise.resolve();

    const micro = attacherMicro({
        ...options,
        surEtat(etat, detail) {
            bouton.dataset.etat = etat;
            bouton.title = TITRES[etat];
            // ⚠️ SEUL « indisponible » désactive. Un refus de permission laisse
            // le bouton cliquable, parce que son message dit d'aller rétablir
            // la permission puis de recliquer (spec §10) : le désactiver
            // rendrait ce conseil inapplicable.
            bouton.disabled = etat === 'indisponible';
            if (detail) surMessage?.(detail);
            options.surEtat?.(etat, detail);
        },
    });

    // L'état initial est écrit tout de suite : sans lui, le CSS n'aurait aucun
    // sélecteur à accrocher avant le premier clic.
    bouton.dataset.etat = micro.etat();
    bouton.title = TITRES[micro.etat()];

    const onClick = (): void => {
        // `basculer` ne rejette jamais (spec §10) ; le `catch` est une ceinture
        // pour que rien ne remonte en « unhandled rejection » si cet invariant
        // venait à être rompu par une évolution future.
        enVol = micro.basculer().catch((erreur: unknown) => {
            console.warn('bascule micro en échec', erreur);
        });
    };
    bouton.addEventListener('click', onClick);

    return {
        annoncerDisponibilite(mic) {
            // ⚠️ `mic` est lu TEL QUEL, et son absence vaut `false` (spec §10) :
            // un agent d'avant le chantier E ne porte pas le champ, et un
            // bouton qui ne mènerait nulle part est pire que pas de bouton.
            // `!!` et non `!== false` : c'est la falsité qui décide.
            bouton.hidden = !mic;
        },
        enCours: () => enVol,
        detacher() {
            bouton.removeEventListener('click', onClick);
            micro.detacher();
            bouton.hidden = true;
        },
    };
}
