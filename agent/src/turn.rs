//! Client TURN : machine à états et encapsulation, sans entrées-sorties.
//!
//! Ce module consomme des octets et rend des octets. Il ne possède aucun
//! socket, ne connaît pas `Rtc`, et ne dépend d'aucune API Windows — c'est ce
//! qui rend toute la machine à états testable sur Linux, sans coturn.
//!
//! Partage du travail avec `is::stun` : on LIT les réponses avec son parseur
//! (qui couvre tout le vocabulaire TURN dont on a besoin), on ÉCRIT les
//! requêtes soi-même. Son builder ne connaît pas `REQUESTED-TRANSPORT`
//! (0x0019), sans lequel un serveur conforme refuse toute allocation en 400.
//!
//! Découpé en sous-modules suivant les frontières du protocole : `messages`
//! (sérialisation des requêtes et dérivation des clés). Même raison d'être que
//! le découpage de `congestion` et de `transport` : la limite de 500 lignes par
//! fichier de `CLAUDE.md`.
//!
//! Visibilité : `messages` est un module privé, donc ce qu'il déclare `pub`
//! reste borné au sous-arbre de `turn` — accessible à ses futurs frères
//! (`allocation`, `canaux`) par `super::messages`, invisible à l'extérieur.
//! Les réexports `pub use` n'apparaîtront ici que pour ce que le transport
//! devra vraiment voir, au moment où il le consommera.

mod allocation;
mod canaux;
#[cfg(test)]
mod fixtures;
mod messages;
