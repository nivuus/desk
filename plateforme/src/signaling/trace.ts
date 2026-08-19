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
// oubli.
//
// ✅ CETTE DÉCISION ANNONÇAIT SA PROPRE RÉOUVERTURE — « le jour où l'on voudra
// observer les agents présents, ce qui est le sujet de P3 (`vu_a`) » —, ET
// P3 A EU LIEU SANS LA ROUVRIR (19 août 2026, revue transverse de fin de
// branche). Observer les agents ne passe PAS par la trace de session : c'est
// `agent_enrole.vu_a`, avancé par le battement du canal `/agent`
// (`agents/canal.ts`), et jugé par `agents/fraicheur.ts`. Un agent qui se
// déclare et repart y est donc bien vu — simplement ailleurs, et par un
// mécanisme qui ne dépend d'aucun appariement.
//
// **Le pronostic était juste sur le BESOIN et faux sur le LIEU**, et c'est la
// forme la plus fréquente de pronostic périmé dans ce dépôt : ce module
// n'avait rien à changer.
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
// 🔴 C'EST ICI QUE `session.vm_id` SE RÉSOUT, ET NON DANS LE RELAIS (E10 du
// plan P3). `ObservateurDeSession.apparie` est SYNCHRONE et sans retour, à
// dessein (`relais.ts`) : une résolution qui vivrait là-bas obligerait le
// relais à connaître la base, ce qu'il ne connaît pas et ne doit pas
// connaître. La trace, elle, la connaît déjà — c'est sa seule raison d'être.
//
// Le nom de session PORTE la VM : `<préfixe>:bureau` (`agents/prefixe.ts`).
// On le découpe, on cherche le préfixe dans `agent_enrole`, et on inscrit
// l'identifiant trouvé. DEUX cas rendent `null`, et ni l'un ni l'autre n'est
// une erreur :
//   - AUCUN PRÉFIXE (`bureau` tout court) : c'est le mode d'essai local que
//     la spec §10 pose comme légitime, et il ne se journalise pas — il est
//     nominal, pas anormal ;
//   - PRÉFIXE INCONNU : la VM n'est pas (ou n'est plus) enrôlée. Celui-là SE
//     JOURNALISE, parce qu'il est anormal et qu'il serait autrement
//     indiscernable du précédent.
//
// ⚠️ INSCRIRE MALGRÉ TOUT FERAIT MENTIR LA COLONNE : elle nommerait une VM
// que la base ne connaît pas. `session.vm_id` reste NULLABLE pour cette
// raison même — `NOT NULL` y serait FAUX, pas seulement coûteux.
//
// L'horloge est un PARAMÈTRE, jamais lue ici : même règle que `depot/session.ts`
// et `src/signaling/ice.ts`, et c'est ce qui rend les instants assertables sur des
// valeurs exactes.

import type { Pilote } from '../base/pilote';
import { decouper } from '../agents/prefixe';
import { lireParPrefixe } from '../depot/agent';
import { clore, ouvrirSession } from '../depot/session';
import type { ObservateurDeSession } from './relais';

/// Motif posé quand les deux pairs sont partis d'eux-mêmes — à distinguer de
/// `MOTIF_BALAYAGE`, qui marque une ligne qu'un arrêt brutal a laissée ouverte.
///
/// Passé en PARAMÈTRE de la requête, jamais écrit dans le SQL : une valeur
/// littérale ferait lever `rendreMarqueurs` côté Postgres.
export const MOTIF_DEPART = 'les deux pairs sont partis';

/// Résout la VM que le nom de session désigne, ou `undefined`.
///
/// ⚠️ ELLE NE LÈVE PAS SUR UN PRÉFIXE INCONNU : c'est un état légitime du
/// point de vue de la trace, qui observe et n'arbitre rien. Une exception ici
/// remonterait dans le `.catch` de l'appelant et ferait perdre la LIGNE
/// ENTIÈRE — on aurait échangé une colonne `null` contre aucune trace du tout.
///
/// Elle laisse en revanche remonter une panne de la BASE, qui est un tout
/// autre événement : le `.catch` de l'appelant la journalise, et la ligne
/// manquante est alors le symptôme juste.
async function resoudreVm(base: Pilote, nomSession: string): Promise<string | undefined> {
    const { prefixe } = decouper(nomSession);
    // Le mode d'essai local. Nominal, donc muet : le journaliser noierait le
    // cas anormal ci-dessous sous une ligne par session.
    if (prefixe === '') return undefined;

    const ligne = await lireParPrefixe(base, prefixe);
    if (ligne === undefined) {
        // ⚠️ LE PRÉFIXE EST DANS LA LIGNE, et ce n'est pas un oracle : cette
        // trace reste CHEZ NOUS, elle ne part sur aucun fil. C'est le même
        // partage que `identite/garde.ts` entre `message` et `journal`.
        console.warn(
            `session ${nomSession} : préfixe ${prefixe} inconnu de agent_enrole, vm_id non inscrit`,
        );
        return undefined;
    }
    return ligne.vm_id;
}

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
            // 🔴 L'INSTANT EST PRIS **AVANT** LA RÉSOLUTION, jamais après :
            // `resoudreVm` lit la base, donc son temps d'exécution est
            // inconnu, et `ouverte_a` doit dater l'APPARIEMENT — pas la fin
            // d'une requête. Lire l'horloge dans l'appel à `ouvrirSession`
            // ferait dériver l'instant d'autant.
            const instant = horloge();
            ouvertes.set(
                nomSession,
                // L'identifiant d'utilisateur vient du verdict de garde,
                // relayé par le relais. Il est absent quand le second pair à
                // arriver est l'agent — dont l'identité existe depuis P3, mais
                // qui ne revendique toujours rien (`identite/garde.ts`).
                resoudreVm(base, nomSession)
                    .then((vmId) => ouvrirSession(base, nomSession, instant, utilisateurId, vmId))
                    .catch((cause) => {
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
