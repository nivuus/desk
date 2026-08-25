// Le serveur HTTP du service, et le routage de la montée WebSocket.
//
// `noServer` plutôt que `{ server }` : le routage du chemin est explicite.
// ✅ P3 Y A AJOUTÉ `/agent` SANS TOUCHER AU RELAIS — la branche d'abord, puis
// la BOUCLE du canal qu'elle sert (`agents/canal.ts`) —, ce que cette phrase
// annonçait : la branche est une seconde comparaison, et `wssRacine` n'a pas
// bougé d'une ligne. Avec `{ server }`, `ws` accepterait toute montée sur tout
// chemin — c'est le comportement d'avant P1 (le `new WebSocketServer({ port })`
// de l'ex-`server.ts`, qui ne posait ni `host` ni `path`), et il n'est pas
// extensible.
//
// ⚠️ DEUX CHEMINS, DEUX COMPARAISONS, PAS DE TABLE DE ROUTAGE. Une table pour
// deux entrées serait de l'abstraction non payée, et elle rendrait moins
// visible ce qui compte ici : le refus par DÉFAUT. Tout chemin qui n'est ni
// `/signal` ni `/agent` reçoit un `404` et son socket se ferme — c'est une
// liste blanche, jamais une liste noire, et c'est ce qui fait qu'un chemin
// ajouté un jour par mégarde n'ouvre rien.
//
// 🔴 LE RELAIS A QUITTÉ LA RACINE LE 21 AOÛT 2026, ET LA RAISON EST LE PROXY,
// PAS LE GOÛT. Sur la racine, la page et la montée WebSocket se disputaient
// le même chemin, départagées par le seul en-tête `Upgrade` — un critère sur
// lequel Pomerium ne sait pas router. Le relais servant DEUX pairs de natures
// différentes (le navigateur, que le proxy authentifie ; l'agent Windows, qui
// n'a ni navigateur ni cookie), il fallait un chemin distinct pour que le
// proxy puisse garder la racine sans couper l'agent. La page de session
// (`client/src/main.ts`) et la page-shell (`client/src/shell-page.ts`) visent
// désormais `/signal` (via `client/src/adresse-plateforme.ts`), comme l'agent
// (`agent/src/signaling.rs`, `url_du_relais`). Aucun pair connu n'en est
// affecté — chacun a été déplacé dans le même commit.

import { createServer, type Server } from 'node:http';
import { WebSocketServer } from 'ws';
import type { Config } from '../config';
import type { Pilote } from '../base/pilote';
import { garde } from '../identite/garde';
import { ouvrirMagasin } from '../apps/icones';
import { ouvrirMagasinTranches } from '../apps/magasin-tranches';
import { CacheSante } from './routes-sante';
import { ENTETES_SECURITE } from './entetes';
import { createSignalingServer } from '../signaling/relais';
import { ProprieteDeSession } from '../signaling/propriete';
import { observateurDeSession } from '../signaling/trace';
import { servirLeCanalAgent } from '../agents/canal';
import { RegistreAgents } from '../agents/registre';
import { Frein } from '../securite/frein';
import { servirTout } from './chaine';
import { repondreIntrouvable } from './introuvable';
import {
    annonceProxyDeConfiance,
    annonceRacinePage,
    ecrire,
    etatRacinePage,
} from './annonces';

/// Le chemin du canal plateforme <-> agent (P3). ⚠️ Il est comparé
/// EXACTEMENT : voir le routage plus bas.
const CHEMIN_AGENT = '/agent';

