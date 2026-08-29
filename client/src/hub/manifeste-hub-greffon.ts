// La FORME de la balise que le greffon Vite `guac-manifeste-hub` injecte dans
// `hub.html` — extraite de `client/vite.config.ts` pour la MÊME raison que
// `design/amorce-theme-greffon.ts` : RESTER TYPECHECKÉE. `vite.config.ts`
// n'est jamais typechecké (hors des deux motifs de `client/tsconfig.json:12`),
// et un défaut sur la forme de cette balise ne serait donc jamais vu par
// `npx tsc --noEmit` s'il restait écrit là-bas.
//
// 🔴 LE DÉFAUT QUE CE FICHIER CORRIGE, mesuré le 29 août 2026
// (`https://app.allanic.me/hub.html`, Chrome, console du propriétaire) :
//
//   Loading a manifest from 'https://authenticate.allanic.me/.pomerium/
//   sign_in?...&pomerium_redirect_uri=…%2Fhub.webmanifest&...' violates the
//   following Content Security Policy directive: "default-src 'self'".
//
// Un `<link rel="manifest">` SANS `crossorigin` est allé chercher par le
// navigateur SANS les cookies de session (c'est la règle HTML : un lien de
// manifeste ne voyage en mode `credentials: same-origin` QUE s'il porte
// `crossorigin="use-credentials"` — l'absence de l'attribut vaut
// `crossorigin="anonymous"`, donc AUCUN cookie). Pomerium, recevant une
// requête non authentifiée sur une route par ailleurs protégée
// (`config.yaml` : seul `email: maxime.g.allanic@gmail.com` ou les extensions
// `.ico`/`.png`/`manifest.json` passent), répond par une redirection vers son
// propre domaine `authenticate.allanic.me` — une AUTRE origine, que
// `default-src 'self'` (et `manifest-src 'self'`, désormais explicite —
// `entetes-page.ts::CSP`) refuse de charger. Le message de CSP est le
// SYMPTÔME ; la redirection non authentifiée est la CAUSE.
//
// 🔴 POURQUOI `crossorigin="use-credentials"` PLUTÔT QU'OUVRIR LA ROUTE DANS
// POMERIUM : le manifeste du hub reste comportement PRIVÉ — aucune entrée
// nouvelle dans la politique `ends_with: …` de `config.yaml`, aucun trou. Le
// navigateur envoie désormais les cookies avec la requête de manifeste,
// exactement comme il le fait pour `hub.html` lui-même : Pomerium authentifie
// la requête normalement et sert `/hub.webmanifest`, qui répond `200`
// (vérifié : `curl http://192.168.3.1:3445/hub.webmanifest`).
export function baliseManifesteHub() {
    return {
        tag: 'link' as const,
        attrs: { rel: 'manifest', href: '/hub.webmanifest', crossorigin: 'use-credentials' },
        injectTo: 'head' as const,
    };
}
