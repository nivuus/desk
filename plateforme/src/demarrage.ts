// La séquence de démarrage du service, et son refus de démarrer dégradé.
//
// 🔴 L'ORDRE EST NON NÉGOCIABLE :
//     lireConfig -> ouvrirBase -> appliquerMigrations -> balayerLesOuvertes
//     -> demarrerServeur
//
// LE PORT NE S'OUVRE QU'EN DERNIER. Un pair ne doit jamais atteindre un
// service dont la base n'est pas prête : spec §6, « un signaling qui apparie
// sans rien enregistrer serait indiscernable du bon fonctionnement ». Le
// service REFUSE de démarrer, avec la cause, plutôt que de servir à moitié.
//
// Ce module est séparé de `index.ts` pour être testable : `index.ts` lit
// `process.env` et s'exécute à l'import, ce qu'un test ne peut pas faire
// plusieurs fois.

import type { Config } from './config';
import { appliquerMigrations, REPERTOIRE_MIGRATIONS } from './base/migrations';
import { ouvrirBase } from './base/ouvrir';
import type { Pilote } from './base/pilote';
import { balayerLesOuvertes } from './depot/session';
import { demarrerServeur, type ServicePlateforme } from './http/serveur';

export interface Service {
    port: number;
    base: Pilote;
    arreter(): Promise<void>;
}

export async function demarrer(config: Config, maintenant = Date.now()): Promise<Service> {
    let base: Pilote;
    try {
        base = await ouvrirBase(config);
        await appliquerMigrations(base, REPERTOIRE_MIGRATIONS, maintenant);
    } catch (cause) {
        // Aucun port n'a été ouvert à ce stade, et c'est le point : le rejet
        // laisse le service ENTIÈREMENT absent, jamais à moitié présent.
        throw new Error(`base injoignable ou migrations en échec : ${String(cause)}`, { cause });
    }

    const balayees = await balayerLesOuvertes(base, maintenant);
    if (balayees > 0) {
        console.log(`${balayees} session(s) restée(s) ouverte(s) closes au démarrage`);
    }

    // LE PORT NE S'OUVRE QU'ICI, après la base et ses migrations.
    let service: ServicePlateforme;
    try {
        service = await demarrerServeur(config, base);
    } catch (cause) {
        // La base est déjà ouverte : la refermer plutôt que de laisser une
        // connexion pendante derrière un démarrage avorté.
        await base.fermer();
        throw cause;
    }

    return {
        port: service.port,
        base,
        async arreter() {
            await service.close();
            await base.fermer();
        },
    };
}
