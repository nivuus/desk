// Le préfixe opaque de session, et les deux fonctions pures qui le composent
// et le découpent.
//
// 🔴 POURQUOI IL EXISTE : jusqu'ici, l'espace de noms des sessions était
// LOCAL À UNE VM. `agent/src/superviseur/protocole.rs` nomme sa session de
// contrôle `bureau` — une constante littérale — et `agent/src/superviseur/
// table.rs` numérote ses fenêtres `w-1`, `w-2`… par un compteur d'INSTANCE.
// Deux VMs branchées sur la même plateforme produisent donc toutes deux
// `bureau` et toutes deux `w-1`, et se disputent la même entrée de la table
// d'appariement. Le préfixe rend le nom global sans toucher au compteur.
//
// 🔴 CE MODULE EST PUR À UNE EXCEPTION NOMMÉE : `nouveauPrefixe` tire de
// `randomBytes`. Tout le reste — `composer`, `decouper` — est une fonction
// de chaîne, sans horloge, sans base, sans état.
//
// ⚠️ L'ALPHABET N'EST PAS UN DÉTAIL. `base64url` (`A-Za-z0-9_-`) et non
// `base64` ordinaire, pour deux raisons dont une porte tout le reste :
//   1. il ne contient PAS le séparateur `:`, donc l'identifiant TURN
//      `<expiration>:<préfixe>:<nom>` (`signaling/ice.ts`) garde une PREMIÈRE
//      borne non ambiguë — coturn coupe sur le premier `:` en mode
//      `use-auth-secret`. ⚠️ Cette dernière propriété est une lecture de la
//      convention coturn, JAMAIS ÉPROUVÉE contre un coturn vivant ;
//   2. il ne contient ni `+` ni `/`, qui casseraient une chaîne de requête ou
//      un composant de chemin le jour où le préfixe y voyagerait.

import { randomBytes } from 'node:crypto';

/// Le séparateur entre le préfixe et le nom local de session, tel que la
/// spec §3.4 l'écrit : `<préfixe>:bureau`.
export const SEPARATEUR = ':';

/// 16 octets, soit 128 bits — le minimum que la spec §3.4 exige d'un préfixe
/// « non devinable ». ⚠️ NON CALIBRÉE au-delà de ce minimum : aucune mesure
/// n'a jugé qu'il fallait plus, elle rejoint la liste des constantes non
/// calibrées du dépôt.
export const OCTETS_PREFIXE = 16;

/// Tire un préfixe neuf. 22 caractères, sans remplissage.
export function nouveauPrefixe(): string {
    return randomBytes(OCTETS_PREFIXE).toString('base64url');
}

/// Compose le nom de session global.
///
/// 🔴 UN PRÉFIXE VIDE REND LE NOM INCHANGÉ, et c'est la propriété la plus
/// importante de ce fichier : elle restitue EXACTEMENT le comportement d'avant
/// P3 (`bureau`, `w-1`). Poser le séparateur inconditionnellement rendrait
/// `:bureau`, qui n'est le nom d'aucune session existante — et rien, nulle
/// part, ne le signalerait. C'est la classe de panne muette contre laquelle
/// tout ce dépôt est écrit.
export function composer(prefixe: string, nom: string): string {
    if (prefixe === '') return nom;
    return `${prefixe}${SEPARATEUR}${nom}`;
}

/// Défait la composition. Rend un préfixe VIDE quand il n'y en a pas, jamais
/// `undefined` et jamais une exception : le mode d'essai local (spec §10) est
/// un état légitime, pas une erreur.
///
/// La coupe se fait sur le PREMIER séparateur : le préfixe n'en contient
/// jamais (voir l'alphabet), mais rien ne garantit qu'un nom local n'en porte
/// pas un jour. Couper sur le dernier ferait alors passer le début du nom
/// pour une partie du préfixe.
export function decouper(session: string): { prefixe: string; nom: string } {
    const i = session.indexOf(SEPARATEUR);
    if (i === -1) return { prefixe: '', nom: session };
    return { prefixe: session.slice(0, i), nom: session.slice(i + SEPARATEUR.length) };
}
