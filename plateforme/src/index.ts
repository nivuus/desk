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
import { demarrer } from './demarrage';

const config = lireConfig(process.env);
const service = await demarrer(config);
console.log(`plateforme à l'écoute sur ${config.hote}, le port ${service.port}`);

process.on('SIGINT', async () => {
    await service.arreter();
    process.exit(0);
});
