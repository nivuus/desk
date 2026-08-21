// Le registre des sockets d'agent VIVANTS, et les demandes de lancement en vol.
//
// 🔴 POURQUOI IL EXISTE. `agents/canal.ts` tient son état dans la FERMETURE de
// la connexion (`vmId`, `prefixe`) : il n'existait, avant ce module, aucun
// moyen de retrouver le socket d'une VM donnée. `POST /application/:id/lancer`
// en a besoin — l'ordre part de la route HTTP et la réponse revient par le
// socket, deux chemins que rien ne reliait.
//
// 🔴 C'EST UN OBJET DE SERVICE, sur le patron exact de `ProprieteDeSession`
// (`signaling/propriete.ts`) : construit UNE FOIS dans `demarrerServeur`, passé
// à ses deux consommateurs, et vivant pour la durée du processus.
//
// ⚠️ IL A LE MÊME COÛT QUE `ProprieteDeSession`, ET IL EST ÉCRIT ICI POUR LA
// MÊME RAISON : IL NE SURVIT PAS À UN REDÉMARRAGE. Après un redémarrage, aucun
// agent n'y figure tant qu'il ne s'est pas ré-enrôlé, et tout lancement rend
// `agent-injoignable` — bruyamment, jamais en silence. Ce n'est pas rattrapable
// ici : un socket vit dans un processus et un seul. Le rattrapage réel est la
// reconnexion de l'agent, à repli exponentiel, qui reconstitue le registre
// sans que personne n'ait à le persister.
//
// ⚠️ AUCUNE PROMESSE N'EST LAISSÉE SANS ISSUE. Toute demande enregistrée ici
// finit résolue : par la réponse de l'agent, par la mort de son socket, ou par
// l'échéance. Une promesse qu'aucun chemin ne résout tiendrait une requête HTTP
// ouverte jusqu'au bout du monde.

import type { IssueLancement } from '../../../proto/ts/plateforme';
import { encodeLancer } from '../../../proto/ts/plateforme';

/// Le délai au bout duquel un ordre sans réponse est abandonné.
///
/// ⚠️ NON CALIBRÉE. Elle est majorante à vue : un `ShellExecuteExW` rend la
/// main sans attendre que l'application soit visible, donc l'agent répond en
/// quelques millisecondes dans le cas nominal. Cinq secondes couvrent une VM
/// chargée sans faire attendre un humain indéfiniment. Aucune mesure ne la
/// fonde, et le dire est plus honnête que de la présenter comme réglée.
export const DELAI_LANCEMENT_MS = 5_000;

/// Le code de fermeture WebSocket 1008, « violation de politique » — le même
/// que `agents/canal.ts` emploie sur un refus. Un pair qui lit les deux
/// canaux n'a pas à connaître deux conventions.
const FERMETURE_POLITIQUE = 1008;

/// Ce que le registre exige d'un socket, et RIEN DE PLUS.
///
/// 🔴 UNE FRONTIÈRE STRUCTURELLE, PAS UN TYPE `ws`. Un `ws.WebSocket` la
/// satisfait sans rien déclarer, et `registre.test.ts` peut passer un double
/// — ce qui rend les sept cas du registre jouables sans ouvrir un port. C'est
/// la même figure que l'horloge en paramètre : la dépendance est nommée au
/// lieu d'être importée.
export interface SocketAgent {
    /// 1 = OPEN, dans la convention `ws` comme dans celle du navigateur.
    readonly readyState: number;
    send(donnees: string): void;
    close(code?: number, raison?: string): void;
}

/// Ce que `lancer` peut rendre, en plus des issues du protocole.
///
/// ⚠️ `agent-injoignable` ET `delai` NE SONT PAS DES `IssueLancement`, et ne
/// doivent jamais le devenir : une `IssueLancement` est ce que l'AGENT a fait,
/// et ces deux-là disent que l'agent n'a rien dit du tout. Les confondre ferait
/// répondre « lancement échoué » là où la vérité est « on ne sait pas ».
export type EchecLancement = 'agent-injoignable' | 'delai';

interface EnVol {
    vmId: string;
    resoudre(issue: IssueLancement | EchecLancement): void;
}

export class RegistreAgents {
    /// La VM et son socket courant. Au plus UN par VM.
    private readonly sockets = new Map<string, SocketAgent>();
    /// Les ordres émis dont la réponse n'est pas encore arrivée, par demande.
    private readonly enVol = new Map<string, EnVol>();

    /// Inscrit le socket d'une VM. LE DERNIER GAGNE.
    ///
    /// 🔴 L'ANCIEN EST FERMÉ, jamais laissé à flotter. Deux sockets pour la
    /// même VM poseraient la question à laquelle la clé primaire
    /// d'`agent_enrole` répond déjà (`0003-agents.sql`) : « laquelle serait la
    /// bonne ? ». Le cas est ordinaire — un agent relancé se reconnecte avant
    /// que la fermeture de son ancien socket ne soit notifiée.
    inscrire(vmId: string, socket: SocketAgent): void {
        const ancien = this.sockets.get(vmId);
        this.sockets.set(vmId, socket);
        if (ancien !== undefined && ancien !== socket) {
            ancien.close(FERMETURE_POLITIQUE, 'remplace');
        }
    }

