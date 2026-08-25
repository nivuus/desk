// Ce que le service DIT de sa configuration au démarrage, avant d'écouter.
//
// 🔴 POURQUOI CE MODULE EXISTE, ET LA RÈGLE QU'IL APPLIQUE ÉTAIT DÉJÀ ÉCRITE
// DANS `serveur.ts`. Les deux magasins de disque facultatifs y journalisent
// leur chemin retenu, et la raison y est nommée : « la variable étant
// facultative, c'est la seule chose qui rende visible à l'opérateur le magasin
// sur lequel il travaille réellement ». Le lot « page derrière Pomerium » a
// ajouté une TROISIÈME racine disque facultative — `PLATEFORME_PAGE` — sans
// lui appliquer cette règle : mal posée, elle rendait `404 introuvable` sur
// TOUTE page, ce qui est STRICTEMENT INDISCERNABLE de « variable absente », et
// rien ne le disait, ni au démarrage ni à la requête.
//
// 🔴 ET L'ENSEMBLE DE CONFIANCE EST DE LA MÊME CLASSE, PAS D'UNE AUTRE : un
// nom d'hôte écrit dans `PLATEFORME_PROXY_DE_CONFIANCE` ne correspond à
// AUCUNE `remoteAddress` — qui est toujours une adresse IP —, si bien que
// `pairDeConfiance` refuse TOUT LE MONDE, que `/auth/moi` rend `401
// pair-non-de-confiance` à Pomerium lui-même, et que le service répond quand
// même. Même panne muette, même remède : dire ce qu'on a RETENU.
//
// ⚠️ ON JOURNALISE CE QUE LE SERVICE A RETENU, JAMAIS CE QU'ON LUI A DONNÉ.
// Une trace qui recopierait la valeur brute de l'environnement ne prouverait
// que la lecture de l'environnement ; ce que l'exploitant doit pouvoir
// reconnaître est le chemin ABSOLU RÉSOLU et les entrées SURVIVANTES au
// découpage — c'est-à-dire ce sur quoi le produit travaille réellement.
//
// ⚠️ CE MODULE REND SES LIGNES, IL NE LES ÉCRIT PAS — même convention que
// `obs/journal.ts`, et pour la même raison : c'est ce qui les rend éprouvables
// sans `spyOn(console)`. L'écriture est un geste séparé (`ecrire`), appelé par
// `serveur.ts`.

import { constants } from 'node:fs';
import { access, stat } from 'node:fs/promises';
import { resolve } from 'node:path';
import { ligne } from '../obs/journal';

/// Une ligne prête à écrire, et le NIVEAU qui la porte.
///
/// 🔴 LE NIVEAU EST DANS LA DONNÉE, PAS DANS L'APPELANT : une racine posée
/// mais illisible doit être BRUYANTE (`console.error`), et laisser ce choix au
/// site d'appel le rendrait invisible au test qui l'éprouve.
export interface Annonce {
    readonly niveau: 'info' | 'erreur';
    readonly texte: string;
}

/// Ce que le service a retenu de `PLATEFORME_PAGE`, une fois le disque sondé.
export type EtatRacinePage =
    | { readonly arme: false }
    | { readonly arme: true; readonly chemin: string; readonly lisible: true }
    | {
          readonly arme: true;
          readonly chemin: string;
          readonly lisible: false;
          readonly cause: string;
      };

/// PURE. La ligne que `PLATEFORME_PAGE` mérite, dans les trois cas.
///
/// 🔴 « AUCUNE PAGE SERVIE » EST UNE INFORMATION D'EXPLOITATION, PAS UN
/// SILENCE. C'est le montage nginx (mode `motdepasse`), où la plateforme ne
/// doit rien servir : le dire est ce qui distingue « je ne sers rien parce
/// qu'on ne me l'a pas demandé » de « je ne sers rien parce que ma racine est
/// fausse ». Sans cette ligne, les deux se lisent comme un `404` identique.
export function annonceRacinePage(etat: EtatRacinePage): Annonce {
    if (!etat.arme) {
        return {
            niveau: 'info',
            texte: ligne('page servie', {
                racine: 'aucune',
                raison: 'PLATEFORME_PAGE absente ou vide',
                effet: 'GET / rend 404 introuvable',
            }),
        };
    }
    if (etat.lisible) {
        return {
            niveau: 'info',
            texte: ligne('page servie', { racine: etat.chemin, lisible: 'oui' }),
        };
    }
    // 🔴 `erreur`, JAMAIS `info` : c'est le cas que ce module existe pour
    // rendre bruyant. Le service DÉMARRE quand même — refuser de démarrer
    // couperait l'API et le signaling pour une page, ce qui serait
    // disproportionné —, mais il ne le fait plus en silence.
    return {
        niveau: 'erreur',
        texte: ligne('page servie', {
            racine: etat.chemin,
            lisible: 'non',
            cause: etat.cause,
            effet: 'toute page rendra 404 introuvable',
        }),
    };
}

