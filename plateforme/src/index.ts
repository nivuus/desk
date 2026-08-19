// Point d'entrée du service : une seule lecture d'environnement, la base
// d'abord, le port ensuite, un arrêt propre.
//
// La séquence elle-même vit dans `demarrage.ts`, qui est testable ; ce fichier
// n'est que le branchement sur `process.env` et sur les signaux.
//
// 🔴 COUPLAGE NOMMÉ, à ne pas casser par inadvertance : la ligne d'annonce
// ci-dessous doit contenir la sous-chaîne `le port <n>`.
// `src/signaling/resilience.test.ts` lance ce fichier comme processus enfant
// et lit son port par `output.match(/le port (\d+)/)` ; toute autre forme fait
// expirer le harnais au bout de 10 s sur « démarrage du process signaling
// expiré », sans que rien ne désigne la cause. Le couplage est écrit ici
// plutôt que subi, et le test le vérifie de lui-même en échouant.

import { lireConfig } from './config';
import { demarrer } from './demarrage';

const config = lireConfig(process.env);
const service = await demarrer(config);
console.log(`plateforme à l'écoute sur ${config.hote}, le port ${service.port}`);

process.on('SIGINT', async () => {
    await service.arreter();
    process.exit(0);
});
