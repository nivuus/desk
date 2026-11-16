// Service entry point: a single environment read, the database
// first, the port next, a clean shutdown.
//
// The sequence itself lives in `demarrage.ts`, which is testable; this file
// is only the wiring to `process.env` and to the signals.
//
// 🔴 NAMED COUPLING, not to be broken by accident: the announcement line
// below must contain the substring `le port <n>`.
// `src/signaling/resilience.test.ts` starts this file as a child process
// and reads its port via `output.match(/le port (\d+)/)`; any other shape makes
// the harness time out after 10 s on "signaling process startup
// timed out", without anything pointing at the cause. The coupling is written here
// rather than suffered, and the test checks it by itself by failing.

import { lireConfig } from './config';
import { start } from './demarrage';

const config = lireConfig(process.env);
const service = await start(config);

// 🔴 SIGTERM AS WELL AS SIGINT (audit 2026-11-16, D11): `systemctl stop`,
// `systemctl restart` and `docker stop` send SIGTERM, and without a handler the
// process died on the default action — sockets cut, sessions left open — so the
// clean path only ever ran under Ctrl-C. A second signal during the shutdown
// does not start another one.
let arret: Promise<void> | undefined;
function arreterSur(signal: NodeJS.Signals): void {
    if (arret !== undefined) return;
    console.log(`${signal} received: closing the platform`);
    arret = service.arreter().then(
        () => process.exit(0),
        (cause) => {
            console.error(`shutdown failed: ${String(cause)}`);
            process.exit(1);
        },
    );
}
process.on('SIGINT', arreterSur);
process.on('SIGTERM', arreterSur);

// ⚠️ ANNOUNCED ONLY ONCE THE HANDLERS ARE IN PLACE: a supervisor that reads this
// line as "ready" and stops the service at once must reach the clean path, not
// the default action of a signal nobody listens to yet.
console.log(`platform listening on ${config.hote}, port ${service.port}`);
