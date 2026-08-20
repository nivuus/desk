// Le branchement du redimensionnement sur le DOM : `ResizeObserver`, lissage
// de 200 ms, émission sur le canal de contrôle, et l'instrumentation du legs
// n°7 de D9.
//
// **Extrait de `main.ts` VERBATIM le 20 août 2026**, tâche 15 du sous-bloc P1
// du presse-papier. Le plan posait une PORTE DE MESURE sur `main.ts` — « si
// l'après dépasse 480 lignes, extraire » — en la calculant sur un fichier à
// 451 lignes. Il en faisait 460 au moment de l'addition, et le câblage du
// presse-papier, pourtant déjà réduit à sa portion congrue par l'extraction de
// `presse-papier-dom.ts` que le plan nommait, laissait `main.ts` à 487 : la
// porte était franchie. Ce fichier-ci est la seconde extraction, et elle suit
// la doctrine du dépôt — **extraire, jamais comprimer**.
//
// Le partage est celui, déjà employé, de `presse-papier.ts` /
// `presse-papier-dom.ts` : `resize.ts` porte la règle PURE (`RejeuResize`,
// sans DOM, testée), ce fichier-ci porte sa seule branche sur le navigateur.
//
// 🔴 **L'INVARIANT DE SYNCHRONICITÉ TRAVERSE CETTE EXTRACTION, ET IL FALLAIT LE
// VÉRIFIER** (leg n°12 de D9, texte intégral plus bas) : le `.then()` de
// `main.ts` doit s'exécuter sans `await` intercalé jusqu'à l'`addEventListener`
// final. La fonction ci-dessous est appelée SYNCHRONEMENT depuis ce `.then()`
// et n'attend rien elle-même, donc l'invariant est préservé — le déplacer dans
// une fonction `async` ou l'appeler derrière un `await` le romprait
// exactement comme un `await` intercalé.

import { RejeuResize } from './resize';
import { encodeResize } from '../../proto/ts/control';

/// Ce dont ce module a besoin de la session : le canal de contrôle, et rien
/// d'autre. L'objet rendu par `connectSession` s'y conforme.
export interface SessionResize {
    controlChannel: RTCDataChannel;
}

/// Branche le suivi de taille. **À appeler SYNCHRONEMENT** — voir l'en-tête.
export function attacherResizeAuDOM(video: HTMLVideoElement, session: SessionResize): void {
    // Le redimensionnement reconstruit la chaîne d'encodage côté agent :
    // on n'émet donc qu'une fois le geste terminé, pas à chaque pixel
    // parcouru pendant que l'utilisateur tire un bord.
    const rejeu = new RejeuResize();
    const emettreSiPossible = () => {
        const taille = rejeu.aEmettre();
        if (!taille) return;
        if (session.controlChannel.readyState !== 'open') {
            // Tracé, et non plus muet : c'est ce `return` silencieux qui perdait
            // les `Resize` sans laisser la moindre trace (leg 10).
            console.warn('Resize différé : canal de contrôle non ouvert');
            return;
        }
        // Instrumentation du legs n°7 de D9 (tâche 18, D10) — confirmation au
        // point d'émission. Grille de lecture complète sur le log
        // « declenchement ResizeObserver » ci-dessous ; celui-ci ne fait que
        // confirmer, pour CETTE tentative de `Resize`, laquelle des trois
        // issues s'est produite.
        console.debug('[instrumentation resize] emission', {
            taille,
            clientWidth: video.clientWidth,
            clientHeight: video.clientHeight,
            innerWidth: window.innerWidth,
            innerHeight: window.innerHeight,
        });
        session.controlChannel.send(encodeResize(taille.largeur, taille.hauteur));
        rejeu.confirmer(taille);
    };

    let resizeTimer: number | undefined;
    const observer = new ResizeObserver(() => {
        // Instrumentation du legs n°7 de D9 (tâche 18, D10) : le sous-bloc D8
        // avait désigné ce maillon — entre `window.innerWidth` (ce que la page
        // annonce à l'ouverture) et l'émission réelle du `Resize`, « leg 10 »
        // dans la numérotation de D8 — sans jamais l'avoir mesuré. Trois
        // issues sont lisibles depuis ce log et celui d'émission ci-dessus,
        // dans CET ORDRE de lecture — aucune ne conclut au-delà de ce
        // qu'elle établit, et le canal de contrôle reste une hypothèse à
        // part entière (voir le `console.warn` ci-dessus) :
        //   1. AUCUN log « declenchement » pour une session qui n'émet
        //      jamais de `Resize` (cas D9 : w-2, w-5) ⟹ l'observateur ne
        //      s'arme jamais ou n'est jamais rappelé — le maillon est EN
        //      AMONT de la mise en page, dans le câblage de
        //      `observer.observe(video)` ou la construction de la session.
        //   2. Log présent, `clientWidth`/`clientHeight` SUIT
        //      `innerWidth`/`innerHeight` ⟹ ni l'observateur ni la mise en
        //      page ne sont en cause ; le maillon est ailleurs.
        //   3. Log présent, `clientWidth`/`clientHeight` NE SUIT PAS
        //      `innerWidth`/`innerHeight` ⟹ la mise en page CSS de
        //      l'élément `<video>` est en cause.
        // Ce log-ci, pris avant le lissage de 200 ms, tranche le cas 1 ;
        // le log d'émission ci-dessus confirme 2 ou 3 pour la tentative
        // qui aboutit réellement.
        console.debug('[instrumentation resize] declenchement ResizeObserver', {
            clientWidth: video.clientWidth,
            clientHeight: video.clientHeight,
            innerWidth: window.innerWidth,
            innerHeight: window.innerHeight,
        });
        window.clearTimeout(resizeTimer);
        resizeTimer = window.setTimeout(() => {
            rejeu.observer({
                largeur: Math.round(video.clientWidth * window.devicePixelRatio),
                hauteur: Math.round(video.clientHeight * window.devicePixelRatio),
            });
            emettreSiPossible();
        }, 200);
    });
    observer.observe(video);
    // Le rejeu : à l'ouverture du canal, la taille retenue repart.
    //
    // ⚠️ INVARIANT NON ÉVIDENT (leg n°12 de D9) : ce `.then()` doit
    // s'exécuter INTÉGRALEMENT DE FAÇON SYNCHRONE, sans `await` intercalé
    // entre la construction de `rejeu` / du `ResizeObserver` ci-dessus et
    // cet `addEventListener`. Un `await` glissé là rendrait la main à la
    // boucle d'événements : si le canal s'ouvrait pendant l'attente,
    // l'écouteur serait posé APRÈS l'événement `open`, il ne serait jamais
    // appelé, et le rejeu serait rompu EN SILENCE — aucune erreur, aucun
    // log, juste une taille perdue. C'est exactement le mode de
    // défaillance que le rejeu existe pour réparer.
    //
    // Aucun test ne garde cet invariant, et c'est une décision, pas un
    // oubli : le voir rouge exigerait de simuler `RTCDataChannel` et tout
    // le cycle de `createSession`, c'est-à-dire de mocker la session
    // entière. Un test qu'on ne peut pas voir rouge à coût raisonnable
    // n'ajouterait rien à ce que ce commentaire dit déjà.
    session.controlChannel.addEventListener('open', emettreSiPossible);
}
