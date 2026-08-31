// CE QU'UNE FENÊTRE CONNUE DEVIENT À L'ÉCRAN — la règle, pure et testée.
// La pose dans le gabarit vit dans `fenetres-dom.ts`, qui ne décide rien.
//
// 🔴 POURQUOI CETTE SCISSION PLUTÔT QU'UN SEUL MODULE DOM. `client/` n'a **ni
// jsdom ni happy-dom**, et ce n'est pas un manque : `accent-dom.test.ts` le
// déclare comme la convention du dépôt — « les dépendances sont injectées, et
// c'est ce qui rend ce module éprouvable là où `main.ts` ne l'est pas ». Ce
// fichier-ci prend l'autre voie, également admise : sortir la DÉCISION du
// câblage, et ne laisser en face du DOM que ce qui pose des valeurs déjà
// calculées.

import type { FenetreConnue } from '../shell';

export interface LigneFenetre {
    session: string;
    titre: string;
    /// Le mot montré dans la pastille — du texte pour un humain, donc accentué.
    etat: 'ouverte' | 'fermée';
    /// 🔴 DEUX LITTÉRAUX, ET NON UNE CLASSE COMPOSÉE : une classe calculée est
    /// invisible au contrôle §7.9, qui ne voit que les littéraux passés à
    /// `classList.add('…')` et à `className = '…'`. Le type les énumère, ce
    /// qui les rend aussi vérifiables par `tsc`.
    classePastille: 'bureau__pastille--ouverte' | 'bureau__pastille--fermee';
    /// Une fenêtre ouverte n'a rien à rouvrir : son bouton PART plutôt que
    /// d'être désactivé — il n'y a pas d'action à suggérer.
    rouvrable: boolean;
}

export function lignes(fenetres: FenetreConnue[]): LigneFenetre[] {
    return fenetres.map((f) => ({
        session: f.session,
        titre: f.titre,
        etat: f.ouverte ? 'ouverte' : 'fermée',
        classePastille: f.ouverte ? 'bureau__pastille--ouverte' : 'bureau__pastille--fermee',
        rouvrable: !f.ouverte,
    }));
}

/// La section « Mes fenêtres » doit-elle paraître ?
///
/// 🔴 ABSENTE, PAS VIDE (spec §4.1) : une section montrant en permanence
/// « aucune fenêtre ouverte » serait du bruit sur l'état NOMINAL d'un hub
/// qu'on vient d'ouvrir. ⚠️ Une fenêtre FERMÉE compte : elle porte son bouton
/// « Rouvrir », donc la cacher priverait l'utilisateur du seul geste qui la
/// ramène.
export function sectionVisible(fenetres: FenetreConnue[]): boolean {
    return fenetres.length > 0;
}