/// Le chemin du relais de signaling. ⚠️ Il est comparé EXACTEMENT.
///
/// 🔴 IL A QUITTÉ LA RACINE LE 21 AOÛT 2026, ET LA RAISON EST LE PROXY, PAS LE
/// GOÛT. Sur la racine, la page et la montée WebSocket se disputaient le même
/// chemin, départagées par le seul en-tête `Upgrade` — un critère sur lequel
/// Pomerium ne sait pas router. Le relais servant DEUX pairs de natures
/// différentes (le navigateur, que le proxy authentifie ; l'agent Windows, qui
/// n'a ni navigateur ni cookie), il fallait un chemin distinct pour que le
/// proxy puisse garder la racine sans couper l'agent.
///
/// ✅ CE DÉPLACEMENT SOLDE UN LEGS DÉCLARÉ de `deploiement/nginx.conf`, que le
/// sous-bloc P5 avait écarté faute d'avoir le droit de toucher `agent/`.
const CHEMIN_SIGNAL = '/signal';

/// 🔴 LA TAILLE MAXIMALE D'UNE TRAME WEBSOCKET, SUR LES DEUX SERVEURS.
///
/// MESURÉ le 20 août 2026 :
/// `plateforme/node_modules/ws/lib/websocket-server.js:74` porte
/// `maxPayload: 100 * 1024 * 1024` — CENT MÉBIOCTETS par défaut. Les deux
/// serveurs de ce fichier étaient construits sans cette option.
///
/// CE QUE CELA OUVRAIT, et ce n'est pas théorique : UN PAIR ANONYME POUVAIT
/// POUSSER UNE TRAME DE 100 Mio AVANT TOUTE AUTHENTIFICATION. Le contrôle de
/// FORME court avant la garde — `signaling/relais.ts:84-86` l'écrit lui-même,
/// « `isJsonObject` est appelé une trentaine de lignes avant
/// `garde.verifier` » —, si bien que `JSON.parse` sur 100 Mio est une
/// allocation puis un pic CPU, par socket et par trame, offerts à quiconque
/// atteint le port. Et le canal `/agent` est la SECONDE porte anonyme : le
/// borner sur `/signal` seulement laisserait la moitié du problème entière.
///
/// Avec cette option, `ws` ferme le socket en 1009 (« message trop grand »)
/// SANS JAMAIS transmettre la trame au gestionnaire `message`.
///
/// ⚠️ LA VALEUR N'EST PAS CALIBRÉE, ET SON PLANCHER EST RAISONNÉ, PAS MESURÉ.
/// Le plus gros message légitime est une offre ou une réponse SDP, dont les
/// sessions de ce dépôt tiennent en quelques kilo-octets ; 256 Kio laisse deux
/// ordres de grandeur de marge. AUCUNE SDP RÉELLE N'A ÉTÉ MESURÉE pour poser
/// ce chiffre, et le dire vaut mieux que de laisser croire à un calibrage.
///
/// ⚠️ **CE QU'ELLE NE FERME PAS — CETTE PHRASE ÉTAIT DEVENUE FAUSSE DE MOITIÉ
/// AU ROUND DE CORRECTION 1 (25 août 2026), qui a borné le NOMBRE de
/// connexions sur `/signal` SEULEMENT.** Elle disait « un pair peut toujours
/// ouvrir BEAUCOUP DE CONNEXIONS », vrai des deux chemins à l'écriture, plus
/// vrai que d'un seul désormais : `/signal` (`signaling/relais.ts`) borne le
/// nombre de connexions par adresse dès `connection`, AVANT le premier
/// message (`securite/frein.ts::BUDGET_REQUETES`) ; `/agent`
/// (`agents/canal.ts`) NE L'EST TOUJOURS PAS — son frein n'est consulté
/// qu'au MESSAGE (une tentative `{vm, secret}`), jamais à la connexion, et
/// un pair qui se tait après avoir ouvert n'est compté par rien, ni par lui
/// ni par `deploiement/nginx.conf` (aucun `limit_conn`/`limit_req`).
export const TRAME_MAX_OCTETS = 256 * 1024;

export interface ServicePlateforme {
    port: number;
    close(): Promise<void>;
}

