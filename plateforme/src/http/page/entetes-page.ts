// Les en-têtes que la plateforme pose sur ce qu'elle SERT comme page.
//
// 🔴 CE MODULE EXISTE PARCE QUE `../entetes.ts` NE POUVAIT PAS SERVIR ICI, et
// la raison est une inversion, pas un manque : son `Cache-Control: no-store`
// est INCONDITIONNEL, et il est là pour les réponses de `/auth/*`, qui portent
// des jetons en clair. Les ressources ont le besoin EXACTEMENT OPPOSÉ — des
// noms empreintés par Vite, cachables un an. Réutiliser `ENTETES_SECURITE` tel
// quel est le geste naturel, et c'est le défaut.
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

export const ENTETES_RESSOURCE: Readonly<Record<string, string>> = Object.freeze({
    'X-Content-Type-Options': 'nosniff',
    'Cache-Control': 'public, max-age=31536000, immutable',
});
