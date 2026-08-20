// La boucle du canal `/agent` : enrôlement, battement, jeton frais.
//
// C'est l'UNIQUE consommateur du protocole `plateforme`
// (`proto/ts/plateforme.ts`), et il ne recopie aucune forme de message : il
// appelle les encodeurs et le parseur du miroir. Une copie divergerait en
// silence, et `plateforme-vectors.json` ne comparerait plus rien de ce que la
// plateforme met réellement sur le fil.
//
// 🔴 CE CANAL NE PARTAGE RIEN AVEC LE RELAIS — ni garde, ni registre
// d'appartenance, ni observateur de session. C'est la conséquence directe
// d'E4 : l'enrôlement est ASYNCHRONE (il lit `agent_enrole` et dérive une
// empreinte `scrypt`), quand la garde du relais est PURE et SYNCHRONE. Les
// faire cohabiter obligerait l'un des deux à céder.
//
// 🔴 CE QUE LE CANAL DÉLIVRE EST EXACTEMENT CE QUE LA GARDE EXIGE, et les deux
// bouts doivent être lus ensemble (`identite/garde.ts`) :
//   - le jeton est de TYPE `agent` (claim `sty`), sans quoi la garde le refuse
//     pour le rôle `agent` ;
//   - son SUJET est le PRÉFIXE de la VM, parce que la garde exige que le sujet
//     préfixe le nom de session demandé.
// Un canal qui signerait l'identifiant de VM au lieu du préfixe délivrerait des
// jetons parfaitement valides que RIEN n'accepterait — panne muette de bout en
// bout, éprouvée pour cela par un test qui traverse les deux modules.
//
// ⚠️ AUCUNE PROMESSE N'EST ATTENDUE DANS UN GESTIONNAIRE `message`, ET AUCUNE
// N'EST LAISSÉE SANS `catch`. Une promesse rejetée dans un gestionnaire
// d'évènement `ws` abat tout le process Node — mode de défaillance que
// `signaling/relais.ts` et `signaling/trace.ts` documentent tous deux. Tout ce
// qui est asynchrone ici part par `void … .catch(…)`.

import type { WebSocket, WebSocketServer } from 'ws';
import {
    encodeBattementRecu,
    encodeEnrole,
    encodeRefus,
    parseVersLaPlateforme,
    type MotifCanal,
} from '../../../proto/ts/plateforme';
import type { Pilote } from '../base/pilote';
import { fusionner } from '../apps/catalogue';
import { marquerVu } from '../depot/agent';
import { appliquer, lireConnues } from '../depot/application';
import { DUREE_JETON_ACCES_MS, signer } from '../identite/jeton';
import { verifierEnrolement } from './enrolement';
import type { RegistreAgents } from './registre';

export interface OptionsCanal {
    base: Pilote;
    /// Le secret de SIGNATURE des jetons — jamais celui de l'enrôlement d'une
    /// VM, qui vit haché en base et ne quitte jamais `agent_enrole`.
    secretJeton: string;
    /// L'horloge est un PARAMÈTRE, jamais `Date.now()` lue ici : c'est ce qui
    /// rend l'expiration d'un jeton assertable sur une valeur EXACTE, et ce
    /// qui permet au test de voir un jeton frais succéder à un jeton mort.
    maintenant: () => number;
    /// Le registre des sockets d'agent vivants.
    ///
    /// 🔴 IL EST REQUIS, JAMAIS OPTIONNEL, et pour la raison exacte qui rend
    /// `base` requise dans `http/serveur.ts` : un canal qui n'inscrirait
    /// personne serait indiscernable du bon fonctionnement vu du pair — il
    /// s'enrôlerait, battrait, pousserait son catalogue, et TOUT lancement
    /// rendrait `agent-injoignable`. Panne muette, et de celles qu'on ne
    /// diagnostique qu'en lisant ce fichier.
    registre: RegistreAgents;
    dureeJetonMs?: number;
}

/// Les motifs qui FERMENT le socket, et ceux qui le laissent ouvert.
///
/// 🔴 `enrolement` ferme : un pair refusé qui garderait sa connexion pourrait
/// réessayer sans limite sur le même socket, ce qui est le déni de service que
/// P5 doit freiner. Fermer ne l'empêche pas de se reconnecter — cela lui en
/// fait payer le coût, et rend le nombre de tentatives comptable à l'étage
/// au-dessus le jour où on voudra le brider.
///
/// ⚠️ `version` ferme AUSSI, et c'est une décision de ce module que le plan ne
/// prescrivait pas : un pair qui ne parle pas notre version ne réussira JAMAIS
/// sur cette connexion. Le laisser ouvert le ferait boucler à plein régime, là
/// où la fermeture rend la main à sa reprise à repli exponentiel.
///
/// `forme` et `sequence` laissent le socket OUVERT : ce sont des erreurs dont
/// le pair peut se relever — un message mal formé se reprend, une séquence
/// inversée se corrige en s'enrôlant. Même partage que le relais, qui laisse
/// retenter un message malformé et ferme sur une poignée de main refusée.
const MOTIFS_FERMANTS: readonly MotifCanal[] = ['enrolement', 'version'];

