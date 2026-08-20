// Décider si une touche est un raccourci de COLLAGE, et rien d'autre.
//
// **PUR, sans DOM** — il ne connaît ni `KeyboardEvent`, ni `window`, ni le
// canal : il prend les cinq champs qui décident. C'est le patron de
// `resize.ts` et de `presse-papier.ts`.
//
// 🔴 **Ce prédicat est la SEULE défense contre le risque R5 de la spec**, et
// c'est pourquoi il vit à part plutôt qu'inliné dans l'écouteur de
// `input.ts`.
//
// ❌ **Ce paragraphe disait « AUJOURD'HUI `input.ts` appelle `preventDefault()`
// SANS CONDITION sur chaque `keydown` », et la tâche 18 de CETTE MÊME BRANCHE
// l'a rendu faux quelques commits plus tard.** Relevé par la revue transverse
// du 21 août 2026 ; c'est le mode de défaillance dominant de ce dépôt.
//
// L'état d'AVANT P2 — vrai jusqu'au commit `4cf2206` — était bien un
// `preventDefault()` inconditionnel : le navigateur ne voyait passer aucun
// raccourci, ce qui est précisément ce qui rend la fenêtre de session
// utilisable, `Ctrl+W` n'y fermant rien et `Ctrl+T` n'y ouvrant rien.
// **Aujourd'hui l'exception existe, elle est exactement celle que ce prédicat
// décrit, et TOUTE condition plus large rendrait le navigateur au clavier.**
//
// Une condition inlinée dans un écouteur ne se teste pas ; celle-ci porte une
// table de vérité de **quinze cas, dont treize refus** — comptés par
// `npx vitest run src/raccourcis.test.ts` le 21 août 2026, `it.each` expansé,
// et non estimés. ⚠️ Une première rédaction annonçait « treize cas, dont neuf
// refus » : les DEUX nombres étaient faux, relevés par la revue transverse.
//
// ⚠️ **L'exception peut être aussi étroite, et c'est MESURÉ, pas supposé.** La
// sonde du 20 août 2026 (`docs/superpowers/plans/journaux-presse-papier-p2/`,
// `p2-paste-video-{1,2}.json`, deux exécutions) établit que dans la cellule qui
// décide — focus sur le `<video>`, régime « exception étroite » — le `keydown`
// de `ControlLeft` **garde son `preventDefault`** (`dp: true`) et **seul `KeyV`
// passe** (`dp: false`), le `paste` de confiance arrivant quand même. Il suffit
// donc de laisser passer le `keydown` du raccourci lui-même.

/// Ce dont le prédicat a besoin d'un `keydown`. Un `KeyboardEvent` s'y
/// conforme structurellement, sans conversion.
export interface ToucheObservee {
    ctrlKey: boolean;
    shiftKey: boolean;
    altKey: boolean;
    metaKey: boolean;
    /// `event.code` — la touche PHYSIQUE, jamais `event.key` : `key` dépend de
    /// la disposition, et sur un clavier AZERTY `Ctrl+V` porte bien `code:
    /// 'KeyV'`. C'est aussi l'unité de `SCANCODES`.
    code: string;
}

/// Vrai pour les deux seuls raccourcis de collage que D6 retient.
///
/// **Les négations sont écrites, et chacune a son test.** Sans elles,
/// `Ctrl+Shift+V` (« coller sans mise en forme » dans plusieurs applications)
/// et `Ctrl+Alt+V` passeraient — c'est-à-dire que le produit Windows les
/// perdrait —, et `Meta+V` serait traité alors que le produit ne le traite pas
/// en v1.
export function estUnRaccourciDeCollage(e: ToucheObservee): boolean {
    const ctrlV = e.ctrlKey && !e.shiftKey && !e.altKey && !e.metaKey && e.code === 'KeyV';
    const shiftInsert =
        e.shiftKey && !e.ctrlKey && !e.altKey && !e.metaKey && e.code === 'Insert';
    return ctrlV || shiftInsert;
}