/// `base` est REQUISE, jamais optionnelle : un service qui apparierait sans
/// rien enregistrer serait indiscernable du bon fonctionnement (spec §6), et
/// c'est la classe exacte de panne muette contre laquelle tout ce dépôt est
/// écrit. `demarrage.ts` garantit par ailleurs que le port ne s'ouvre qu'après
/// la base et ses migrations.
/// 🔴 SANS CETTE FONCTION, `TRAME_MAX_OCTETS` DONNE UN DÉNI DE SERVICE PIRE
/// QUE CELUI QU'IL FERME, et ce n'est pas une conjecture : MESURÉ le 20 août
/// 2026 sur le vrai point d'entrée, `connect ECONNREFUSED` — LE PROCESS ÉTAIT
/// MORT, tué par UNE SEULE TRAME ANONYME.
///
/// LA CHAÎNE, en trois maillons dont chacun est banal : `ws` refuse une trame
/// au-delà de `maxPayload` et ÉMET `error` sur le socket serveur ; aucun
/// socket serveur de ce service n'avait d'écouteur `error` (vérifié :
/// `grep -n "on('error'" relais.ts canal.ts serveur.ts` ne rendait que le
/// `http.once('error', reject)` du démarrage) ; et un `EventEmitter` qui émet
/// `error` sans écouteur LÈVE. L'exception traverse alors un gestionnaire
/// d'évènement Node, qui n'a personne pour l'attraper — le mode de défaillance
/// exact que `signaling/relais.ts` et `signaling/trace.ts` documentent tous
/// deux, atteint ici par une porte neuve.
///
/// ⚠️ AUCUN TEST « DANS » VITEST NE POUVAIT LE VOIR : vitest installe son
/// propre gestionnaire d'exceptions non interceptées, si bien que les tests de
/// `http/serveur.test.ts` restaient VERTS pendant que le service réel mourait
/// (ils signalaient seulement « Vitest caught N unhandled errors »). La preuve
/// vit donc dans `signaling/resilience.test.ts`, qui lance `index.ts` comme un
/// vrai process enfant — c'est précisément la raison d'être de ce fichier-là,
/// et son en-tête l'écrivait avant P5.
///
/// ⚠️ ELLE NE JOURNALISE RIEN, ET C'EST UN CHOIX MOTIVÉ, PAS UNE NÉGLIGENCE.
/// `CLAUDE.md` porte la règle depuis le chantier TURN : « ne jamais tracer par
/// paquet dans la boucle de transport — compter ou échantillonner, jamais
/// tracer par paquet », après qu'une trace par `Transmit` a écrit 18 619
/// lignes en quelques secondes et détruit la mesure qu'elle servait. Une ligne
/// par socket fautif rendrait ici le service à nouveau amplificateur : un
/// attaquant ouvrant N sockets ferait écrire N lignes, sur le chemin même que
/// `TRAME_MAX_OCTETS` vient de fermer.
///
/// ⚠️ LE COÛT EST NOMMÉ : une erreur de socket est donc INVISIBLE à
/// l'exploitant. Ce qui reste observable est la FERMETURE, que le pair voit
/// (code 1009), et le fait que le service continue de servir. Le jour où il
/// faudra les compter, c'est un compteur qu'il faudra — pas une trace.
function encaisserLesErreursDeSocket(wss: WebSocketServer): void {
    // Enregistré AVANT `createSignalingServer` et `servirLeCanalAgent`, qui
    // posent leurs propres gestionnaires `connection` : les écouteurs courent
    // dans leur ordre d'enregistrement, et celui-ci doit être attaché au
    // socket avant que quoi que ce soit d'autre ne lui parle.
    wss.on('connection', (socket) => {
        socket.on('error', () => {
            // Volontairement vide — voir ci-dessus. La seule chose qui compte
            // est qu'un écouteur EXISTE : c'est lui, et lui seul, qui empêche
            // `EventEmitter` de lever.
        });
    });
}

