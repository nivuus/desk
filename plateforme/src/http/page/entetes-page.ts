// Les en-têtes que la plateforme pose sur ce qu'elle SERT comme page.
//
// 🔴 CE MODULE EXISTE PARCE QUE `../entetes.ts` NE POUVAIT PAS SERVIR ICI, et
// la raison est une inversion, pas un manque : son `Cache-Control: no-store`
// est INCONDITIONNEL, et il est là pour les réponses de `/auth/*`, qui portent
// des jetons en clair. Les ressources EMPREINTÉES ont le besoin EXACTEMENT
// OPPOSÉ — cachables un an. Réutiliser `ENTETES_SECURITE` tel quel est le
// geste naturel, et c'est le défaut.
//
// 🔴 MAIS « LES RESSOURCES » ÉTAIT UN MOT TROP LARGE, ET IL A COÛTÉ UN AN DE
// CACHE SUR LE MANIFESTE PWA DU HUB. La première rédaction n'avait que DEUX
// jeux d'en-têtes, document et ressource, et classait par EXTENSION : la liste
// MIME admet `webmanifest`, `json`, `ico`, `png`, que Vite n'empreinte JAMAIS
// à la racine. MESURÉ sur le vrai `client/dist` : `/hub.webmanifest` rendait
// `public, max-age=31536000, immutable`, donc **non révisable pendant un an**
// chez tout navigateur l'ayant vu. D'où TROIS jeux, et non deux — la
// distinction elle-même vit dans `resolution.ts` (`empreinte`), parce que
// c'est la RÈGLE qui classe, pas le servant.
//
// 🔴 HSTS N'EST PAS ICI, ET C'EST DÉLIBÉRÉ. La ligne de partage avec le proxy
// devient : ce qui dépend du DOCUMENT suit le document ; ce qui dépend de TLS
// reste chez qui termine TLS. La plateforme est joignable en clair.

/// La politique de sécurité du contenu.
///
/// 🔴 ELLE EST RECOPIÉE DE `deploiement/nginx.conf`, ET `entetes-page.test.ts`
/// LIT LES DEUX ET LES COMPARE. Sans ce test, un durcissement appliqué d'un
/// seul côté livrerait deux montages aux sécurités différentes.
///
/// 🔴 `script-src 'self'` SANS `'unsafe-inline'` NI HASH, ET C'EST DÉLIBÉRÉ —
/// mesuré cassé le 29 août 2026 (`https://app.allanic.me`, Chrome) tant que
/// l'amorce anti-FOUC de `client/` partait EN LIGNE dans le HTML : ce module
/// et `client/vite.config.ts` vivent dans deux paquets, et rien ne les
/// reliait avant `client/src/design/amorce-theme.csp.test.ts`. Corrigé côté
/// CLIENT (l'amorce est désormais un fichier externe `'self'`, jamais en
/// ligne) plutôt qu'ici par un hash : ce fichier a une copie STATIQUE dans
/// `deploiement/nginx.conf` (ligne ci-dessus) qui ne peut PAS calculer un
/// hash à la volée sur le contenu qu'il sert — un hash aurait donc dû être
/// recopié à la main dans les DEUX fichiers, le « naufrage du 487 » que
/// `CLAUDE.md` interdit. Lire le grand commentaire au-dessus de
/// `NOM_FICHIER_AMORCE` dans `client/vite.config.ts` avant de reproposer un
/// hash ici.
///
/// 🔴 `manifest-src 'self'` EST EXPLICITE, ET NON UN REPLI SUR `default-src` —
/// mesuré cassé le 29 août 2026 (`https://app.allanic.me/hub.html`, Chrome) :
/// « manifest-src was not explicitly set, so default-src is used as a
/// fallback » (le navigateur le DIT lui-même dans le message de violation).
/// Le défaut réel n'était pas la directive manquante mais un
/// `<link rel="manifest">` sans `crossorigin="use-credentials"` — corrigé
/// côté CLIENT (`client/src/hub/manifeste-hub-greffon.ts`), sur le même
/// principe que `script-src` deux paragraphes plus haut : un repli implicite
/// est une règle que personne n'a écrite, donc on l'écrit, même quand ce
/// n'était pas elle qui bloquait.
///
/// 🔴 `blob:` A DÛ REJOINDRE `manifest-src` LE 30 AOÛT 2026 — régression
/// commandée par CE dépôt LA VEILLE, et trouvée EN PRODUCTION par le
/// propriétaire (`https://app.allanic.me`, boucle de violations) :
///
///   Loading a manifest from 'blob:https://app.allanic.me/…' violates the
///   following Content Security Policy directive: "manifest-src 'self'".
///
/// Le manifeste DU HUB est servi par HTTP (`hub.webmanifest`, couvert par
/// `'self'`), mais le manifeste PAR APPLICATION ne peut pas l'être : aucune
/// route authentifiée ne le sert (⑤ ne pose aucun cookie), donc
/// `client/src/hub/page.ts::publierLeManifeste` le construit en mémoire et le
/// publie par `URL.createObjectURL` — voie V1 de G5, documentée dans
/// `client/hub.html` et `client/src/hub/manifeste.ts`. En n'écrivant QUE
/// `'self'`, l'explicitation d'hier a rendu VISIBLE — et donc BLOQUANT — ce
/// que le repli implicite sur `default-src 'self'` bloquait déjà en silence
/// (`default-src` ne portait pas non plus `blob:`) : la boucle du propriétaire
/// est le symptôme d'un défaut préexistant, pas une régression de comportement
/// pur — mais une régression de VISIBILITÉ suffit à casser une fonctionnalité
/// livrée (G5), et c'est bien ce qui s'est produit.
///
/// ⚠️ CE QUE `blob:` ADMET ICI, ET POURQUOI C'EST ACCEPTABLE : une origine
/// `blob:` n'est pas un tiers — c'est une URL que LA PAGE ELLE-MÊME fabrique,
/// à partir d'octets qu'ELLE a construits (`new Blob([JSON.stringify(...)])`),
/// et qu'aucune requête réseau ne peut produire depuis l'extérieur : un
/// attaquant qui n'a pas déjà de JavaScript actif dans cette origine ne peut
/// pas faire naviguer `<link rel="manifest">` vers une `blob:` de son choix.
/// Admettre `manifest-src blob:` revient donc à dire « je fais confiance à ce
/// que MON script produit », pas « je fais confiance à une origine externe » —
/// c'est la même confiance que `script-src 'self'` accorde déjà à tout le
/// code de cette page, un cran plus bas. Le risque théorique est qu'une
/// injection XSS réussie pourrait de toute façon fabriquer sa propre `blob:`
/// (ou pire, exécuter du script directement) : `manifest-src blob:` n'ouvre
/// donc aucune surface qu'une XSS n'ouvrirait pas déjà. **Acceptable.**
export const CSP =
    "default-src 'self'; connect-src 'self' wss: https:; img-src 'self' data: blob:; " +
    "media-src 'self' blob:; script-src 'self'; style-src 'self' 'unsafe-inline'; " +
    "font-src 'self'; manifest-src 'self' blob:; frame-ancestors 'none'; base-uri 'self'; " +
    "form-action 'self'";

