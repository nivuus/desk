// LE BALISAGE D'UNE CARTE D'APPLICATION — cloné depuis le `<template>` du
// hub, jamais construit élément par élément.
//
// 🔴 EXTRAIT DE `hub/page.ts::entree` LE 31 AOÛT 2026, DANS UNE TÂCHE DÉDIÉE
// ET AVANT L'ADDITION QU'ELLE PRÉPARE (le hub absorbe les deux sections du
// bureau). C'est la forme forte que `CLAUDE.md` exige : extraire, jamais
// comprimer, et jamais dans le commit qui ajoute.
//
// ⚠️ CE FICHIER N'EST PAS TESTÉ UNITAIREMENT, ET C'EST DÉCLARÉ PLUTÔT QUE
// SUBI — la convention de `hub/page.ts`, `bureau/porteur-dom.ts` et
// `main.ts` (`shell-page.ts` jusqu'à la revue finale du 31 août 2026 : il
// n'est plus qu'une redirection de seize lignes). Ce qui
// la rend tenable est la clause qui l'accompagne : **une condition est une
// RÈGLE si la changer change ce que le produit DÉCIDE ; elle est du CÂBLAGE si
// elle ne fait que router une décision déjà prise ailleurs.** Cloner un
// gabarit et y poser un nom ne décide rien. Et `client/` n'a **ni jsdom ni
// happy-dom** : les modules qui veulent être éprouvés injectent leurs
// dépendances (`accent-dom.ts`, `presse-papier-dom.ts`) — ce qui n'a pas de
// sens ici, où tout le travail EST la manipulation du gabarit.
//
// ⚠️ UNE EXTRACTION N'EST JAMAIS RIGOUREUSEMENT VERBATIM : elle laisse ses
// imports derrière elle (un `TS6133` est un ÉCHEC de `tsc`, pas un
// avertissement) et casse les déictiques. Relire `hub/page.ts` APRÈS ce
// déplacement, pas seulement avant.

import type { ApplicationListee } from './catalogue';

export interface DepsCarte {
    /// Le `<template id="modele-application">` du hub.
    modele: HTMLTemplateElement;
    /// L'URL absolue de l'icône, ou `null`. 🔴 LE CALCUL DE LA BASE RESTE CHEZ
    /// L'APPELANT : ce module n'a aucune raison de connaître l'adresse de la
    /// plateforme, et la lui donner en ferait un second endroit à tenir
    /// d'accord avec `adresse-plateforme.ts`.
    ///
    /// 🔴 ❌ ~~L'ICÔNE NE PEUT PAS ÊTRE POSÉE PAR `src` VERS LA ROUTE : un
    ///    `<img src>` ne porte pas d'`Authorization`. Elle est LUE par `fetch`
    ///    authentifié, puis publiée en objet — la seule voie.~~ **PLUS VRAI
    ///    DEPUIS LE 30 AOÛT 2026**, décision du propriétaire du dépôt : la
    ///    route d'icône s'atteint par une URL SIGNÉE, que le catalogue frappe
    ///    sous jeton porteur et rend dans `icone_url`. **Elle se pose
    ///    directement dans `src`**, et c'est très exactement ce que le lot
    ///    livre. Le détour par `fetch` + `createObjectURL` disparaît d'ici —
    ///    il survit dans `publierLeManifeste`, qui a besoin des OCTETS pour
    ///    bâtir le `data:` du manifeste, la seule forme que G5 ait mesurée
    ///    installable et la seule qu'un manifeste atteigne sans cookie.
    ///
    /// ⚠️ CE QUE CETTE LIGNE N'ÉTABLIT PAS : qu'un navigateur RÉEL l'affiche.
    ///    Ce fichier n'est pas testé unitairement (voir l'en-tête), et aucun
    ///    jugement visuel n'a été porté sur le hub à ce jour.
    urlIcone: string | null;
    lancer(): void;
    installer(): void;
}

export function batirCarte(application: ApplicationListee, deps: DepsCarte): DocumentFragment {
    const fragment = deps.modele.content.cloneNode(true) as DocumentFragment;

    const icone = fragment.querySelector<HTMLImageElement>('[data-icone]')!;
    // 🔴 UN LITTÉRAL, PAS UN TERNAIRE SUR `className`. Une classe calculée est
    // invisible au contrôle §7.9, qui ne voit que les littéraux passés à
    // `classList.add('…')` et `className = '…'`. C'est la forme que
    // `fenetres-dom` emploie déjà pour ses pastilles.
    if (application.icone === null) icone.classList.add('hub__icone--absente');
    if (deps.urlIcone !== null) icone.src = deps.urlIcone;

    fragment.querySelector('[data-nom]')!.textContent = application.nom;
    fragment
        .querySelector<HTMLButtonElement>('[data-lancer]')!
        .addEventListener('click', () => deps.lancer());
    fragment
        .querySelector<HTMLButtonElement>('[data-installer]')!
        .addEventListener('click', () => deps.installer());
    return fragment;
}
