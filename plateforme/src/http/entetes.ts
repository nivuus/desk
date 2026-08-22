// Les deux en-têtes de sécurité que la PLATEFORME pose sur TOUTE RÉPONSE
// JSON — pas l'ensemble de ce qu'elle pose : depuis le lot « page derrière
// Pomerium » (22 août 2026), un second jeu existe pour le DOCUMENT qu'elle
// peut désormais servir (Content-Security-Policy, Referrer-Policy,
// X-Frame-Options), et vit dans `page/entetes-page.ts` — jamais ici.
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
//   proxy OU plateforme (le HTML)   | Content-Security-Policy | porte sur le
//                                   |                        | DOCUMENT — et
//                                   |                        | depuis le lot
//                                   |                        | « page derrière
//                                   |                        | Pomerium » (22
//                                   |                        | août 2026), la
//                                   |                        | plateforme PEUT
//                                   |                        | le servir : voir
//                                   |                        | page/entetes-page.ts
//   proxy                           | Strict-Transport-Security | c'est lui
//                                   |                        | qui termine TLS
//   proxy OU plateforme (le HTML)   | Referrer-Policy,       | idem — mêmes
//                                   | frame-ancestors        | raison et
//                                   |                        | module que la
//                                   |                        | CSP ci-dessus :
//                                   |                        | `frame-ancestors`
//                                   |                        | est UNE
//                                   |                        | directive DE
//                                   |                        | la CSP, pas un
//                                   |                        | en-tête séparé
//
// 🔴 POURQUOI `no-store` EST ICI ET NON CHEZ LE PROXY : les réponses de
// `/auth/*` PORTENT DES JETONS — le jeton d'accès et le jeton de
// rafraîchissement, en clair dans le corps JSON. Un cache intermédiaire, ou
// simplement le disque du navigateur, les retiendrait. C'est la plateforme qui
// sait laquelle de ses réponses porte un secret ; le proxy ne le sait pas.
//
// ⚠️ CE QUE CES DEUX EN-TÊTES NE FONT PAS, ET CE QUI A CHANGÉ : ils ne
// remplacent NI la CSP, NI HSTS, NI `frame-ancestors`. **HSTS reste
// exclusivement au proxy** — lui seul termine TLS (spec §9), et l'affirmer
// depuis une origine servie en clair serait une affirmation que la plateforme
// n'est pas en position de faire (voir `page/entetes-page.ts`). **CSP,
// `frame-ancestors` et `Referrer-Policy`, en revanche, NE restent PLUS
// exclusivement au proxy** depuis le lot « page derrière Pomerium » : la
// plateforme peut désormais servir la page elle-même, et pose alors ses
// propres en-têtes de document — `ENTETES_DOCUMENT` de `page/entetes-page.ts`,
// jamais `ENTETES_SECURITE` d'ici, réservé aux réponses JSON. Un déploiement
// sans proxy n'est donc PAS couvert par CE module (`entetes.ts`), mais peut
// l'être par `page/entetes-page.ts` pour tout ce qui dépend du document plutôt
// que de TLS.

export const ENTETES_SECURITE: Readonly<Record<string, string>> = Object.freeze({
    // Le navigateur ne DEVINE pas le type : une réponse JSON qu'un attaquant
    // ferait interpréter comme du HTML deviendrait un vecteur XSS.
    'X-Content-Type-Options': 'nosniff',
    // Voir l'en-tête : `/auth/*` rend des jetons. `no-store` est plus fort que
    // `no-cache`, qui autorise l'écriture sur disque à condition de
    // revalider.
    'Cache-Control': 'no-store',
});
