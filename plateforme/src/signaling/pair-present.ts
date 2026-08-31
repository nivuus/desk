// L'annonce « ton pair est arrivé », et la règle qui décide à qui elle part.
//
// 🔴 LE JUMEAU SYMÉTRIQUE DE `peer-gone`, ET C'EST TOUTE SA JUSTIFICATION.
// Le relais savait déjà dire « ton pair est parti » (`relais.ts`,
// gestionnaire `close`) et ne savait pas dire « ton pair est arrivé ». Ce
// module ajoute la moitié manquante d'un mécanisme EXISTANT — il n'en
// introduit pas un second, ce que la doctrine du dépôt interdit nommément.
//
// 🔴 CE QU'IL RÉPARE, MESURÉ EN PRODUCTION LE 30 AOÛT 2026 (capture réseau
// sur `vnet30`, `tcpdump` + `tshark`, le WebSocket étant en clair côté VM) :
// à t = 12,0 s après son démarrage, le superviseur annonce ses fenêtres
// (`fenetre-ouverte` × 3) dans une session où AUCUN pair `client` n'est
// encore connecté. `send(peer, …)` est alors un no-op silencieux : les
// annonces sont PERDUES, sans une trace. À t = 43,1 s, faute de `viewport`
// en retour, l'agent refuse ses propres fenêtres
// (`superviseur/table/orphelines.rs`, `DELAI_ATTENTE_VIEWPORT_MAX`). Or
// l'utilisateur passe précisément ces secondes-là dans l'authentification du
// proxy : il arrive donc devant un bureau vide, sur une VM pleine de
// fenêtres bien vivantes. Le seul contournement connu avant ce lot était de
// relancer l'agent PENDANT que l'utilisateur regarde la page.
//
// 🔴 CE QUE CE MESSAGE N'EST PAS : un TAMPON. La plateforme ne mémorise
// AUCUNE annonce de fenêtre, à dessein. Une liste de fenêtres tamponnée ici
// serait une COPIE d'une vérité qui vit dans l'agent — elle vieillirait dès
// qu'une fenêtre se ferme, et rien, dans ce service, ne saurait quand
// l'expirer. On ne rejoue donc pas la vérité d'hier : on prévient celui qui
// la détient qu'on la lui demande maintenant. (La seule chose que ce relais
// mémorise reste l'offre SDP — `Appariement::retenirOffre` —, et elle a une
// péremption naturelle : elle est REMISE UNE FOIS puis oubliée.)

/// Le type porté sur le fil. Lu par `agent/src/superviseur/protocole.rs`
/// (`DepuisLaShell::PairPresent`) : le changer est un changement de
/// protocole, des deux côtés à la fois.
///
/// ⚠️ **IL NE DOIT JAMAIS ENTRER DANS `TYPES_RELAYES`** (`relais.ts`) : il
/// est ÉMIS PAR le relais, comme `peer-gone`, `ice-config` et `error`. L'y
/// mettre autoriserait un pair à le FABRIQUER vers l'autre, donc à faire
/// réannoncer l'agent à volonté — un amplificateur offert à qui a un jeton.
export const TYPE_PAIR_PRESENT = 'pair-present';

/// Le message complet, tel qu'il part.
export function messagePairPresent(): { type: string } {
    return { type: TYPE_PAIR_PRESENT };
}

