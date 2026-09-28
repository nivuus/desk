// THE MARKUP OF AN APPLICATION CARD — cloned from the hub's `<template>`,
// never built element by element.
//
// 🔴 EXTRACTED FROM `hub/page.ts::entree` ON AUGUST 31ST, 2026, IN A DEDICATED TASK
// AND BEFORE THE ADDITION IT PREPARES (the hub absorbs the two sections of the
// desktop). It is the strong form `CLAUDE.md` requires: extract, never
// compress, and never in the commit that adds.
//
// ⚠️ THIS FILE IS NOT UNIT TESTED, AND IT IS DECLARED RATHER THAN
// ENDURED — the convention of `hub/page.ts`, `bureau/porteur-dom.ts` and
// `main.ts` (`shell-page.ts` until the final review of August 31st, 2026: it
// is now only a sixteen-line redirect). What
// makes it tenable is the clause accompanying it: **a condition is a
// RULE if changing it changes what the product DECIDES; it is WIRING if
// it only routes a decision already taken elsewhere.** Cloning a
// template and setting a name in it decides nothing. And `client/` has **neither jsdom nor
// happy-dom**: the modules wanting to be tested inject their
// dependencies (`accent-dom.ts`, `presse-papier-dom.ts`) — which makes no
// sense here, where all the work IS template manipulation.
//
// ⚠️ AN EXTRACTION IS NEVER STRICTLY VERBATIM: it leaves its
// imports behind (a `TS6133` is a `tsc` FAILURE, not a
// warning) and breaks deictic references. Reread `hub/page.ts` AFTER this
// move, not only before.

import type { ApplicationListee } from './catalogue';

export interface DepsCarte {
    /// Le `<template id="modele-application">` du hub.
    modele: HTMLTemplateElement;
    /// The absolute URL of the icon, or `null`. 🔴 COMPUTING THE BASE STAYS WITH
    /// THE CALLER: this module has no reason to know the platform's
    /// address, and giving it to it would make a second place to keep
    /// in agreement with `adresse-plateforme.ts`.
    ///
    /// 🔴 ❌ ~~THE ICON CANNOT BE SET THROUGH `src` TOWARDS THE ROUTE: an
    ///    `<img src>` carries no `Authorization`. It is READ by an authenticated
    ///    `fetch`, then published as an object — the only way.~~ **NO LONGER TRUE
    ///    SINCE AUGUST 30TH, 2026**, a decision of the repository owner: the
    ///    icon route is reached through a SIGNED URL, which the catalogue mints
    ///    under a bearer token and returns in `icone_url`. **It is set
    ///    directly in `src`**, and that is exactly what the batch
    ///    delivers. The detour through `fetch` + `createObjectURL` disappears from here —
    ///    it survives in `publierLeManifeste`, which needs the BYTES to
    ///    build the manifest's `data:`, the only form G5 measured
    ///    installable and the only one a manifest reaches without a cookie.
    ///
    /// ⚠️ WHAT THIS LINE DOES NOT ESTABLISH: that a REAL browser displays it.
    ///    This file is not unit tested (see the header), and no
    ///    visual judgement has been passed on the hub to date.
    urlIcone: string | null;
    lancer(): void;
    installer(): void;
}

export function batirCarte(application: ApplicationListee, deps: DepsCarte): DocumentFragment {
    const fragment = deps.modele.content.cloneNode(true) as DocumentFragment;

    const icone = fragment.querySelector<HTMLImageElement>('[data-icone]')!;
    // 🔴 A LITERAL, NOT A TERNARY ON `className`. A computed class is
    // invisible to check §7.9, which only sees the literals passed to
    // `classList.add('…')` and `className = '…'`. It is the form
    // `fenetres-dom` already uses for its badges.
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
