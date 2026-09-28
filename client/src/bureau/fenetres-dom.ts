// PLACING THE "MY WINDOWS" LIST IN THE TEMPLATE — wiring, and nothing
// else. The decisions (which word, which class, a button or not, the
// section visible or not) live in `fenetres.ts`, which is pure and tested.
//
// 🔴 EXTRACTED FROM `shell-page.ts` ON AUGUST 31st, 2026. It is not tidying up:
// `shell-page.ts` itself declares, in its header, that its wiring is
// testable by NOTHING. This extraction does not make THIS file testable
// either — it moves out of it everything that could be.
//
// ⚠️ THE MARKUP COMES FROM A `<template>` OF THE PAGE, NOT FROM HERE: the classes
// stay in the HTML, where check §7.9 reads them without having to parse
// TypeScript.

import type { FenetreConnue } from '../shell';
import { lignes, sectionVisible } from './fenetres';

export interface DepsFenetres {
    liste: HTMLUListElement;
    modele: HTMLTemplateElement;
    /// The section to reveal or to hide again.
    ///
    /// ⚠️ **MANDATORY SINCE THE FINAL REVIEW (Minor ①).** Its doc said
    /// "`undefined` when the page shows the list without a fold — that is the case of
    /// `shell.html`": **wrong since task 9**, where `shell.html` became
    /// a redirect without any UI. The only caller
    /// (`bureau/porteur-dom.ts`) ALWAYS passed a section, so that
    /// the optional was nothing more than dead code justified by a
    /// false sentence.
    section: HTMLElement;
    rouvrir(session: string): void;
}

export function dessinerFenetres(fenetres: FenetreConnue[], deps: DepsFenetres): void {
    deps.liste.replaceChildren();
    for (const ligne of lignes(fenetres)) {
        const item = deps.modele.content.cloneNode(true) as DocumentFragment;
        item.querySelector('[data-titre]')!.textContent = ligne.titre;

        const pastille = item.querySelector<HTMLElement>('[data-etat]')!;
        pastille.textContent = ligne.etat;
        // 🔴 TWO LITERALS, AND NOT A COMPUTED CLASS. `design/classes.ts::
        // classesEmployeesTs` only recognises `classList.add('…')` and
        // `className = '…'` with a literal IN the call: passing a name through a
        // variable would make these two classes invisible to check §7.9, hence
        // orphaned without anything saying so. It is the form
        // `shell-page.ts` had, and it is kept on purpose.
        if (ligne.ouverte) pastille.classList.add('bureau__pastille--ouverte');
        else pastille.classList.add('bureau__pastille--fermee');

        const bouton = item.querySelector<HTMLButtonElement>('[data-rouvrir]')!;
        if (ligne.rouvrable) bouton.addEventListener('click', () => deps.rouvrir(ligne.session));
        else bouton.remove();

        deps.liste.append(item);
    }
    // ⚠️ REVEAL **AND** HIDE AGAIN. Doing only the first would leave an
    // empty section on a hub that has nothing left to show.
    deps.section.hidden = !sectionVisible(fenetres);
}
