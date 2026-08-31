// L'annonce du viewport à la page-shell : la mesure, et les deux moments où
// elle part.
//
// **Extrait de `main.ts` VERBATIM (lot 33).** Le fichier était à **500 lignes
// EXACTEMENT** — sa porte — et l'addition du lot (le réannonceur, sans lequel
// le superviseur garde pour toujours la taille du jour de l'ouverture) l'a
// porté à 535. `CLAUDE.md` interdit de comprimer : on extrait.
//
// ⚠️ **Cette extraction SUIT son addition au lieu de la précéder**, et c'est
// dit plutôt que maquillé : le besoin du réannonceur n'est apparu qu'une fois
// le trajet du `Resize` mesuré, à mi-lot. C'est la TROISIÈME extraction de
// `main.ts` (après `resize-dom.ts` et `presse-papier-dom.ts`), et la deuxième
// fois que ce fichier franchit son plafond par une addition d'une vingtaine
// de lignes : il vaut mieux le savoir avant d'y toucher.
//
// Le partage est celui, déjà employé, de `resize.ts` / `resize-dom.ts` :
// `viewport.ts` porte la règle PURE (`viewportPair`, sans DOM, testée), ce
// fichier-ci porte sa seule branche sur le navigateur.

import { viewportPair } from './viewport';
import type { AnnonceViewport } from './resize-dom';

// Annonce du viewport à la page-shell qui nous a ouverts.
//
// C'est cette taille qui décide de la résolution de la sortie virtuelle, donc
// de la résolution native du flux : rien ne peut être créé côté agent avant
// qu'elle soit connue. L'annonce part donc AVANT toute connexion WebRTC.
//
// `window.opener` est nul quand la page est ouverte à la main (essais,
// rechargement direct) : dans ce cas l'agent tourne déjà et il n'y a rien à
// demander — on ne fait rien plutôt que d'échouer.
export function annoncerLeViewportInitial(sessionId: string): void {
    if (!window.opener || window.opener.closed) return;
    // MÊME UNITÉ que le `Resize` émis plus bas (`clientWidth × devicePixelRatio`).
    //
    // ⚠️ **La raison écrite ici à l'origine était déjà périmée quand elle a été
    // écrite, et c'est la revue TRANSVERSE de fin de branche D9 qui l'a
    // rattrapée.** Elle disait : « sans ce facteur, CHAQUE connexion de CHAQUE
    // fenêtre déclencherait un changement de mode, avec 25 à 100 % d'écart
    // (leg 7 du sous-bloc D8) ». Or la tâche 3 du même sous-bloc D9 — un commit
    // AVANT celui qui a écrit cette phrase — avait retiré le changement de mode
    // de sortie sur mesure (voir le constat en tête de
    // `agent/src/capteur/plein_ecran.rs`). Il n'y a donc plus aucun changement
    // de mode à déclencher. ⚠️ **La suite de cette phrase — « `WindowsSource::
    // resize` retourne avant tout en mode `SortieEntiere`, et le court-circuit
    // *taille inchangée* n'est même plus atteint » — est FAUSSE depuis le lot
    // 33** : `resize` fait désormais suivre le recadrage et la fenêtre au
    // viewport, et le court-circuit « taille inchangée » est de nouveau ce qui
    // évite de reconstruire un encodeur toutes les 200 ms pendant qu'on tire
    // un bord. **Ce qui reste vrai, et qui est la raison de garder le
    // facteur** : aucun mode d'AFFICHAGE n'est changé, ni ici ni là-bas.
    //
    // ✅ **Ce que le facteur corrige RÉELLEMENT, et qui justifie de le garder** :
    // l'annonce de viewport DÉCIDE la taille de la sortie virtuelle créée par le
    // superviseur (`superviseur/boucle.rs::creer_sortie`). Sans dpr, un client
    // HiDPI recevait une sortie plus PETITE que sa surface d'affichage réelle,
    // donc une image mise à l'échelle vers le haut par le navigateur. Avec, la
    // sortie naît en pixels périphériques, l'unité dans laquelle le `Resize` de
    // routine parle déjà.
    //
    // ⚠️ **Conséquence non mesurée, et déclarée comme telle (legs de D9)** : à
    // `devicePixelRatio = 2`, une fenêtre de 1280×720 CSS demande désormais une
    // sortie de 2560×1440, soit QUATRE fois les pixels à capturer et à encoder,
    // et **rien ne borne cette demande** — `windows_source/sortie.rs::
    // borner_a_la_taille_max` (1920×1080) a perdu son dernier appelant avec le
    // changement de mode et n'est plus branchée nulle part. D6 a relevé le
    // décodeur du navigateur saturé dès huit fenêtres de 1280×720.
    //
    // Il n'y a qu'un `devicePixelRatio` en jeu : c'est CETTE page qui annonce, et
    // c'est son propre `ResizeObserver` qui émettra le `Resize`.
    //
    // Multiplier PUIS arrondir en pair — `viewportPair` a un plancher à 2, et
    // l'ordre inverse laisserait passer une hauteur impaire à dpr impair.
    const dpr = window.devicePixelRatio;
    const { largeur, hauteur } = viewportPair(
        Math.round(window.innerWidth * dpr),
        Math.round(window.innerHeight * dpr),
    );
    window.opener.postMessage(
        { type: 'viewport', session: sessionId, largeur, hauteur },
        window.location.origin,
    );
}

// L'annonceur des redimensionnements SUIVANTS, passé à `attacherResizeAuDOM`.
//
// ⚠️ **Il ne remultiplie PAS par `devicePixelRatio`** : `resize-dom.ts` mesure
// déjà en pixels périphériques (`video.clientWidth * window.devicePixelRatio`),
// et l'appliquer deux fois demanderait quatre fois les pixels à un client
// HiDPI. C'est la même unité que l'annonce initiale ci-dessus, qui multiplie
// parce qu'elle part d'`innerWidth`, lui en pixels CSS.
//
// `undefined` — donc aucune annonce — quand la page n'a pas d'ouvreur : il n'y
// a alors aucune page-shell à qui parler, et `postMessage` sur `null`
// lèverait.
export function annonceurDeViewport(sessionId: string): AnnonceViewport | undefined {
    return window.opener && !window.opener.closed
        ? (largeur: number, hauteur: number) => {
              window.opener.postMessage(
                  { type: 'viewport', session: sessionId, largeur, hauteur },
                  window.location.origin,
              );
          }
        : undefined;
}
