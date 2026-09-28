// The harness for the AMBIENT TURN variables, for the test files that
// open a real relay.
//
// 🔴 WHY IT EXISTS. `relais.ts` reads `process.env` at EACH peer
// declaration (`configurationIce(process.env, …)`) and, if `TURN_URL` and `TURN_SECRET`
// are both set, sends an `ice-config` to the peer BEFORE any other
// message. A test that reads "the next message" then receives this
// `ice-config` in place of what it expected. The consequence is not
// theoretical: `scripts/verify-all.sh` launched from a shell where one had run
// `set -a && source .env && set +a` — the sequence that `CLAUDE.md` prescribes
// for all the rest of the repository — made SIX tests of
// `server.test.ts` fail, when the same script from a bare shell exited 0.
// The service was healthy, the tests were measuring the developer's environment.
//
// 🔴 AND WHY WE DO NOT RESTORE "BY HAND". The naive restoration
//
//     const prior = process.env.TURN_URL;   // undefined if absent
//     …
//     process.env.TURN_URL = prior;         // ⚠️ writes the STRING "undefined"
//
// does not return the variable to its absence: `process.env` coerces everything to a string,
// and `"undefined"` is TRUTHY. A variable "restored" this way thus makes the relay
// deliver an ICE configuration whose URL is the word `undefined` — measured.
// Only `delete` makes a variable absent.

/// The two variables that `configurationIce` reads, and only those.
const CLES = ['TURN_URL', 'TURN_SECRET'] as const;

export interface TurnAmbiant {
    url?: string;
    secret?: string;
}

/// Sets the requested ambient TURN state and returns the function that restores the
/// previous state — `delete` included, for the keys that were absent.
///
/// Called without an argument (or with `{}`), it NEUTRALISES: the relay then behaves
/// as on a machine with no TURN server configured, which is the
/// only way for a "next message" test to be sealed off from
/// the environment of whoever runs it.
export function poserTurnAmbiant(values: TurnAmbiant = {}): () => void {
    const before = CLES.map((cle) => [cle, process.env[cle]] as const);

    appliquer('TURN_URL', values.url);
    appliquer('TURN_SECRET', values.secret);

    return () => {
        for (const [cle, value] of before) appliquer(cle, value);
    };
}

function appliquer(cle: (typeof CLES)[number], value: string | undefined): void {
    if (value === undefined) delete process.env[cle];
    else process.env[cle] = value;
}