/// Vrai si le pair DÉJÀ EN PLACE doit être prévenu de cette arrivée.
///
/// `roleArrivant` est le rôle du pair qui vient de se déclarer ; le pair
/// prévenu est donc celui d'EN FACE.
///
/// 🔴 SEUL LE RÔLE `agent` EST PRÉVENU, ET L'ASYMÉTRIE EST DÉLIBÉRÉE — elle
/// n'est pas un oubli du sens inverse.
///
/// ① L'agent, et lui seul, détient un état REJOUABLE : sa table de fenêtres.
///    Le client, lui, apprend la présence de l'agent en recevant sa réponse
///    SDP ; cette nouvelle-ci ne lui apprendrait rien.
/// ② Le sens inverse serait du bruit MESURABLE. Sur chaque session de
///    fenêtre `w-N`, c'est la page navigateur qui se connecte la première et
///    l'enfant qui arrive ensuite : prévenir « le pair d'en face » sans
///    regarder son rôle enverrait ce message à TOUTES les pages de session,
///    où `client/src/webrtc.ts::parseSignalingMessage` ne le reconnaît pas
///    et journalise « message de signaling illisible ou de forme inattendue,
///    ignoré ». Le client l'IGNORE donc sans casser — le vérifier était la
///    condition pour ne pas toucher à `client/`, hors périmètre de ce lot —
///    mais une trace par ouverture de fenêtre est un coût sans contrepartie.
///
/// ⚠️ Le jour où un client aurait besoin de cette nouvelle, c'est ce
/// commentaire qu'il faudra CONTREDIRE, pas compléter en silence.
export function prevenirLePairEnPlace(roleArrivant: 'agent' | 'client'): boolean {
    return roleArrivant === 'client';
}

/// Vrai si le pair QUI VIENT D'ARRIVER doit être prévenu qu'un pair l'attendait
/// déjà.
///
/// 🔴 **LE TROISIÈME CAS, ET IL MANQUAIT — C'EST L'AUTRE MOITIÉ DU MÊME
/// MÉCANISME, PAS UN MÉCANISME DE PLUS.** `prevenirLePairEnPlace` ci-dessus
/// répond à « le client arrive, l'agent est là ». Celle-ci répond à la
/// question SYMÉTRIQUE, que rien ne posait : **« l'agent arrive, le client est
/// déjà là »**. Les deux ensemble énoncent une règle unique et complète :
/// *l'agent est prévenu dès que l'appariement est complet, quel que soit le
/// côté arrivé en dernier.*
///
/// 🔴 **POURQUOI CE CAS EXISTE MAINTENANT ET PAS AVANT.** Jusqu'à ce lot,
/// l'agent n'ouvrait sa session de contrôle QU'UNE FOIS, au démarrage : il
/// était donc toujours le premier arrivé, et le cas ne pouvait pas se
/// produire. Depuis que `agent/src/superviseur/signalisation.rs` la ROUVRE
/// après une chute, l'ordre s'inverse dès que la page-shell revient avant lui
/// — ce qui est le cas ORDINAIRE après un redémarrage du service : le
/// navigateur de l'utilisateur est rechargé à la main en quelques secondes,
/// l'agent, lui, respecte un repli exponentiel qui peut atteindre trente
/// secondes. Sans cette règle, l'agent se reconnecterait **sans jamais
/// apprendre que quelqu'un l'attend**, et ne réannoncerait rien : une
/// reconnexion parfaitement réussie, et un bureau vide malgré tout — la panne
/// muette d'hier, déplacée d'un cran.
///
/// ⚠️ **ET LE PAIR EN FACE EST NÉCESSAIREMENT UN `client`** : le relais
/// n'admet qu'un `agent` et un `client` par session
/// (`appariement.ts::declarer`), donc quand l'arrivant est l'`agent`, celui
/// qui l'attendait ne peut être que le `client`. C'est ce qui autorise cette
/// règle à ne regarder QUE le rôle de l'arrivant, comme sa voisine.
///
/// ⚠️ **CE QUE CE MESSAGE COÛTE AILLEURS, MESURÉ EN LISANT LE CODE PLUTÔT QUE
/// SUPPOSÉ.** Il part désormais aussi aux agents des sessions `w-N` et
/// `fichiers`, où la page se connecte d'ordinaire la première : ces processus
/// lisent leur signaling dans `agent/src/signaling.rs`, dont l'aiguillage
/// finit par `other => tracing::debug!(?other, "message de signaling
/// ignoré")` — un fourre-tout qui journalise et **continue**, jamais un
/// `break`. Ils l'ignorent donc sans casser, au prix d'une ligne de `debug!`
/// qui n'est même pas émise sous le `RUST_LOG=info` de l'exploitation.
/// ⚠️ Le jour où cet aiguillage cesserait de tolérer un type inconnu, c'est
/// ce paragraphe qu'il faudra CONTREDIRE, pas découvrir.
export function prevenirLArrivant(roleArrivant: 'agent' | 'client'): boolean {
    return roleArrivant === 'agent';
}
