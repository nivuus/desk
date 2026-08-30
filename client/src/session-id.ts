// L'identifiant de session, tiré des paramètres de l'URL — PUR, sans DOM,
// donc éprouvable sur l'hôte (la convention de `viewportPair`, `texteLien`,
// `adresseSignaling` : `main.ts` délègue la RÈGLE, il ne la porte pas).
//
// 🔴 PLUS AUCUN REPLI SUR `'demo'` — TROUVÉ EN PRODUCTION LE 30 AOÛT 2026.
// `https://app.allanic.me/` servait alors `index.html` (la page de session,
// avant que la racine ne serve le hub — voir
// `plateforme/src/http/page/resolution.ts::PAGE`), et sans paramètre
// `?session=`, l'ancienne ligne de `main.ts` — `params.get('session') ??
// 'demo'` — INVENTAIT une session `demo`, sans jeton. L'agent journalisait
// alors « poignée de main refusée : poignée de main sans jeton sur la
// session demo », et le propriétaire voyait « Échec de la session » : une
// panne INDISCERNABLE d'une vraie pour qui ouvre juste l'adresse du
// service. `'demo'` était un vestige du temps où il n'y avait qu'UNE
// session — voir `CLAUDE.md`, « il y a N SESSIONS, pas N pistes dans une
// session ».
//
// Une ABSENCE reste donc une absence : `undefined`, jamais une valeur
// inventée. C'est `main.ts` qui décide quoi faire de cette absence (refuser
// et le dire), pas cette fonction.
export function sessionIdDepuisParametres(params: URLSearchParams): string | undefined {
    return params.get('session') ?? undefined;
}
