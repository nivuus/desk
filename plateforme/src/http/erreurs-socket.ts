// L'écouteur qui empêche une trame trop grosse d'abattre le service.
//
// 🔴 EXTRAIT DE `serveur.ts` LE 25 AOÛT 2026 (round de correction 1, chantier
// « legs sans VM »), POUR LUI FAIRE DE LA PLACE — sans changer une ligne de
// comportement. `serveur.ts` était à 494/500 ; le câblage du nettoyage de
// fond des magasins (`apps/nettoyage.ts`) l'aurait porté au-delà. La règle du
// dépôt est EXTRAIRE, jamais comprimer.

import type { WebSocketServer } from 'ws';

/// 🔴 SANS CETTE FONCTION, `TRAME_MAX_OCTETS` (`serveur.ts`) DONNE UN DÉNI DE
/// SERVICE PIRE QUE CELUI QU'IL FERME, et ce n'est pas une conjecture :
/// MESURÉ le 20 août 2026 sur le vrai point d'entrée, `connect ECONNREFUSED`
/// — LE PROCESS ÉTAIT MORT, tué par UNE SEULE TRAME ANONYME.
///
/// LA CHAÎNE, en trois maillons dont chacun est banal : `ws` refuse une trame
/// au-delà de `maxPayload` et ÉMET `error` sur le socket serveur ; aucun
/// socket serveur de ce service n'avait d'écouteur `error` (vérifié :
/// `grep -n "on('error'" relais.ts canal.ts serveur.ts` ne rendait que le
/// `http.once('error', reject)` du démarrage) ; et un `EventEmitter` qui émet
/// `error` sans écouteur LÈVE. L'exception traverse alors un gestionnaire
/// d'évènement Node, qui n'a personne pour l'attraper — le mode de défaillance
/// exact que `signaling/relais.ts` et `signaling/trace.ts` documentent tous
/// deux, atteint ici par une porte neuve.
///
/// ⚠️ AUCUN TEST « DANS » VITEST NE POUVAIT LE VOIR : vitest installe son
/// propre gestionnaire d'exceptions non interceptées, si bien que les tests de
/// `http/serveur.test.ts` restaient VERTS pendant que le service réel mourait
/// (ils signalaient seulement « Vitest caught N unhandled errors »). La preuve
/// vit donc dans `signaling/resilience.test.ts`, qui lance `index.ts` comme un
/// vrai process enfant — c'est précisément la raison d'être de ce fichier-là,
/// et son en-tête l'écrivait avant P5.
///
/// ⚠️ ELLE NE JOURNALISE RIEN, ET C'EST UN CHOIX MOTIVÉ, PAS UNE NÉGLIGENCE.
/// `CLAUDE.md` porte la règle depuis le chantier TURN : « ne jamais tracer par
/// paquet dans la boucle de transport — compter ou échantillonner, jamais
/// tracer par paquet », après qu'une trace par `Transmit` a écrit 18 619
/// lignes en quelques secondes et détruit la mesure qu'elle servait. Une ligne
/// par socket fautif rendrait ici le service à nouveau amplificateur : un
/// attaquant ouvrant N sockets ferait écrire N lignes, sur le chemin même que
/// `TRAME_MAX_OCTETS` vient de fermer.
///
/// ⚠️ LE COÛT EST NOMMÉ : une erreur de socket est donc INVISIBLE à
/// l'exploitant. Ce qui reste observable est la FERMETURE, que le pair voit
/// (code 1009), et le fait que le service continue de servir. Le jour où il
/// faudra les compter, c'est un compteur qu'il faudra — pas une trace.
export function encaisserLesErreursDeSocket(wss: WebSocketServer): void {
    // Enregistré AVANT `createSignalingServer` et `servirLeCanalAgent`, qui
    // posent leurs propres gestionnaires `connection` : les écouteurs courent
    // dans leur ordre d'enregistrement, et celui-ci doit être attaché au
    // socket avant que quoi que ce soit d'autre ne lui parle.
    wss.on('connection', (socket) => {
        socket.on('error', () => {
            // Volontairement vide — voir ci-dessus. La seule chose qui compte
            // est qu'un écouteur EXISTE : c'est lui, et lui seul, qui empêche
            // `EventEmitter` de lever.
        });
    });
}
