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
export const CSP =
    "default-src 'self'; connect-src 'self' wss: https:; img-src 'self' data: blob:; " +
    "media-src 'self' blob:; script-src 'self'; style-src 'self' 'unsafe-inline'; " +
    "font-src 'self'; frame-ancestors 'none'; base-uri 'self'; form-action 'self'";

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