export async function demarrerServeur(config: Config, base: Pilote): Promise<ServicePlateforme> {
    // Les routeurs sont essayés DANS L'ORDRE ; si aucun ne reconnaît le
    // chemin, le 404 de P1 est conservé MOT POUR MOT. ⚠️ Ne pas changer son
    // corps : rien ne le testait avant P2, et le changer serait un effet de
    // bord non déclaré. `routes-auth.test.ts` le fige désormais.
    //
    // 🔴 LE CHAÎNAGE NE SE FAIT PLUS ICI DEPUIS LE 22 AOÛT 2026 : IL A ÉTÉ
    // EXTRAIT VERS `./chaine.ts` (`servirTout`, exporté), PARCE QUE CE
    // FICHIER ATTEIGNAIT 475/500 LIGNES ET QUE LE LOT SUIVANT DEVAIT Y
    // AJOUTER UN DIXIÈME ROUTEUR — l'extraction a libéré la marge AVANT
    // l'ajout, comme `CLAUDE.md` le prescrit. Ce qui reste ICI est le POINT
    // D'APPEL, sous la forme `void … .then(servie => …).catch(…)`,
    // CONSERVÉE TELLE QUELLE. C'est ce que ce fichier s'impose depuis P1 : le
    // `.catch` est la seule chose qui empêche une promesse rejetée dans un
    // gestionnaire d'évènement Node d'abattre tout le processus, et une
    // réécriture de ce corps le perdrait sans que rien ne le dise. Le diff
    // sur le corps du `createServer` reste ainsi d'une seule ligne — l'appel
    // à `servirTout`, désormais importé, plutôt que défini localement.
    //
    // ⚠️ LES DIX ROUTEURS PARTAGENT LEURS DÉPENDANCES, et `Date.now` est
    // passée ici comme à la garde, à la trace et au canal : aucun module du
    // service ne lit d'horloge lui-même. C'est ce qui rend la borne de
    // fraîcheur assertable sur une valeur exacte dans les tests de route.
    // ⚠️ « LES TROIS ROUTEURS » ÉTAIT LE MOT, ET IL DATAIT DE P4 : sept ont
    // été chaînés depuis, sans que cette phrase ne bouge. Le compte se
    // relance, il ne se recopie pas :
    //   grep -cE '^    (if \(await servir|return servir)' plateforme/src/http/chaine.ts
    //     -> 10
    // Le registre des sockets d'agent vivants, construit UNE FOIS et partagé
    // entre le canal (qui y inscrit) et les routes (qui y lancent). C'est le
    // seul endroit du service qui en fabrique un.
    //
    // ⚠️ IL A LE MÊME COÛT QUE `ProprieteDeSession`, ET IL EST NOMMÉ AU MÊME
    // ENDROIT : il ne survit pas à un redémarrage. Après un redémarrage, aucun
    // agent n'y figure tant qu'il ne s'est pas ré-enrôlé, et tout lancement
    // rend `agent-injoignable` — bruyamment. Le rattrapage est la reconnexion
    // de l'agent, qui le reconstitue sans que personne ne le persiste.
    const registreAgents = new RegistreAgents();

    // 🔴 UN SEUL FREIN POUR TOUT LE SERVICE, construit ICI et partagé entre
    // les routes d'authentification et le canal `/agent`. Deux freins
    // distincts DIVERGERAIENT le jour où l'un serait durci (D4), et leurs
    // budgets d'adresse s'additionneraient : un attaquant obtiendrait le
    // double de ce que les constantes annoncent en alternant les deux portes.
    //
    // ⚠️ IL A LE MÊME COÛT QUE `ProprieteDeSession` ET `RegistreAgents`, ET IL
    // EST NOMMÉ AU MÊME ENDROIT : il ne survit pas à un redémarrage, et LE
    // FREIN D'UNE INSTANCE NE PROTÈGE QUE CETTE INSTANCE. Deux instances
    // multiplieraient chaque budget par deux, sans que rien ne le dise — c'est
    // l'une des raisons pour lesquelles le déploiement n'en déclare qu'une.
    const frein = new Frein();

    // Le cache de `/sante`, construit UNE fois et vivant pour la durée du
    // service — comme `ProprieteDeSession`, `RegistreAgents` et le frein.
    // Un cache par requête ne cacherait rien.
    const cacheSante = new CacheSante();

    // Le magasin d'icônes, ouvert UNE fois pour la durée du service. Il crée
    // son répertoire s'il manque et JOURNALISE le chemin retenu : la variable
    // étant facultative, c'est la seule chose qui rende visible à l'opérateur
    // le magasin sur lequel il travaille réellement.
    const magasin = ouvrirMagasin(config.repertoireIcones, (chemin) => {
        console.info(`magasin d icones : ${chemin}`);
    });

    // Le magasin de TRANCHES, ouvert UNE fois lui aussi, et journalisant son
    // chemin pour exactement la même raison que celui des icônes.
    //
    // 🔴 IL EST LU PAR **DEUX** ROUTEURS — `servirTeleversement` y écrit les
    // tranches, `servirInstallation` en sert la concaténation à l'agent — et
    // c'est le MÊME, jamais deux : deux magasins ouverts sur le même répertoire
    // seraient deux vues d'un même disque, et le second ne verrait pas
    // nécessairement ce que le premier vient d'écrire.
    const magasinTranches = ouvrirMagasinTranches(config.repertoireTeleversements, (chemin) => {
        console.info(`magasin de tranches : ${chemin}`);
    });

    // 🔴 LA TROISIÈME RACINE DISQUE FACULTATIVE S'ANNONCE COMME LES DEUX
    // AUTRES, ET ELLE NE LE FAISAIT PAS. Les deux magasins ci-dessus
    // journalisent leur chemin retenu depuis G2 et G3, avec la raison écrite
    // au-dessus d'eux ; `PLATEFORME_PAGE`, ajoutée le 22 août 2026, ne
    // journalisait RIEN — ni au démarrage ni à la requête —, si bien qu'une
    // racine inexistante rendait `404 introuvable` sur toute page, strictement
    // indiscernable de la variable absente. Voir `./annonces.ts`.
    //
    // ⚠️ C'EST ICI, AVANT `http.listen`, ET PAS AILLEURS : une annonce postée
    // après l'ouverture du port arriverait après la première requête servie.
    ecrire(annonceRacinePage(await etatRacinePage(config.racinePage)));
    // 🔴 MÊME CLASSE DE PANNE MUETTE, MÊME REMÈDE — et la revue finale les a
    // classés ensemble à raison : un nom d'hôte dans
    // `PLATEFORME_PROXY_DE_CONFIANCE` ne correspond à aucune `remoteAddress`,
    // donc `pairDeConfiance` refuse tout le monde, `/auth/moi` rend `401` à
    // Pomerium lui-même, et le service répond quand même. Le runbook le
    // documente déjà — mais un runbook ne rougit pas.
    ecrire(annonceProxyDeConfiance(config.proxyDeConfiance));

    const deps = {
        base,
        secretJeton: config.secretJeton,
        origineClient: config.origineClient,
        maintenant: Date.now,
        // ⚠️ **DEUX** ROUTEURS LE LISENT — `servirIdentite` ET `servirAuth`. Ce
        // commentaire a dit « SEUL `servirIdentite` » jusqu'au 21 août 2026 :
        // c'est LA MÊME FAUTE que la cicatrice de `registre`, vingt lignes plus
        // bas, refaite dans le commit suivant. Les deux gardes sont de
        // POLARITÉS OPPOSÉES et partitionnent les modes — `/auth/moi` en
        // `pomerium`, `/auth/connexion` et `/auth/rafraichir` en `motdepasse` ;
        // l'invariant qui les lie est écrit aux deux endroits.
        auth: config.auth,
        // ⚠️ `servirApplications` ET `servirInstallation` LE LISENT — le
        // premier pour lancer une application, le second pour pousser un ordre
        // d'installation à une VM déjà connectée. Les autres l'ignorent. Il est posé ici plutôt que passé à part pour que le
        // chaînage reste une seule ligne par routeur, et parce qu'un objet de
        // dépendances par routeur ferait DIX listes à tenir à jour — le
        // compte disait « quatre », et il datait lui aussi de P4.
        registre: registreAgents,
        // 🔴 CETTE PHRASE A DÉJÀ MENTI DEUX FOIS DE SUITE — « SEUL servirAuth
        // LE LIT », PUIS « servirAuth ET routes-identite.ts, désormais DEUX »
        // — À CHAQUE FOIS PARCE QU'UN LOT SUIVANT A AJOUTÉ UN LECTEUR SANS
        // REVENIR CORRIGER CETTE LIGNE. Le legs des freins manquants (D24,
        // 25 août 2026) en ajoute encore DEUX : `routes-vm.ts` et
        // `routes-session.ts` consultent désormais `frein` ET
        // `proxyDeConfiance` eux aussi, pour le budget « toute requête »
        // (`securite/frein.ts::BUDGET_REQUETES`) — un budget de VOLUME,
        // distinct de celui d'ÉCHECS que `servirAuth` consulte seul.
        //
        // ✅ TOUT CE BLOC REVÉRIFIÉ le 25 août 2026 PAR LA COMMANDE, non par
        // la lecture : `grep -ln 'deps\.<clé>' plateforme/src/http/routes-*.ts`
        // (hors `*.test.ts`, qui POSENT la dépendance sans la consommer).
        // Lecteurs — `frein` : `routes-auth.ts`, `routes-vm.ts`,
        // `routes-session.ts` — TROIS ; `proxyDeConfiance` : les trois
        // ci-dessus **ET** `routes-identite.ts` — QUATRE ; `cache` :
        // `routes-sante.ts` ; `magasin` : `routes-icone.ts`.
        //
        // ⚠️ `signaling/relais.ts` LIT LES DEUX AUSSI, POUR LE MÊME BUDGET,
        // MAIS PAS PAR CE CHEMIN : il ne reçoit pas cet objet `deps` — il est
        // construit à part, plus bas dans cette fonction, et `frein` comme
        // `config.proxyDeConfiance` lui sont passés en PARAMÈTRES POSITIONNELS
        // de `createSignalingServer`. La commande `grep -ln 'deps\.frein'`
        // ci-dessus ne le voit donc PAS — chercher `createSignalingServer`
        // pour ce lecteur-là (voir plus bas dans cette même fonction).
        frein,
        proxyDeConfiance: config.proxyDeConfiance,
        cache: cacheSante,
        // ⚠️ SEUL `servirIcone` LE LIT — même raison que `registre` et `frein`
        // ci-dessus, et revérifié par la même commande.
        magasin,
        // ⚠️ IL S'APPELLE `tranches` ET NON `magasin`, parce que `magasin` est
        // DÉJÀ PRIS par celui des icônes, juste au-dessus. `tsc` a attrapé la
        // collision — les deux routeurs de G3 l'avaient d'abord nommée
        // `magasin` chacun de son côté — parce que les deux types diffèrent.
        // **Le jour où deux magasins auront la même forme, le service servirait
        // des icônes à la place des tranches sans qu'aucun contrôle ne
        // bronche.**
        tranches: magasinTranches,
        // ⚠️ `registre` EST DÉJÀ PLUS HAUT, et il sert désormais à DEUX
        // routeurs : `servirApplications` (le lancement) et `servirInstallation`
        // (la poussée de l'ordre). Le commentaire qui le disait lu par un seul
        // a été corrigé à sa place.
        // ⚠️ SEUL `servirPage` LE LIT. Absent ⇒ le servant se retire et le 404
        // générique reprend la main — le comportement d'avant le lot.
        racinePage: config.racinePage,
    };

    // `servirTout` : voir son extraction vers `./chaine.ts`, expliquée plus
    // haut dans cette fonction.
    const http: Server = createServer((requete, reponse) => {
        void servirTout(requete, reponse, deps)
            .then((servie) => {
                if (servie) return;
                // ⚠️ LE 404 GÉNÉRIQUE NE VIENT D'AUCUN ROUTEUR, et c'est
                // pourquoi il doit être traité ici : sans cette ligne, un
                // chemin inconnu serait la SEULE réponse du service à ne pas
                // porter `nosniff`. Le CORPS n'est pas touché — « rien ne le
                // testait avant P2, et le changer serait un effet de bord non
                // déclaré ».
                //
                // 🔴 IL A DÉMÉNAGÉ VERS `./introuvable.ts` LE 22 AOÛT 2026, ET
                // CE N'EST PAS UNE FACTORISATION DE CONFORT : les deux gardes
                // de mode répondent désormais CE 404-CI elles-mêmes, parce que
                // le repli SPA du servant de page avalait celui d'ici. Un
                // second texte écrit à la main dans chaque garde dériverait de
                // celui-ci sans que rien ne le dise. Voir l'en-tête de ce
                // module-là.
                repondreIntrouvable(reponse);
            })
            .catch((cause) => {
                // Une promesse rejetée sans `catch` dans un gestionnaire
                // d'évènement Node abat tout le process — c'est le mode de
                // défaillance que `signaling/relais.ts` documente déjà. La
                // cause est journalisée SANS le corps de la requête, qui
                // porterait le mot de passe (critère ④).
                // ⚠️ LE LIBELLÉ NE NOMME PLUS « l'authentification » : depuis
                // P4 ce `catch` couvre TOUS les routeurs — ils sont DIX, non
                // trois comme cette phrase l'a dit jusqu'au 22 août 2026, et
                // le compte se relance depuis `chaine.ts`. Un message qui
                // désignerait le mauvais ferait chercher au mauvais endroit.
                // C'est la seule ligne de ce bloc que P4 change, et elle est
                // changée parce qu'elle serait devenue FAUSSE autrement.
                console.error(`route HTTP en échec : ${String(cause)}`);
                if (!reponse.headersSent) {
                    reponse.writeHead(500, {
                        'content-type': 'application/json; charset=utf-8',
                        ...ENTETES_SECURITE,
                    });
                    reponse.end(JSON.stringify({ refus: 'interne' }));
                }
            });
    });

    // `maxPayload` sur les DEUX serveurs, jamais un seul : voir
    // `TRAME_MAX_OCTETS`. La borne s'applique dans `ws`, donc AVANT le
    // gestionnaire `message` — c'est ce qui la rend utile, le contrôle de
    // forme du relais courant avant la garde.
    const wssRacine = new WebSocketServer({ noServer: true, maxPayload: TRAME_MAX_OCTETS });
    encaisserLesErreursDeSocket(wssRacine);
    // Le canal `/agent` (P3) : son propre `WebSocketServer`, qui ne partage
    // avec le relais ni garde, ni registre d'appartenance, ni observateur de
    // session. C'est la conséquence directe d'E4 : l'enrôlement est
    // ASYNCHRONE (il lit `agent_enrole`), et la garde du relais est PURE et
    // SYNCHRONE. Les faire cohabiter dans le même serveur obligerait l'un des
    // deux à céder.
    const wssAgent = new WebSocketServer({ noServer: true, maxPayload: TRAME_MAX_OCTETS });
    encaisserLesErreursDeSocket(wssAgent);
    // La garde est construite ICI, à partir du secret de configuration, et
    // c'est le SEUL endroit du service qui en fabrique une. Elle est REQUISE
    // par le relais : il n'existe aucun chemin qui produise une garde ouverte
    // hors d'un test, `PLATEFORME_SECRET_JETON` n'ayant aucun défaut.
    //
    // Le registre d'appartenance vit ici aussi, donc pour la durée du service.
    // Son coût — il ne survit pas à un redémarrage — est écrit dans
    // `signaling/propriete.ts`.
    const gardeDuService = garde(config.secretJeton, Date.now, new ProprieteDeSession());
    // 🔴 LE CANAL EST BRANCHÉ ICI, ET C'EST LA SEULE LIGNE QUI LE FAIT VIVRE.
    // Sans elle, `wssAgent` accepterait toujours la montée sur `/agent` et
    // n'écouterait RIEN : le pair verrait une connexion réussie, puis un
    // silence — la panne muette exacte que ce fichier invoque déjà pour rendre
    // `base` REQUISE. `canal.test.ts` la tient par un test dédié.
    //
    // `Date.now` est passée ici, comme à la garde et à la trace : les trois la
    // reçoivent de cette fonction, et aucun module du service ne lit d'horloge
    // lui-même. C'est ce qui rend l'expiration d'un jeton d'agent assertable
    // sur une valeur EXACTE dans `canal.test.ts`.
    servirLeCanalAgent(wssAgent, {
        base,
        secretJeton: config.secretJeton,
        maintenant: Date.now,
        registre: registreAgents,
        // Le MÊME frein que les routes d'authentification : voir sa
        // construction plus haut.
        frein,
        proxyDeConfiance: config.proxyDeConfiance,
        // Le MÊME magasin que la route d'icône, jamais un second : deux
        // magasins divergeraient, et l'inventaire des manquantes désignerait
        // un disque que la route ne sert pas.
        magasin,
    });

    // `Date.now` est passée ICI, et une seule fois pour la trace : c'est le
    // seul endroit du chemin de la trace qui lise une horloge réelle, tout le
    // reste la reçoit.
    //
    // ⚠️ `frein` ET `config.proxyDeConfiance` : LE MÊME frein que les routes
    // HTTP et le canal `/agent`, jamais un second — voir sa construction plus
    // haut et `securite/frein.ts::BUDGET_REQUETES`. Avant ce lot, aucune
    // borne n'existait sur le nombre de connexions qu'une même adresse
    // pouvait ouvrir sur `/signal` — le legs que `TRAME_MAX_OCTETS`,
    // au-dessus, nommait déjà sans le fermer.
    const relais = createSignalingServer(
        wssRacine,
        gardeDuService,
        frein,
        config.proxyDeConfiance,
        observateurDeSession(base, Date.now),
    );

    http.on('upgrade', (requete, socket, tete) => {
        // `requete.url` peut porter une chaîne de requête ; seul le chemin
        // décide du routage.
        const chemin = new URL(requete.url ?? '/', 'http://placeholder').pathname;
        // Comparaison EXACTE, jamais un `startsWith` : `/agentaire` n'est pas
        // `/agent`, et un préfixe ouvrirait une famille entière de chemins que
        // personne n'a décidés.
        const wss =
            chemin === CHEMIN_SIGNAL ? wssRacine : chemin === CHEMIN_AGENT ? wssAgent : undefined;
        if (wss === undefined) {
            // Refus explicite AVANT toute montée : le pair reçoit un 404 HTTP
            // et son socket se ferme, plutôt que de rester ouvert sur un
            // service qui ne l'écoutera jamais.
            socket.write('HTTP/1.1 404 Not Found\r\nConnection: close\r\n\r\n');
            socket.destroy();
            return;
        }
        wss.handleUpgrade(requete, socket, tete, (client) => {
            wss.emit('connection', client, requete);
        });
    });

    await new Promise<void>((resolve, reject) => {
        http.once('error', reject);
        // Les DEUX arguments, toujours : sans `config.hote`, Node écoute sur
        // toutes les interfaces, ce que le critère ④ existe pour empêcher.
        http.listen(config.port, config.hote, () => {
            http.removeListener('error', reject);
            resolve();
        });
    });

    const adresse = http.address();
    const port = typeof adresse === 'object' && adresse ? adresse.port : config.port;

    return {
        port,
        async close(): Promise<void> {
            await relais.close();
            // ⚠️ Le second serveur se ferme AUSSI, et explicitement. Un
            // `WebSocketServer` en `noServer` ne s'arrête pas avec le serveur
            // HTTP : ses sockets déjà montés survivraient, et `close()`
            // rendrait la main sur un service qui écoute encore.
            await new Promise<void>((resolve) => wssAgent.close(() => resolve()));
            await new Promise<void>((resolve) => http.close(() => resolve()));
        },
    };
}
