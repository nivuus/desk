// La trace en base d'une session appariée : une ligne ouverte quand les DEUX
// rôles sont présents, close quand la session se vide.
//
// 🔴 QUAND, exactement, et ce n'est pas évident. Le relais connaît deux
// instants : la DÉCLARATION d'un pair, et l'APPARIEMENT du second. Un seul pair
// n'est pas un appariement — le superviseur se déclare `agent` sur `bureau` au
// démarrage de la VM et peut y rester seul des heures
// (`agent/src/superviseur/protocole.rs`). La ligne s'ouvre donc au SECOND rôle,
// et se clôt quand `Appariement::retirer` rend `{ vide: true }`.
//
// ⚠️ CONSÉQUENCE ASSUMÉE : un agent qui se déclare et repart sans jamais
// rencontrer de client NE LAISSE AUCUNE TRACE. C'est une décision, pas un
// oubli ; elle se rouvrira le jour où l'on voudra observer les agents présents,
// ce qui est le sujet de P3 (`vu_a`), pas de P1.
//
// 🔴 L'ÉCRITURE NE DOIT JAMAIS POUVOIR TUER UNE SESSION. `ouvrirSession` est
// asynchrone, le gestionnaire `message` du relais est synchrone, et une
// promesse rejetée sans `catch` dans un gestionnaire d'événement `ws` abat tout
// le process Node — c'est exactement le mode de défaillance que le commentaire
// d'`isJsonObject` (`relais.ts`) décrit. L'écriture est donc lancée SANS être
// attendue, avec un `.catch` qui journalise et n'interrompt rien.
//
// Le coût est nommé : une écriture perdue ne se voit qu'au journal. C'est
// pourquoi `trace.test.ts` attend la ligne avec une BORNE et échoue sur
// expiration, plutôt que de se contenter de « la ligne finit par exister ».
//
// L'horloge est un PARAMÈTRE, jamais lue ici : même règle que `depot/session.ts`
// et `src/signaling/ice.ts`, et c'est ce qui rend les instants assertables sur des
// valeurs exactes.

import type { Pilote } from '../base/pilote';
import { clore, ouvrirSession } from '../depot/session';
import type { ObservateurDeSession } from './relais';

/// Motif posé quand les deux pairs sont partis d'eux-mêmes — à distinguer de
/// `MOTIF_BALAYAGE`, qui marque une ligne qu'un arrêt brutal a laissée ouverte.
///
/// Passé en PARAMÈTRE de la requête, jamais écrit dans le SQL : une valeur
/// littérale ferait lever `rendreMarqueurs` côté Postgres.
export const MOTIF_DEPART = 'les deux pairs sont partis';

export function observateurDeSession(
    base: Pilote,
    horloge: () => number,
): ObservateurDeSession {
    // La PROMESSE, et non l'identifiant : `separe` peut arriver avant que
    // l'INSERT n'ait abouti (deux pairs qui se ferment aussitôt appariés).
    // Retenir la promesse et enchaîner dessus ferme cette course sans verrou.
    const ouvertes = new Map<string, Promise<string | undefined>>();

    return {
        apparie(nomSession, utilisateurId) {
            // Une session déjà tracée ne rouvre pas de seconde ligne : un pair
            // qui se reconnecte pendant que l'autre reste en place rapparie la
            // session, et la première ligne resterait sinon orpheline — jamais
            // close, jusqu'au balayage du prochain démarrage.
            if (ouvertes.has(nomSession)) return;
            ouvertes.set(
                nomSession,
                // L'identifiant vient du verdict de garde, relayé par le
                // relais. Il est absent quand le second pair à arriver est
                // l'agent, qui n'a aucune identité avant P3.
                ouvrirSession(base, nomSession, horloge(), utilisateurId).catch((cause) => {
                    console.error(
                        `trace de session non écrite pour ${nomSession} : ${String(cause)}`,
                    );
                    return undefined;
                }),
            );
        },

        separe(nomSession) {
            const attendue = ouvertes.get(nomSession);
            // Rien à clore : la session n'a jamais été appariée. Ce n'est pas
            // une anomalie — voir l'en-tête.
            if (!attendue) return;
            ouvertes.delete(nomSession);
            void attendue
                .then((id) => (id ? clore(base, id, horloge(), MOTIF_DEPART) : undefined))
                .catch((cause) => {
                    console.error(
                        `trace de session non close pour ${nomSession} : ${String(cause)}`,
                    );
                });
        },
    };
}
