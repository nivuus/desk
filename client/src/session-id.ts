// The session identifier, taken from the URL parameters — PURE, DOM-free,
// hence testable on the host (the convention of `viewportPair`, `texteLien`,
// `adresseSignaling`: `main.ts` delegates the RULE, it does not carry it).
//
// 🔴 NO MORE FALLBACK TO `'demo'` — FOUND IN PRODUCTION ON AUGUST 30TH, 2026.
// `https://app.allanic.me/` then served `index.html` (the session page,
// before the root served the hub — see
// `plateforme/src/http/page/resolution.ts::PAGE`), and without a
// `?session=` parameter, the old line of `main.ts` — `params.get('session') ??
// 'demo'` — INVENTED a `demo` session, without a token. The agent then logged
// "handshake refused: handshake without a token on
// session demo", and the owner saw "Session failed": a
// failure INDISTINGUISHABLE from a real one for whoever just opens the service's
// address. `'demo'` was a vestige of the time when there was only ONE
// session — see `CLAUDE.md`, "there are N SESSIONS, not N tracks in one
// session".
//
// An ABSENCE therefore stays an absence: `undefined`, never an invented
// value. It is `main.ts` that decides what to do with that absence (refuse
// and say so), not this function.
export function sessionIdDepuisParametres(params: URLSearchParams): string | undefined {
    return params.get('session') ?? undefined;
}