export const ENTETES_DOCUMENT: Readonly<Record<string, string>> = Object.freeze({
    'X-Content-Type-Options': 'nosniff',
    'Cache-Control': 'no-store',
    'Content-Security-Policy': CSP,
    'Referrer-Policy': 'no-referrer',
    'X-Frame-Options': 'DENY',
});

/// Pour les seuls noms qui portent RÉELLEMENT une empreinte — ceux du
/// répertoire d'actifs de Vite (`resolution.ts::REPERTOIRE_ACTIFS`).
///
/// 🔴 `immutable` EST UNE PROMESSE QU'ON NE PEUT PAS REPRENDRE : un navigateur
/// qui l'a vue ne redemandera pas la ressource avant un an, quoi qu'on
/// déploie. Elle n'est tenable que si le NOM change à chaque contenu — ce que
/// l'empreinte garantit, et ce que rien d'autre ne garantit.
export const ENTETES_RESSOURCE_EMPREINTEE: Readonly<Record<string, string>> = Object.freeze({
    'X-Content-Type-Options': 'nosniff',
    'Cache-Control': 'public, max-age=31536000, immutable',
});

/// Pour TOUTE autre ressource : `hub.webmanifest`, `favicon.ico`, une icône
/// posée à la racine — des noms STABLES dont le contenu change.
///
/// 🔴 RÉVALIDABLE, JAMAIS UN AN ET JAMAIS `no-store`. Les deux extrêmes sont
/// faux ici : un an rend la ressource non révisable (c'est le défaut qu'on
/// corrige), et `no-store` interdirait jusqu'au stockage, donc referait payer
/// le transfert entier à chaque visite pour un fichier qui, la plupart du
/// temps, n'a pas changé. `max-age=0, must-revalidate` garde la copie et exige
/// qu'elle soit revalidée : un `304` suffit alors, et une révision se voit
/// immédiatement.
///
/// ⚠️ ELLE EST PLUS PERMISSIVE QUE NGINX, ET C'EST ASSUMÉ : `location /` de
/// `deploiement/nginx.conf` n'émet AUCUN `Cache-Control`, ce qui laisse le
/// navigateur heuristiquer. Émettre une politique explicite est un
/// resserrement, pas un relâchement — l'inverse de l'`immutable` d'un an, qui
/// en était un vrai.
export const ENTETES_RESSOURCE_REVALIDABLE: Readonly<Record<string, string>> = Object.freeze({
    'X-Content-Type-Options': 'nosniff',
    'Cache-Control': 'public, max-age=0, must-revalidate',
});
