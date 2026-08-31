// LA POSE DE LA LISTE « MES FENÊTRES » DANS LE GABARIT — du câblage, et rien
// d'autre. Les décisions (quel mot, quelle classe, un bouton ou pas, la
// section visible ou non) vivent dans `fenetres.ts`, qui est pur et testé.
//
// 🔴 EXTRAIT DE `shell-page.ts` LE 31 AOÛT 2026. Ce n'est pas un rangement :
// `shell-page.ts` déclare lui-même, dans son en-tête, que son câblage n'est
// éprouvable par RIEN. Cette extraction ne rend pas CE FICHIER-ci éprouvable
// non plus — elle sort de lui tout ce qui pouvait l'être.
//
// ⚠️ LE BALISAGE VIENT D'UN `<template>` DE LA PAGE, PAS D'ICI : les classes
// restent dans le HTML, où le contrôle §7.9 les lit sans avoir à analyser du
// TypeScript.

import type { FenetreConnue } from '../shell';
import { lignes, sectionVisible } from './fenetres';

export interface DepsFenetres {
    liste: HTMLUListElement;
    modele: HTMLTemplateElement;
    /// La section à révéler. `undefined` quand la page affiche la liste sans
    /// pli — c'est le cas de `shell.html`, qui n'a pas de section masquable.
    section?: HTMLElement;
    rouvrir(session: string): void;
}

export function dessinerFenetres(fenetres: FenetreConnue[], deps: DepsFenetres): void {
    deps.liste.replaceChildren();
    for (const ligne of lignes(fenetres)) {
        const item = deps.modele.content.cloneNode(true) as DocumentFragment;
        item.querySelector('[data-titre]')!.textContent = ligne.titre;

        const pastille = item.querySelector<HTMLElement>('[data-etat]')!;
        pastille.textContent = ligne.etat;
        // 🔴 DEUX LITTÉRAUX, ET NON UNE CLASSE CALCULÉE. `design/classes.ts::
        // classesEmployeesTs` ne reconnaît que `classList.add('…')` et
        // `className = '…'` avec un littéral DANS l'appel : passer un nom par une
        // variable rendrait ces deux classes invisibles au contrôle §7.9, donc
        // orphelines sans que rien ne le dise. C'est la forme qu'avait
        // `shell-page.ts`, et elle est conservée à dessein.
        if (ligne.ouverte) pastille.classList.add('bureau__pastille--ouverte');
        else pastille.classList.add('bureau__pastille--fermee');

        const bouton = item.querySelector<HTMLButtonElement>('[data-rouvrir]')!;
        if (ligne.rouvrable) bouton.addEventListener('click', () => deps.rouvrir(ligne.session));
        else bouton.remove();

        deps.liste.append(item);
    }
    // ⚠️ RÉVÉLER **ET** RECACHER. Ne faire que le premier laisserait une
    // section vide sur un hub qui n'a plus rien à montrer.
    if (deps.section !== undefined) deps.section.hidden = !sectionVisible(fenetres);
}
