// La boucle du canal `/agent` : enrôlement, battement, jeton frais — et,
// depuis le sous-bloc G1, le CATALOGUE qui monte et les ORDRES DE LANCEMENT
// qui descendent.
//
// ⚠️ LE CANAL N'EST DONC PLUS SEULEMENT UN CANAL D'IDENTITÉ, et cette
// première ligne disait le contraire jusqu'au 20 août 2026. Trois variantes
// s'y sont ajoutées (`catalogue` et `lancee` montantes, `lancer` descendante),
// et `PLATEFORME_VERSION` est passée à 2 pour cela.
//
// C'est l'UNIQUE consommateur du protocole `plateforme`
// (`proto/ts/plateforme.ts`), et il ne recopie aucune forme de message : il
// appelle les encodeurs et le parseur du miroir. Une copie divergerait en
// silence, et `plateforme-vectors.json` ne comparerait plus rien de ce que la
// plateforme met réellement sur le fil.
//
// 🔴 CE CANAL NE PARTAGE RIEN AVEC LE RELAIS — ni garde, ni registre
// d'appartenance, ni observateur de session.
//
// ⚠️ NE PAS CONFONDRE DEUX REGISTRES DEPUIS G1. La phrase ci-dessus reste
// vraie du registre d'appartenance du RELAIS (`identite/garde.ts`), qu'il ne
// partage toujours pas. Mais ce canal en tient désormais un AUTRE, qui lui est
// propre : `agents/registre.ts`, la table des sockets d'agent vivants, sans
// laquelle `POST /application/:id/lancer` n'aurait aucun moyen de joindre la
// VM. Les deux ne se ressemblent que par le nom. C'est la conséquence directe
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

import type { IncomingMessage } from 'node:http';
import type { WebSocket, WebSocketServer } from 'ws';
import {
    encodeBattementRecu,
    encodeEnrole,
    encodeIconesManquantes,
    encodeRefus,
    parseVersLaPlateforme,
    type CatalogueMessage,
    type MotifCanal,
} from '../../../proto/ts/plateforme';
import type { Magasin } from '../apps/icones';
import type { Pilote } from '../base/pilote';
import { fusionner } from '../apps/catalogue';
import { marquerVu } from '../depot/agent';
import { appliquer, lireConnues } from '../depot/application';
import { DUREE_JETON_ACCES_MS, signer } from '../identite/jeton';
import { adresseSource } from '../http/adresse-source';
import { ligne as ligneDeJournal } from '../obs/journal';
import {
    BUDGET_ADRESSE,
    BUDGET_COMPTE,
    cleAdresse,
    cleVm,
    type Budget,
    type Frein,
} from '../securite/frein';
import { estMontantDeQuatre, reemettreLesInstallations, traiter } from './canal-apps';
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
    /// Le magasin d'icônes, interrogé après chaque `catalogue` pour savoir ce
    /// qui MANQUE. `undefined` = aucun inventaire n'est poussé.
    ///
    /// ⚠️ FACULTATIF, contrairement à `registre`, et l'asymétrie est dans les
    /// conséquences : un registre absent rend TOUT lancement injoignable —
    /// panne muette —, là qu'un magasin absent coûte seulement des icônes qui
    /// n'arrivent pas. Les tests du canal qui ne parlent pas d'icônes n'ont
    /// donc pas à en monter un.
    magasin?: Magasin;
    /// Le frein, PARTAGÉ avec les routes d'authentification — une seule table,
    /// jamais deux. Deux freins distincts divergeraient le jour où l'un serait
    /// durci, et leurs budgets d'ADRESSE s'additionneraient : un attaquant
    /// obtiendrait le double de ce que les constantes annoncent en alternant
    /// les deux portes.
    frein: Frein;
    /// Les proxys dont on croit l'en-tête `X-Forwarded-For`. VIDE par défaut.
    proxyDeConfiance: ReadonlySet<string>;
    dureeJetonMs?: number;
}