    /// Pousse un message brut vers l'agent d'une VM, et dit s'il est parti.
    ///
    /// 🔴 CE REGISTRE EST LE SEUL À CONNAÎTRE LES SOCKETS, donc le seul à
    /// pouvoir répondre « cette VM est-elle joignable À CET INSTANT ». Exposer
    /// la `Map` à la place aurait laissé chaque appelant refaire le test de
    /// `readyState`, et un seul l'aurait oublié.
    ///
    /// ⚠️ **`false` N'EST PAS UNE ERREUR** : il dit « aucun socket ouvert pour
    /// cette VM », qui est l'état ordinaire d'une VM éteinte. L'appelant décide
    /// quoi en faire — et pour l'ordre d'installation, la réponse est « rien » :
    /// la ligne reste `en_attente` en base, et `reemettreLesInstallations` la
    /// livrera au prochain enrôlement. C'est le filet qui existait déjà.
    pousser(vmId: string, brut: string): boolean {
        const socket = this.sockets.get(vmId);
        if (socket === undefined || socket.readyState !== 1) return false;
        socket.send(brut);
        return true;
    }

    /// Retire une VM et REJETTE ses demandes en vol.
    ///
    /// 🔴 LE REJET N'EST PAS UNE COMMODITÉ. Sans lui, la route attendrait
    /// `DELAI_LANCEMENT_MS` après une mort d'agent connue à l'instant même —
    /// cinq secondes pour une réponse dont on sait déjà qu'elle n'arrivera
    /// jamais.
    ///
    /// ⚠️ `socket` EST OPTIONNEL ET IL COMPTE : passé, le retrait n'a lieu que
    /// si c'est bien CE socket qui est inscrit. Un agent qui se relance
    /// s'inscrit AVANT que la fermeture du précédent ne soit notifiée ; un
    /// retrait nu effacerait alors l'inscription du NEUF, et la VM deviendrait
    /// injoignable alors qu'elle vient de se reconnecter.
    retirer(vmId: string, socket?: SocketAgent): void {
        const courant = this.sockets.get(vmId);
        if (socket !== undefined && courant !== socket) return;
        this.sockets.delete(vmId);
        for (const [demande, attente] of [...this.enVol]) {
            if (attente.vmId !== vmId) continue;
            this.enVol.delete(demande);
            attente.resoudre('agent-injoignable');
        }
    }

    /// Émet un ordre de lancement et attend son issue.
    ///
    /// 🔴 LE MESSAGE EST ENCODÉ PAR `proto/ts/plateforme.ts`, jamais recopié
    /// ici : une copie divergerait en silence, et `plateforme-vectors.json` ne
    /// comparerait plus rien de ce que la plateforme met réellement sur le fil.
    /// C'est la règle que `agents/canal.ts` s'impose depuis P3.
    ///
    /// ⚠️ L'APPARIEMENT SE FAIT SUR LA DEMANDE, JAMAIS SUR LA CLÉ. Deux
    /// lancements concurrents de la même application sont ordinaires — deux
    /// onglets du hub suffisent — et apparier sur la clé les mélangerait.
    lancer(
        vmId: string,
        cle: string,
        demande: string,
    ): Promise<IssueLancement | EchecLancement> {
        const socket = this.sockets.get(vmId);
        // Immédiat, et sans minuteur : faire attendre pour une réponse connue
        // d'avance serait payer cinq secondes pour rien.
        if (socket === undefined || socket.readyState !== 1 /* OPEN */) {
            return Promise.resolve('agent-injoignable');
        }

        return new Promise((resoudre) => {
            const minuteur = setTimeout(() => {
                this.enVol.delete(demande);
                resoudre('delai');
            }, DELAI_LANCEMENT_MS);
            // ⚠️ `unref` n'est PAS appelé : un minuteur détaché laisserait le
            // processus se terminer avec une requête HTTP en attente. Il est en
            // revanche TOUJOURS annulé, sur les deux autres chemins de sortie.
            this.enVol.set(demande, {
                vmId,
                resoudre: (issue) => {
                    clearTimeout(minuteur);
                    resoudre(issue);
                },
            });
            socket.send(encodeLancer(demande, cle));
        });
    }

    /// Délivre l'issue que l'agent a rapportée.
    ///
    /// 🔴 UNE DEMANDE INCONNUE EST IGNORÉE AVEC SA TRACE, JAMAIS UNE
    /// EXCEPTION. Son appelant est le gestionnaire `message` d'un socket, et
    /// une exception qui le traverse abat TOUT LE PROCESS Node — mode de
    /// défaillance que `relais.ts`, `trace.ts` et `canal.ts` documentent tous
    /// les trois. Le cas n'a d'ailleurs rien d'anormal : un agent peut
    /// répondre à un ordre dont l'attente a déjà expiré.
    resoudre(demande: string, issue: IssueLancement): void {
        const attente = this.enVol.get(demande);
        if (attente === undefined) {
            console.warn(
                `lancement sans attente : la demande ${demande} a rendu ${issue}, `
                    + `mais plus personne ne l'attendait (expiration, ou agent remplacé)`,
            );
            return;
        }
        this.enVol.delete(demande);
        attente.resoudre(issue);
    }
}
