// Le socle commun des sondes de la recette P4 : démarrer un service RÉEL,
// peupler sa base comme un administrateur le ferait, et frapper ses routes.
//
// 🔴 LE SERVICE EST LE VRAI, monté par `demarrer()` — la même séquence que
// `src/index.ts`. Rien n'est simulé côté plateforme : c'est du HTTP sur une
// socket, contre une vraie base, sur les deux moteurs. Ce que la sonde
// fabrique, ce sont les PAIRS (un navigateur scripté par `fetch`) et
// l'administrateur (qui crée des comptes et enrôle des VMs) — jamais le
// service lui-même.
//
// 🔴 AUCUNE DÉPENDANCE N'EST IMPORTÉE DEPUIS CE RÉPERTOIRE. `fetch` est global
// depuis Node 18 ; importer quoi que ce soit d'ici résoudrait le paquet de la
// RACINE du dépôt, donc une SECONDE instance de version potentiellement autre
// que celle que le service emploie.

import { lireConfig } from '../../../../../plateforme/src/config';
import { demarrer, type Service } from '../../../../../plateforme/src/demarrage';
import { ouvrirPostgres } from '../../../../../plateforme/src/base/pilote-postgres';
import type { Pilote } from '../../../../../plateforme/src/base/pilote';
import { creerUtilisateur } from '../../../../../plateforme/src/depot/utilisateur';
import { enrolerLaVm } from '../../../../../plateforme/src/admin/enroler-agent';
import { signer } from '../../../../../plateforme/src/identite/jeton';

export type Moteur = 'sqlite' | 'postgres';

/// FIXE et long : `lireConfig` refuse en dessous de `LONGUEUR_SECRET_MIN`, et
/// un secret tiré au sort rendrait les journaux non comparables d'une
/// exécution à l'autre pour rien. Il ne protège rien — le service ne vit que
/// le temps de la sonde, sur un port éphémère de la boucle locale.
export const SECRET_JETON = '***RETIRE-DE-L-HISTORIQUE***';

const URL_POSTGRES =
    process.env.PLATEFORME_BASE_URL ??
    'postgres://plateforme:plateforme-test@127.0.0.1:5433/plateforme_test';

/// Démarre un service NEUF sur le moteur demandé, port éphémère.
///
/// Postgres : un SCHÉMA jetable, comme `base/harnais.ts` le fait pour les
/// tests — deux exécutions successives ne doivent pas se marcher dessus.
export async function demarrerService(moteur: Moteur, nom: string): Promise<Service> {
    let urlBase = ':memory:';
    if (moteur === 'postgres') {
        const schema = `p4_${nom.replace(/[^a-z0-9]/gi, '_')}_${process.pid}`;
        const admin = ouvrirPostgres(URL_POSTGRES);
        await admin.executer(`DROP SCHEMA IF EXISTS ${schema} CASCADE`, []);
        await admin.executer(`CREATE SCHEMA ${schema}`, []);
        await admin.fermer();
        urlBase = `${URL_POSTGRES}?options=-c%20search_path%3D${schema}`;
    }
    const config = lireConfig({
        PLATEFORME_HOTE: '127.0.0.1',
        // 0 = port éphémère. `demarrerServeur` relit l'adresse réelle.
        PLATEFORME_PORT: '0',
        PLATEFORME_BASE: moteur,
        PLATEFORME_BASE_URL: urlBase,
        PLATEFORME_SECRET_JETON: SECRET_JETON,
    });
    return demarrer(config);
}

/// L'ordre de grandeur d'une époque en millisecondes. C'est la leçon la plus
/// chère de P1 : la double passe n'écrivait que des `1_000` et déclarait
/// portable un schéma que Postgres refusait pour toute écriture réelle.
export const INSTANT = 1_700_000_000_000;

/// Crée un compte. L'empreinte est un LITTÉRAL non dérivable : cette sonde ne
/// se connecte jamais par mot de passe, elle signe ses jetons directement.
export async function creerCompte(p: Pilote, email: string): Promise<string> {
    return creerUtilisateur(p, email, 'scrypt$16384$8$1$sel-de-recette$aucune-connexion-par-ce-chemin', INSTANT);
}