/// Le code de fermeture WebSocket 1008 — « violation de politique ». C'est
/// celui du relais sur une poignée de main refusée : un pair qui lit les deux
/// canaux n'a pas à connaître deux conventions.
const FERMETURE_POLITIQUE = 1008;

export function servirLeCanalAgent(wss: WebSocketServer, options: OptionsCanal): void {
    const { base, secretJeton, maintenant, registre } = options;
    const dureeJetonMs = options.dureeJetonMs ?? DUREE_JETON_ACCES_MS;

    wss.on('connection', (socket: WebSocket) => {
        // L'état de CETTE connexion, et rien d'autre. Il naît vide : tant
        // qu'un enrôlement n'a pas abouti, ce pair n'est personne.
        let vmId: string | undefined;
        let prefixe: string | undefined;

        function refuser(motif: MotifCanal): void {
            // ⚠️ ENVOYER PUIS FERMER, jamais l'inverse : une fermeture qui
            // précèderait le message tronquerait le motif, et le pair verrait
            // sa connexion tomber sans savoir s'il doit se corriger, se mettre
            // à jour ou renoncer.
            envoyer(socket, encodeRefus(motif));
            if (MOTIFS_FERMANTS.includes(motif)) socket.close(FERMETURE_POLITIQUE, motif);
        }

        /// Signe un jeton d'agent et rend le couple (jeton, expiration).
        ///
        /// 🔴 L'INSTANT EST LU UNE SEULE FOIS et sert aux DEUX : signer avec un
        /// instant et annoncer une expiration calculée sur un autre ferait
        /// mentir `expire_a` de l'écart entre les deux lectures — un mensonge
        /// petit, permanent, et que rien ne rattraperait puisque l'agent croit
        /// l'annonce.
        function jetonNeuf(sujet: string): { jeton: string; expireA: number } {
            const instant = maintenant();
            return {
                jeton: signer(sujet, secretJeton, instant, dureeJetonMs, 'agent'),
                expireA: instant + dureeJetonMs,
            };
        }

        socket.on('message', (brut) => {
            const lecture = parseVersLaPlateforme(brut.toString());
            if (!lecture.ok) {
                refuser(lecture.motif);
                return;
            }

            if (lecture.message.type === 'battement') {
                if (vmId === undefined || prefixe === undefined) {
                    // 🔴 UN BATTEMENT N'AUTHENTIFIE PERSONNE. Y répondre
                    // `battement-recu` délivrerait un JETON D'AGENT à un pair
                    // qui n'a présenté aucun secret — c'est-à-dire la fuite que
                    // tout ce sous-bloc existe pour fermer, par une autre
                    // porte.
                    refuser('sequence');
                    return;
                }

                const instant = maintenant();
                // ⚠️ LANCÉE SANS ÊTRE ATTENDUE, avec son `catch` : le battement
                // est une OBSERVATION, jamais une dépendance du canal. Une base
                // momentanément indisponible ne doit pas abattre la connexion
                // d'un agent qui, lui, va très bien. Le coût est nommé : une
                // écriture perdue ne se voit qu'au journal.
                void marquerVu(base, vmId, instant).catch((cause) => {
                    console.error(`vu_a non avancé pour la VM ${vmId} : ${String(cause)}`);
                });

                const { jeton, expireA } = jetonNeuf(prefixe);
                envoyer(socket, encodeBattementRecu(jeton, expireA));
                return;
            }

            if (lecture.message.type === 'catalogue' || lecture.message.type === 'lancee') {
                if (vmId === undefined) {
                    // 🔴 NI CATALOGUE NI ISSUE SANS ENRÔLEMENT. Accepter un
                    // catalogue ici laisserait un pair anonyme ÉCRIRE DANS LA
                    // TABLE `application` d'une VM qu'il n'a pas authentifiée ;
                    // accepter une issue lui laisserait résoudre la demande
                    // d'un autre, et faire croire à un lancement qui n'a pas eu
                    // lieu. C'est le trou que le refus `sequence` du battement
                    // ferme déjà, par deux autres portes.
                    refuser('sequence');
                    return;
                }

                if (lecture.message.type === 'lancee') {
                    // ⚠️ SYNCHRONE, et rien à écrire : `resoudre` ne touche
                    // qu'une `Map` en mémoire, et IGNORE une demande inconnue
                    // plutôt que de lever (`agents/registre.ts`).
                    registre.resoudre(lecture.message.demande, lecture.message.issue);
                    return;
                }

                const message = lecture.message;
                const identifiant = vmId;
                const instant = maintenant();
                // ⚠️ LANCÉE SANS ÊTRE ATTENDUE, avec son `catch`, exactement
                // comme `marquerVu` ci-dessus et pour la même raison : un
                // `await` ici ferait qu'une base momentanément indisponible
                // ABATTRAIT LA CONNEXION d'un agent qui va très bien, et une
                // promesse rejetée sans `catch` abattrait tout le process.
                //
                // Le coût est nommé : un catalogue perdu ne se voit qu'au
                // journal. Il se rattrape tout seul — l'agent renvoie un état
                // COMPLET à chaque (ré)enrôlement, ce qui donne un terme à la
                // divergence sans que personne n'ait à réessayer.
                void lireConnues(base, identifiant)
                    .then((connues) => appliquer(base, identifiant, fusionner(connues, message), instant))
                    .catch((cause) => {
                        console.error(
                            `catalogue non écrit pour la VM ${identifiant} : ${String(cause)}`,
                        );
                    });
                return;
            }

            const { vm, secret } = lecture.message;
            // ⚠️ LE `catch` EST OBLIGATOIRE ET IL N'EST PAS DÉCORATIF :
            // `verifierEnrolement` LÈVE sur une empreinte écrite par une
            // version future du service (`identite/mot-de-passe.ts` refuse un
            // algorithme inconnu plutôt que de rendre un `false` indiscernable
            // d'un mauvais secret). Cette exception est traduite ICI en refus
            // `enrolement` — le même que tous les autres, pour ne rien
            // énumérer — et journalisée AVEC sa cause, qui ne porte jamais le
            // secret.
            void verifierEnrolement(base, vm, secret, (ligne) => console.warn(ligne))
                .then((verdict) => {
                    if (!verdict.ok) {
                        refuser(verdict.motif);
                        return;
                    }
                    vmId = verdict.vmId;
                    prefixe = verdict.prefixe;
                    // 🔴 C'EST ICI, ET NULLE PART AILLEURS, QUE LA VM DEVIENT
                    // JOIGNABLE. L'inscription suit l'authentification et ne la
                    // précède jamais : un pair qui n'a pas prouvé son secret ne
                    // doit pas pouvoir recevoir les ordres de lancement d'une
                    // VM. Le DERNIER inscrit gagne, et l'ancien socket est
                    // fermé (`agents/registre.ts`).
                    registre.inscrire(verdict.vmId, socket);

                    const instant = maintenant();
                    // ⚠️ L'ENRÔLEMENT AVANCE `vu_a` LUI AUSSI, et ce n'est pas
                    // une commodité : il EST un signe de vie, le premier. Sans
                    // lui, une VM qui vient de se connecter resterait `vu_a =
                    // null` — donc `injoignable` (`agents/fraicheur.ts`) —
                    // jusqu'à son premier battement, et une console la
                    // dirait éteinte alors qu'elle parle. La colonne garde tout
                    // son sens : `null` continue de dire « enrôlée par
                    // l'administrateur, jamais connectée depuis ».
                    void marquerVu(base, verdict.vmId, instant).catch((cause) => {
                        console.error(
                            `vu_a non posé à l'enrôlement de la VM ${verdict.vmId} : ${String(cause)}`,
                        );
                    });

                    const { jeton, expireA } = jetonNeuf(verdict.prefixe);
                    envoyer(socket, encodeEnrole(verdict.prefixe, jeton, expireA));
                })
                .catch((cause) => {
                    // Le nom de VM est journalisé, le secret jamais : il vient
                    // d'être refusé, le réécrire ailleurs n'aurait aucun sens
                    // et l'exposerait dans un fichier de traces.
                    console.error(`enrôlement en échec pour la VM ${vm} : ${String(cause)}`);
                    refuser('enrolement');
                });
        });

        socket.on('close', () => {
            // ⚠️ LE SOCKET EST PASSÉ, ET IL COMPTE. Un agent qui se relance
            // s'inscrit AVANT que la fermeture du précédent ne soit notifiée :
            // un retrait nu effacerait alors l'inscription du NEUF, et la VM
            // deviendrait injoignable au moment même où elle se reconnecte.
            //
            // Sans ce retrait, une VM morte resterait « joignable » jusqu'au
            // prochain enrôlement, et chaque lancement coûterait
            // `DELAI_LANCEMENT_MS` avant d'échouer sur un silence.
            if (vmId !== undefined) registre.retirer(vmId, socket);
        });
    });
}

/// N'écrit que sur un socket OUVERT. Un `send` sur un socket en cours de
/// fermeture lève, et cette exception traverserait le gestionnaire `message`.
function envoyer(socket: WebSocket, brut: string): void {
    if (socket.readyState === 1 /* OPEN */) socket.send(brut);
}
