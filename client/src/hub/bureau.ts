// LE CHEMIN DU HUB VERS LE BUREAU — la règle, pure et testée.
//
// 🔴 POURQUOI CE MODULE EXISTE. DÉFAUT TROUVÉ EN PRODUCTION LE 30 AOÛT 2026,
// sur la machine du propriétaire, et CONSÉQUENCE DIRECTE DU PASSAGE DU HUB À
// LA RACINE (lot 14, `plateforme/src/http/page/resolution.ts`) : `/` sert
// `hub.html`, le hub n'ouvre AUCUNE connexion de signaling, et AUCUN lien de
// navigation ne menait à `shell.html`, la seule page qui traite
// `fenetre-ouverte` (`client/src/shell-page.ts`). Le superviseur annonçait
// donc ses fenêtres dans une session où personne n'occupait le rôle `client`
// — `signaling/relais.ts` relaie par `send(peer, …)`, et sur un pair absent
// c'est un no-op SILENCIEUX — puis les refusait trente secondes plus tard.
// L'utilisateur lançait une application et ne voyait jamais rien.
//
// 🔴 POURQUOI CE CHEMIN-LÀ PLUTÔT QUE « LE HUB TIENT LUI-MÊME LA SESSION ».
// Le diagnostic du lot 20 nommait les deux voies. Celle-ci est retenue pour
// une raison qui n'est pas de goût : **le rôle `client` est EXCLUSIF**
// (`plateforme/src/signaling/appariement.ts::declarer` — « un client est déjà
// connecté à la session … », mesuré le 30 août 2026 : un pilote s'est vu
// refuser la place que la page du propriétaire occupait). Si le hub prenait
// ce rôle, alors toute PWA PAR APPLICATION — dont le `start_url` est
// `shell.html?app=<id>` (`hub/manifeste.ts`), et le propriétaire vient d'en
// installer une — se verrait refuser la session de contrôle dès qu'un onglet
// du hub serait ouvert quelque part. Ce serait casser un chemin qui marche
// pour en réparer un autre. Fusionner les deux surfaces est l'autre voie du
// diagnostic : c'est une décision de conception qui appartient au
// propriétaire, pas à un correctif.
//
// 🔴 ET SURTOUT : LE GESTE. `shell-page.ts` ouvre les fenêtres de session par
// `window.open` EN RÉPONSE À UN MESSAGE WebSocket, donc HORS geste
// utilisateur, ce que les navigateurs bloquent par défaut — le produit le
// sait et le dit (`shell.ts::ouvrir`, « le navigateur a bloqué la pop-up »).
// Câbler le hub sans traiter cela n'aurait fait que déplacer le mur d'un
// cran. Ici, l'ouverture du bureau part d'un CLIC : c'est un geste, donc
// aucune pop-up n'est bloquée sur CE maillon. Le maillon suivant — la fenêtre
// de session elle-même — garde son repli déjà livré et déjà testé : la carte
// de la fenêtre et son bouton « Rouvrir » (`shell.html`, `shell.ts::rouvrir`),
// qui est lui aussi un geste. Aucun troisième mécanisme n'est inventé.

/// La page du bureau. C'est la SEULE qui traite `fenetre-ouverte`, et c'est
/// aussi le `start_url` des manifestes par application (`hub/manifeste.ts`).
export const PAGE_DU_BUREAU = 'shell.html';

/// Le NOM de la fenêtre navigateur qui porte le bureau.
///
/// 🔴 UN NOM, ET NON `_blank` : deux clics successifs sur « Lancer » doivent
/// retrouver LE MÊME bureau, jamais en ouvrir un second. Un second bureau se
/// verrait refuser le rôle `client` (voir l'en-tête) et afficherait un bandeau
/// rouge à l'utilisateur qui n'a fait que cliquer deux fois.
export const NOM_FENETRE_BUREAU = 'nivuus-bureau';

export interface DepsBureau {
    /// `null` quand le navigateur a bloqué l'ouverture — le même contrat que
    /// `OptionsBureau.ouvrirFenetre` de `shell.ts`, et pour la même raison.
    ouvrir(url: string, nom: string): Window | null;
}

/// Ouvre — ou ramène au premier plan — la fenêtre du bureau.
///
/// ⚠️ **À N'APPELER QUE DEPUIS UN GESTE UTILISATEUR.** Hors geste, le
/// navigateur refuse et cette fonction rend `false` ; elle ne ment pas, mais
/// elle ne peut rien y faire.
///
/// Rend `true` si le bureau est là — ouvert à l'instant, ou déjà ouvert et
/// remonté. `focus()` est appelé dans les DEUX cas : sur une fenêtre déjà
/// ouverte, `window.open` avec un nom connu ne fait que la réutiliser sans la
/// montrer, et l'utilisateur croirait que son clic n'a rien fait.
export function ouvrirLeBureau(deps: DepsBureau): boolean {
    const fenetre = deps.ouvrir(PAGE_DU_BUREAU, NOM_FENETRE_BUREAU);
    if (fenetre === null) return false;
    // ⚠️ `focus` peut ne pas exister sur une fenêtre déjà fermée par
    // l'utilisateur entre-temps : on ne suppose rien de ce que le navigateur
    // rend, on éprouve.
    if (typeof fenetre.focus === 'function') fenetre.focus();
    return true;
}