export interface VmEssai {
    vmId: string;
    prefixe: string;
    nom: string;
}

/// Enrôle une VM comme `npm run admin:agent` le ferait — MÊME fonction.
export async function enroler(p: Pilote, nom: string): Promise<VmEssai> {
    const e = await enrolerLaVm(p, nom, '192.168.3.2', INSTANT);
    return { vmId: e.vmId, prefixe: e.prefixe, nom };
}

/// Un jeton d'accès d'utilisateur, signé du MÊME secret que le service.
export function jetonDe(utilisateurId: string): string {
    return signer(utilisateurId, SECRET_JETON, Date.now());
}

/// Écrit `vu_a` à la valeur voulue. C'est ainsi que la sonde assiège la borne
/// de fraîcheur à travers le service RÉEL : `Date.now` n'y est pas injectable
/// (`http/serveur.ts` la passe telle quelle aux trois routeurs), mais la
/// DONNÉE l'est, et c'est la même soustraction qui est éprouvée.
export async function poserVuA(p: Pilote, vmId: string, vuA: number | null): Promise<void> {
    await p.executer('UPDATE agent_enrole SET vu_a = ? WHERE vm_id = ?', [vuA, vmId]);
}

export interface Reponse {
    code: number;
    corps: any;
    dureeMs: number;
}

/// Une requête HTTP RÉELLE contre le service, chronométrée.
export async function appeler(
    port: number,
    chemin: string,
    methode: string,
    jeton?: string,
): Promise<Reponse> {
    const debut = performance.now();
    const r = await fetch(`http://127.0.0.1:${port}${chemin}`, {
        method: methode,
        headers: jeton === undefined ? {} : { authorization: `Bearer ${jeton}` },
    });
    const corps = await r.json().catch(() => undefined);
    return { code: r.status, corps, dureeMs: performance.now() - debut };
}

export function ligne(cle: string, valeur: unknown): string {
    return `${cle.padEnd(46)}: ${typeof valeur === 'string' ? valeur : JSON.stringify(valeur)}`;
}

/// L'en-tête que TOUT journal de cette recette porte.
export function entete(titre: string, moteur: Moteur, commit: string): string {
    return [
        `# ${titre}`,
        `# Exécution ${moteur === 'sqlite' ? '1' : '2'} — moteur ${moteur}`,
        `# Jouée le ${new Date().toISOString()}, commit ${commit}`,
        '#',
    ].join('\n');
}

/// Le collecteur de verdicts. Il ne fait qu'une chose de plus qu'un `console.log` :
/// il RETIENT si l'un d'eux a échoué, ce qui donne son code de sortie à la sonde.
export class Verdicts {
    private toutTenu = true;
    dire(texte: string): void {
        process.stdout.write(`${texte}\n`);
    }
    /// ⚠️ UN VERDICT PORTE SON ATTENDU ET SON OBTENU, TOUJOURS. Un « TENU » nu
    /// ne se relit pas : on ne saurait pas ce qui a été comparé.
    juger(nom: string, obtenu: unknown, attendu: unknown): void {
        const ok = JSON.stringify(obtenu) === JSON.stringify(attendu);
        if (!ok) this.toutTenu = false;
        this.dire(ligne(nom, ok ? 'TENU' : `🔴 NON TENU — obtenu ${JSON.stringify(obtenu)}, attendu ${JSON.stringify(attendu)}`));
    }
    /// Pour ce qui se juge par une INÉGALITÉ (une durée sous une borne).
    jugerSous(nom: string, obtenu: number, borne: number): void {
        const ok = obtenu < borne;
        if (!ok) this.toutTenu = false;
        this.dire(ligne(nom, ok ? `TENU (${obtenu.toFixed(2)} < ${borne})` : `🔴 NON TENU — ${obtenu.toFixed(2)} ≥ ${borne}`));
    }
    conclure(): number {
        this.dire('');
        this.dire(ligne('VERDICT', this.toutTenu ? 'TOUT TENU' : '🔴 AU MOINS UNE ASSERTION NON TENUE'));
        return this.toutTenu ? 0 : 1;
    }
}