/// Les motifs qui FERMENT le socket, et ceux qui le laissent ouvert.
///
/// 🔴 `enrolement` ferme : un pair refusé qui garderait sa connexion pourrait
/// réessayer sans limite sur le même socket. Fermer ne l'empêche pas de se
/// reconnecter — cela lui en fait payer le coût, et rend le nombre de
/// tentatives comptable à l'étage au-dessus.
///
/// ✅ **CET ÉTAGE EXISTE DEPUIS P5, ET IL EST DANS CE FICHIER** : le frein est
/// consulté cent-soixante lignes plus bas, avant `verifierEnrolement`. Cette
/// phrase disait « le déni de service que P5 doit freiner … le jour où on
/// voudra le brider » : ce jour est arrivé, et le même fichier l'écrit en
/// toutes lettres à ce site-là. La fermeture reste ce qu'elle était — la
/// moitié gratuite —, et le frein est l'autre.
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
    const { base, secretJeton, maintenant, registre, frein, proxyDeConfiance, magasin } = options;
    const dureeJetonMs = options.dureeJetonMs ?? DUREE_JETON_ACCES_MS;

    // ⚠️ LA REQUÊTE DE MONTÉE EST DÉSORMAIS REÇUE, et c'est `ws` qui la
    // fournit : `http/serveur.ts` fait déjà `wss.emit('connection', client,
    // requete)`, et un `WebSocketServer` autonome la passe nativement. C'est
    // le seul endroit d'où l'adresse du pair soit lisible — un WebSocket, une
    // fois monté, ne la porte plus.
    wss.on('connection', (socket: WebSocket, requete?: IncomingMessage) => {
        // Lue UNE FOIS par connexion : elle ne change pas en cours de route,
        // et la relire à chaque message coûterait sans rien apprendre.
        const adresse = adresseSource(
            requete?.socket.remoteAddress,
            Array.isArray(requete?.headers['x-forwarded-for'])
                ? requete.headers['x-forwarded-for'].join(',')
                : requete?.headers['x-forwarded-for'],
            proxyDeConfiance,
        );
        const parAdresse: readonly [string, Budget] = [cleAdresse(adresse), BUDGET_ADRESSE];

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

            // 🔴 UNE GARDE DE TYPE, PLUS UNE CHUTE. `enroler` était le RESTE
            // d'un `if/else`, et l'élargissement de l'union par le sous-bloc G3
            // a fait de `progression` et `termine` deux membres de ce reste —
            // donc deux messages que la destructuration ci-dessous aurait lus
            // comme un enrôlement. `tsc` l'a dit, et il a eu de la chance : la
            // même fragilité sur une valeur plutôt qu'un type serait passée en
            // silence. Le prédicat NOMME les quatre types de ④.
            if (estMontantDeQuatre(lecture.message)) {
                if (vmId === undefined) {
                    // 🔴 NI CATALOGUE, NI ISSUE, NI PROGRESSION SANS
                    // ENRÔLEMENT. Accepter un catalogue ici laisserait un pair
                    // anonyme ÉCRIRE DANS LA TABLE `application` d'une VM qu'il
                    // n'a pas authentifiée ; accepter une issue lui laisserait
                    // résoudre la demande d'un autre, et faire croire à un
                    // lancement qui n'a pas eu lieu ; accepter une progression
                    // ou un `termine` lui laisserait écrire dans la table
                    // `installation` d'une VM qui n'est pas la sienne — et donc
                    // déclarer réussie, ou refusée, l'installation d'autrui.
                    // C'est le trou que le refus `sequence` du battement ferme
                    // déjà, par quatre autres portes.
                    refuser('sequence');
                    return;
                }
                traiter(
                    { base, registre, magasin, socket, vmId, maintenant, envoyer: (brut) => envoyer(socket, brut) },
                    lecture.message,
                );
                return;
            }

            const { vm, secret } = lecture.message;

            // 🔴 LE FREIN EST CONSULTÉ ICI, ET C'EST LA POSITION QUI COMPTE :
            // AVANT `verifierEnrolement`, donc avant qu'il ne lise
            // `agent_enrole` ET avant qu'il ne dérive une empreinte `scrypt`.
            // `scrypt` est à mémoire dure et coûte délibérément cher (68 ms
            // mesurés le 20 août 2026) : un attaquant qui le déclenche à
            // volonté épuise le service sans jamais deviner un secret. UN
            // FREIN POSTÉ APRÈS LA VÉRIFICATION NE PROTÈGE RIEN.
            //
            // Ce fichier annonçait ce jour depuis P3, dans le commentaire de
            // `MOTIFS_FERMANTS` : « fermer ne l'empêche pas de se reconnecter
            // — cela rend le nombre de tentatives comptable à l'étage
            // au-dessus le jour où on voudra le brider ». P5 est ce jour-là.
            const cles: readonly (readonly [string, Budget])[] = [
                [cleVm(vm), BUDGET_COMPTE],
                parAdresse,
            ];
            if (frein.consulter(cles, maintenant()).freine) {
                // 🔴 LE MOTIF EST `enrolement`, ET RIEN D'AUTRE. Un motif
                // `frein` distinct rendrait à l'attaquant l'information
                // « cette VM existe et je l'ai fait déclencher » : c'est
                // l'ORACLE D'ÉNUMÉRATION que `agents/enrolement.ts` ferme sur
                // trois paragraphes, rouvert par la porte du frein. Le
                // JOURNAL, lui, distingue les deux — même partage que
                // `identite/garde.ts` (`message` sur le fil, `journal` chez
                // nous).
                journaliserLeFrein(frein, cles, adresse, maintenant());
                refuser('enrolement');
                return;
            }
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
                        compterLEchec(frein, cles, adresse, maintenant());
                        refuser(verdict.motif);
                        return;
                    }
                    // 🔴 LE SUCCÈS EFFACE LA CLÉ DE LA VM, JAMAIS CELLE DE
                    // L'ADRESSE — même règle que `/auth/connexion`. L'effacer
                    // aussi blanchirait un attaquant qui possède une VM
                    // valide : il lui suffirait de s'enrôler entre deux
                    // rafales pour rendre son budget d'adresse à zéro.
                    frein.succes(cleVm(verdict.vmId));
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

                    // 🔴 LA RÉÉMISSION DES INSTALLATIONS EN ATTENTE. Un `push`
                    // WebSocket n'a AUCUNE garantie de livraison : sans elle,
                    // un ordre émis pendant une coupure serait perdu SANS
                    // TERME, et l'utilisateur attendrait une installation que
                    // personne ne relancerait jamais. C'est le même filet que
                    // le `complet = true` du catalogue, et la recette de G1 a
                    // vu ce filet-là fonctionner sur le chemin réel.
                    //
                    // ⚠️ ELLE NE VISE QUE LES `en_attente`, ET C'EST LA
                    // PREMIÈRE DES DEUX CEINTURES CONTRE UNE DOUBLE
                    // EXÉCUTION : dès qu'un agent a rapporté une progression,
                    // la ligne passe `en_cours` et cesse d'être réémise. La
                    // seconde ceinture est le marqueur sur le disque de la VM,
                    // et elle protège du cas où la première a perdu sa base.
                    //
                    // ⚠️ `void … .catch(…)`, JAMAIS `await` : un enrôlement
                    // parfaitement valide ne doit pas échouer parce que la
                    // base est momentanément indisponible, et une promesse
                    // rejetée sans `catch` abattrait tout le process Node.
                    void reemettreLesInstallations(
                        { base, socket, vmId: verdict.vmId, envoyer: (brut) => envoyer(socket, brut) },
                    ).catch((cause) => {
                        console.error(
                            `réémission des installations impossible pour la VM `
                                + `${verdict.vmId} : ${String(cause)}`,
                        );
                    });
                })
                .catch((cause) => {
                    // Le nom de VM est journalisé, le secret jamais : il vient
                    // d'être refusé, le réécrire ailleurs n'aurait aucun sens
                    // et l'exposerait dans un fichier de traces.
                    console.error(`enrôlement en échec pour la VM ${vm} : ${String(cause)}`);
                    // ⚠️ CE CHEMIN COMPTE AUSSI. Une empreinte écrite par une
                    // version future du service fait LEVER `verifier` : sans
                    // ce comptage, un attaquant qui trouverait de quoi la
                    // faire lever aurait un chemin de coût plein et non
                    // freiné.
                    compterLEchec(frein, cles, adresse, maintenant());
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

/// Enregistre l'échec, et journalise SI ET SEULEMENT SI le frein vient de
/// mordre.
///
/// 🔴 POURQUOI PAS UNE LIGNE PAR REFUS — même raison qu'`http/routes-auth.ts`,
/// et elle est plus mordante ici : une tentative d'enrôlement refusée FERME le
/// socket, si bien qu'un attaquant en boucle ouvre une connexion par
/// tentative. Une trace par refus ferait donc écrire une ligne par connexion,
/// sur le chemin même que le frein vient de rendre gratuit. `CLAUDE.md` porte
/// la règle depuis le chantier TURN : « compter ou échantillonner, jamais
/// tracer par paquet ».
///
/// La transition est détectée en reconsultant APRÈS l'échec : la tentative
/// suivante est refusée AVANT d'atteindre `verifierEnrolement`, donc n'appelle
/// jamais cette fonction. Il y a EXACTEMENT une ligne par clé et par fenêtre.
function compterLEchec(
    frein: Frein,
    cles: readonly (readonly [string, Budget])[],
    adresse: string,
    instant: number,
): void {
    frein.echec(cles, instant);
    if (!frein.consulter(cles, instant).freine) return;
    journaliserLeFrein(frein, cles, adresse, instant);
}

/// La ligne que l'exploitant lit, et que le demandeur ne verra jamais.
///
/// ⚠️ ELLE NOMME L'ADRESSE RETENUE, et c'est le SEUL remède au mode de
/// défaillance de `http/adresse-source.ts` : un exploitant qui a posé un proxy
/// sans déclarer sa confiance verra ici l'adresse de son proxy sur toutes les
/// lignes, et comprendra que son frein par adresse est devenu GLOBAL.
function journaliserLeFrein(
    frein: Frein,
    cles: readonly (readonly [string, Budget])[],
    adresse: string,
    instant: number,
): void {
    const verdict = frein.consulter(cles, instant);
    console.warn(
        ligneDeJournal('frein', {
            route: '/agent',
            adresse,
            cles: cles.map(([cle]) => cle).join(' '),
            retry_apres_s: verdict.retryApresS,
            // Sans ces deux-là, la SATURATION du frein serait invisible : sous
            // saturation une éviction rend son budget à une VM visée.
            entrees: frein.taille(),
            evictions: frein.evictions(),
        }),
    );
}

/// N'écrit que sur un socket OUVERT. Un `send` sur un socket en cours de
/// fermeture lève, et cette exception traverserait le gestionnaire `message`.
function envoyer(socket: WebSocket, brut: string): void {
    if (socket.readyState === 1 /* OPEN */) socket.send(brut);
}
