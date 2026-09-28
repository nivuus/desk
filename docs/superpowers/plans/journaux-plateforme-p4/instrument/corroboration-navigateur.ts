// LE SERVICE QUE LA CORROBORATION NAVIGATEUR PILOTE — un vrai service, une
// vraie base, trois utilisateurs dans trois situations.
//
//     tsx corroboration-navigateur.ts <port> <origine-client>
//
// 🔴 POURQUOI CETTE PIÈCE EXISTE ALORS QUE LA TÂCHE 14 N'A PAS DE TEST. Elle
// n'en a pas par construction : `connexion.ts` est du câblage DOM, et la
// convention du répertoire veut que ce qui s'y trouve soit du câblage ou rien.
// Mais deux défauts CORS ont été trouvés dans ce même sous-bloc — l'en-tête
// `Authorization` non permis, et la requête préalable `OPTIONS` non traitée —
// et AUCUN test de Node ne pouvait les voir : ils ne se manifestent que sous
// la politique d'origine d'un vrai navigateur. La classe « ce qu'un navigateur
// exige et qu'un test serveur ne voit pas » est donc OUVERTE, et la tâche 14
// est précisément côté navigateur.
//
// ⚠️ CE N'EST PAS UN CRITÈRE DE LA RECETTE, et ce n'est pas non plus la
// corroboration sur VM réelle de la tâche 16 : aucune VM Windows n'est
// allumée, aucun agent ne bat. C'est une corroboration de CÂBLAGE, sur les
// trois issues que `connexion.ts` distingue.

import { hacher } from '../../../../../plateforme/src/identite/mot-de-passe';
import { creerUtilisateur } from '../../../../../plateforme/src/depot/utilisateur';
import { lireConfig } from '../../../../../plateforme/src/config';
import { demarrer } from '../../../../../plateforme/src/demarrage';
import { inventaireStatique } from '../../../../../plateforme/src/orchestration/inventaire-statique';
import { enroler, INSTANT, poserVuA, SECRET_JETON } from './socle';
import { SEUIL_INJOIGNABLE_MS } from '../../../../../plateforme/src/agents/fraicheur';

const [port, origine] = process.argv.slice(2);
const MDP = 'mot-de-passe-de-recette-p4';

const service = await demarrer(
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

// ① Un utilisateur À QUI UNE VM EST ATTRIBUÉE, et dont l'agent bat.
const prete = await creerUtilisateur(service.base, 'prete@essai.local', empreinte, INSTANT);
const vmPrete = await enroler(service.base, 'w-prete');
await orch.attribuer(vmPrete.vmId, prete);
await poserVuA(service.base, vmPrete.vmId, Date.now());

// ② Un utilisateur SANS aucune VM.
await creerUtilisateur(service.base, 'sansvm@essai.local', empreinte, INSTANT);

// ③ Un utilisateur dont la VM est là mais dont l'agent s'est TU.
const muette = await creerUtilisateur(service.base, 'muette@essai.local', empreinte, INSTANT);
const vmMuette = await enroler(service.base, 'w-muette');
await orch.attribuer(vmMuette.vmId, muette);
await poserVuA(service.base, vmMuette.vmId, Date.now() - SEUIL_INJOIGNABLE_MS - 1);

process.stdout.write(
    `${JSON.stringify({ port: service.port, origine, prefixePrete: vmPrete.prefixe, prefixeMuette: vmMuette.prefixe, mdp: MDP })}\n`,
);
// Le service reste debout jusqu'au signal : c'est le pilote qui décide.
