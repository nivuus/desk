// THE SERVICE THE BROWSER CORROBORATION DRIVES — a real service, a
// real database, three users in three situations.
//
//     tsx corroboration-navigateur.ts <port> <origine-client>
//
// 🔴 WHY THIS PIECE EXISTS ALTHOUGH TASK 14 HAS NO TEST. It
// has none by construction: `connexion.ts` is DOM wiring, and the
// directory's convention wants what lives there to be wiring or nothing.
// But two CORS defects were found in this same sub-block — the
// `Authorization` header not allowed, and the `OPTIONS` preflight request not handled —
// and NO Node test could see them: they only show up under
// a real browser's origin policy. The class "what a browser
// requires and a server test does not see" is therefore OPEN, and task 14
// is precisely on the browser side.
//
// ⚠️ IT IS NOT A CRITERION OF THE ACCEPTANCE RUN, and it is not the
// corroboration on a real VM of task 16 either: no Windows VM is
// powered on, no agent beats. It is a WIRING corroboration, on the
// three outcomes `connexion.ts` distinguishes.

import { hacher } from '../../../../../plateforme/src/identite/mot-de-passe';
import { createUser } from '../../../../../plateforme/src/depot/utilisateur';
import { lireConfig } from '../../../../../plateforme/src/config';
import { start } from '../../../../../plateforme/src/demarrage';
import { inventaireStatique } from '../../../../../plateforme/src/orchestration/inventaire-statique';
import { enroler, INSTANT, poserVuA, SECRET_JETON } from './socle';
import { SEUIL_INJOIGNABLE_MS } from '../../../../../plateforme/src/agents/fraicheur';

const [port, origine] = process.argv.slice(2);
const MDP = 'mot-de-passe-de-recette-p4';

const service = await start(
    lireConfig({
        PLATEFORME_HOTE: '127.0.0.1',
        PLATEFORME_PORT: port,
        PLATEFORME_BASE: 'sqlite',
        PLATEFORME_BASE_URL: ':memory:',
        PLATEFORME_SECRET_JETON: SECRET_JETON,
        PLATEFORME_ORIGINE_CLIENT: origine,
    }),
);

const empreinte = await hacher(MDP);
const orch = inventaireStatique(service.base, Date.now);

// ① A user WHO HAS A VM ASSIGNED, and whose agent beats.
const prete = await createUser(service.base, 'prete@essai.local', empreinte, INSTANT);
const vmPrete = await enroler(service.base, 'w-prete');
await orch.attribuer(vmPrete.vmId, prete);
await poserVuA(service.base, vmPrete.vmId, Date.now());

// ② A user WITHOUT any VM.
await createUser(service.base, 'sansvm@essai.local', empreinte, INSTANT);

// ③ A user whose VM is there but whose agent has gone QUIET.
const muette = await createUser(service.base, 'muette@essai.local', empreinte, INSTANT);
const vmMuette = await enroler(service.base, 'w-muette');
await orch.attribuer(vmMuette.vmId, muette);
await poserVuA(service.base, vmMuette.vmId, Date.now() - SEUIL_INJOIGNABLE_MS - 1);

process.stdout.write(
    `${JSON.stringify({ port: service.port, origine, prefixePrete: vmPrete.prefixe, prefixeMuette: vmMuette.prefixe, mdp: MDP })}\n`,
);
// The service stays up until the signal: it is the driver that decides.