/// PURE. La ligne de l'ensemble de confiance RETENU.
///
/// 🔴 **L'ENSEMBLE VIDE EST `info` ET NON `erreur` — MAIS PAS PARCE QUE CE
/// SERAIT LE « DÉFAUT SÛR » DU MODE `motdepasse` : cette phrase-là a été
/// FALSIFIÉE par la revue du round de correction 3 de `frein(pont)`, et
/// corrigée à trois endroits (`docker-compose.plateforme.yml`, `frein.ts`,
/// ici).** Un ensemble vide veut dire « `X-Forwarded-For` n'est pas cru, et
/// `adresseSource` retombe sur `req.socket.remoteAddress` » — sûr SEULEMENT
/// si cette adresse est celle du CLIENT réel, c'est-à-dire seulement si la
/// plateforme est exposée DIRECTEMENT. **Ce n'est PAS le montage que ce
/// dépôt LIVRE** : `docker-compose.plateforme.yml` place nginx devant elle,
/// même en mode `motdepasse`, si bien que `remoteAddress` est TOUJOURS
/// l'adresse du conteneur nginx pour toute requête réelle — l'ensemble vide y
/// fait dégénérer `BUDGET_ADRESSE` **et** `BUDGET_REQUETES` (celui-ci
/// couvrant `GET /vm`, `POST /session` et `/signal`, HTTP et WebSocket
/// confondus depuis le lot « frein(volume) ») en un budget PARTAGÉ par TOUT
/// LE TRAFIC, sans qu'aucun attaquant n'ait à forger quoi que ce soit — la
/// dégénérescence est automatique dès que le second proxy existe.
///
/// ⚠️ **POURQUOI CE NIVEAU RESTE `info` MALGRÉ CETTE GRAVITÉ** : cette
/// fonction est PURE et ne reçoit que l'ensemble RETENU — elle n'a AUCUN
/// moyen de savoir si le processus qui l'appelle tourne DERRIÈRE un proxy ou
/// exposé directement, et c'est une question de TOPOLOGIE DE DÉPLOIEMENT,
/// pas de configuration que `config.ts` puisse trancher pour elle. Rendre ce
/// cas `erreur` inconditionnellement alarmerait à tort le déploiement où
/// l'ensemble vide est légitimement sûr (exposition directe, sans proxy).
/// **La ligne `info` reste donc le seul témoin de l'exploitant** — c'est
/// pour cela qu'elle nomme déjà l'effet exact (`X-Forwarded-For n est pas
/// cru, et /auth/moi refuse tout pair`) plutôt qu'un simple booléen — et
/// c'est `deploiement/README.md` (invariant ③) qui porte la responsabilité
/// de dire, pour LE montage que ce dépôt livre spécifiquement, que cette
/// ligne `info` annonce en réalité un risque `erreur`.
export function annonceProxyDeConfiance(confiance: ReadonlySet<string>): Annonce {
    if (confiance.size === 0) {
        return {
            niveau: 'info',
            texte: ligne('proxys de confiance', {
                retenus: 'aucun',
                effet: 'X-Forwarded-For n est pas cru, et /auth/moi refuse tout pair',
            }),
        };
    }
    return {
        niveau: 'info',
        texte: ligne('proxys de confiance', {
            // ⚠️ TOUTES LES ENTRÉES, JAMAIS UN ÉCHANTILLON NI UNE TRONCATURE :
            // `obs/journal.ts` porte déjà la raison — une adresse tronquée est
            // AMBIGUË, et l'exploitant ne reconnaîtrait plus la sienne. C'est
            // aussi ce qui rend un nom d'hôte VISIBLE là où il ne le serait
            // pas dans un compte.
            retenus: [...confiance].join(' '),
            nombre: confiance.size,
        }),
    };
}

/// Le sondage RÉEL du disque. Rejette avec une cause lisible.
///
/// ⚠️ TROIS CHOSES SONT SONDÉES, PAS UNE : que le chemin existe, que ce soit
/// un RÉPERTOIRE, et qu'il soit LISIBLE ET TRAVERSABLE. Un fichier ordinaire
/// posé comme racine, ou un répertoire sans bit `x`, produisent exactement le
/// même `404` muet qu'une racine absente — les distinguer au démarrage est
/// tout l'objet de ce module.
export async function sonderRepertoire(chemin: string): Promise<void> {
    const infos = await stat(chemin);
    if (!infos.isDirectory()) throw new Error('ce n est pas un repertoire');
    await access(chemin, constants.R_OK | constants.X_OK);
}

/// Résout et sonde, sans rien écrire.
///
/// ⚠️ `resolve()` EST APPELÉ ICI ET NULLE PART AILLEURS DANS CE MODULE : c'est
/// le chemin RÉSOLU qui part au journal, parce que c'est lui que le servant
/// emploie (`page/routes-page.ts` fait le même `resolve`). Journaliser la
/// valeur brute laisserait un chemin relatif ambigu, dont l'ancrage dépend du
/// répertoire courant du processus.
///
/// ⚠️ LE SONDEUR EST INJECTÉ, avec un défaut réel : c'est ce qui rend les
/// trois états éprouvables sans fabriquer un répertoire illisible sur le
/// disque du test — et l'injection ne coûte rien au produit, qui n'appelle
/// jamais cette fonction autrement qu'avec son défaut.
export async function etatRacinePage(
    brut: string | undefined,
    sonder: (chemin: string) => Promise<void> = sonderRepertoire,
): Promise<EtatRacinePage> {
    // ⚠️ `''` EST TRAITÉ COMME L'ABSENCE, exactement comme dans
    // `page/routes-page.ts` : `resolve('')` rend le répertoire COURANT du
    // processus, et l'annoncer comme racine servie serait un mensonge.
    if (brut === undefined || brut === '') return { arme: false };
    const chemin = resolve(brut);
    try {
        await sonder(chemin);
    } catch (cause) {
        return { arme: true, chemin, lisible: false, cause: String(cause) };
    }
    return { arme: true, chemin, lisible: true };
}

/// L'écriture, isolée en un seul geste — le seul de ce module qui touche la
/// console.
export function ecrire(annonce: Annonce): void {
    if (annonce.niveau === 'erreur') console.error(annonce.texte);
    else console.info(annonce.texte);
}
