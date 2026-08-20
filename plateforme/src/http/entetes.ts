// Les deux en-têtes de sécurité que la PLATEFORME pose, et rien de plus.
//
// 🔴 CE MODULE EST SÉPARÉ DE `cors.ts`, ET LA SÉPARATION EST LE POINT.
// `entetesCors` rend `undefined` quand l'origine n'est pas autorisée ; ceux-ci
// sont INCONDITIONNELS. Les fusionner ferait dépendre la sécurité d'une
// configuration CORS FACULTATIVE (`PLATEFORME_ORIGINE_CLIENT` n'est pas
// obligatoire, voir `config.ts`) — c'est-à-dire qu'un déploiement sur une
// origine unique, où aucun en-tête CORS n'a de sens, perdrait aussi ses
// en-têtes de sécurité, sans que rien ne le dise.
//
// LE PARTAGE AVEC LE PROXY, et il suit ce que chacun SERT :
//
//   plateforme (toute réponse JSON) | X-Content-Type-Options | elle seule
//                                   |                        | connaît son
//                                   |                        | type de contenu
//   plateforme (toute réponse JSON) | Cache-Control          | voir ci-dessous
//   proxy (le HTML)                 | Content-Security-Policy | porte sur le
//                                   |                        | DOCUMENT, que
//                                   |                        | la plateforme
//                                   |                        | ne sert pas
//   proxy                           | Strict-Transport-Security | c'est lui
//                                   |                        | qui termine TLS
//   proxy                           | Referrer-Policy,       | idem
//                                   | frame-ancestors        |
//
// 🔴 POURQUOI `no-store` EST ICI ET NON CHEZ LE PROXY : les réponses de
// `/auth/*` PORTENT DES JETONS — le jeton d'accès et le jeton de
// rafraîchissement, en clair dans le corps JSON. Un cache intermédiaire, ou
// simplement le disque du navigateur, les retiendrait. C'est la plateforme qui
// sait laquelle de ses réponses porte un secret ; le proxy ne le sait pas.
//
// ⚠️ CE QUE CES DEUX EN-TÊTES NE FONT PAS : ils ne remplacent NI la CSP, NI
// HSTS, NI `frame-ancestors`, qui restent au proxy. Un déploiement sans proxy
// n'est donc PAS couvert par ce module — et il n'est pas couvert non plus par
// le reste de P5, qui ne termine jamais TLS (spec §9).

export const ENTETES_SECURITE: Readonly<Record<string, string>> = Object.freeze({
    // Le navigateur ne DEVINE pas le type : une réponse JSON qu'un attaquant
    // ferait interpréter comme du HTML deviendrait un vecteur XSS.
    'X-Content-Type-Options': 'nosniff',
    // Voir l'en-tête : `/auth/*` rend des jetons. `no-store` est plus fort que
    // `no-cache`, qui autorise l'écriture sur disque à condition de
    // revalider.
    'Cache-Control': 'no-store',
});
